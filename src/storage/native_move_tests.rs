use super::*;
use crate::embedding::{EmbeddingError, EmbeddingProvider};
use crate::model::{DecayPhase, NamespaceConfig};
use crate::storage::control::MutationOutcome;
use crate::storage::native_catalog::NativeNamespaceInput;
use crate::storage::native_lifecycle::{NativeLifecycleSession, NativePolicyInputs};
use crate::storage::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
use crate::storage::{RedbStorageEngine, StorageEngine};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub(crate) struct TestProvider {
    pub calls: AtomicUsize,
    pub fail_at: AtomicUsize,
}
#[async_trait::async_trait]
impl EmbeddingProvider for TestProvider {
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "move-fixture"
    }
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        Some(crate::embedding::runtime::tests::profile())
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, EmbeddingError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_at.load(Ordering::SeqCst) == call {
            return Err(EmbeddingError::InvalidResponse(
                "injected move provider failure".into(),
            ));
        }
        Ok(vec![0., 2., 0., 0.])
    }
}

/// Isolated on-disk source: unknown legacy vectors, real Summary loss history,
/// and parent/supersedes edges. No runtime configuration or personal paths.
pub(crate) fn fixture(
    root: &Path,
) -> (
    PathBuf,
    NativeLifecycleSession,
    Arc<TestProvider>,
    Vec<MemoryId>,
) {
    let source = root.join("legacy");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let mut config = NamespaceConfig::default_namespace(100);
    config.embedding_dim = 4;
    let namespace = engine.create_namespace(&config).unwrap();
    let mut ids = Vec::new();
    for i in 0..2 {
        let input = crate::storage::insertion::InsertMemory {
            id: None,
            summary: format!("move memory {i}"),
            full_text: Some(format!("retained full text {i}")),
            tags: vec![crate::model::Tag::new("entity/Alice").unwrap()],
            initial_stability: None,
            created_at: Some(1000 + i),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: ids.first().copied(),
            supersedes: ids.first().copied(),
        };
        let receipt = engine
            .insert_memory_with_receipt(&MutationContext::unkeyed(namespace), &input, &[], 1000 + i)
            .unwrap();
        ids.push(crate::storage::insertion::inserted_id(&receipt).unwrap());
    }
    let row = engine.get_record(ids[1]).unwrap().unwrap();
    let namespace_config = engine.get_namespace(namespace).unwrap().unwrap();
    engine
        .apply_legacy_decay_with_receipt(
            &MutationContext::unkeyed(namespace),
            ids[1],
            &row,
            &namespace_config,
            DecayPhase::Summary,
            0.01,
            0.01,
        )
        .unwrap();
    engine.sync().unwrap();
    drop(engine);
    let target = root.join("native");
    migrate_schema(
        &source,
        &target,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let provider = Arc::new(TestProvider {
        calls: AtomicUsize::new(0),
        fail_at: AtomicUsize::new(0),
    });
    let mut session = NativeLifecycleSession::open(
        &target,
        NativePolicyInputs::default(),
        Arc::new(crate::time::ManualClock::new(3000)),
    )
    .unwrap();
    for name in ["project-mtx", "project-next"] {
        let ctx = MutationContext {
            store_id: Some(session.engine.control_state().unwrap().store_id),
            scope: MutationScope::Catalog,
            idempotency_key: Some(format!("create-{name}")),
            expected_generation: None,
            contract_version: 1,
        };
        session
            .create_namespace(
                &ctx,
                &NativeNamespaceInput {
                    name: name.into(),
                    embedding_dim: None,
                    embedding_profile_id: None,
                    initial_stability: None,
                    desired_retention: None,
                    decay_rate_multiplier: None,
                },
                provider.as_ref(),
            )
            .unwrap();
    }
    (target, session, provider, ids)
}

pub(crate) fn request(
    engine: &RedbStorageEngine,
    ids: Vec<MemoryId>,
    key: &str,
    source: &str,
    destination: &str,
) -> NativeMoveInput {
    NativeMoveInput {
        store_id: engine.control_state().unwrap().store_id,
        idempotency_key: key.into(),
        source_namespace: source.into(),
        destination_namespace: destination.into(),
        ids,
        embedding_mode: NativeMoveEmbeddingMode::Regenerate,
    }
}

async fn move_records(
    engine: &mut RedbStorageEngine,
    input: NativeMoveInput,
    provider: &dyn EmbeddingProvider,
) -> Result<WriteReceipt, StorageError> {
    if let Some(receipt) = engine.meta_store().lookup_native_move(&input, None)? {
        return Ok(receipt);
    }
    let capture = engine.capture_native_move(input, None)?;
    let mut generated = Vec::new();
    for document in &capture.documents {
        generated.push(
            crate::embedding::runtime::generate_native(
                provider,
                &capture.destination,
                &document.text,
            )
            .await
            .map_err(|e| StorageError::ProviderVectorUnavailable(e.to_string()))?,
        );
    }
    engine.move_native_prepared(
        &capture,
        &generated,
        &Arc::new(std::sync::Mutex::new(MoveProgress::Preparing)),
        None,
    )
}

fn assert_preserved(before: &crate::model::DiskRecord, after: &crate::model::DiskRecord) {
    let mut expected = before.clone();
    expected.namespace_id = after.namespace_id;
    expected.vector_slot = after.vector_slot;
    let schema = expected.schema.as_mut().unwrap();
    schema.vector = after.schema.as_ref().unwrap().vector.clone();
    schema.embedding_origin = None;
    schema.lifecycle.record_generation = schema.lifecycle.record_generation.next().unwrap();
    assert_eq!(expected.encode().unwrap(), after.encode().unwrap());
}

#[tokio::test]
async fn move_twice_preserves_records_edges_and_summary_loss_on_strict_reopen() {
    let root = tempfile::tempdir().unwrap();
    let (path, mut session, provider, ids) = fixture(root.path());
    let original: Vec<_> = ids
        .iter()
        .map(|id| session.engine.get_record(*id).unwrap().unwrap())
        .collect();
    let edges = session.engine.load_all_edges().unwrap();
    let first = request(
        &session.engine,
        ids.clone(),
        "first",
        "default",
        "project-mtx",
    );
    let receipt = move_records(&mut session.engine, first.clone(), provider.as_ref())
        .await
        .unwrap();
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativeMemoriesMoved { .. }
    ));
    let once: Vec<_> = ids
        .iter()
        .map(|id| session.engine.get_record(*id).unwrap().unwrap())
        .collect();
    for (before, after) in original.iter().zip(&once) {
        assert_preserved(before, after);
    }
    let second = request(
        &session.engine,
        vec![ids[0]],
        "second",
        "project-mtx",
        "project-next",
    );
    let second_receipt = move_records(&mut session.engine, second.clone(), provider.as_ref())
        .await
        .unwrap();
    assert_preserved(
        &once[0],
        &session.engine.get_record(ids[0]).unwrap().unwrap(),
    );
    assert_eq!(session.engine.load_all_edges().unwrap(), edges);
    let calls = provider.calls.load(Ordering::SeqCst);
    let replay = move_records(&mut session.engine, first.clone(), provider.as_ref())
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
    let mut conflict = first;
    conflict.ids.reverse();
    assert!(matches!(
        move_records(&mut session.engine, conflict, provider.as_ref()).await,
        Err(StorageError::IdempotencyConflict)
    ));
    drop(session);
    let mut engine = RedbStorageEngine::open_native(&path).unwrap();
    assert_eq!(engine.load_all_edges().unwrap(), edges);
    assert_eq!(
        engine.get_record(ids[1]).unwrap().unwrap().phase,
        DecayPhase::Summary
    );
    // A further allocation after reopen must not free or overwrite either
    // destination vector when source slots have been returned to their stores.
    let third = request(
        &engine,
        vec![ids[1]],
        "third",
        "project-mtx",
        "project-next",
    );
    move_records(&mut engine, third, provider.as_ref())
        .await
        .unwrap();
    engine
        .begin_native_purge_named_admitted("project-mtx", false, None, 2 * 1024 * 1024)
        .unwrap()
        .unwrap();
    engine.resume_pending_purges().unwrap();
    assert!(
        engine
            .get_namespace_by_name("project-mtx")
            .unwrap()
            .is_none()
    );
    let replay = move_records(&mut engine, second, provider.as_ref())
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, second_receipt.operation_id);
    drop(engine);
    crate::storage::offline_snapshot::OfflineSnapshot::open(&path).unwrap();
}

