use super::*;
use crate::embedding::{EmbeddingError, EmbeddingProvider};
use crate::storage::RedbStorageEngine;
use crate::storage::control::MutationOutcome;
use crate::storage::native_lifecycle::{NativeLifecycleSession, NativePolicyInputs};
use crate::storage::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
use redb::ReadableTable;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Clock(AtomicUsize);
impl crate::time::Clock for Clock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        2000
    }
}
struct Provider(bool);
#[async_trait::async_trait]
impl EmbeddingProvider for Provider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, EmbeddingError> {
        panic!("catalog called embedding")
    }
    fn dimensions(&self) -> usize {
        assert!(self.0, "replay consulted provider");
        4
    }
    fn model_name(&self) -> &str {
        "catalog-fixture"
    }
    fn profile(&self) -> Option<EmbeddingProfile> {
        assert!(self.0, "replay consulted provider");
        Some(profile())
    }
}
fn profile() -> EmbeddingProfile {
    crate::embedding::runtime::tests::profile()
}
fn input() -> NativeNamespaceInput {
    NativeNamespaceInput {
        name: "created".into(),
        embedding_dim: None,
        embedding_profile_id: None,
        initial_stability: Some(3.),
        desired_retention: None,
        decay_rate_multiplier: None,
    }
}
fn fixture(root: &Path) -> (PathBuf, Arc<Clock>, NativeLifecycleSession) {
    let source = root.join("legacy");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let mut ns = NamespaceConfig::default_namespace(100);
    ns.embedding_dim = 4;
    engine.create_namespace(&ns).unwrap();
    engine.sync().unwrap();
    drop(engine);
    let target = root.join("native");
    migrate_schema(
        &source,
        &target,
        &MigrationOptions {
            migration_now: 1000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let clock = Arc::new(Clock(AtomicUsize::new(0)));
    let session =
        NativeLifecycleSession::open(&target, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    (target, clock, session)
}
fn context(session: &NativeLifecycleSession, key: &str) -> MutationContext {
    let state = session.engine.control_state().unwrap();
    MutationContext {
        store_id: Some(state.store_id),
        scope: MutationScope::Catalog,
        idempotency_key: Some(key.into()),
        expected_generation: Some(state.committed_generation),
        contract_version: 2,
    }
}
fn bytes(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            out.extend(bytes(&entry.path()));
        } else {
            out.insert(entry.path(), std::fs::read(entry.path()).unwrap());
        }
    }
    out
}

#[tokio::test]
async fn native_catalog_replay_before_provider_cas_and_real_record_reopen() {
    let root = tempfile::tempdir().unwrap();
    let (path, clock, mut session) = fixture(root.path());
    let ctx = context(&session, "create");
    let receipt = session
        .create_namespace(&ctx, &input(), &Provider(true))
        .unwrap();
    let MutationOutcome::NativeNamespaceCreated { transition } = &receipt.outcome else {
        panic!()
    };
    let id = transition.namespace_id;
    let ns = session.engine.get_namespace(id).unwrap().unwrap();
    let inherited = ns
        .schema
        .as_ref()
        .unwrap()
        .inherited_decay
        .as_ref()
        .unwrap();
    assert!(inherited.matches_receipt(&receipt));
    assert_eq!(inherited.multiplier_bits, 1f32.to_bits());
    assert_eq!(transition.inherited_decay.as_ref(), Some(inherited));
    assert!(crate::storage::inherited_policy::resolve_bound(&ns, &Default::default(), 1.).is_ok());
    assert!(crate::storage::inherited_policy::resolve_bound(&ns, &Default::default(), 0.).is_err());
    assert_eq!(
        ns.schema.as_ref().unwrap().lifecycle_version,
        crate::model::schema::LifecycleVersion::LegacyAccessV1
    );
    assert!(matches!(
        ns.schema.as_ref().unwrap().embedding_binding,
        EmbeddingBinding::Bound {
            provenance: BindingProvenance::NativeV2,
            ..
        }
    ));
    assert!(path.join("created/vectors.dat").is_file());
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let replay = session
        .create_namespace(&ctx, &input(), &Provider(false))
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(replay.committed_generation, receipt.committed_generation);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let memory = crate::storage::native_records::NativeInsert {
        memory: crate::storage::insertion::InsertMemory {
            id: None,
            summary: "native retained text".into(),
            full_text: None,
            tags: vec![],
            initial_stability: None,
            created_at: None,
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        },
        observed_at: None,
        entities: vec!["日本語".into()],
        topics: vec![],
        emotions: vec![],
        embedding_profile: Some(profile()),
    };
    let intent = session
        .capture_insert(
            crate::storage::native_lifecycle::LifecycleScope {
                namespace_id: id,
                namespace_name: ns.name.clone(),
                source: None,
            },
            &memory,
        )
        .unwrap();
    let mut insert_ctx = context(&session, "insert");
    insert_ctx.scope = MutationScope::Namespace(id);
    session
        .insert_prepared(
            &insert_ctx,
            &intent,
            &crate::embedding::runtime::GeneratedEmbedding::unverified(vec![], Some(profile())),
        )
        .unwrap();
    session.engine.sync().unwrap();
    drop(session);
    let mut reopened =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
    assert!(
        reopened
            .create_namespace(&ctx, &input(), &Provider(false))
            .unwrap()
            .replayed
    );
    assert!(reopened.engine.get_namespace(id).unwrap().is_some());
    drop(reopened);
    let before = bytes(&path);
    assert!(RedbStorageEngine::open(&path).is_err());
    assert_eq!(bytes(&path), before);

    // Real offline consumers must retain the creation proof and provider
    // binding, including its historical origin after a production fork.
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let stream = root.path().join("created.jsonl");
    let copy = root.path().join("copy");
    assert_eq!(
        export_store(&path, &stream).unwrap(),
        import_store(&stream, &copy).unwrap()
    );
    assert_eq!(bytes(&path), before);
    let snapshot = inspect_schema(&copy).unwrap();
    assert_eq!(snapshot.control.store_id, receipt.store_id);
    let backup = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("created.zip"),
        Some(&copy),
        false,
    )
    .await
    .unwrap();
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: copy.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let restored = inspect_schema(&copy).unwrap();
    assert_ne!(restored.control.store_id, receipt.store_id);
    assert_eq!(restored.schema, snapshot.schema);
    let mut reopened = NativeLifecycleSession::open(
        &copy,
        NativePolicyInputs::default(),
        Arc::new(Clock(AtomicUsize::new(0))),
    )
    .unwrap();
    assert_eq!(
        control::encode(&reopened.engine.get_namespace(id).unwrap().unwrap()).unwrap(),
        control::encode(&ns).unwrap()
    );
    assert!(matches!(
        reopened.create_namespace(&ctx, &input(), &Provider(false)),
        Err(StorageError::StoreLineageMismatch)
    ));
    drop(reopened);
    let snapshot = crate::storage::offline_snapshot::OfflineSnapshot::open(&copy).unwrap();
    let read = snapshot.metadata.begin_read().unwrap();
    let raw = read
        .open_table(control::RECEIPTS)
        .unwrap()
        .get(
            control::scoped_key(
                receipt.store_id,
                MutationScope::Catalog,
                receipt.operation_id.as_bytes(),
            )
            .as_slice(),
        )
        .unwrap()
        .unwrap()
        .value()
        .to_vec();
    assert_eq!(control::validate_receipt(&raw).unwrap().receipt, receipt);
}

