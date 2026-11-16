use super::*;
use crate::embedding::runtime::GeneratedEmbedding;
use crate::model::schema::LifecycleVersion;
use crate::storage::lifecycle_migration::{LifecycleMigrationOptions, migrate_lifecycle};
use crate::storage::native_lifecycle::NativeLifecycleSession;
use crate::storage::schema_migration::{
    MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema,
};
use crate::storage::{RedbStorageEngine, StorageEngine};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[path = "reader_tests.rs"]
mod readers;

#[path = "batch_tests.rs"]
mod batches;
#[path = "exact_scan_tests.rs"]
mod exact_scan;
#[path = "sweep_tests.rs"]
mod sweep;

#[path = "phase_clock_tests.rs"]
mod phase_clock;

#[path = "recovery_tests.rs"]
mod recovery;

struct FixedClock(AtomicUsize);
impl Clock for FixedClock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        86_400_000
    }
}
struct Data {
    path: PathBuf,
    ns: NamespaceId,
    parent: MemoryId,
    profile: crate::model::schema::EmbeddingProfile,
}
#[path = "certified_positive_fixture.rs"]
mod certified_positive_fixture;
use certified_positive_fixture::{certified_fixture, certified_fixture_with_seed};
fn fixture(root: &Path) -> Data {
    fixture_with_seed(root, |_| {})
}
fn fixture_with_ghost(root: &Path, ghost: bool) -> Data {
    fixture_with_seed_and_ghost(root, ghost, |_| {})
}
fn fixture_with_seed(root: &Path, seed: impl FnOnce(&mut RedbStorageEngine)) -> Data {
    fixture_with_seed_and_ghost(root, false, seed)
}
fn fixture_with_seed_and_ghost(
    root: &Path,
    ghost: bool,
    seed: impl FnOnce(&mut RedbStorageEngine),
) -> Data {
    fixture_with_policy(root, seed, ghost, None, true)
}
fn fixture_with_policy(
    root: &Path,
    seed: impl FnOnce(&mut RedbStorageEngine),
    ghost: bool,
    multiplier: Option<f32>,
    adopt: bool,
) -> Data {
    let mut config = NamespaceConfig::default_namespace(0);
    config.decay_rate_multiplier = multiplier;
    fixture_with_config(root, ghost, config, seed, adopt)
}
fn fixture_configured(
    root: &Path,
    ghost: bool,
    config: NamespaceConfig,
    seed: impl FnOnce(&mut RedbStorageEngine),
) -> Data {
    // This matrix already supplies an explicit namespace multiplier.
    fixture_with_config(root, ghost, config, seed, false)
}
fn fixture_with_config(
    root: &Path,
    ghost: bool,
    mut config: NamespaceConfig,
    seed: impl FnOnce(&mut RedbStorageEngine),
    adopt: bool,
) -> Data {
    let source = root.join("legacy");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    config.name = "native".into();
    config.embedding_dim = 4;
    let ns = engine.create_namespace(&config).unwrap();
    let input = crate::storage::insertion::InsertMemory {
        id: None,
        summary: "original parent".into(),
        full_text: Some("original parent full text".into()),
        tags: vec![],
        initial_stability: None,
        created_at: Some(1000),
        embedding: Some(vec![1., 0., 0., 0.]),
        parent_id: None,
        supersedes: None,
    };
    let receipt = engine
        .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &input, &[], 1000)
        .unwrap();
    let parent = crate::storage::insertion::inserted_id(&receipt).unwrap();
    seed(&mut engine);
    if ghost {
        // Real legacy phase-loss receipts before explicit migration/attestation.
        // Never author a schema-v2 row or bypass native startup validation.
        for phase in [DecayPhase::Summary, DecayPhase::Ghost] {
            let record = engine.get_record(parent).unwrap().unwrap();
            let namespace = engine.get_namespace(ns).unwrap().unwrap();
            assert!(matches!(
                engine
                    .apply_legacy_decay_with_receipt(
                        &MutationContext::unkeyed(ns),
                        parent,
                        &record,
                        &namespace,
                        phase,
                        0.01,
                        0.01,
                    )
                    .unwrap(),
                crate::storage::maintenance::DecayMutationResult::Committed(_)
            ));
        }
    }
    engine.sync().unwrap();
    drop(engine);
    let profile = crate::embedding::runtime::tests::profile();
    let canonical = root.join("schema");
    migrate_schema(
        &source,
        &canonical,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![ProfileAttestationInput {
                namespace_id: ns,
                namespace_name: "native".into(),
                descriptor: profile.clone(),
            }],
        },
    )
    .unwrap();
    let path = root.join("v2");
    migrate_lifecycle(
        &canonical,
        &path,
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(&canonical).unwrap().control.generation_ref(),
            namespace_id: ns,
            expected_policy_revision: Generation::default(),
            target: LifecycleVersion::ExplicitFeedbackV2,
            idempotency_key: "v2".into(),
        },
    )
    .unwrap();
    // Dependent native writer fixtures explicitly adopt present-day multiplier1.
    // The actual migration above still preserves unknown historical inheritance.
    if adopt {
        crate::test_support::explicitly_adopt_inherited_policy(&path, ns, 1., 2000);
    }
    Data {
        path,
        ns,
        parent,
        profile,
    }
}
fn scope(d: &Data) -> LifecycleScope {
    LifecycleScope {
        namespace_id: d.ns,
        namespace_name: "native".into(),
        source: None,
    }
}
fn policy_revision(d: &Data) -> Generation {
    inspect_schema(&d.path)
        .unwrap()
        .namespaces
        .into_iter()
        .find(|n| n.id == d.ns)
        .unwrap()
        .schema
        .unwrap()
        .policy_revision
}
fn input(d: &Data) -> NativeInsert {
    NativeInsert {
        memory: crate::storage::insertion::InsertMemory {
            id: None,
            summary: "native exact summary".into(),
            full_text: Some("native full text é中🙂".into()),
            tags: vec![],
            initial_stability: None,
            created_at: Some(3000),
            embedding: Some(vec![2., 0., 0., 0.]),
            parent_id: Some(d.parent),
            supersedes: None,
        },
        observed_at: Some(2500),
        entities: vec!["é中🙂".into(), "unchanged Case".into()],
        topics: vec!["Topic / exact".into()],
        emotions: vec!["🙂".into()],
        embedding_profile: Some(d.profile.clone()),
    }
}
fn ctx(session: &NativeLifecycleSession, d: &Data, key: &str) -> MutationContext {
    MutationContext {
        store_id: Some(session.metadata().control_state().unwrap().store_id),
        scope: MutationScope::Namespace(d.ns),
        idempotency_key: Some(key.into()),
        expected_generation: None,
        contract_version: 2,
    }
}
fn generated() -> GeneratedEmbedding {
    GeneratedEmbedding::unverified(vec![], None)
}
fn open(d: &Data, clock: Arc<FixedClock>) -> NativeLifecycleSession {
    NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).unwrap()
}
fn inserted(receipt: &WriteReceipt) -> MemoryId {
    match &receipt.outcome {
        MutationOutcome::NativeMemoryInserted { transition } => transition.memory_id,
        _ => panic!("not native insertion"),
    }
}

