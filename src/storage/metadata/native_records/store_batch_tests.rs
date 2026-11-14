//! Version 3 is a batch writer selector, never a public contract version.
use super::*;
use crate::storage::batch::{self, BatchInput, BatchItemPlan, BatchItemState, BatchKind};
use crate::storage::native_store::{NativeStoreInput, NativeStoreSettings};
use serde_json::json;

fn request(d: &Data, store_id: uuid::Uuid) -> BatchInput {
    BatchInput {
        expected_generation: None,
        mutation_version: 3,
        result_version: 1,
        kind: BatchKind::Insert,
        items: vec![
            json!({"summary":"linked batch","entities":["José"],"embedding":[2.,0.,0.,0.],"embeddingProfile":d.profile}),
        ],
        default_namespace: "native".into(),
        store_id: Some(store_id),
        idempotency_key: Some("linked-batch".into()),
        batch_operation_id: None,
    }
}
fn plans(r: &BatchInput) -> Vec<BatchItemPlan> {
    crate::mcp::batches::prepare_native_store(r)
        .unwrap()
        .into_iter()
        .map(|i| i.plan)
        .collect()
}
fn settings() -> NativeStoreSettings {
    crate::graph::native_selection::freeze_settings(&crate::config::GraphConfig {
        temporal_window_ms: u64::MAX,
        ..Default::default()
    })
    .unwrap()
}
fn native(plan: &BatchItemPlan) -> &NativeStoreInput {
    let BatchItemPlan::NativeStore { input, .. } = plan else {
        panic!()
    };
    input
}
fn reserve(
    s: &NativeLifecycleSession,
    r: &BatchInput,
    plans: &[BatchItemPlan],
) -> batch::BatchProgress {
    s.metadata()
        .reserve_native_store_batch_with_work(
            &r.context(),
            plans,
            r.batch_operation_id,
            None,
            || Ok(settings()),
        )
        .unwrap()
}
fn commit(
    s: &mut NativeLifecycleSession,
    d: &Data,
    p: &batch::BatchProgress,
    input: &NativeStoreInput,
) -> WriteReceipt {
    let child = p.child(0).unwrap();
    let frozen = child
        .defaults
        .native
        .as_ref()
        .unwrap()
        .store_settings
        .as_ref()
        .unwrap();
    let intent = s.capture_store(scope(d), input, frozen).unwrap();
    let candidates =
        crate::graph::native_selection::capture(&s.engine, intent.insert_intent(), &generated())
            .unwrap();
    let prepared = s
        .prepare_store_links(&intent, &candidates.frozen_plan(&intent).unwrap())
        .unwrap();
    s.engine
        .store_native_prepared_reserved(
            &child.context(),
            &prepared,
            &generated(),
            &s.policy,
            s.clock.as_ref(),
            None,
            Some(&child),
        )
        .unwrap()
}

#[test]
fn linked_batch_same_tx_reserved_child_reopen_replay_before_mutable_settings() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let r = request(&d, s.metadata().control_state().unwrap().store_id);
    let plans = plans(&r);
    assert_eq!(r.context().contract_version, 2);
    let p = reserve(&s, &r, &plans);
    let child = p.child(0).unwrap();
    assert_eq!(child.context().contract_version, 2);
    assert_eq!(p.version, 3);
    assert_ne!(child.operation_id, p.reservation_receipt.operation_id);
    drop(s);
    s = open(&d, clock.clone());
    let receipt = commit(&mut s, &d, &p, native(&plans[0]));
    assert_eq!(receipt.operation_id, child.operation_id);
    assert_eq!(
        receipt.committed_generation,
        p.reservation_receipt.committed_generation.next().unwrap()
    );
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
        panic!()
    };
    assert!(transition.edges.len() >= 2);
    assert!(transition.advisory.is_some());
    let completed = s
        .metadata()
        .lookup_batch(receipt.store_id, p.reservation_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        completed.items[0].state,
        BatchItemState::Ok {
            receipt: receipt.clone()
        }
    );
    let before = s.metadata().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    let replay = s
        .metadata()
        .reserve_native_store_batch_with_work(&r.context(), &plans, None, None, || {
            panic!("completed replay inspected current settings")
        })
        .unwrap();
    assert_eq!(replay.items, completed.items);
    let actual = s
        .metadata()
        .lookup_native_store_child(&child, native(&plans[0]), None)
        .unwrap()
        .unwrap();
    let mut expected = receipt.clone();
    expected.replayed = true;
    assert_eq!(actual, expected);
    assert_eq!(s.metadata().control_state().unwrap(), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    drop(s);
    let s = open(&d, clock);
    assert_eq!(
        s.metadata()
            .lookup_native_store_child(&child, native(&plans[0]), None)
            .unwrap(),
        Some(expected)
    );
}

