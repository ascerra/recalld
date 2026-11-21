//! Automatic schema admission for production startup.
use super::*;
use crate::storage::StorageError;

pub(super) fn validate_native(
    _config: &RecalldConfig,
    engine: &RedbStorageEngine,
    provider: &dyn EmbeddingProvider,
) -> std::result::Result<(), StorageError> {
    if engine.control_state()?.control_schema_version != 2 {
        return Err(StorageError::RecoveryRequired(
            "runtime requires native schema2".into(),
        ));
    }
    if let Some(profile) = provider.profile() {
        profile.validate().map_err(StorageError::InvalidMutation)?;
        if provider.dimensions() != profile.dimension as usize {
            return Err(StorageError::ProviderConfigurationUnavailable);
        }
    }
    let namespaces = engine.list_namespaces()?;
    if !namespaces
        .iter()
        .any(|ns| ns.name == crate::model::constants::DEFAULT_NAMESPACE_NAME)
    {
        return Err(StorageError::RecoveryRequired(
            "native runtime requires a default namespace".into(),
        ));
    }
    for namespace in namespaces {
        namespace
            .schema
            .as_ref()
            .ok_or_else(|| StorageError::InvalidControl("missing native namespace schema".into()))?
            .validate(&namespace)
            .map_err(StorageError::InvalidControl)?;
    }

    Ok(())
}

/// Adopt a changed `decay_rate_multiplier` for every namespace that inherits
/// it, through the receipted preview/adopt path, before anything is served. A
/// namespace that cannot be adopted keeps its stored value and startup goes on.
pub(super) fn adopt_changed_multiplier(
    config: &RecalldConfig,
    engine: &mut RedbStorageEngine,
    now: i64,
) {
    let configured = config.decay.decay_rate_multiplier;
    let (Ok(bits), Ok(namespaces)) = (
        crate::storage::inherited_policy::normalize(configured),
        engine.list_namespaces(),
    ) else {
        return;
    };
    for ns in namespaces {
        let Some(stored) = ns
            .schema
            .as_ref()
            .and_then(|schema| schema.inherited_decay.as_ref())
            .filter(|b| ns.decay_rate_multiplier.is_none() && b.multiplier_bits != bits)
        else {
            continue;
        };
        let stored = f32::from_bits(stored.multiplier_bits);
        match engine.adopt_inherited_multiplier(&ns, bits, now) {
            Ok(counts) => tracing::warn!(
                namespace = %ns.name,
                old_multiplier = stored,
                new_multiplier = configured,
                full_to_summary = counts.full_to_summary,
                summary_to_ghost = counts.summary_to_ghost,
                deletions = counts.ghost_to_delete,
                "decay_rate_multiplier changed: adopted for this namespace; at the next decay sweep \
                 the counted memories move to Summary or Ghost, or are deleted"
            ),
            Err(error) => tracing::warn!(
                namespace = %ns.name,
                namespace_id = ns.id.get(),
                stored_multiplier = stored,
                configured_multiplier = configured,
                %error,
                "decay_rate_multiplier change is not yet applied to this namespace; it keeps \
                 the stored value. To apply it, stop the daemon and run \
                 `recalld-cli policy preview-inherited` then `recalld-cli policy adopt-inherited`"
            ),
        }
    }
}

pub(super) fn open_storage(
    config: &RecalldConfig,
    clock: &dyn crate::time::Clock,
    ids: Box<dyn crate::storage::ids::IdSource>,
    provider: &dyn EmbeddingProvider,
) -> std::result::Result<
    (
        RedbStorageEngine,
        Option<crate::storage::automatic_upgrade::UpgradeTransaction>,
    ),
    StorageError,
> {
    use crate::storage::automatic_upgrade::{self, StoreFormat};
    let path = std::path::Path::new(&config.storage.data_dir);
    if automatic_upgrade::classify(path)? != StoreFormat::Missing {
        return automatic_upgrade::prepare_upgrade(path, clock.now_millis(), ids)
            .map(|(engine, transaction)| (engine, Some(transaction)));
    }
    // Resolve the actual provider before creating any canonical files. This
    // binds only a newly created empty namespace, never existing vectors.
    let profile = provider
        .profile()
        .ok_or(StorageError::ProviderConfigurationUnavailable)?;
    profile.validate().map_err(StorageError::InvalidMutation)?;
    if provider.dimensions() != profile.dimension as usize {
        return Err(StorageError::ProviderConfigurationUnavailable);
    }
    let mut engine = RedbStorageEngine::open_with_ids(path, true, ids)?;
    if engine.list_namespaces()?.is_empty() && engine.scan_all()?.is_empty() {
        let state = engine.control_state()?;
        engine.create_native_namespace_with_provider(
            &crate::storage::control::MutationContext {
                store_id: Some(state.store_id),
                scope: crate::storage::control::MutationScope::Catalog,
                idempotency_key: None,
                expected_generation: Some(state.committed_generation),
                contract_version: 2,
            },
            &crate::storage::native_catalog::NativeNamespaceInput {
                name: crate::model::constants::DEFAULT_NAMESPACE_NAME.into(),
                embedding_dim: None,
                embedding_profile_id: None,
                initial_stability: None,
                desired_retention: None,
                decay_rate_multiplier: None,
            },
            provider,
            &crate::storage::native_lifecycle::NativePolicyInputs {
                global: DecayConfig::default(),
                multiplier: config.decay.decay_rate_multiplier,
            },
            clock,
            None,
        )?;
        engine.sync()?;
    }
    validate_native(config, &engine, provider)?;
    Ok((engine, None))
}