#[test]
fn actual_native_insert_feedback_tombstone_replay_and_reopen_preserve_proof() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let original = input(&d);
    let parent_before = session
        .engine
        .get_record(d.parent)
        .unwrap()
        .unwrap()
        .schema
        .unwrap()
        .lifecycle
        .record_generation;
    let intent = session.capture_insert(scope(&d), &original).unwrap();
    let key = ctx(&session, &d, "create");
    let receipt = session
        .insert_prepared(&key, &intent, &generated())
        .unwrap();
    let id = inserted(&receipt);
    let row = session.engine.get_record(id).unwrap().unwrap();
    assert_eq!(row.schema.as_ref().unwrap().labels, original.labels());
    assert_eq!(row.schema.as_ref().unwrap().lifecycle.observed_at, 2500);
    assert_eq!(row.schema.as_ref().unwrap().lifecycle.decay_anchor_at, 3000);
    assert_eq!(
        row.schema
            .as_ref()
            .unwrap()
            .lifecycle
            .record_generation
            .get(),
        1
    );
    let parent = session.engine.get_record(d.parent).unwrap().unwrap();
    assert_eq!(parent.edge_count, 1);
    assert_eq!(
        parent
            .schema
            .as_ref()
            .unwrap()
            .lifecycle
            .record_generation
            .get(),
        parent_before.next().unwrap().get()
    ); // Exactly one edge mutation after the explicit fixture adoption.
    let feedback = session.capture_feedback(id, None, 3).unwrap();
    session
        .apply_feedback(&ctx(&session, &d, "feedback"), &feedback)
        .unwrap();
    let terminal = session.capture_tombstone(id, None).unwrap();
    let forget = ctx(&session, &d, "forget");
    session.tombstone(&forget, &terminal).unwrap();
    let row = session.engine.get_record(id).unwrap().unwrap();
    assert_eq!(row.phase, DecayPhase::Tombstone);
    assert!(row.summary.is_empty());
    assert!(row.schema.as_ref().unwrap().labels.entities.is_empty());
    assert!(row.schema.as_ref().unwrap().vector.is_none());
    assert!(
        session
            .insert_prepared(
                &key,
                &intent,
                &GeneratedEmbedding::unverified(vec![f32::NAN], None)
            )
            .unwrap()
            .replayed
    );
    assert!(session.tombstone(&forget, &terminal).unwrap().replayed);
    assert_eq!(clock.0.load(Ordering::SeqCst), 3);
    drop(session);
    inspect_schema(&d.path).unwrap();
    let mut session = open(&d, clock.clone());
    assert!(
        session
            .insert_prepared(&key, &intent, &generated())
            .unwrap()
            .replayed
    );
    assert_eq!(
        session.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
    assert!(RedbStorageEngine::open(&d.path).is_err());
}