#[test]
fn linked_batch_hash_domains_fixed_defaults_original_bits_and_unknown_versions() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock);
    let r = request(&d, s.metadata().control_state().unwrap().store_id);
    let original = plans(&r);
    let p = reserve(&s, &r, &original);
    let before = s.metadata().control_state().unwrap();
    let mut fixed = r.clone();
    fixed.items[0]["checkDuplicates"] = json!(true);
    fixed.items[0]["tags"] = json!([]);
    fixed.items[0]["topics"] = json!([]);
    fixed.items[0]["emotions"] = json!([]);
    assert_eq!(plans(&fixed)[0].request_hash(), original[0].request_hash());
    for (field, value) in [
        ("embedding", json!([1., 0., 0., 0.])),
        ("embedding", json!([2., -0., 0., 0.])),
        ("checkDuplicates", json!(false)),
    ] {
        let mut changed = r.clone();
        changed.items[0][field] = value;
        let changed = plans(&changed);
        assert_ne!(changed[0].request_hash(), original[0].request_hash());
        assert!(matches!(
            s.metadata().reserve_native_store_batch_with_work(
                &r.context(),
                &changed,
                None,
                None,
                || panic!("conflict resolved settings")
            ),
            Err(StorageError::IdempotencyConflict)
        ));
    }
    let mut changed = r.clone();
    changed.default_namespace = "other".into();
    assert_ne!(
        plans(&changed)[0].request_hash(),
        original[0].request_hash()
    );
    for version in [0, 4, u32::MAX] {
        assert!(batch::caller_hash_version(version, BatchKind::Insert, &[]).is_err());
        assert!(batch::resolution_hash_version(version, &p.items).is_err());
        assert!(batch::contract_version(version).is_err());
    }
    let hashes = [original[0].request_hash()];
    let one = batch::caller_hash_version(1, BatchKind::Insert, &hashes).unwrap();
    let two = batch::caller_hash_version(2, BatchKind::Insert, &hashes).unwrap();
    let three = batch::caller_hash_version(3, BatchKind::Insert, &hashes).unwrap();
    assert_ne!(one, two);
    assert_ne!(two, three);
    assert_eq!(s.metadata().control_state().unwrap(), before);
    assert!(
        crate::mcp::batches::prepare(&r).is_err(),
        "unvalidated public consumer remains gated"
    );
}