#[test]
fn native_catalog_rejections_do_not_allocate_or_publish() {
    let root = tempfile::tempdir().unwrap();
    let (path, clock, mut session) = fixture(root.path());
    let ctx = context(&session, "bad");
    let control = session.engine.control_state().unwrap();
    for dim in [0, 5, 8193] {
        let mut bad = input();
        bad.embedding_dim = Some(dim);
        assert!(
            session
                .create_namespace(&ctx, &bad, &Provider(true))
                .is_err()
        );
        assert_eq!(session.engine.control_state().unwrap(), control);
        assert!(!path.join("created").exists());
    }
    let mut wrong = input();
    wrong.embedding_profile_id = Some(format!("ep1:{}", "0".repeat(64)));
    assert!(
        session
            .create_namespace(&ctx, &wrong, &Provider(true))
            .is_err()
    );
    let receipt = session
        .create_namespace(&ctx, &input(), &Provider(true))
        .unwrap();
    let committed = session.engine.control_state().unwrap();
    let mut changed = input();
    changed.initial_stability = Some(4.);
    assert!(matches!(
        session.create_namespace(&ctx, &changed, &Provider(false)),
        Err(StorageError::IdempotencyConflict)
    ));
    let mut foreign = ctx.clone();
    foreign.store_id = Some(uuid::Uuid::new_v4());
    assert!(matches!(
        session.create_namespace(&foreign, &input(), &Provider(false)),
        Err(StorageError::StoreLineageMismatch)
    ));
    assert_eq!(session.engine.control_state().unwrap(), committed);
    assert_eq!(committed.committed_generation, receipt.committed_generation);
    assert!(clock.0.load(Ordering::SeqCst) >= 1);
}