struct Provider {
    profile: crate::model::schema::EmbeddingProfile,
    calls: AtomicUsize,
    fail_if_called: bool,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for Provider {
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        assert!(!self.fail_if_called);
        self.calls.fetch_add(1, Ordering::SeqCst);
        Some(self.profile.clone())
    }
    fn dimensions(&self) -> usize {
        assert!(!self.fail_if_called);
        self.calls.fetch_add(1, Ordering::SeqCst);
        4
    }
    fn model_name(&self) -> &str {
        "synthetic"
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        assert!(!self.fail_if_called);
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![1., 0., 0., 0.])
    }
}
#[tokio::test]
async fn original_expansion_replay_precedes_aborted_default_changes_and_real_policy_migration_provider()
 {
    let root = tempfile::tempdir().unwrap();
    let mut d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let mut original = input(&d);
    original.memory.embedding = None;
    original.embedding_profile = None;
    let key = ctx(&session, &d, "generated");
    let provider = Provider {
        profile: d.profile.clone(),
        calls: AtomicUsize::new(0),
        fail_if_called: false,
    };
    let receipt = session
        .insert_generated(&key, scope(&d), &original, &provider)
        .await
        .unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    let state = session.metadata().control_state().unwrap();
    {
        // This is deliberately an ABORTED transaction, not a valid published
        // namespace-update API or a forged policy receipt. It tests only replay's
        // original expansion lookup against mutable default/profile temptation.
        let tx = session.metadata().db.begin_write().unwrap();
        let mut ns = NamespaceConfig::decode(
            tx.open_table(NAMESPACE_TABLE)
                .unwrap()
                .get(d.ns.get())
                .unwrap()
                .unwrap()
                .value(),
        )
        .unwrap();
        ns.initial_stability = 333.;
        ns.default_difficulty = 9.;
        ns.embedding_dim = 8;
        let crate::model::schema::EmbeddingBinding::Bound {
            descriptor,
            profile_id,
            ..
        } = &mut ns.schema.as_mut().unwrap().embedding_binding
        else {
            panic!("unbound")
        };
        descriptor.dimension = 8;
        *profile_id = descriptor.profile_id().unwrap();
        tx.open_table(NAMESPACE_TABLE)
            .unwrap()
            .insert(d.ns.get(), serde_json::to_vec(&ns).unwrap().as_slice())
            .unwrap();
        let replay = super::replay_insert(&tx, &key, &original).unwrap().unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.operation_id, receipt.operation_id);
        drop(tx);
    }
    assert_eq!(session.metadata().control_state().unwrap(), state);
    drop(session);
    let rollback = root.path().join("rollback");
    migrate_lifecycle(
        &d.path,
        &rollback,
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(&d.path).unwrap().control.generation_ref(),
            namespace_id: d.ns,
            expected_policy_revision: policy_revision(&d),
            target: LifecycleVersion::LegacyAccessV1,
            idempotency_key: "rollback".into(),
        },
    )
    .unwrap();
    d.path = rollback;
    let mut session = open(&d, clock.clone());
    let never = Provider {
        profile: d.profile.clone(),
        calls: AtomicUsize::new(0),
        fail_if_called: true,
    };
    let replay = session
        .insert_generated(&key, scope(&d), &original, &never)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(never.calls.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
}