#[test]
fn linked_batch_forged_version_settings_or_outcome_rejects_on_reopen() {
    for corruption in [0, 1, 2] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut s = open(&d, clock.clone());
        let r = request(&d, s.metadata().control_state().unwrap().store_id);
        let plans = plans(&r);
        let mut p = reserve(&s, &r, &plans);
        commit(&mut s, &d, &p, native(&plans[0]));
        p = s
            .metadata()
            .lookup_batch(
                p.reservation_receipt.store_id,
                p.reservation_receipt.operation_id,
            )
            .unwrap()
            .unwrap();
        match corruption {
            0 => p.version = 4,
            1 => {
                p.items[0]
                    .defaults
                    .as_mut()
                    .unwrap()
                    .native
                    .as_mut()
                    .unwrap()
                    .store_settings = None
            }
            _ => p.version = 2,
        }
        let tx = s.metadata().db.begin_write().unwrap();
        tx.open_table(crate::storage::metadata::batch::BATCHES)
            .unwrap()
            .insert(
                p.reservation_receipt.operation_id.as_bytes().as_slice(),
                control::encode(&p).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
        drop(s);
        let before = std::fs::read(d.path.join("meta.db")).unwrap();
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err()
        );
        assert_eq!(std::fs::read(d.path.join("meta.db")).unwrap(), before);
    }
}

#[tokio::test]
async fn linked_batch_export_import_retained_restore_preserves_version_and_proof() {
    export_import_and_restore(false).await;
}
#[tokio::test]
async fn batch_cas_export_import_retained_restore_preserves_original_proof() {
    export_import_and_restore(true).await;
}
async fn export_import_and_restore(cas: bool) {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let mut r = request(&d, s.metadata().control_state().unwrap().store_id);
    if cas {
        r.expected_generation = Some(s.metadata().control_state().unwrap().committed_generation);
    }
    let plans = plans(&r);
    let p = reserve(&s, &r, &plans);
    drop(s);
    let stream = root.path().join("linked.jsonl");
    assert!(export_store(&d.path, &stream).is_err());
    assert!(!stream.exists());
    s = open(&d, clock.clone());
    let receipt = commit(&mut s, &d, &p, native(&plans[0]));
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
        panic!()
    };
    let id = transition.memory_id;
    drop(s);
    let expected = export_store(&d.path, &stream).unwrap();
    let imported = root.path().join("imported");
    assert_eq!(import_store(&stream, &imported).unwrap(), expected);
    let imported =
        NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let replay = imported
        .metadata()
        .reserve_native_store_batch_with_work(&r.context(), &plans, None, None, || {
            panic!("import replay resolved settings")
        })
        .unwrap();
    assert_eq!(replay.version, 3);
    assert_eq!(replay.reservation_precondition, p.reservation_precondition);
    assert_eq!(
        replay.items[0].state,
        BatchItemState::Ok {
            receipt: receipt.clone()
        }
    );
    drop(imported);
    let backup = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("snapshot.zip"),
        Some(&d.path),
        false,
    )
    .await
    .unwrap();
    let mut s = open(&d, clock.clone());
    let intent = s.capture_tombstone(id, None).unwrap();
    s.tombstone(&ctx(&s, &d, "after-backup-forget"), &intent)
        .unwrap();
    drop(s);
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: d.path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let s = open(&d, clock);
    assert_ne!(
        s.metadata().control_state().unwrap().store_id,
        receipt.store_id
    );
    assert_eq!(
        s.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
    assert!(matches!(
        s.metadata().reserve_native_store_batch_with_work(
            &r.context(),
            &plans,
            None,
            None,
            || panic!("foreign lineage resolved settings")
        ),
        Err(StorageError::StoreLineageMismatch)
    ));
    let tx = s.metadata().db.begin_read().unwrap();
    let table = tx
        .open_table(crate::storage::metadata::batch::BATCHES)
        .unwrap();
    let raw = table
        .get(p.reservation_receipt.operation_id.as_bytes().as_slice())
        .unwrap()
        .unwrap();
    let historical: batch::BatchProgress = control::decode(raw.value()).unwrap();
    assert_eq!(
        historical.reservation_precondition,
        p.reservation_precondition
    );
    assert_eq!(historical.reservation_receipt, p.reservation_receipt);
    assert_eq!(
        historical.reservation_expected_generation().unwrap(),
        r.expected_generation
    );
}