#[test]
fn native_catalog_publication_fault_reconciles_receipt_on_reopen() {
    for point in [
        "before_stage",
        "after_payload_publish",
        "before_metadata_commit",
        "after_metadata_commit",
        "before_handle_publish",
        "after_handle_publish",
        "commit_unknown",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (path, clock, mut session) = fixture(root.path());
        let ctx = context(&session, "fault");
        crate::storage::catalog::FAILURE.with(|f| f.set(Some(point)));
        let result = session.create_namespace(&ctx, &input(), &Provider(true));
        crate::storage::catalog::FAILURE.with(|f| f.set(None));
        assert!(result.is_err(), "{point}");
        let known_commit = matches!(
            point,
            "after_metadata_commit" | "before_handle_publish" | "after_handle_publish"
        );
        let carried = match &result {
            Err(StorageError::CommittedMutation { receipt, .. }) => Some((**receipt).clone()),
            _ => None,
        };
        assert_eq!(carried.is_some(), known_commit, "{point}: {result:?}");
        if point == "commit_unknown" {
            assert!(matches!(
                result,
                Err(StorageError::CommitOutcomeUnknown { .. })
            ));
        }
        if let Some(receipt) = &carried {
            // No post-error readback can manufacture the response: the owner
            // is poisoned, so the error must carry the receipt already in hand.
            assert!(
                session
                    .engine
                    .lookup_operation(
                        receipt.store_id,
                        MutationScope::Catalog,
                        receipt.operation_id,
                    )
                    .is_err()
            );
            assert!(!receipt.replayed);
        }
        drop(session);
        let mut reopened =
            NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
        let receipt = reopened
            .create_namespace(
                &ctx,
                &input(),
                &Provider(!known_commit && point != "commit_unknown"),
            )
            .unwrap();
        assert_eq!(
            receipt.replayed,
            matches!(
                point,
                "after_metadata_commit"
                    | "before_handle_publish"
                    | "after_handle_publish"
                    | "commit_unknown"
            ),
            "{point}"
        );
        if let Some(carried) = carried {
            let retained = reopened
                .engine
                .lookup_operation(
                    carried.store_id,
                    MutationScope::Catalog,
                    carried.operation_id,
                )
                .unwrap()
                .unwrap();
            assert_eq!(carried, retained);
            let mut original = receipt.clone();
            original.replayed = false;
            assert_eq!(carried, original);
        }
        assert!(path.join("created/vectors.dat").is_file());
    }
}

#[test]
fn forged_native_catalog_configuration_is_rejected_immutably() {
    let root = tempfile::tempdir().unwrap();
    let (path, _, mut session) = fixture(root.path());
    let ctx = context(&session, "create");
    let receipt = session
        .create_namespace(&ctx, &input(), &Provider(true))
        .unwrap();
    let MutationOutcome::NativeNamespaceCreated { transition } = receipt.outcome else {
        panic!()
    };
    drop(session);
    let db = redb::Database::open(path.join("meta.db")).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx
            .open_table(crate::storage::metadata::NAMESPACE_TABLE)
            .unwrap();
        let mut ns = NamespaceConfig::decode(
            table
                .get(transition.namespace_id.get())
                .unwrap()
                .unwrap()
                .value(),
        )
        .unwrap();
        ns.initial_stability += 1.;
        table
            .insert(ns.id.get(), control::encode(&ns).unwrap().as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let before = bytes(&path);
    assert!(inspect_schema(&path).is_err());
    assert_eq!(bytes(&path), before);
}

#[test]
fn native_catalog_review_expanded_fixed_defaults_replay_but_dynamic_selectors_conflict() {
    let root = tempfile::tempdir().unwrap();
    let (_, clock, mut session) = fixture(root.path());
    let ctx = context(&session, "defaults");
    let mut omitted = input();
    omitted.initial_stability = None;
    omitted.desired_retention = None;
    let receipt = session
        .create_namespace(&ctx, &omitted, &Provider(true))
        .unwrap();
    let defaults = NamespaceConfig::default_namespace(0);
    let mut explicit = omitted.clone();
    explicit.initial_stability = Some(defaults.initial_stability);
    explicit.desired_retention = Some(defaults.desired_retention);
    let replay = session
        .create_namespace(&ctx, &explicit, &Provider(false))
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, receipt.outcome);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(replay.committed_generation, receipt.committed_generation);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    for which in 0..3 {
        let mut dynamic = omitted.clone();
        match which {
            0 => dynamic.embedding_dim = Some(profile().dimension),
            1 => dynamic.embedding_profile_id = Some(profile().profile_id().unwrap()),
            _ => dynamic.decay_rate_multiplier = Some(1.),
        }
        assert!(matches!(
            session.create_namespace(&ctx, &dynamic, &Provider(false)),
            Err(StorageError::IdempotencyConflict)
        ));
    }
}

