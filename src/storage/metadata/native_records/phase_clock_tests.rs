//! Real legacy insertion, schema/policy migration and native forget. Raw table
//! access is read-only evidence collection, never fixture construction.
use super::*;
use std::sync::atomic::AtomicI64;

pub(super) struct RegressedClock {
    pub(super) time: AtomicI64,
    pub(super) calls: AtomicUsize,
}
impl Clock for RegressedClock {
    fn now_millis(&self) -> i64 {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.time.load(Ordering::SeqCst)
    }
}

fn receipts(session: &NativeLifecycleSession) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
    session
        .metadata()
        .db
        .begin_read()
        .unwrap()
        .open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|row| {
            let (key, value) = row.unwrap();
            (key.value().to_vec(), value.value().to_vec())
        })
        .collect()
}

#[test]
fn actual_migration_native_forget_preserves_phase_marker_under_regressed_clock() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(RegressedClock {
        time: AtomicI64::new(500),
        calls: AtomicUsize::new(0),
    });
    let mut session =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let before = session.engine.get_record(d.parent).unwrap().unwrap();
    let old = before.schema.as_ref().unwrap().lifecycle.clone();
    assert_eq!(before.created_at, 1000);
    assert_eq!(old.lifecycle_updated_at, 1000);
    assert_eq!(old.phase_changed_at, 2000);
    assert_eq!(old.timestamp_origin, TimestampOrigin::LegacyInferred);
    assert_eq!(old.retrieved_at, None);
    assert_eq!(old.reinforced_at, None);
    let historical = receipts(&session);
    let publication = crate::storage::canonical_schema::read_state(
        &session.metadata().db.begin_read().unwrap(),
        &session.metadata().control_state().unwrap(),
    )
    .unwrap();
    let intent = session.capture_tombstone(d.parent, None).unwrap();
    let key = ctx(&session, &d, "regressed-native-forget");
    let receipt = session.tombstone(&key, &intent).unwrap();
    let MutationOutcome::NativeMemoryTombstoned { transition } = &receipt.outcome else {
        panic!("wrong forget receipt");
    };
    assert_eq!(transition.sampled_at, 500);
    assert_eq!(
        transition.phase_time_rule,
        Some(PhaseTimeRule::MonotonicMarkerV1)
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
    let after = session.engine.get_record(d.parent).unwrap().unwrap();
    let next = &after.schema.as_ref().unwrap().lifecycle;
    assert_eq!(after.phase, DecayPhase::Tombstone);
    assert!(after.summary.is_empty());
    assert_eq!(after.text_length, 0);
    assert!(after.schema.as_ref().unwrap().vector.is_none());
    assert_eq!(next.lifecycle_updated_at, 1000); // A3 effective_now rule.
    assert_eq!(next.decay_anchor_at, old.decay_anchor_at);
    assert_eq!(next.observed_at, old.observed_at);
    assert_eq!(next.retrieved_at, old.retrieved_at);
    assert_eq!(next.reinforced_at, old.reinforced_at);
    assert_eq!(next.timestamp_origin, old.timestamp_origin);
    assert_eq!(after.last_accessed_at, before.last_accessed_at);
    assert_eq!(after.access_history, before.access_history);
    assert_eq!(
        next.record_generation,
        old.record_generation.next().unwrap()
    );
    assert_eq!(
        crate::storage::canonical_schema::read_state(
            &session.metadata().db.begin_read().unwrap(),
            &session.metadata().control_state().unwrap(),
        )
        .unwrap(),
        publication
    );
    for (key, value) in &historical {
        assert_eq!(receipts(&session).get(key), Some(value));
    }
    clock.time.store(99, Ordering::SeqCst);
    let replay = session.tombstone(&key, &intent).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(replay.outcome, receipt.outcome);
    assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
    drop(session);
    inspect_schema(&d.path).unwrap();
    let mut reopened =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let retained = reopened.engine.get_record(d.parent).unwrap().unwrap();
    assert_eq!(retained.schema, after.schema);
    assert!(reopened.tombstone(&key, &intent).unwrap().replayed);
    assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
    assert!(RedbStorageEngine::open(&d.path).is_err());
    drop(reopened);
    // Used only by the recorded BEFORE invocation to preserve a real old-format
    // fixture through the production exporter, before changing its writer.
    if let Ok(path) = std::env::var("RECALLD_TEST_PHASE_CLOCK_EXPORT") {
        crate::storage::logical_export::export_store(&d.path, Path::new(&path)).unwrap();
    }

    // Unknown historical phase time remains the immutable migration marker.
    // A later real phase change may clamp that marker, but cannot move it back.
    assert!(
        next.phase_changed_at >= old.phase_changed_at,
        "phase marker regressed after actual migration/forget/reopen: {} -> {}",
        old.phase_changed_at,
        next.phase_changed_at
    );
    assert_eq!(next.phase_changed_at, 2000);
}

