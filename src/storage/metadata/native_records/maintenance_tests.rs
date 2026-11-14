use super::*;
use crate::storage::native_maintenance::{NativeDecayResult, NativeDecaySkip};
use std::sync::atomic::AtomicI64;
const DAY: i64 = 86_400_000;
struct Clock {
    now: AtomicI64,
    calls: AtomicUsize,
}
impl Clock {
    fn new(now: i64) -> Self {
        Self {
            now: AtomicI64::new(now),
            calls: AtomicUsize::new(0),
        }
    }
    fn set(&self, now: i64) {
        self.now.store(now, Ordering::SeqCst);
    }
}
impl crate::time::Clock for Clock {
    fn now_millis(&self) -> i64 {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.now.load(Ordering::SeqCst)
    }
}
fn session(d: &Data, c: Arc<Clock>) -> NativeLifecycleSession {
    NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c).unwrap()
}
fn create(
    s: &mut NativeLifecycleSession,
    d: &Data,
    key: &str,
    mut original: NativeInsert,
) -> MemoryId {
    original.memory.initial_stability = Some(1.);
    let intent = s.capture_insert(scope(d), &original).unwrap();
    inserted(
        &s.insert_prepared(&ctx(s, d, key), &intent, &generated())
            .unwrap(),
    )
}
fn committed(r: NativeDecayResult) -> WriteReceipt {
    match r {
        NativeDecayResult::Committed(r) => r,
        other => panic!("expected commit, got {other:?}"),
    }
}
fn record(s: &NativeLifecycleSession, id: MemoryId) -> DiskRecord {
    s.engine.get_record(id).unwrap().unwrap()
}
fn immutable_lifecycle(a: &DiskRecord, b: &DiskRecord) {
    assert_eq!(a.stability, b.stability);
    assert_eq!(a.difficulty, b.difficulty);
    assert_eq!(a.last_accessed_at, b.last_accessed_at);
    assert_eq!(a.access_history, b.access_history);
    assert_eq!(a.is_permastore, b.is_permastore);
    let x = &a.schema.as_ref().unwrap().lifecycle;
    let y = &b.schema.as_ref().unwrap().lifecycle;
    assert_eq!(x.decay_anchor_at, y.decay_anchor_at);
    assert_eq!(x.observed_at, y.observed_at);
    assert_eq!(x.retrieved_at, y.retrieved_at);
    assert_eq!(x.reinforced_at, y.reinforced_at);
    assert_eq!(x.timestamp_origin, y.timestamp_origin);
}
#[test]
fn native_maintenance_one_phase_retains_ghost_vector_then_deletes_with_witness_peers_and_replay() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let c = Arc::new(Clock::new(3000));
    let mut s = session(&d, c.clone());
    let id = create(&mut s, &d, "create", input(&d));
    let exposure = s
        .capture_exposure(
            scope(&d),
            &[(id, crate::model::AccessKind::DirectRetrieval)],
        )
        .unwrap();
    s.record_exposure(&ctx(&s, &d, "exposure"), &exposure)
        .unwrap();
    let original = record(&s, id);
    let vector = original.schema.as_ref().unwrap().vector.clone();
    let parent = record(&s, d.parent);
    c.set(10_000 * DAY);
    let first = committed(s.maintain(&ctx(&s, &d, "summary"), id).unwrap());
    let summary = record(&s, id);
    assert_eq!(summary.phase, DecayPhase::Summary);
    assert_eq!(summary.summary, original.summary);
    assert_eq!(
        summary.schema.as_ref().unwrap().labels,
        original.schema.as_ref().unwrap().labels
    );
    assert_eq!(summary.text_length, 0);
    immutable_lifecycle(&original, &summary);
    let mut phase_ledger = s.metadata().removal_ledger_export().unwrap();
    crate::storage::native_maintenance::validate_ledger(&first, Some(&phase_ledger)).unwrap();
    phase_ledger
        .entries
        .iter_mut()
        .find(|e| e.memory_uuid == id.into_inner())
        .unwrap()
        .operation_id = uuid::Uuid::new_v4();
    assert!(
        crate::storage::native_maintenance::validate_ledger(&first, Some(&phase_ledger)).is_err()
    );
    drop(s);
    let mut s = session(&d, c.clone());
    let second = committed(s.maintain(&ctx(&s, &d, "ghost"), id).unwrap());
    let ghost = record(&s, id);
    assert_eq!(ghost.phase, DecayPhase::Ghost);
    assert!(ghost.summary.is_empty());
    assert!(ghost.tags.is_empty());
    assert!(ghost.schema.as_ref().unwrap().labels.is_empty());
    assert_eq!(ghost.schema.as_ref().unwrap().vector, vector);
    immutable_lifecycle(&original, &ghost);
    assert!(
        !s.engine
            .get_vector(d.ns, ghost.vector_slot)
            .unwrap()
            .unwrap()
            .is_empty()
    );
    drop(s);
    let mut s = session(&d, c.clone());
    let key = ctx(&s, &d, "delete");
    let third = committed(s.maintain(&key, id).unwrap());
    let MutationOutcome::NativeMemoryHardDeleted { transition } = &third.outcome else {
        panic!("wrong outcome")
    };
    assert_eq!(transition.deletion.history, original.access_history);
    assert_eq!(transition.peers.len(), 1);
    assert!(s.engine.get_record(id).unwrap().is_none());
    let peer = record(&s, d.parent);
    assert_eq!(peer.edge_count, 0);
    assert_eq!(
        peer.schema.as_ref().unwrap().lifecycle.record_generation,
        parent
            .schema
            .as_ref()
            .unwrap()
            .lifecycle
            .record_generation
            .next()
            .unwrap()
    );
    assert_eq!(
        third.committed_generation,
        second.committed_generation.next().unwrap()
    );
    assert_eq!(
        second.committed_generation,
        first.committed_generation.next().unwrap()
    );
    assert!(matches!(
        s.maintain(&key, d.parent),
        Err(StorageError::IdempotencyConflict)
    ));
    let raw = serde_json::to_string(&third).unwrap();
    assert!(!raw.contains(&original.summary));
    assert!(!raw.contains("native full text"));
    assert!(!raw.contains("Topic / exact"));
    let read = s.metadata().db.begin_read().unwrap();
    let ledger = removal_ledger::read_export(&read, Some(&s.metadata().control_state().unwrap()))
        .unwrap()
        .unwrap();
    assert!(
        ledger
            .entries
            .iter()
            .any(|e| e.memory_uuid == id.into_inner()
                && e.terminal_kind == Some(removal_ledger::TerminalKind::HardDelete)
                && e.removed_fields_mask == removal_ledger::ALL
                && e.operation_id == third.operation_id)
    );
    drop(read);
    let calls = c.calls.load(Ordering::SeqCst);
    let replay = committed(s.maintain(&key, id).unwrap());
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, third.operation_id);
    assert_eq!(c.calls.load(Ordering::SeqCst), calls);
    let mut new = input(&d);
    new.memory.parent_id = None;
    let replacement = create(&mut s, &d, "replacement", new);
    let before = record(&s, replacement);
    assert_eq!(before.vector_slot, original.vector_slot);
    assert!(committed(s.maintain(&key, id).unwrap()).replayed);
    assert_eq!(
        record(&s, replacement).encode().unwrap(),
        before.encode().unwrap()
    );
    assert!(
        !s.engine
            .get_vector(d.ns, before.vector_slot)
            .unwrap()
            .unwrap()
            .is_empty()
    );
    let mut reuse = input(&d);
    reuse.memory.id = Some(id);
    let intent = s.capture_insert(scope(&d), &reuse).unwrap();
    assert!(
        s.insert_prepared(&ctx(&s, &d, "reuse"), &intent, &generated())
            .is_err()
    );
    drop(s);
    inspect_schema(&d.path).unwrap();
    let _reopened = session(&d, c);
}