#[tokio::test]
async fn native_creation_access_metadata_and_structured_labels_survive_round_trip() {
    use crate::storage::native_lifecycle::LifecycleScope;
    use crate::storage::native_records::NativeInsert;
    let root = tempfile::tempdir().unwrap();
    let (path, mut session, provider, _) = fixture(root.path());
    let ns = session
        .engine
        .get_namespace_by_name("default")
        .unwrap()
        .unwrap()
        .id;
    let id = MemoryId::from_uuid(uuid::Uuid::from_u128(404));
    let scope = LifecycleScope {
        namespace_id: ns,
        namespace_name: "default".into(),
        source: None,
    };
    let input = NativeInsert {
        memory: crate::storage::insertion::InsertMemory {
            id: Some(id),
            summary: "native structured labels".into(),
            full_text: Some("native text preserved".into()),
            tags: vec![crate::model::Tag::new("topic/Rust").unwrap()],
            initial_stability: None,
            created_at: Some(2000),
            embedding: None,
            parent_id: None,
            supersedes: None,
        },
        observed_at: Some(2500),
        entities: vec!["José".into()],
        topics: vec!["Storage".into()],
        emotions: vec!["careful".into()],
        embedding_profile: None,
    };
    session
        .insert_generated(
            &MutationContext::unkeyed(ns),
            scope.clone(),
            &input,
            provider.as_ref(),
        )
        .await
        .unwrap();
    let exposure = session
        .capture_exposure(scope, &[(id, crate::model::AccessKind::DirectRetrieval)])
        .unwrap();
    session
        .record_exposure(&MutationContext::unkeyed(ns), &exposure)
        .unwrap()
        .unwrap();
    let before = session.engine.get_record(id).unwrap().unwrap();
    assert!(!before.access_history.is_empty());
    let first = request(
        &session.engine,
        vec![id],
        "native-first",
        "default",
        "project-mtx",
    );
    move_records(&mut session.engine, first, provider.as_ref())
        .await
        .unwrap();
    assert_preserved(&before, &session.engine.get_record(id).unwrap().unwrap());
    for (key, source, destination) in [
        ("native-second", "project-mtx", "project-next"),
        ("native-return", "project-next", "project-mtx"),
    ] {
        let before = session.engine.get_record(id).unwrap().unwrap();
        let input = request(&session.engine, vec![id], key, source, destination);
        move_records(&mut session.engine, input, provider.as_ref())
            .await
            .unwrap();
        assert_preserved(&before, &session.engine.get_record(id).unwrap().unwrap());
    }
    drop(session);
    crate::storage::offline_snapshot::OfflineSnapshot::open(&path).unwrap();
    RedbStorageEngine::open_native(&path).unwrap();
}