#[test]
fn invalid_native_vectors_profiles_labels_and_stale_intents_never_allocate() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let state = session.metadata().control_state().unwrap();
    let files =
        || ["meta.db", "native/vectors.dat"].map(|p| std::fs::read(d.path.join(p)).unwrap());
    for vector in [
        vec![0.; 4],
        vec![f32::NAN; 4],
        vec![f32::INFINITY; 4],
        vec![1.; 3],
    ] {
        let mut original = input(&d);
        original.memory.embedding = Some(vector);
        let intent = session.capture_insert(scope(&d), &original).unwrap();
        let before = files();
        assert!(
            session
                .insert_prepared(&ctx(&session, &d, "bad"), &intent, &generated())
                .is_err()
        );
        assert_eq!(files(), before);
    }
    let mut original = input(&d);
    original.embedding_profile.as_mut().unwrap().model_id = "other".into();
    let intent = session.capture_insert(scope(&d), &original).unwrap();
    let before = files();
    assert!(
        session
            .insert_prepared(&ctx(&session, &d, "bad-profile"), &intent, &generated())
            .is_err()
    );
    assert_eq!(files(), before);
    let mut original = input(&d);
    original.entities = vec!["é".repeat(65)];
    assert!(session.capture_insert(scope(&d), &original).is_err());
    assert_eq!(session.metadata().control_state().unwrap(), state);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    let intent = session.capture_insert(scope(&d), &input(&d)).unwrap();
    let feedback = session.capture_feedback(d.parent, None, 3).unwrap();
    session
        .apply_feedback(&ctx(&session, &d, "parent-changed"), &feedback)
        .unwrap();
    let before = files();
    assert!(matches!(
        session.insert_prepared(&ctx(&session, &d, "stale"), &intent, &generated()),
        Err(StorageError::LifecycleConflict(_))
    ));
    assert_eq!(files(), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
}

#[test]
fn real_precommit_abandonment_and_ambiguous_commit_recover_records_edges_and_receipts() {
    for after in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock.clone());
        let mut original = input(&d);
        let id = MemoryId::new();
        original.memory.id = Some(id);
        let intent = session.capture_insert(scope(&d), &original).unwrap();
        let key = ctx(&session, &d, "uncertain-create");
        let before = session.metadata().control_state().unwrap();
        if after {
            effects::FAIL_AFTER_RECORD_COMMIT.with(|f| f.set(true));
        } else {
            effects::FAIL_BEFORE_RECORD_COMMIT.with(|f| f.set(true));
        }
        let failure = session
            .insert_prepared(&key, &intent, &generated())
            .unwrap_err();
        if after {
            assert!(matches!(failure, StorageError::CommitOutcomeUnknown { .. }));
        }
        assert!(session.capture_feedback(d.parent, None, 3).is_err()); // staged failure poisons this owned handle
        drop(session);
        let mut session = open(&d, clock.clone());
        assert_eq!(session.engine.get_record(id).unwrap().is_some(), after);
        assert_eq!(
            session
                .metadata()
                .control_state()
                .unwrap()
                .committed_generation
                .get(),
            before.committed_generation.get() + u64::from(after)
        );
        assert_eq!(
            session
                .engine
                .get_record(d.parent)
                .unwrap()
                .unwrap()
                .edge_count,
            u16::from(after)
        );
        let fresh = session.capture_insert(scope(&d), &original).unwrap();
        let receipt = session.insert_prepared(&key, &fresh, &generated()).unwrap();
        assert_eq!(receipt.replayed, after);
        assert_eq!(
            session
                .engine
                .get_record(d.parent)
                .unwrap()
                .unwrap()
                .edge_count,
            1
        );
        let forget = ctx(&session, &d, "uncertain-forget");
        let intent = session.capture_tombstone(id, None).unwrap();
        let before = session.metadata().control_state().unwrap();
        if after {
            effects::FAIL_AFTER_RECORD_COMMIT.with(|f| f.set(true));
        } else {
            effects::FAIL_BEFORE_RECORD_COMMIT.with(|f| f.set(true));
        }
        assert!(session.tombstone(&forget, &intent).is_err());
        drop(session);
        let mut session = open(&d, clock.clone());
        assert_eq!(
            session.engine.get_record(id).unwrap().unwrap().phase,
            if after {
                DecayPhase::Tombstone
            } else {
                DecayPhase::Full
            }
        );
        assert_eq!(
            session
                .metadata()
                .control_state()
                .unwrap()
                .committed_generation
                .get(),
            before.committed_generation.get() + u64::from(after)
        );
        let fresh = session.capture_tombstone(id, None).unwrap();
        assert_eq!(session.tombstone(&forget, &fresh).unwrap().replayed, after);
        drop(session);
        inspect_schema(&d.path).unwrap();
    }
}