#[test]
fn native_maintenance_rechecks_canonical_donors_and_fan_with_original_clock_for_phase_and_strength_only()
 {
    for days in [2.0, 4.5] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let c = Arc::new(Clock::new(3000));
        let mut s = session(&d, c.clone());
        let mut target = input(&d);
        let created = 10_000 * DAY;
        target.memory.created_at = Some(created);
        c.set(created);
        let id = create(&mut s, &d, "target", target);
        let now = created + (days * DAY as f64) as i64;
        c.set(now);
        let intent = s.capture_decay(id, d.ns).unwrap();
        let before = record(&s, id);
        let ns = s.engine.get_namespace(d.ns).unwrap().unwrap();
        let policy = NativePolicyInputs::default().resolve(&ns).unwrap();
        let raw = crate::decay::lifecycle::project(1., created, &policy, now, 0.).unwrap();
        if days == 4.5 {
            assert!(raw.decay_strength < 0.7);
        }
        let mut donor = input(&d);
        // Actual donor feedback refreshes its anchor; another actual insertion
        // changes its canonical fan. Neither mutates the captured target token.
        let feedback = s.capture_feedback(d.parent, None, 3).unwrap();
        s.apply_feedback(&ctx(&s, &d, "donor-feedback"), &feedback)
            .unwrap();
        donor.memory.created_at = Some(now);
        create(&mut s, &d, "donor", donor);
        assert_eq!(
            record(&s, id)
                .schema
                .as_ref()
                .unwrap()
                .lifecycle
                .record_generation,
            before.schema.as_ref().unwrap().lifecycle.record_generation
        );
        c.set(20_000 * DAY);
        let calls = c.calls.load(Ordering::SeqCst);
        committed(s.apply_decay(&ctx(&s, &d, "maintain"), &intent).unwrap());
        let after = record(&s, id);
        assert_eq!(after.phase, DecayPhase::Full);
        assert_eq!(after.strength, raw.strength);
        assert!(after.decay_strength > raw.decay_strength);
        assert_eq!(
            after
                .schema
                .as_ref()
                .unwrap()
                .lifecycle
                .lifecycle_updated_at,
            now
        );
        assert_eq!(c.calls.load(Ordering::SeqCst), calls);
        immutable_lifecycle(&before, &after);
        drop(s);
        inspect_schema(&d.path).unwrap();
    }
}