#[test]
fn native_catalog_review_invalid_inherited_policy_rejects_before_allocation() {
    for bad_global in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (path, _, mut session) = fixture(root.path());
        let ctx = context(&session, "invalid-policy");
        let control = session.engine.control_state().unwrap();
        let before = bytes(&path);
        if bad_global {
            session.policy.global.max_connection_bonus = f32::NAN;
        } else {
            session.policy.multiplier = f64::NAN;
        }
        assert!(
            session
                .create_namespace(&ctx, &input(), &Provider(true))
                .is_err()
        );
        assert_eq!(session.engine.control_state().unwrap(), control);
        assert_eq!(bytes(&path), before);
        assert!(!path.join("created").exists());
        if !bad_global {
            let mut overriding = input();
            overriding.decay_rate_multiplier = Some(2.);
            let receipt = session
                .create_namespace(&ctx, &overriding, &Provider(true))
                .unwrap();
            session.policy.global.max_connection_bonus = f32::NAN;
            let replay = session
                .create_namespace(&ctx, &overriding, &Provider(false))
                .unwrap();
            assert!(replay.replayed);
            assert_eq!(replay.outcome, receipt.outcome);
        }
    }
}

#[test]
fn native_context_api1_catalog_replay_cross_contract_and_unkeyed() {
    let root = tempfile::tempdir().unwrap();
    let (path, clock, mut session) = fixture(root.path());
    let mut key = context(&session, "original-api1");
    key.contract_version = 1;
    let receipt = session
        .create_namespace(&key, &input(), &Provider(true))
        .unwrap();
    let before = bytes(&path);
    let calls = clock.0.load(Ordering::SeqCst);
    let mut expected = receipt.clone();
    expected.replayed = true;
    assert_eq!(
        session
            .create_namespace(&key, &input(), &Provider(false))
            .unwrap(),
        expected
    );
    let mut other = key.clone();
    other.contract_version = 2;
    assert!(matches!(
        session.create_namespace(&other, &input(), &Provider(false)),
        Err(StorageError::IdempotencyConflict)
    ));
    let mut changed = input();
    changed.desired_retention = Some(0.8);
    assert!(matches!(
        session.create_namespace(&key, &changed, &Provider(false)),
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(bytes(&path), before);
    let mut unkeyed = key;
    unkeyed.idempotency_key = None;
    unkeyed.expected_generation = None;
    let mut fresh = input();
    fresh.name = "unkeyed-api1".into();
    session
        .create_namespace(&unkeyed, &fresh, &Provider(true))
        .unwrap();
    let v2_key = context(&session, "original-api2");
    let mut v2_input = input();
    v2_input.name = "api2-original".into();
    session
        .create_namespace(&v2_key, &v2_input, &Provider(true))
        .unwrap();
    let frozen = bytes(&path);
    let count = clock.0.load(Ordering::SeqCst);
    let mut wrong_api = v2_key;
    wrong_api.contract_version = 1;
    assert!(matches!(
        session.create_namespace(&wrong_api, &v2_input, &Provider(false)),
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(bytes(&path), frozen);
    assert_eq!(clock.0.load(Ordering::SeqCst), count);
    drop(session);
    let mut session =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
    other.contract_version = 1;
    assert_eq!(
        session
            .create_namespace(&other, &input(), &Provider(false))
            .unwrap(),
        expected
    );
}

#[path = "native_catalog_tests/owned_ids.rs"]
mod owned_ids;