#[test]
fn native_terminal_uuid_reuse_and_retained_payload_corruption_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let original = input(&d);
    let intent = session.capture_insert(scope(&d), &original).unwrap();
    let receipt = session
        .insert_prepared(&ctx(&session, &d, "create"), &intent, &generated())
        .unwrap();
    let id = inserted(&receipt);
    drop(session);
    let original_bytes = {
        let db = redb::Database::open(d.path.join("meta.db")).unwrap();
        let read = db.begin_read().unwrap();
        let t = read.open_table(META_TABLE).unwrap();
        let value = t.get(id.as_bytes().as_slice()).unwrap().unwrap();
        value.value().to_vec()
    };
    for case in 0..3 {
        let db = redb::Database::open(d.path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        let mut row = DiskRecord::from_bytes(&original_bytes).unwrap();
        match case {
            0 => row.summary = "changed retained text".into(),
            1 => row.schema.as_mut().unwrap().labels.entities[0] = "changed label".into(),
            2 => row.schema.as_mut().unwrap().vector.as_mut().unwrap().sha256[0] ^= 1,
            _ => unreachable!(),
        };
        tx.open_table(META_TABLE)
            .unwrap()
            .insert(id.as_bytes().as_slice(), row.encode().unwrap().as_slice())
            .unwrap();
        tx.commit().unwrap();
        drop(db);
        let bytes = std::fs::read(d.path.join("meta.db")).unwrap();
        assert!(inspect_schema(&d.path).is_err());
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
                .is_err()
        );
        assert_eq!(std::fs::read(d.path.join("meta.db")).unwrap(), bytes);
        let db = redb::Database::open(d.path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        tx.open_table(META_TABLE)
            .unwrap()
            .insert(id.as_bytes().as_slice(), original_bytes.as_slice())
            .unwrap();
        tx.commit().unwrap();
        drop(db);
    }
    let mut session = open(&d, clock.clone());
    let forget = session.capture_tombstone(id, None).unwrap();
    session
        .tombstone(&ctx(&session, &d, "forget"), &forget)
        .unwrap();
    let mut reuse = input(&d);
    reuse.memory.id = Some(id);
    let intent = session.capture_insert(scope(&d), &reuse).unwrap();
    let before = session.metadata().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    assert!(
        session
            .insert_prepared(&ctx(&session, &d, "reuse"), &intent, &generated())
            .is_err()
    );
    assert_eq!(session.metadata().control_state().unwrap(), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    let receipt = control::encode(&receipt).unwrap();
    for forbidden in [
        &original.memory.summary,
        original.memory.full_text.as_ref().unwrap(),
        &original.entities[0],
    ] {
        assert!(!String::from_utf8_lossy(&receipt).contains(forbidden));
    }
}

#[tokio::test]
async fn native_records_survive_export_and_restore_without_replaying_foreign_receipts_or_payload() {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let original = input(&d);
    let intent = session.capture_insert(scope(&d), &original).unwrap();
    let key = ctx(&session, &d, "create-before-backup");
    let receipt = session
        .insert_prepared(&key, &intent, &generated())
        .unwrap();
    let id = inserted(&receipt);
    let survivor_intent = session.capture_insert(scope(&d), &original).unwrap();
    let survivor = inserted(
        &session
            .insert_prepared(
                &ctx(&session, &d, "survivor"),
                &survivor_intent,
                &generated(),
            )
            .unwrap(),
    );
    drop(session);
    let schema = inspect_schema(&d.path).unwrap().schema.unwrap();
    let stream = root.path().join("native.jsonl");
    let imported = root.path().join("imported");
    let exported = export_store(&d.path, &stream).unwrap();
    assert_eq!(import_store(&stream, &imported).unwrap(), exported);
    assert_eq!(inspect_schema(&imported).unwrap().schema.unwrap(), schema);
    let mut imported_session =
        NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    assert_eq!(
        imported_session
            .engine
            .get_record(id)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .labels,
        original.labels()
    );
    assert!(
        imported_session
            .insert_prepared(&key, &intent, &generated())
            .unwrap()
            .replayed
    );
    drop(imported_session);
    let backup = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("native.zip"),
        Some(&d.path),
        false,
    )
    .await
    .unwrap();
    let mut session = open(&d, clock.clone());
    let terminal = session.capture_tombstone(id, None).unwrap();
    session
        .tombstone(&ctx(&session, &d, "forget-after-backup"), &terminal)
        .unwrap();
    drop(session);
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: d.path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let inspection = inspect_schema(&d.path).unwrap();
    assert_eq!(inspection.schema.unwrap(), schema);
    assert_ne!(inspection.control.store_id, receipt.store_id);
    let mut session = open(&d, clock.clone());
    let row = session.engine.get_record(id).unwrap().unwrap();
    assert_eq!(row.phase, DecayPhase::Tombstone);
    assert!(row.summary.is_empty());
    assert!(row.schema.as_ref().unwrap().labels.entities.is_empty());
    assert!(row.schema.as_ref().unwrap().vector.is_none());
    assert_eq!(
        session
            .engine
            .get_record(survivor)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .labels,
        original.labels()
    );
    assert!(matches!(
        session.insert_prepared(&key, &intent, &generated()),
        Err(StorageError::StoreLineageMismatch)
    ));
    let read = session.metadata().db.begin_read().unwrap();
    let receipt_key = control::scoped_key(
        receipt.store_id,
        MutationScope::Namespace(d.ns),
        receipt.operation_id.as_bytes(),
    );
    let stored = control::validate_receipt(
        read.open_table(control::RECEIPTS)
            .unwrap()
            .get(receipt_key.as_slice())
            .unwrap()
            .unwrap()
            .value(),
    )
    .unwrap();
    assert_eq!(stored.receipt, receipt);
    drop(read);
    let fresh = session.capture_feedback(survivor, None, 3).unwrap();
    session
        .apply_feedback(&ctx(&session, &d, "fresh-after-fork"), &fresh)
        .unwrap();
    drop(session);
    inspect_schema(&d.path).unwrap();
    assert!(RedbStorageEngine::open(&d.path).is_err());
}