#[tokio::test]
async fn actual_original_phase_receipt_survives_reopen_export_and_fork_restore() {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    let stream = root.path().join("original.jsonl");
    // Produced by the recorded, failing BEFORE binary through export_store.
    // It contains the real old writer's marker regression, not an authored row.
    std::fs::write(&stream, include_bytes!("fixtures/phase-clock-legacy.jsonl")).unwrap();
    let path = root.path().join("imported");
    import_store(&stream, &path).unwrap();
    let clock = Arc::new(RegressedClock {
        time: AtomicI64::new(99),
        calls: AtomicUsize::new(0),
    });
    let mut session =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock.clone()).unwrap();
    // Genuine BEFORE bytes: no adoption, current-input blessing or proof rewrite
    // is needed to inspect/export or replay an already committed old operation.
    assert!(
        session.engine.list_namespaces().unwrap().iter().all(|n| n
            .schema
            .as_ref()
            .unwrap()
            .inherited_decay
            .is_none())
    );
    let untouched_generation = session.engine.control_state().unwrap().generation_ref();
    let all = receipts(&session);
    let (receipt_key, raw) = all
        .iter()
        .find(|(_, bytes)| {
            matches!(
                control::validate_receipt(bytes).unwrap().receipt.outcome,
                MutationOutcome::NativeMemoryTombstoned { .. }
            )
        })
        .unwrap();
    let stored = control::validate_receipt(raw).unwrap();
    assert_eq!(control::encode(&stored).unwrap(), *raw);
    let receipt = stored.receipt;
    let MutationOutcome::NativeMemoryTombstoned { transition } = &receipt.outcome else {
        unreachable!();
    };
    assert_eq!(transition.phase_time_rule, None);
    let id = transition.memory_id;
    let ns = transition.namespace_id;
    assert_eq!(transition.sampled_at, 500);
    assert_eq!(
        transition
            .primary()
            .unwrap()
            .after
            .lifecycle
            .lifecycle
            .phase_changed_at,
        1000
    );
    assert_eq!(
        transition
            .primary()
            .unwrap()
            .before
            .as_ref()
            .unwrap()
            .lifecycle
            .lifecycle
            .phase_changed_at,
        2000
    );
    let key = MutationContext {
        store_id: Some(receipt.store_id),
        scope: MutationScope::Namespace(ns),
        idempotency_key: Some("regressed-native-forget".into()),
        expected_generation: None,
        contract_version: 2,
    };
    let intent = session.capture_tombstone(id, None).unwrap();
    let replay = session.tombstone(&key, &intent).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, receipt.outcome);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(clock.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        session.engine.control_state().unwrap().generation_ref(),
        untouched_generation
    );
    assert!(
        session.engine.list_namespaces().unwrap().iter().all(|n| n
            .schema
            .as_ref()
            .unwrap()
            .inherited_decay
            .is_none())
    );
    assert_eq!(receipts(&session), all);
    drop(session);

    let publication = inspect_schema(&path).unwrap().schema;
    let exported = root.path().join("again.jsonl");
    let copy = root.path().join("copy");
    assert_eq!(
        export_store(&path, &exported).unwrap(),
        import_store(&exported, &copy).unwrap()
    );
    let session =
        NativeLifecycleSession::open(&copy, NativePolicyInputs::default(), clock.clone()).unwrap();
    assert_eq!(receipts(&session), all);
    drop(session);
    let backup = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("old.zip"),
        Some(&path),
        false,
    )
    .await
    .unwrap();
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let inspection = inspect_schema(&path).unwrap();
    assert_eq!(inspection.schema, publication);
    assert_ne!(inspection.control.store_id, receipt.store_id);
    let mut session =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock.clone()).unwrap();
    assert_eq!(receipts(&session).get(receipt_key), Some(raw));
    let row = session.engine.get_record(id).unwrap().unwrap();
    assert_eq!(row.phase, DecayPhase::Tombstone);
    assert_eq!(row.schema.unwrap().lifecycle.phase_changed_at, 1000);
    assert!(matches!(
        session.tombstone(&key, &intent),
        Err(StorageError::StoreLineageMismatch)
    ));
    assert_eq!(clock.calls.load(Ordering::SeqCst), 0);
    drop(session);
    assert!(RedbStorageEngine::open(&path).is_err());
}