pub(super) async fn build_embedding(config: &RecalldConfig) -> Result<Arc<dyn EmbeddingProvider>> {
    let emb_config = embedding::EmbeddingConfig {
        provider: match config.embedding.provider {
            crate::config::EmbeddingProvider::OpenAI => embedding::ProviderType::OpenAI,
            crate::config::EmbeddingProvider::Ollama => embedding::ProviderType::Ollama,
            crate::config::EmbeddingProvider::Passthrough => embedding::ProviderType::Passthrough,
            crate::config::EmbeddingProvider::Bedrock => embedding::ProviderType::Bedrock,
        },
        dimensions: config.embedding.dimensions,
        model: config.embedding.model_name.clone(),
        api_key: std::env::var(&config.embedding.api_key_env).ok(),
        base_url: Some(config.embedding.base_url.clone()),
        batch_size: Some(config.embedding.batch_size),
        timeout_secs: None,
        region: Some(config.embedding.region.clone()),
        cache_embeddings: false,
        cache_max_entries: 1000,
    };
    let provider = embedding::build_provider(
        &emb_config,
        &config.embedding.document_prefix,
        &config.embedding.query_prefix,
    )
    .await
    .map_err(|e| RecalldError::Init {
        step: "create_embedding_provider",
        message: format!("failed to create embedding provider: {}", e,),
        source: None,
    })?;
    tracing::info!(
        provider = ?config.embedding.provider,
        model = %config.embedding.model_name,
        dimensions = config.embedding.dimensions,
        document_prefix = %config.embedding.document_prefix,
        query_prefix = %config.embedding.query_prefix,
        "embedding provider ready"
    );
    Ok(Arc::from(provider))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(path: &std::path::Path) -> RecalldConfig {
        let mut config = RecalldConfig::default();
        config.storage.data_dir = path.to_str().unwrap().into();
        config.embedding.provider = crate::config::EmbeddingProvider::Ollama;
        config.embedding.model_name = "isolated-startup-fixture".into();
        config.embedding.dimensions = 4;
        // Construction does not contact Ollama. Never use a personal endpoint
        // or credential variable even if a test accidentally requests embedding.
        config.embedding.base_url = "http://127.0.0.1:1".into();
        config.embedding.api_key_env = "RECALLD_ISOLATED_STARTUP_UNUSED_KEY".into();
        config.decay.disable_sweep = true;
        config.rif.enabled = false;
        config.graph.autolink_enabled = false;
        config
    }

    #[tokio::test]
    async fn fresh_startup_is_native_and_reopens_without_rebinding() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("db");
        let config = config(&path);
        let mut prior = None;
        for _ in 0..2 {
            let core = Recalld::with_clock(
                config.clone(),
                Arc::new(crate::time::ManualClock::new(1000)),
            )
            .await
            .unwrap();
            {
                let storage = core.storage.read().unwrap();
                assert_eq!(storage.control_state().unwrap().control_schema_version, 2);
                let namespace = storage.get_namespace_by_name("default").unwrap().unwrap();
                assert_eq!(namespace.created_at, 1000);
                assert!(matches!(
                    namespace.schema.as_ref().unwrap().embedding_binding,
                    crate::model::schema::EmbeddingBinding::Bound {
                        provenance: crate::model::schema::BindingProvenance::NativeV2,
                        ..
                    }
                ));
                let current = (
                    storage.control_state().unwrap().generation_ref(),
                    serde_json::to_vec(&namespace).unwrap(),
                );
                if let Some(prior) = &prior {
                    assert_eq!(prior, &current);
                }
                prior = Some(current);
            }
            core.shutdown().await.unwrap();
        }
        assert!(
            RedbStorageEngine::open(&path).is_err(),
            "no legacy fallback"
        );
    }

    #[tokio::test]
    async fn migration_preserves_unknown_provenance_in_native_runtime() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("db");
        {
            let mut legacy = RedbStorageEngine::open(&path).unwrap();
            let mut namespace = crate::model::NamespaceConfig::default_namespace(1000);
            namespace.embedding_dim = 4;
            legacy.create_namespace(&namespace).unwrap();
            legacy.sync().unwrap();
        }
        let core = Recalld::new(config(&path)).await.unwrap();
        let storage = core.storage.read().unwrap();
        assert_eq!(storage.control_state().unwrap().control_schema_version, 2);
        let namespace = storage.get_namespace_by_name("default").unwrap().unwrap();
        assert!(matches!(
            namespace.schema.unwrap().embedding_binding,
            crate::model::schema::EmbeddingBinding::UnknownLegacy
        ));
        drop(storage);
        core.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn fresh_passthrough_does_not_invent_a_profile_or_create_database() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("db");
        let mut config = config(&path);
        config.embedding.provider = crate::config::EmbeddingProvider::Passthrough;
        assert!(Recalld::new(config).await.is_err());
        assert!(!path.join("meta.db").exists());
    }

    #[tokio::test]
    async fn native_startup_preserves_enabled_features() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("db");
        let mut config = config(&path);
        config.decay.disable_sweep = false;
        config.rif.enabled = true;
        config.graph.autolink_enabled = true;
        let core = Recalld::new(config).await.unwrap();
        assert!(!core.config.decay.disable_sweep);
        assert!(core.config.rif.enabled);
        assert!(core.config.graph.autolink_enabled);
        assert!(core.decay_runner.is_some());
        core.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn prepared_runtime_has_no_background_writer_before_commit() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("db");
        let mut config = config(&path);
        config.decay.disable_sweep = false;
        let provider = build_embedding(&config).await.unwrap();
        let clock = Arc::new(crate::time::ManualClock::new(1000));
        let (engine, transaction) = open_storage(
            &config,
            clock.as_ref(),
            Box::new(crate::storage::ids::SystemIds),
            provider.as_ref(),
        )
        .unwrap();
        assert!(transaction.is_none());
        let core = Recalld::with_prepared_storage(config, clock, engine, provider)
            .await
            .unwrap();
        assert!(core.decay_runner.is_some());
        assert!(core.decay_handle.is_none());
        let before = core
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
        tokio::task::yield_now().await;
        assert_eq!(
            before,
            core.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .generation_ref()
        );
        drop(core);
        RedbStorageEngine::open_native(&path).expect("all runtime storage handles closed");
    }

    #[tokio::test]
    async fn late_runtime_failure_releases_handles_before_rollback() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("db");
        drop(RedbStorageEngine::open(&path).unwrap());
        let before = std::fs::read(path.join("meta.db")).unwrap();
        let mut config = config(&path);
        config.decay.disable_sweep = false;
        config.decay.sweep_interval_hours = f64::NAN;
        let provider = build_embedding(&config).await.unwrap();
        let clock = Arc::new(crate::time::ManualClock::new(1000));
        let (mut engine, transaction) = crate::storage::automatic_upgrade::prepare_upgrade(
            &path,
            1000,
            Box::new(crate::storage::ids::SystemIds),
        )
        .unwrap();
        // Synthetic test setup: an empty legacy store has no vector provenance
        // to invent. Add a native namespace so the real initializer reaches its
        // async publication and late runner-validation failure. Production
        // never uses this to bind migrated namespaces or accept user writes.
        engine
            .create_native_namespace_with_provider(
                &crate::storage::control::MutationContext {
                    store_id: None,
                    scope: crate::storage::control::MutationScope::Catalog,
                    idempotency_key: None,
                    expected_generation: None,
                    contract_version: 2,
                },
                &crate::storage::native_catalog::NativeNamespaceInput {
                    name: "default".into(),
                    embedding_dim: None,
                    embedding_profile_id: None,
                    initial_stability: None,
                    desired_retention: None,
                    decay_rate_multiplier: None,
                },
                provider.as_ref(),
                &crate::storage::native_lifecycle::NativePolicyInputs::default(),
                clock.as_ref(),
                None,
            )
            .unwrap();
        let error = Recalld::with_prepared_storage(config, clock, engine, provider)
            .await
            .err()
            .expect("late runtime failure");
        assert!(matches!(
            error,
            RecalldError::Init {
                step: "prepare_decay_runner",
                ..
            }
        ));
        transaction
            .abort()
            .expect("no runtime handles may prevent rollback");
        assert_eq!(before, std::fs::read(path.join("meta.db")).unwrap());
        assert!(
            RedbStorageEngine::open(&path)
                .unwrap()
                .list_namespaces()
                .unwrap()
                .is_empty()
        );
    }
}