#[test]
fn native_maintenance_uses_anchor_and_rejects_stale_feedback_and_global_policy_without_receipts() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let c = Arc::new(Clock::new(3000));
    let mut s = session(&d, c.clone());
    let mut original = input(&d);
    original.memory.parent_id = None;
    let id = create(&mut s, &d, "target", original);
    c.set(3000 + DAY);
    let access = s
        .capture_exposure(
            scope(&d),
            &[(id, crate::model::AccessKind::DirectRetrieval)],
        )
        .unwrap();
    s.record_exposure(&ctx(&s, &d, "access"), &access).unwrap();
    let before = record(&s, id);
    assert_eq!(
        before.schema.as_ref().unwrap().lifecycle.decay_anchor_at,
        3000
    );
    assert_eq!(before.last_accessed_at, 3000 + DAY);
    c.set(3000 + DAY * 9 / 2);
    committed(s.maintain(&ctx(&s, &d, "phase"), id).unwrap());
    assert_eq!(record(&s, id).phase, DecayPhase::Summary);
    let intent = s.capture_decay(id, d.ns).unwrap();
    let feedback = s.capture_feedback(id, None, 3).unwrap();
    s.apply_feedback(&ctx(&s, &d, "feedback"), &feedback)
        .unwrap();
    let state = s.metadata().control_state().unwrap();
    let before = record(&s, id).encode().unwrap();
    assert!(matches!(
        s.apply_decay(&ctx(&s, &d, "stale"), &intent).unwrap(),
        NativeDecayResult::Skipped(NativeDecaySkip::Stale)
    ));
    assert_eq!(s.metadata().control_state().unwrap(), state);
    assert_eq!(record(&s, id).encode().unwrap(), before);
    let intent = s.capture_decay(id, d.ns).unwrap();
    s.policy.multiplier = 2.;
    assert!(matches!(
        s.apply_decay(&ctx(&s, &d, "policy"), &intent).unwrap(),
        NativeDecayResult::Skipped(NativeDecaySkip::Stale)
    ));
    assert_eq!(s.metadata().control_state().unwrap(), state);
    s.policy.multiplier = 0.;
    assert!(matches!(
        s.maintain(&ctx(&s, &d, "exempt"), id).unwrap(),
        NativeDecayResult::Skipped(NativeDecaySkip::Exempt)
    ));
    assert_eq!(s.metadata().control_state().unwrap(), state);
    drop(s);
    inspect_schema(&d.path).unwrap();
}

#[test]
fn native_maintenance_process_child() {
    let Ok(path) = std::env::var("RECALLD_NATIVE_MAINTENANCE_PATH") else {
        return;
    };
    let ns = NamespaceId::new(
        std::env::var("RECALLD_NATIVE_MAINTENANCE_NS")
            .unwrap()
            .parse()
            .unwrap(),
    );
    let id = MemoryId::from_uuid(
        std::env::var("RECALLD_NATIVE_MAINTENANCE_ID")
            .unwrap()
            .parse()
            .unwrap(),
    );
    let mut s = NativeLifecycleSession::open(
        Path::new(&path),
        NativePolicyInputs::default(),
        Arc::new(Clock::new(10_000 * DAY)),
    )
    .unwrap();
    let key = MutationContext {
        store_id: Some(s.metadata().control_state().unwrap().store_id),
        scope: MutationScope::Namespace(ns),
        idempotency_key: Some("crash-maintenance".into()),
        expected_generation: None,
        contract_version: 2,
    };
    s.maintain(&key, id).unwrap();
    panic!("expected process exit hook");
}
#[test]
fn native_harddelete_process_exit_recovers_exact_incident_edges_before_and_after_commit() {
    for point in ["before_metadata_commit", "after_metadata_commit"] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let c = Arc::new(Clock::new(3000));
        let mut s = session(&d, c.clone());
        let id = create(&mut s, &d, "target", input(&d));
        c.set(10_000 * DAY);
        committed(s.maintain(&ctx(&s, &d, "summary"), id).unwrap());
        committed(s.maintain(&ctx(&s, &d, "ghost"), id).unwrap());
        let initial = s.metadata().control_state().unwrap();
        let key = ctx(&s, &d, "crash-maintenance");
        drop(s);
        let out=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","storage::metadata::native_records::tests::maintenance::native_maintenance_process_child","--nocapture"]).env("RECALLD_NATIVE_MAINTENANCE_PATH",&d.path).env("RECALLD_NATIVE_MAINTENANCE_NS",d.ns.get().to_string()).env("RECALLD_NATIVE_MAINTENANCE_ID",id.to_string()).env("RECALLD_TEST_EFFECT_CRASH",point).output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        if point == "after_metadata_commit" {
            // Use the real validated pending witness, not a permissive fabricated
            // plan. Only its deleted primary may be absent from the snapshot.
            let source = files(&d.path);
            let (_copy, db) =
                crate::storage::validation::inspect_database_copy(&d.path.join("meta.db"), true)
                    .unwrap();
            let read = db.begin_read().unwrap();
            let pending = effects::validate_existing(&read).unwrap();
            let effect = pending
                .iter()
                .find(|e| {
                    matches!(
                        e.receipt.outcome,
                        MutationOutcome::NativeMemoryHardDeleted { .. }
                    )
                })
                .unwrap();
            let mut rows = read
                .open_table(META_TABLE)
                .unwrap()
                .iter()
                .unwrap()
                .map(|item| {
                    let (key, value) = item.unwrap();
                    (
                        MemoryId::from_bytes(key.value().try_into().unwrap()),
                        DiskRecord::from_bytes(value.value()).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            effects::validate_plan(effect, &rows).unwrap();
            rows.retain(|(id, _)| *id != d.parent);
            assert!(matches!(effects::validate_plan(effect, &rows),
                Err(StorageError::InvalidControl(message)) if message.contains("unproven missing endpoint")));
            assert_eq!(source, files(&d.path));
        }
        let mut s = NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c.clone())
            .unwrap_or_else(|e| panic!("{point}: native harddelete recovery failed: {e:?}"));
        let done = point == "after_metadata_commit";
        assert_eq!(s.engine.get_record(id).unwrap().is_none(), done);
        let receipt = committed(s.maintain(&key, id).unwrap());
        assert_eq!(receipt.replayed, done);
        assert_eq!(
            receipt.committed_generation,
            initial.committed_generation.next().unwrap()
        );
        assert_eq!(record(&s, d.parent).edge_count, 0);
        assert!(s.engine.get_record(id).unwrap().is_none());
        drop(s);
        inspect_schema(&d.path).unwrap();
    }
}
#[test]
fn native_maintenance_precommit_failure_and_unknown_commit_reconcile_real_transactions() {
    for unknown in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let c = Arc::new(Clock::new(3000));
        let mut s = session(&d, c.clone());
        let id = create(&mut s, &d, "target", input(&d));
        c.set(10_000 * DAY);
        let before = record(&s, id);
        let initial = s.metadata().control_state().unwrap();
        let key = ctx(&s, &d, "phase-failure");
        if unknown {
            effects::FAIL_AFTER_RECORD_COMMIT.with(|f| f.set(true));
        } else {
            effects::FAIL_BEFORE_RECORD_COMMIT.with(|f| f.set(true));
        }
        let result = s.maintain(&key, id);
        assert!(result.is_err());
        if unknown {
            assert!(matches!(
                result,
                Err(StorageError::CommitOutcomeUnknown { .. })
            ));
        } else {
            assert_eq!(record(&s, id).encode().unwrap(), before.encode().unwrap());
            assert_eq!(s.metadata().control_state().unwrap(), initial);
        }
        drop(s);
        let mut s = session(&d, c.clone());
        let receipt = committed(s.maintain(&key, id).unwrap());
        assert_eq!(receipt.replayed, unknown);
        assert_eq!(
            receipt.committed_generation,
            initial.committed_generation.next().unwrap()
        );
        assert_eq!(record(&s, id).phase, DecayPhase::Summary);
        drop(s);
        inspect_schema(&d.path).unwrap();
    }
}