fn files(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, result: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, result);
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().into(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = Default::default();
    visit(root, root, &mut result);
    result
}

#[test]
fn malformed_phase_rule_or_wrong_transition_rejects_without_open_mutation() {
    for case in [
        "unknown",
        "null",
        "number",
        "stripped",
        "regressed",
        "insertion",
    ] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(RegressedClock {
            time: AtomicI64::new(500),
            calls: AtomicUsize::new(0),
        });
        let mut session =
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
                .unwrap();
        let receipt = if case == "insertion" {
            let input = input(&d);
            let intent = session.capture_insert(scope(&d), &input).unwrap();
            session
                .insert_prepared(&ctx(&session, &d, "create"), &intent, &generated())
                .unwrap()
        } else {
            let intent = session.capture_tombstone(d.parent, None).unwrap();
            session
                .tombstone(&ctx(&session, &d, "forget"), &intent)
                .unwrap()
        };
        let key = control::scoped_key(
            receipt.store_id,
            MutationScope::Namespace(d.ns),
            receipt.operation_id.as_bytes(),
        );
        let original = receipts(&session).remove(&key).unwrap();
        drop(session);
        let mut bad: serde_json::Value = serde_json::from_slice(&original).unwrap();
        let transition = &mut bad["receipt"]["outcome"]["transition"];
        match case {
            "unknown" => transition["phaseTimeRule"] = serde_json::json!("future_rule"),
            "null" => transition["phaseTimeRule"] = serde_json::Value::Null,
            "number" => transition["phaseTimeRule"] = serde_json::json!(1),
            "stripped" => {
                transition.as_object_mut().unwrap().remove("phaseTimeRule");
            }
            "regressed" => {
                transition["records"][0]["after"]["lifecycle"]["lifecycle"]["phaseChangedAt"] =
                    serde_json::json!(1000)
            }
            "insertion" => transition["phaseTimeRule"] = serde_json::json!("monotonic_marker_v1"),
            _ => unreachable!(),
        }
        let bad = serde_json::to_vec(&bad).unwrap();
        assert!(control::validate_receipt(&bad).is_err(), "{case}");
        let db = redb::Database::open(d.path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        tx.open_table(control::RECEIPTS)
            .unwrap()
            .insert(key.as_slice(), bad.as_slice())
            .unwrap();
        tx.commit().unwrap();
        drop(db);
        let before = files(&d.path);
        assert!(inspect_schema(&d.path).is_err(), "{case}");
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
            "{case}"
        );
        assert_eq!(files(&d.path), before, "{case}");
    }
}