#[tokio::test]
async fn invalid_batch_member_and_provider_failure_leave_every_record_in_source() {
    let root = tempfile::tempdir().unwrap();
    let (_, mut session, provider, ids) = fixture(root.path());
    let before = session.engine.control_state().unwrap();
    let originals: Vec<_> = ids
        .iter()
        .map(|id| {
            session
                .engine
                .get_record(*id)
                .unwrap()
                .unwrap()
                .encode()
                .unwrap()
        })
        .collect();
    let mut bad = request(
        &session.engine,
        ids.clone(),
        "bad-id",
        "default",
        "project-mtx",
    );
    bad.ids[1] = MemoryId::from_uuid(uuid::Uuid::from_u128(9999));
    assert!(
        move_records(&mut session.engine, bad, provider.as_ref())
            .await
            .is_err()
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    provider.fail_at.store(2, Ordering::SeqCst);
    let input = request(
        &session.engine,
        ids.clone(),
        "provider-failure",
        "default",
        "project-mtx",
    );
    assert!(
        move_records(&mut session.engine, input, provider.as_ref())
            .await
            .is_err()
    );
    assert_eq!(session.engine.control_state().unwrap(), before);
    for (id, bytes) in ids.iter().zip(originals) {
        assert_eq!(
            session
                .engine
                .get_record(*id)
                .unwrap()
                .unwrap()
                .encode()
                .unwrap(),
            bytes
        );
    }
}

#[tokio::test]
async fn move_crash_child() {
    let Ok(path) = std::env::var("RECALLD_MOVE_TEST_PATH") else {
        return;
    };
    let ids: Vec<MemoryId> =
        serde_json::from_str(&std::env::var("RECALLD_MOVE_TEST_IDS").unwrap()).unwrap();
    let mut engine = RedbStorageEngine::open_native(Path::new(&path)).unwrap();
    let provider = TestProvider {
        calls: AtomicUsize::new(0),
        fail_at: AtomicUsize::new(0),
    };
    let input = request(&engine, ids, "crash-move", "default", "project-mtx");
    move_records(&mut engine, input, &provider).await.unwrap();
    panic!("move crash boundary did not execute");
}

#[tokio::test]
async fn move_payload_and_commit_crashes_recover_atomically_and_replay() {
    for (boundary, committed) in [
        ("after_move_payload_sync", false),
        ("before_move_metadata_commit", false),
        ("after_move_metadata_commit", true),
        ("after_vector_sync", true),
        ("after_canonical_ack", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (path, session, provider, ids) = fixture(root.path());
        let input = request(
            &session.engine,
            ids.clone(),
            "crash-move",
            "default",
            "project-mtx",
        );
        let source = session
            .engine
            .get_record(ids[0])
            .unwrap()
            .unwrap()
            .namespace_id;
        let target = session
            .engine
            .get_namespace_by_name("project-mtx")
            .unwrap()
            .unwrap()
            .id
            .get();
        drop(session);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::native_move::tests::move_crash_child",
                "--nocapture",
            ])
            .env("RECALLD_MOVE_TEST_PATH", &path)
            .env(
                "RECALLD_MOVE_TEST_IDS",
                serde_json::to_string(&ids).unwrap(),
            )
            .env("RECALLD_TEST_EFFECT_CRASH", boundary)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(91),
            "{boundary}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut engine = RedbStorageEngine::open_native(&path).unwrap();
        for id in &ids {
            assert_eq!(
                engine.get_record(*id).unwrap().unwrap().namespace_id,
                if committed { target } else { source }
            );
        }
        let result = move_records(&mut engine, input, provider.as_ref())
            .await
            .unwrap();
        assert_eq!(result.replayed, committed);
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            if committed { 0 } else { 2 }
        );
        drop(engine);
        crate::storage::offline_snapshot::OfflineSnapshot::open(&path).unwrap();
    }
}