#[test]
fn native_maintenance_pass_pins_one_clock_and_never_advances_an_id_twice() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let c = Arc::new(Clock::new(3000));
    let mut s = session(&d, c.clone());
    let id = create(&mut s, &d, "target", input(&d));
    let initial = s.metadata().control_state().unwrap();
    let before = record(&s, id).encode().unwrap();
    assert!(matches!(
        s.maintain(&ctx(&s, &d, "noop"), id).unwrap(),
        NativeDecayResult::Skipped(NativeDecaySkip::Unchanged)
    ));
    assert_eq!(initial, s.metadata().control_state().unwrap());
    assert_eq!(before, record(&s, id).encode().unwrap());
    c.set(10_000 * DAY);
    let calls = c.calls.load(Ordering::SeqCst);
    let pass = s.capture_decay_pass(d.ns).unwrap();
    assert_eq!(c.calls.load(Ordering::SeqCst), calls + 1);
    assert_eq!(pass.len(), 2);
    c.set(20_000 * DAY);
    for intent in &pass {
        committed(
            s.apply_decay(&ctx(&s, &d, &format!("pass-{}", intent.memory_id)), intent)
                .unwrap(),
        );
        assert_eq!(record(&s, intent.memory_id).phase, DecayPhase::Summary);
        assert!(matches!(
            s.apply_decay(
                &ctx(&s, &d, &format!("duplicate-{}", intent.memory_id)),
                intent
            )
            .unwrap(),
            NativeDecayResult::Skipped(NativeDecaySkip::Stale)
        ));
    }
    assert_eq!(c.calls.load(Ordering::SeqCst), calls + 1);
    drop(s);
    inspect_schema(&d.path).unwrap();
}

