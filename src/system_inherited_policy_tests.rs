//! Actual143 restart consumer: explicit adoption, never fixture blessing.
//! Derived from genuine BEFORE probe 528cf28; the None fixture stays unknown until the real operator flow.
use super::*;
use crate::model::schema::{LifecycleVersion, ProviderKind};
use crate::model::{MemoryId, NamespaceConfig};
use crate::storage::StorageEngine as _;
use crate::storage::control::{Generation, MutationContext};
use crate::storage::insertion::{InsertMemory, inserted_id};
use crate::storage::lifecycle_migration::{LifecycleMigrationOptions, migrate_lifecycle};
use crate::storage::schema_migration::{
    MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema,
};

const NOW: i64 = 30 * 86_400_000;

async fn restart_case(override_value: Option<f32>) -> [f32; 2] {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join("legacy");
    let schema = root.path().join("schema");
    let native = root.path().join("native");
    let mut config = RecalldConfig::default();
    config.storage.data_dir = native.to_str().unwrap().into();
    config.embedding.provider = crate::config::EmbeddingProvider::Ollama;
    config.embedding.model_name = "synthetic-restart-audit".into();
    config.embedding.dimensions = 4;
    config.embedding.base_url = "http://127.0.0.1:1".into();
    config.embedding.document_prefix.clear();
    config.embedding.query_prefix.clear();
    config.decay.disable_sweep = true;
    config.rif.enabled = false;
    config.graph.autolink_enabled = false;
    let mut engine = RedbStorageEngine::open(&legacy).unwrap();
    let mut namespace = NamespaceConfig::default_namespace(1000);
    namespace.embedding_dim = 4;
    namespace.decay_rate_multiplier = override_value;
    let ns = engine.create_namespace(&namespace).unwrap();
    let id: MemoryId = inserted_id(
        &engine
            .insert_memory_with_receipt(
                &MutationContext::unkeyed(ns),
                &InsertMemory {
                    id: None,
                    summary: "synthetic restart audit".into(),
                    full_text: Some("retained synthetic payload".into()),
                    tags: vec![],
                    initial_stability: Some(1.),
                    created_at: Some(1000),
                    embedding: Some(vec![1., 0., 0., 0.]),
                    parent_id: None,
                    supersedes: None,
                },
                &[],
                1000,
            )
            .unwrap(),
    )
    .unwrap();
    engine.sync().unwrap();
    drop(engine);
    // Explicit operator assertion for this synthetic fixture, not inferred from width.
    let descriptor = crate::embedding::runtime::native_profile(
        ProviderKind::Ollama,
        &config.embedding.model_name,
        4,
    );
    migrate_schema(
        &legacy,
        &schema,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&legacy).unwrap().control.generation_ref(),
            attestations: vec![ProfileAttestationInput {
                namespace_id: ns,
                namespace_name: "default".into(),
                descriptor,
            }],
        },
    )
    .unwrap();
    migrate_lifecycle(
        &schema,
        &native,
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(&schema).unwrap().control.generation_ref(),
            namespace_id: ns,
            expected_policy_revision: Generation::default(),
            target: LifecycleVersion::ExplicitFeedbackV2,
            idempotency_key: "explicit-fixture-policy".into(),
        },
    )
    .unwrap();
    let clock = Arc::new(crate::time::ManualClock::new(NOW));
    let mut values = [0.; 2];
    for (index, multiplier) in [1., 0.].into_iter().enumerate() {
        config.decay.decay_rate_multiplier = multiplier;
        let file = root.path().join("config.toml");
        std::fs::write(&file, toml::to_string(&config).unwrap()).unwrap();
        let loaded = crate::config::load_config(Some(&file), &Default::default())
            .unwrap()
            .config;
        assert_eq!(
            serde_json::to_value(&loaded).unwrap(),
            serde_json::to_value(&config).unwrap()
        );
        if override_value.is_none() {
            let before = inspect_schema(&native).unwrap();
            let engine = RedbStorageEngine::open_native(&native).unwrap();
            let core = Recalld::with_owned_storage(loaded.clone(), clock.clone(), engine)
                .await
                .unwrap();
            assert!(
                core.query_engine()
                    .get_memory_with_access(id, false)
                    .await
                    .is_err()
            );
            assert!(
                core.storage()
                    .read()
                    .unwrap()
                    .meta_store()
                    .capture_feedback(
                        id,
                        Some(ns),
                        3,
                        &crate::storage::native_lifecycle::NativePolicyInputs {
                            multiplier,
                            ..Default::default()
                        }
                    )
                    .is_err()
            );
            use crate::api::FsrsEngine as _;
            let adapter = crate::api::adapters::FsrsEngineAdapter::new(
                core.storage.clone(),
                core.cache.clone(),
                core.graph.clone(),
                Arc::new(config.clone()),
                false,
            )
            .with_indexes(
                core.vector_index.clone(),
                core.fts_index.clone(),
                core.entity_index.clone(),
            )
            .with_clock(clock.clone())
            .with_native_lifecycle_policy(
                crate::storage::native_lifecycle::NativePolicyInputs {
                    multiplier,
                    ..Default::default()
                },
            );
            let failed = adapter.trigger_sweep(None).await.unwrap_err();
            assert!(failed.to_string().contains("inherited"));
            drop(adapter);
            core.shutdown().await.unwrap();
            assert_eq!(inspect_schema(&native).unwrap().control, before.control);
            let preview = crate::storage::inherited_policy::preview_inherited_decay(
                &native,
                ns,
                multiplier,
                before.control.generation_ref(),
                clock.as_ref(),
            )
            .unwrap();
            if index == 0 {
                assert!(preview.prior.is_none());
            } else {
                assert!(preview.prior.is_some());
            }
            let request = crate::storage::inherited_policy::AdoptInheritedDecay {
                context: MutationContext {
                    store_id: Some(before.control.store_id),
                    scope: crate::storage::control::MutationScope::Namespace(ns),
                    idempotency_key: Some(format!("explicit-adoption-{index}")),
                    expected_generation: Some(before.control.committed_generation),
                    contract_version: 1,
                },
                preview,
            };
            crate::storage::inherited_policy::adopt_inherited_decay(&native, &request).unwrap();
        }
        let baseline = inspect_schema(&native).unwrap();
        let baseline_ns = baseline.namespaces.iter().find(|n| n.id == ns).unwrap();
        let canonical_namespace = crate::storage::control::encode(baseline_ns).unwrap();
        let revision = baseline_ns.schema.as_ref().unwrap().policy_revision;
        let raw = crate::storage::offline_snapshot::OfflineSnapshot::open(&native).unwrap();
        let canonical_record = raw.record(id).unwrap().unwrap().record.encode().unwrap();
        drop(raw);
        let engine = RedbStorageEngine::open_native(&native).unwrap();
        let core = Recalld::with_owned_storage(loaded, clock.clone(), engine)
            .await
            .unwrap();
        let response = core
            .query_engine()
            .get_memory_with_access(id, false)
            .await
            .unwrap()
            .unwrap();
        values[index] = response.strength;
        {
            let store = core.storage().read().unwrap();
            assert_eq!(
                store.control_state().unwrap().generation_ref(),
                baseline.control.generation_ref()
            );
            let ns_after = store.get_namespace(ns).unwrap().unwrap();
            assert_eq!(
                crate::storage::control::encode(&ns_after).unwrap(),
                canonical_namespace
            );
            assert_eq!(ns_after.schema.as_ref().unwrap().policy_revision, revision);
            let encoded = store.get_record(id).unwrap().unwrap().encode().unwrap();
            assert_eq!(encoded, canonical_record);
            assert_eq!(response.phase, crate::model::DecayPhase::Full);
            println!(
                "AUDIT override={override_value:?} global={multiplier} generation={:?} policyRevision={revision:?} clock={NOW} strength={} strengthBits={} committedPhase={:?}",
                baseline.control.generation_ref(),
                response.strength,
                response.strength.to_bits(),
                response.phase
            );
        }
        if override_value.is_none() && index == 1 {
            use crate::api::FsrsEngine as _;
            let adapter = crate::api::adapters::FsrsEngineAdapter::new(
                core.storage.clone(),
                core.cache.clone(),
                core.graph.clone(),
                Arc::new(config.clone()),
                false,
            )
            .with_indexes(
                core.vector_index.clone(),
                core.fts_index.clone(),
                core.entity_index.clone(),
            )
            .with_clock(clock.clone())
            .with_native_lifecycle_policy(
                crate::storage::native_lifecycle::NativePolicyInputs {
                    multiplier,
                    ..Default::default()
                },
            );
            let swept = adapter.trigger_sweep(None).await.unwrap();
            assert!(swept.publication_error.is_none() && swept.errors.is_empty());
            assert_eq!(
                core.storage
                    .read()
                    .unwrap()
                    .control_state()
                    .unwrap()
                    .generation_ref(),
                baseline.control.generation_ref()
            );
            let completion = crate::derived_completion::DerivedCompletion {
                storage: core.storage.clone(),
                cache: core.cache.clone(),
                graph: core.graph.clone(),
                vectors: core.vector_index.clone(),
                fts: core.fts_index.clone(),
                entities: core.entity_index.clone(),
            };
            let mut ctx = MutationContext::unkeyed(ns);
            ctx.contract_version = 2;
            let receipt = completion
                .feedback_native_with_work(
                    ctx,
                    id,
                    3,
                    None,
                    crate::storage::native_lifecycle::NativePolicyInputs {
                        multiplier,
                        ..Default::default()
                    },
                    clock.clone(),
                    None,
                )
                .await
                .unwrap();
            assert!(matches!(
                receipt.outcome,
                crate::storage::control::MutationOutcome::LifecycleFeedbackApplied { .. }
            ));
            assert!(
                core.query_engine()
                    .get_memory_with_access(id, false)
                    .await
                    .unwrap()
                    .is_some()
            );
            drop(adapter);
        }
        core.shutdown().await.unwrap();
    }
    values
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inherited_policy_actual_restart_requires_explicit_adoption_before_changed_projection() {
    let explicit = restart_case(Some(1.)).await;
    assert_eq!(
        explicit[0].to_bits(),
        explicit[1].to_bits(),
        "explicit namespace override is the control"
    );
    let inherited = restart_case(None).await;
    assert_ne!(inherited[0].to_bits(), inherited[1].to_bits());
    assert_eq!(inherited[1], 1.);
}