#[tokio::test]
async fn move_export_and_serving_restore_preserve_history_without_resurrecting_later_loss() {
    use crate::storage::{logical_export, offline_snapshot::OfflineSnapshot};
    let root = tempfile::tempdir().unwrap();
    let (path, session, provider, ids) = fixture(root.path());
    let source_id = session
        .engine
        .get_record(ids[0])
        .unwrap()
        .unwrap()
        .namespace_id;
    drop(session);
    let config = crate::config::RecalldConfig::default();
    let before =
        crate::backup::run_backup(&config, &root.path().join("before.zip"), Some(&path), false)
            .await
            .unwrap();
    let mut session = NativeLifecycleSession::open(
        &path,
        NativePolicyInputs::default(),
        Arc::new(crate::time::ManualClock::new(4000)),
    )
    .unwrap();
    let input = request(
        &session.engine,
        ids.clone(),
        "restore-move",
        "default",
        "project-mtx",
    );
    let receipt = move_records(&mut session.engine, input.clone(), provider.as_ref())
        .await
        .unwrap();
    let destination_id = session
        .engine
        .get_record(ids[0])
        .unwrap()
        .unwrap()
        .namespace_id;
    let edges = session.engine.load_all_edges().unwrap();
    drop(session);
    let after =
        crate::backup::run_backup(&config, &root.path().join("after.zip"), Some(&path), false)
            .await
            .unwrap();
    let export = root.path().join("moved.jsonl");
    logical_export::export_store(&path, &export).unwrap();
    let imported = root.path().join("imported");
    logical_export::import_store(&export, &imported).unwrap();
    {
        let original = OfflineSnapshot::open(&path).unwrap();
        let copy = OfflineSnapshot::open(&imported).unwrap();
        for id in &ids {
            let mut expected = original.record(*id).unwrap().unwrap();
            let actual = copy.record(*id).unwrap().unwrap();
            assert_eq!(expected.full_text, actual.full_text);
            assert_eq!(expected.embedding, actual.embedding);
            expected.record.text_offset = actual.record.text_offset;
            assert_eq!(
                expected.record.encode().unwrap(),
                actual.record.encode().unwrap()
            );
        }
    }
    let mut imported_engine = RedbStorageEngine::open_native(&imported).unwrap();
    assert_eq!(imported_engine.load_all_edges().unwrap(), edges);
    let replay = move_records(&mut imported_engine, input.clone(), provider.as_ref())
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    drop(imported_engine);

    let mut session = NativeLifecycleSession::open(
        &path,
        NativePolicyInputs::default(),
        Arc::new(crate::time::ManualClock::new(5000)),
    )
    .unwrap();
    let ns = crate::model::NamespaceId::new(destination_id);
    let intent = session.capture_tombstone(ids[1], Some(ns)).unwrap();
    session
        .tombstone(&MutationContext::unkeyed(ns), &intent)
        .unwrap();
    drop(session);
    for (backup, expected_namespace) in [(before, source_id), (after, destination_id)] {
        crate::backup::restore_from_backup(crate::backup::RestoreOptions {
            backup_path: backup,
            data_dir: path.clone(),
            force: true,
            no_stop_daemon: true,
            historical: false,
        })
        .await
        .unwrap();
        let mut engine = RedbStorageEngine::open_native(&path).unwrap();
        let retained = engine.get_record(ids[0]).unwrap().unwrap();
        assert_eq!(retained.namespace_id, expected_namespace);
        assert_eq!(retained.phase, DecayPhase::Full);
        let removed = engine.get_record(ids[1]).unwrap().unwrap();
        assert_eq!(removed.namespace_id, expected_namespace);
        assert_eq!(removed.phase, DecayPhase::Tombstone);
        assert!(removed.summary.is_empty());
        assert_eq!(removed.text_length, 0);
        // Restoring forks the store identity; old keys remain historical evidence,
        // not authority to mutate the new lineage.
        assert!(matches!(
            move_records(&mut engine, input.clone(), provider.as_ref()).await,
            Err(StorageError::StoreLineageMismatch)
        ));
        drop(engine);
        let history = crate::storage::metadata::restoration::capture(&path).unwrap();
        assert!(history.receipts.iter().any(|(_, bytes)| {
            control::validate_receipt(bytes)
                .unwrap()
                .receipt
                .operation_id
                == receipt.operation_id
        }));
        let roundtrip = root
            .path()
            .join(format!("restore-{expected_namespace}.jsonl"));
        logical_export::export_store(&path, &roundtrip).unwrap();
        let copy = root
            .path()
            .join(format!("restore-{expected_namespace}-import"));
        logical_export::import_store(&roundtrip, &copy).unwrap();
        RedbStorageEngine::open_native(&copy).unwrap();
    }
}