#[test]
fn native_maintenance_missing_summary_permastore_and_unknown_phase_marker_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    let mut d = fixture(root.path());
    // A genuine legacy Full row can have only full text. Migrate that supported
    // input through the production schema/policy path; never forge native rows.
    let source = root.path().join("legacy");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let mut r = engine.get_record(d.parent).unwrap().unwrap();
    let missing = MemoryId::new();
    r.id = *missing.as_bytes();
    r.summary.clear();
    r.stability = 1.;
    r.text_offset = 0;
    r.text_length = 0;
    r.vector_slot = 0;
    engine
        .insert_memory(
            missing,
            d.ns,
            &mut r,
            &[1., 0., 0., 0.],
            Some("only retained full text"),
        )
        .unwrap();
    engine.sync().unwrap();
    drop(engine);
    let canonical = root.path().join("shape-schema");
    migrate_schema(
        &source,
        &canonical,
        &MigrationOptions {
            migration_now: 10_000 * DAY,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![ProfileAttestationInput {
                namespace_id: d.ns,
                namespace_name: "native".into(),
                descriptor: d.profile.clone(),
            }],
        },
    )
    .unwrap();
    let destination = root.path().join("shape-v2");
    migrate_lifecycle(
        &canonical,
        &destination,
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(&canonical).unwrap().control.generation_ref(),
            namespace_id: d.ns,
            expected_policy_revision: Generation::default(),
            target: LifecycleVersion::ExplicitFeedbackV2,
            idempotency_key: "v2-shapes".into(),
        },
    )
    .unwrap();
    d.path = destination;
    let c = Arc::new(Clock::new(500 * DAY));
    let mut s = session(&d, c.clone());
    let before = record(&s, missing).encode().unwrap();
    let state = s.metadata().control_state().unwrap();
    assert!(matches!(
        s.maintain(&ctx(&s, &d, "missing"), missing).unwrap(),
        NativeDecayResult::Skipped(NativeDecaySkip::MissingSummary)
    ));
    assert_eq!(state, s.metadata().control_state().unwrap());
    assert_eq!(before, record(&s, missing).encode().unwrap());
    let before = record(&s, d.parent);
    committed(s.maintain(&ctx(&s, &d, "marker"), d.parent).unwrap());
    let after = record(&s, d.parent);
    assert_eq!(after.phase, DecayPhase::Summary);
    assert_eq!(
        after.schema.as_ref().unwrap().lifecycle.phase_changed_at,
        10_000 * DAY
    );
    assert_eq!(
        after
            .schema
            .as_ref()
            .unwrap()
            .lifecycle
            .lifecycle_updated_at,
        500 * DAY
    );
    immutable_lifecycle(&before, &after);
    let mut original = input(&d);
    original.memory.parent_id = None;
    original.memory.initial_stability = Some(2000.);
    original.memory.created_at = Some(500 * DAY);
    let intent = s.capture_insert(scope(&d), &original).unwrap();
    let pinned = inserted(
        &s.insert_prepared(&ctx(&s, &d, "pin-create"), &intent, &generated())
            .unwrap(),
    );
    let feedback = s.capture_feedback(pinned, None, 3).unwrap();
    s.apply_feedback(&ctx(&s, &d, "pin-feedback"), &feedback)
        .unwrap();
    assert_eq!(record(&s, pinned).is_permastore, 1);
    let state = s.metadata().control_state().unwrap();
    c.set(30_000 * DAY);
    assert!(matches!(
        s.maintain(&ctx(&s, &d, "pin-skip"), pinned).unwrap(),
        NativeDecayResult::Skipped(NativeDecaySkip::Exempt)
    ));
    assert_eq!(state, s.metadata().control_state().unwrap());
    let forget = s.capture_tombstone(pinned, None).unwrap();
    s.tombstone(&ctx(&s, &d, "forget-pin"), &forget).unwrap();
    let state = s.metadata().control_state().unwrap();
    assert!(matches!(
        s.maintain(&ctx(&s, &d, "terminal"), pinned).unwrap(),
        NativeDecayResult::Skipped(NativeDecaySkip::Exempt)
    ));
    assert_eq!(state, s.metadata().control_state().unwrap());
    drop(s);
    inspect_schema(&d.path).unwrap();
}

fn receipt_bytes(s: &NativeLifecycleSession) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
    s.metadata()
        .db
        .begin_read()
        .unwrap()
        .open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|row| {
            let (k, v) = row.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}