#[test]
fn linked_batch_process_child() {
    let Ok(path) = std::env::var("RECALLD_LINKED_BATCH_PATH") else {
        return;
    };
    let r: BatchInput = serde_json::from_slice(
        &std::fs::read(std::env::var("RECALLD_LINKED_BATCH_REQUEST").unwrap()).unwrap(),
    )
    .unwrap();
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s =
        NativeLifecycleSession::open(Path::new(&path), NativePolicyInputs::default(), clock)
            .unwrap();
    let plans = plans(&r);
    let p = reserve(&s, &r, &plans);
    let child = p.child(0).unwrap();
    let intent = s
        .capture_store(
            LifecycleScope {
                namespace_id: child.namespace(),
                namespace_name: child.namespace_name().into(),
                source: None,
            },
            native(&plans[0]),
            child
                .defaults
                .native
                .as_ref()
                .unwrap()
                .store_settings
                .as_ref()
                .unwrap(),
        )
        .unwrap();
    let candidates =
        crate::graph::native_selection::capture(&s.engine, intent.insert_intent(), &generated())
            .unwrap();
    let prepared = s
        .prepare_store_links(&intent, &candidates.frozen_plan(&intent).unwrap())
        .unwrap();
    s.engine
        .store_native_prepared_reserved(
            &child.context(),
            &prepared,
            &generated(),
            &s.policy,
            s.clock.as_ref(),
            None,
            Some(&child),
        )
        .unwrap();
    panic!("did not exit at requested boundary");
}
#[test]
fn linked_batch_process_exit_before_after_child_commit_preserves_one_reserved_identity() {
    process_child_boundaries(false);
}
#[test]
fn batch_cas_process_exit_before_after_child_commit_preserves_original_proof() {
    process_child_boundaries(true);
}
fn process_child_boundaries(cas: bool) {
    for point in [
        "before_metadata_commit",
        "after_metadata_commit",
        "after_canonical_ack",
    ] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let s = open(&d, clock.clone());
        let mut r = request(&d, s.metadata().control_state().unwrap().store_id);
        if cas {
            r.expected_generation =
                Some(s.metadata().control_state().unwrap().committed_generation);
        }
        let plans = plans(&r);
        let p = reserve(&s, &r, &plans);
        let child = p.child(0).unwrap();
        drop(s);
        let request_path = root.path().join("request.json");
        std::fs::write(&request_path, serde_json::to_vec(&r).unwrap()).unwrap();
        let status=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","storage::metadata::native_records::tests::store_batches::linked_batch_process_child","--nocapture"]).env("RECALLD_LINKED_BATCH_PATH",&d.path).env("RECALLD_LINKED_BATCH_REQUEST",request_path).env("RECALLD_TEST_EFFECT_CRASH",point).output().unwrap();
        assert_eq!(
            status.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        let mut s = open(&d, clock.clone());
        let already = point != "before_metadata_commit";
        let replay = s
            .metadata()
            .lookup_native_store_child(&child, native(&plans[0]), None)
            .unwrap();
        assert_eq!(replay.is_some(), already);
        let receipt = replay.unwrap_or_else(|| commit(&mut s, &d, &p, native(&plans[0])));
        assert_eq!(receipt.operation_id, child.operation_id);
        assert_eq!(
            receipt.committed_generation,
            p.reservation_receipt.committed_generation.next().unwrap()
        );
        let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
            panic!()
        };
        assert!(transition.edges.len() >= 2);
        let edge_count = transition.edges.len();
        assert_eq!(s.engine.load_all_edges().unwrap().len(), edge_count);
        assert_eq!(s.engine.scan_all().unwrap().len(), 2);
        let completed = s
            .metadata()
            .lookup_batch(receipt.store_id, p.reservation_receipt.operation_id)
            .unwrap()
            .unwrap();
        assert_eq!(completed.unresolved(), 0);
        drop(s);
        let s = open(&d, clock);
        assert_eq!(s.engine.load_all_edges().unwrap().len(), edge_count);
    }
}

#[path = "batch_cas_tests.rs"]
mod cas;