#[tokio::test]
async fn move_after_certification_cutover_survives_numeric_and_lifecycle_policy_changes() {
    use crate::storage::{offline_snapshot::OfflineSnapshot, vector_certification};
    let root = tempfile::tempdir().unwrap();
    let (source, cutover, provider) = vector_certification::tests::fixture(root.path());
    let path = root.path().join("certified");
    vector_certification::reembed_fresh(&source, &path, &cutover, &provider)
        .await
        .unwrap();
    let snapshot = OfflineSnapshot::open(&path).unwrap();
    let mut ids = Vec::new();
    snapshot
        .for_each_record(|row| {
            if row.record.namespace_id == cutover.namespace_id.get()
                && matches!(row.record.phase, DecayPhase::Full | DecayPhase::Summary)
            {
                ids.push(MemoryId::from_bytes(row.record.id));
            }
            Ok(())
        })
        .unwrap();
    assert!(!ids.is_empty());
    drop(snapshot);
    let mut session = NativeLifecycleSession::open(
        &path,
        NativePolicyInputs::default(),
        Arc::new(crate::time::ManualClock::new(100_000_000)),
    )
    .unwrap();
    session
        .create_namespace(
            &MutationContext {
                scope: MutationScope::Catalog,
                ..MutationContext::unkeyed(cutover.namespace_id)
            },
            &NativeNamespaceInput {
                name: "moved-certified".into(),
                embedding_dim: None,
                embedding_profile_id: None,
                initial_stability: None,
                desired_retention: None,
                decay_rate_multiplier: None,
            },
            &provider,
        )
        .unwrap();
    let input = request(
        &session.engine,
        ids.clone(),
        "cutover-move",
        &cutover.namespace_name,
        "moved-certified",
    );
    let before_rejection = session.engine.control_state().unwrap();
    assert!(matches!(
        move_records(&mut session.engine, input.clone(), &provider).await,
        Err(StorageError::InvalidMutation(_))
    ));
    assert_eq!(session.engine.control_state().unwrap(), before_rejection);
    let destination = session
        .engine
        .get_namespace_by_name("moved-certified")
        .unwrap()
        .unwrap();
    let alignment = crate::storage::lifecycle_migration::LifecycleMigrationOptions {
        expected_source: before_rejection.generation_ref(),
        namespace_id: destination.id,
        expected_policy_revision: destination.schema.as_ref().unwrap().policy_revision,
        target: crate::model::schema::LifecycleVersion::ExplicitFeedbackV2,
        idempotency_key: "align-destination-policy".into(),
    };
    drop(session);
    let aligned = root.path().join("aligned");
    crate::storage::lifecycle_migration::migrate_lifecycle(&path, &aligned, &alignment).unwrap();
    let path = aligned;
    let mut session = NativeLifecycleSession::open(
        &path,
        NativePolicyInputs::default(),
        Arc::new(crate::time::ManualClock::new(100_000_000)),
    )
    .unwrap();
    move_records(&mut session.engine, input.clone(), &provider)
        .await
        .unwrap();
    let ns = session
        .engine
        .get_namespace_by_name("moved-certified")
        .unwrap()
        .unwrap();
    let mut policy = crate::storage::catalog::NamespacePolicy::from(&ns);
    policy.initial_stability += 1.;
    session
        .update_policy(
            &MutationContext::unkeyed(ns.id),
            crate::storage::native_lifecycle::LifecycleScope {
                namespace_id: ns.id,
                namespace_name: ns.name.clone(),
                source: None,
            },
            &policy,
        )
        .unwrap()
        .unwrap();
    let current = session.engine.get_namespace(ns.id).unwrap().unwrap();
    let options = crate::storage::lifecycle_migration::LifecycleMigrationOptions {
        expected_source: session.engine.control_state().unwrap().generation_ref(),
        namespace_id: ns.id,
        expected_policy_revision: current.schema.as_ref().unwrap().policy_revision,
        target: crate::model::schema::LifecycleVersion::LegacyAccessV1,
        idempotency_key: "after-move-policy".into(),
    };
    drop(session);
    RedbStorageEngine::open_native(&path).unwrap();
    let migrated = root.path().join("policy-migrated");
    crate::storage::lifecycle_migration::migrate_lifecycle(&path, &migrated, &options).unwrap();
    let mut engine = RedbStorageEngine::open_native(&migrated).unwrap();
    assert!(
        move_records(&mut engine, input, &provider)
            .await
            .unwrap()
            .replayed
    );
    for id in ids {
        let record = engine.get_record(id).unwrap().unwrap();
        assert_eq!(record.namespace_id, ns.id.get());
        assert_eq!(
            record.schema.unwrap().lifecycle.lifecycle_version,
            crate::model::schema::LifecycleVersion::LegacyAccessV1
        );
    }
    drop(engine);
    let export = root.path().join("policy.jsonl");
    crate::storage::logical_export::export_store(&migrated, &export).unwrap();
    let imported = root.path().join("policy-import");
    crate::storage::logical_export::import_store(&export, &imported).unwrap();
    RedbStorageEngine::open_native(&imported).unwrap();
}