fn files(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, at: &Path, result: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for item in std::fs::read_dir(at).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                visit(root, &path, result)
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = Default::default();
    visit(path, path, &mut result);
    result
}
#[tokio::test]
async fn native_maintenance_export_import_fork_and_foreign_malformed_witnesses_remain_strict() {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let c = Arc::new(Clock::new(3000));
    let mut s = session(&d, c.clone());
    let id = create(&mut s, &d, "target", input(&d));
    let access = s
        .capture_exposure(
            scope(&d),
            &[(id, crate::model::AccessKind::DirectRetrieval)],
        )
        .unwrap();
    s.record_exposure(&ctx(&s, &d, "access"), &access).unwrap();
    c.set(10_000 * DAY);
    let phase = committed(s.maintain(&ctx(&s, &d, "summary"), id).unwrap());
    committed(s.maintain(&ctx(&s, &d, "ghost"), id).unwrap());
    let key = ctx(&s, &d, "delete");
    let deleted = committed(s.maintain(&key, id).unwrap());
    let retained = receipt_bytes(&s);
    drop(s);
    let schema = inspect_schema(&d.path).unwrap().schema.unwrap();
    let stream = root.path().join("maintenance.jsonl");
    let imported = root.path().join("imported");
    let report = export_store(&d.path, &stream).unwrap();
    assert_eq!(import_store(&stream, &imported).unwrap(), report);
    assert_eq!(inspect_schema(&imported).unwrap().schema.unwrap(), schema);
    let mut imported_session =
        NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), c.clone()).unwrap();
    assert!(committed(imported_session.maintain(&key, id).unwrap()).replayed);
    assert_eq!(receipt_bytes(&imported_session), retained);
    drop(imported_session);
    let backup = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("maintenance.zip"),
        Some(&d.path),
        false,
    )
    .await
    .unwrap();
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
    assert_ne!(inspection.control.store_id, deleted.store_id);
    assert_eq!(inspection.schema.unwrap(), schema);
    let mut restored = session(&d, c.clone());
    assert_eq!(receipt_bytes(&restored), retained);
    assert!(restored.engine.get_record(id).unwrap().is_none());
    assert!(matches!(
        restored.maintain(&key, id),
        Err(StorageError::StoreLineageMismatch)
    ));
    assert!(
        restored.metadata().pending_effects().unwrap().is_empty(),
        "private restore stage consumed applied effects"
    );
    drop(restored);
    let deletion_key = control::scoped_key(
        deleted.store_id,
        MutationScope::Namespace(d.ns),
        deleted.operation_id.as_bytes(),
    );
    let phase_key = control::scoped_key(
        phase.store_id,
        MutationScope::Namespace(d.ns),
        phase.operation_id.as_bytes(),
    );
    for case in [
        "nil-peer",
        "witness-profile",
        "peer-profile",
        "phase-profile",
        "history-hash",
        "history-payload",
        "history-limit",
        "peer-token",
        "ledger-terminal",
        "unknown-field",
        "wrong-profile",
        "wrong-peer-profile",
        "wrong-phase-profile",
        "peer-namespace",
    ] {
        let receipt_key = if matches!(case, "phase-profile" | "wrong-phase-profile") {
            &phase_key
        } else {
            &deletion_key
        };
        let original = retained.get(receipt_key).unwrap();
        let mut bad: serde_json::Value = serde_json::from_slice(original).unwrap();
        let t = &mut bad["receipt"]["outcome"]["transition"];
        match case {
            "nil-peer" => {
                t["removedEdges"][0]["source"] = serde_json::json!(MemoryId::nil());
                t["peers"][0]["change"]["memoryId"] = serde_json::json!(MemoryId::nil());
            }
            "witness-profile" => {
                t["deletion"]["before"]["content"]["profileId"] = serde_json::json!("not-a-profile")
            }
            "peer-profile" => {
                for state in ["before", "after"] {
                    t["peers"][0]["change"][state]["content"]["profileId"] =
                        serde_json::json!("not-a-profile");
                }
            }
            "phase-profile" => {
                for state in ["before", "after"] {
                    t["change"][state]["content"]["profileId"] = serde_json::json!("not-a-profile");
                }
            }
            "history-hash" => t["deletion"]["history"][0]["timestamp"] = serde_json::json!(5000),
            "history-payload" => {
                t["deletion"]["history"][0]["text"] =
                    serde_json::json!("forbidden retained payload")
            }
            "history-limit" => {
                let event = t["deletion"]["history"][0].clone();
                t["deletion"]["history"] = serde_json::Value::Array(vec![
                    event;
                    crate::model::constants::ACCESS_HISTORY_MAX
                        + 1
                ]);
            }
            "peer-token" => {
                t["peers"][0]["change"]["after"]["lifecycle"]["lifecycle"]["recordGeneration"] =
                    t["peers"][0]["change"]["before"]["lifecycle"]["lifecycle"]["recordGeneration"]
                        .clone()
            }
            "wrong-profile" => {
                t["deletion"]["before"]["content"]["profileId"] =
                    serde_json::json!(format!("ep1:{}", "0".repeat(64)))
            }
            "wrong-peer-profile" => {
                for state in ["before", "after"] {
                    t["peers"][0]["change"][state]["content"]["profileId"] =
                        serde_json::json!(format!("ep1:{}", "0".repeat(64)));
                }
            }
            "wrong-phase-profile" => {
                for state in ["before", "after"] {
                    t["change"][state]["content"]["profileId"] =
                        serde_json::json!(format!("ep1:{}", "0".repeat(64)));
                }
            }
            "peer-namespace" => t["peers"][0]["namespaceId"] = serde_json::json!(999),
            "ledger-terminal" => {}
            "unknown-field" => t["futureProofRule"] = serde_json::json!(1),
            _ => unreachable!(),
        }
        let bytes = serde_json::to_vec(&bad).unwrap();
        if !matches!(
            case,
            "ledger-terminal"
                | "wrong-profile"
                | "wrong-peer-profile"
                | "wrong-phase-profile"
                | "peer-namespace"
        ) {
            assert!(
                control::validate_receipt(&bytes).is_err(),
                "{case}: pure historical proof accepted"
            );
        }
        let db = redb::Database::open(d.path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        let mut saved_ledger = None;
        if case == "ledger-terminal" {
            let k = removal_ledger::key(deleted.store_id, d.ns.get(), id.into_inner());
            let mut table = tx.open_table(removal_ledger::LEDGER).unwrap();
            let old = table.get(k.as_slice()).unwrap().unwrap().value().to_vec();
            let mut entry: serde_json::Value = serde_json::from_slice(&old).unwrap();
            entry["terminalKind"] = serde_json::json!("tombstone");
            table
                .insert(k.as_slice(), serde_json::to_vec(&entry).unwrap().as_slice())
                .unwrap();
            saved_ledger = Some((k, old));
        } else {
            tx.open_table(control::RECEIPTS)
                .unwrap()
                .insert(receipt_key.as_slice(), bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        let before = files(&d.path);
        assert!(inspect_schema(&d.path).is_err(), "{case}");
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c.clone())
                .is_err(),
            "{case}"
        );
        assert_eq!(
            before,
            files(&d.path),
            "{case}: rejected open mutated source"
        );
        let db = redb::Database::open(d.path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        if let Some((k, old)) = saved_ledger {
            tx.open_table(removal_ledger::LEDGER)
                .unwrap()
                .insert(k.as_slice(), old.as_slice())
                .unwrap();
        } else {
            tx.open_table(control::RECEIPTS)
                .unwrap()
                .insert(receipt_key.as_slice(), original.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        inspect_schema(&d.path).unwrap();
    }
}

#[test]
fn native_harddelete_pending_recovery_rejects_unproven_dangling_edges_without_source_writes() {
    for changed_weight in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let c = Arc::new(Clock::new(3000));
        let mut s = session(&d, c.clone());
        let id = create(&mut s, &d, "target", input(&d));
        c.set(10_000 * DAY);
        committed(s.maintain(&ctx(&s, &d, "summary"), id).unwrap());
        committed(s.maintain(&ctx(&s, &d, "ghost"), id).unwrap());
        drop(s);
        let out=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","storage::metadata::native_records::tests::maintenance::native_maintenance_process_child","--nocapture"]).env("RECALLD_NATIVE_MAINTENANCE_PATH",&d.path).env("RECALLD_NATIVE_MAINTENANCE_NS",d.ns.get().to_string()).env("RECALLD_NATIVE_MAINTENANCE_ID",id.to_string()).env("RECALLD_TEST_EFFECT_CRASH","after_metadata_commit").output().unwrap();
        assert_eq!(out.status.code(), Some(91));
        // Negative-only corruption of an actual pending deletion. The two indexes
        // remain mutually consistent, but the new edge is not the committed plan.
        {
            let _owner = crate::storage::ownership::StoreOwnership::acquire(&d.path).unwrap();
            let _lock = crate::storage::ownership::lock_directory(&d.path).unwrap();
            let edges = crate::storage::EdgeStore::open(&d.path.join("edges.db")).unwrap();
            let mut bad = edges
                .load_all_edges()
                .unwrap()
                .into_iter()
                .find(|e| e.target == id)
                .unwrap();
            if changed_weight {
                bad.weight = 0.5;
            } else {
                bad.target = MemoryId::new();
            }
            edges.batch_add_edges(&[bad]).unwrap();
        }
        let before = files(&d.path);
        let captured = before.clone();
        crate::storage::validation::NATIVE_PREFLIGHT_PROBE.with(|p| {
            *p.borrow_mut() = Some(Box::new(move |path| {
                assert_eq!(files(path), captured);
                assert!(matches!(
                    crate::storage::ownership::StoreOwnership::acquire(path),
                    Err(StorageError::DatabaseLocked)
                ));
                assert!(crate::storage::ownership::lock_directory(path).is_err());
            }))
        });
        let result = NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c);
        crate::storage::validation::NATIVE_PREFLIGHT_PROBE.with(|p| {
            p.borrow_mut().take();
        });
        assert!(matches!(result, Err(StorageError::CorruptIndex(_))));
        assert_eq!(before, files(&d.path));
    }
}

#[test]
fn native_maintenance_actual_policy_migration_invalidates_plan_but_replays_old_receipt_first() {
    let root = tempfile::tempdir().unwrap();
    let mut d = fixture(root.path());
    let c = Arc::new(Clock::new(3000));
    let mut s = session(&d, c.clone());
    let id = create(&mut s, &d, "target", input(&d));
    c.set(10_000 * DAY);
    let key = ctx(&s, &d, "summary");
    let receipt = committed(s.maintain(&key, id).unwrap());
    let intent = s.capture_decay(id, d.ns).unwrap();
    let before = record(&s, id);
    drop(s);
    let destination = root.path().join("rollback");
    migrate_lifecycle(
        &d.path,
        &destination,
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(&d.path).unwrap().control.generation_ref(),
            namespace_id: d.ns,
            expected_policy_revision: "1".to_string().try_into().unwrap(),
            target: LifecycleVersion::LegacyAccessV1,
            idempotency_key: "rollback".into(),
        },
    )
    .unwrap();
    d.path = destination;
    let mut s = session(&d, c.clone());
    let state = s.metadata().control_state().unwrap();
    let calls = c.calls.load(Ordering::SeqCst);
    assert!(matches!(
        s.apply_decay(&ctx(&s, &d, "stale-policy"), &intent)
            .unwrap(),
        NativeDecayResult::Skipped(NativeDecaySkip::Stale)
    ));
    assert_eq!(state, s.metadata().control_state().unwrap());
    let replay = committed(s.maintain(&key, id).unwrap());
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(c.calls.load(Ordering::SeqCst), calls);
    let after = record(&s, id);
    immutable_lifecycle(&before, &after);
    assert_eq!(after.phase, DecayPhase::Summary);
    drop(s);
    inspect_schema(&d.path).unwrap();
}