#[test]
fn native_admission_uses_the_owned_tracker_and_rejects_closed_or_foreign_work() {
    use crate::storage::work::{WorkError, WorkTracker};
    use std::time::{Duration, Instant};
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let intent = session.capture_insert(scope(&d), &input(&d)).unwrap();
    let key = ctx(&session, &d, "admission");
    let before = std::fs::read(d.path.join("meta.db")).unwrap();
    let tracker = session.metadata().work_tracker();
    // Same store UUID is insufficient: tickets belong to this owner instance.
    let foreign = WorkTracker::new(tracker.store_id());
    let ticket = foreign.begin().unwrap();
    assert!(matches!(
        session.engine.insert_native_record(
            &key,
            &intent,
            &generated(),
            &NativePolicyInputs::default(),
            clock.as_ref(),
            Some(&ticket)
        ),
        Err(StorageError::Work(WorkError::InvalidTicket))
    ));
    ticket.finish();
    let freeze = tracker
        .close_and_drain(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert!(matches!(
        session.insert_prepared(&key, &intent, &generated()),
        Err(StorageError::Work(WorkError::AdmissionClosed))
    ));
    assert_eq!(std::fs::read(d.path.join("meta.db")).unwrap(), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    drop(freeze);
    session
        .insert_prepared(&key, &intent, &generated())
        .unwrap();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
}

#[test]
fn owned_ids_actual_native_insert_uses_single_clock_sample_and_replay_allocates_nothing() {
    use crate::storage::ids::{IdKind, IdSource, SeededIds};
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let engine =
        RedbStorageEngine::open_with_ids(&d.path, true, Box::new(SeededIds::new([7; 32]))).unwrap();
    let expected = SeededIds::new([7; 32]);
    let expected_id = expected.memory(86_400_000).unwrap();
    let expected_operation = expected.uuid(IdKind::Operation).unwrap();
    let mut session = NativeLifecycleSession {
        engine,
        policy: NativePolicyInputs::default(),
        clock: clock.clone(),
    };
    let original = input(&d);
    let intent = session.capture_insert(scope(&d), &original).unwrap();
    let key = ctx(&session, &d, "scoped-native");
    let receipt = session
        .insert_prepared(&key, &intent, &generated())
        .unwrap();
    assert_eq!(inserted(&receipt), expected_id);
    assert_eq!(receipt.operation_id, expected_operation);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let cursor = session.metadata().id_source().checkpoint().unwrap();
    assert_eq!(cursor, expected.checkpoint().unwrap());
    let replay = session
        .insert_prepared(&key, &intent, &generated())
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(inserted(&replay), expected_id);
    assert_eq!(session.metadata().id_source().checkpoint().unwrap(), cursor);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
}
#[path = "graph_tests.rs"]
mod graph;
#[path = "maintenance_tests.rs"]
mod maintenance;

#[path = "graph_publication_tests.rs"]
mod graph_producers;

#[path = "selection_tests.rs"]
mod selection;

#[path = "store_tests.rs"]
mod store;

#[path = "store_batch_tests.rs"]
mod store_batches;

#[path = "mixed_history_tests.rs"]
mod mixed_history;

#[path = "namespace_binding_tests.rs"]
mod namespace_binding;

#[path = "request_context_tests.rs"]
mod request_context;

#[path = "compaction_tests.rs"]
mod compaction;

#[path = "store_producer_tests.rs"]
mod store_producers;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_native_typed_effect_counts_include_actual_guard() {
    let root = tempfile::tempdir().unwrap();
    let data = fixture(root.path());
    let mut storage = RedbStorageEngine::open_native(&data.path).unwrap();
    let policy = crate::storage::native_lifecycle::NativePolicyInputs::default();
    let intent = storage
        .meta_store()
        .capture_native_tombstone(data.parent, None, &policy)
        .unwrap();
    let receipt = storage
        .tombstone_native_record(
            &MutationContext::unkeyed(data.ns),
            &intent,
            &policy,
            &crate::time::ManualClock::new(86_400_000),
            None,
        )
        .unwrap();
    let at = storage.control_state().unwrap().generation_ref();
    let n = storage.meta_store().pending_effects().unwrap().len();
    assert!(n > 0);
    let (send, receive) = std::sync::mpsc::channel();
    *storage.meta_store().ack_work_probe.lock().unwrap() = Some(send);
    let f = crate::test_support::Fixture::from_open_engine(root, storage);
    readers::completion(&f).rebuild_all().await.unwrap();
    let work = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    eprintln!("STARTUP_ACK_NATIVE backlog={n} {work:?}");
    assert_eq!(work.validation_calls, 1);
    assert_eq!(work.effect_rows, n);
    assert_eq!(work.commit_successes, 1);
    let storage = f.storage.read().unwrap();
    assert_eq!(storage.control_state().unwrap().generation_ref(), at);
    assert_eq!(
        storage
            .meta_store()
            .lookup_operation(
                at.store_id,
                MutationScope::Namespace(data.ns),
                receipt.operation_id
            )
            .unwrap(),
        Some(receipt)
    );
    assert!(storage.meta_store().pending_effects().unwrap().is_empty());
    assert!(
        storage
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
}