#[test]
fn native_maintenance_pure_threshold_boundaries_are_inclusive_and_never_skip_phases() {
    use crate::storage::native_maintenance::{NativeDecayEvidence, PlannedDecay, plan};
    use crate::storage::native_records::NativeRecordState;
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let c = Arc::new(Clock::new(3000));
    let mut s = session(&d, c);
    let mut input = input(&d);
    input.memory.parent_id = None;
    let id = create(&mut s, &d, "target", input);
    let row = record(&s, id);
    let ns = s.engine.get_namespace(d.ns).unwrap().unwrap();
    let p = NativePolicyInputs::default().resolve(&ns).unwrap();
    let mut before = NativeRecordState::capture(
        &row,
        &ns,
        Some(crate::storage::native_records::hash_bytes(
            "native full text é中🙂".as_bytes(),
        )),
    )
    .unwrap();
    // Pure policy-boundary probes over real captured state. They do not persist
    // invented policy receipts or claim these custom thresholds were committed.
    for (phase, index, now) in [
        (DecayPhase::Full, 0, 5 * DAY),
        (DecayPhase::Summary, 1, 10 * DAY),
        (DecayPhase::Ghost, 2, 1000 * DAY),
    ] {
        assert_eq!(before.lifecycle.phase, phase);
        let raw = crate::decay::lifecycle::project(
            before.lifecycle.stability.get(),
            before.lifecycle.lifecycle.decay_anchor_at,
            &p,
            now,
            0.,
        )
        .unwrap()
        .strength;
        let mut evidence = NativeDecayEvidence::capture(&p, 0.).unwrap();
        evidence.thresholds[index] = raw.try_into().unwrap();
        let mut just_above = evidence.clone();
        just_above.thresholds[index] = f32::from_bits(raw.to_bits() - 1).try_into().unwrap();
        match plan(&before, &just_above, now).unwrap() {
            PlannedDecay::Phase(after) => assert_eq!(after.lifecycle.phase, phase),
            PlannedDecay::Skipped(NativeDecaySkip::Unchanged) => {}
            _ => panic!("advanced above threshold"),
        }
        match plan(&before, &evidence, now).unwrap() {
            PlannedDecay::Phase(after) => {
                assert_eq!(
                    after.lifecycle.phase,
                    if phase == DecayPhase::Full {
                        DecayPhase::Summary
                    } else {
                        DecayPhase::Ghost
                    }
                );
                before = after;
            }
            PlannedDecay::Delete => assert_eq!(phase, DecayPhase::Ghost),
            _ => panic!("did not advance at boundary"),
        }
    }
}

#[test]
fn native_harddelete_zero_weight_pending_edge_rejects_signed_zero_change_without_source_writes() {
    let root = tempfile::tempdir().unwrap();
    let mut d = fixture(root.path());
    let source = root.path().join("legacy");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let input = crate::storage::insertion::InsertMemory {
        id: None,
        summary: "zero edge child".into(),
        full_text: Some("retained zero edge child text".into()),
        tags: vec![],
        initial_stability: Some(1.),
        created_at: Some(1000),
        embedding: Some(vec![1., 0., 0., 0.]),
        parent_id: Some(d.parent),
        supersedes: None,
    };
    let inserted = engine
        .insert_memory_with_receipt(&MutationContext::unkeyed(d.ns), &input, &[], 1000)
        .unwrap();
    let id = crate::storage::insertion::inserted_id(&inserted).unwrap();
    let edge = crate::storage::PersistedEdge {
        source: d.parent,
        target: id,
        edge_type: crate::model::EdgeType::ParentChild,
        weight: 0.,
        auto_created: false,
        created_at: 1000,
    };
    engine.put_edges_with_effects(&[edge]).unwrap();
    engine.sync().unwrap();
    drop(engine);
    let canonical = root.path().join("zero-schema");
    migrate_schema(
        &source,
        &canonical,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![ProfileAttestationInput {
                namespace_id: d.ns,
                namespace_name: "native".into(),
                descriptor: d.profile.clone(),
            }],
        },
    )
    .unwrap();
    let destination = root.path().join("zero-v2");
    migrate_lifecycle(
        &canonical,
        &destination,
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(&canonical).unwrap().control.generation_ref(),
            namespace_id: d.ns,
            expected_policy_revision: Generation::default(),
            target: LifecycleVersion::ExplicitFeedbackV2,
            idempotency_key: "zero-v2".into(),
        },
    )
    .unwrap();
    d.path = destination;
    let c = Arc::new(Clock::new(10_000 * DAY));
    let mut s = session(&d, c.clone());
    committed(s.maintain(&ctx(&s, &d, "summary"), id).unwrap());
    committed(s.maintain(&ctx(&s, &d, "ghost"), id).unwrap());
    drop(s);
    let out=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","storage::metadata::native_records::tests::maintenance::native_maintenance_process_child","--nocapture"]).env("RECALLD_NATIVE_MAINTENANCE_PATH",&d.path).env("RECALLD_NATIVE_MAINTENANCE_NS",d.ns.get().to_string()).env("RECALLD_NATIVE_MAINTENANCE_ID",id.to_string()).env("RECALLD_TEST_EFFECT_CRASH","after_metadata_commit").output().unwrap();
    assert_eq!(out.status.code(), Some(91));
    {
        let _owner = crate::storage::ownership::StoreOwnership::acquire(&d.path).unwrap();
        let _lock = crate::storage::ownership::lock_directory(&d.path).unwrap();
        let edges = crate::storage::EdgeStore::open(&d.path.join("edges.db")).unwrap();
        let mut edge = edges
            .load_all_edges()
            .unwrap()
            .into_iter()
            .find(|e| e.target == id)
            .unwrap();
        assert_eq!(edge.weight.to_bits(), 0_f32.to_bits());
        edge.weight = -0.;
        edges.batch_add_edges(&[edge]).unwrap();
    }
    let before = files(&d.path);
    let result = NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c);
    assert!(
        result.is_err(),
        "signed-zero-different edge was accepted as exact pending deletion"
    );
    assert_eq!(before, files(&d.path));
}
