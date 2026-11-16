//! Test-only counting at actual batch entry, including nested guard/validator work.
use super::*;
use crate::storage::{StorageEngine, control::MutationContext, insertion::InsertMemory};
use crate::test_support::Fixture;
use std::cell::Cell;
use std::sync::mpsc::Sender;

pub(crate) type AckHook = Box<dyn FnOnce(&MetadataStore) + Send>;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_empty_ack_rejects_same_generation_addition_without_dirty_clear() {
    let f = seeded(1);
    let effect = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .pending_effects()
        .unwrap()
        .pop()
        .unwrap();
    publisher(&f).rebuild_all().await.unwrap();
    let storage = f.storage.read().unwrap();
    let meta = storage.meta_store();
    let at = storage.control_state().unwrap().generation_ref();
    let receipts = rows(meta, control::RECEIPTS);
    meta.derived_dirty.store(true, Ordering::SeqCst);
    *meta.ack_test_hook.lock().unwrap() = Some(Box::new(move |meta| {
        // Deliberate private-table fault at the existing hook. Legitimate
        // writers cannot pass the held metadata mutation guard here.
        let tx = meta.db.begin_write().unwrap();
        tx.open_table(EFFECTS)
            .unwrap()
            .insert(
                effect.receipt.operation_id.as_bytes().as_slice(),
                control::encode(&effect).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
    }));
    let admission = crate::storage::maintenance::ReceiptAdmission::new(8192).unwrap();
    assert!(
        meta.acknowledge_rebuilt_effects_at_bounded(at, None, Some(&admission))
            .is_err()
    );
    assert_eq!(meta.pending_effects().unwrap().len(), 1);
    assert!(meta.needs_effects_rebuild());
    assert_eq!(storage.control_state().unwrap().generation_ref(), at);
    assert_eq!(rows(meta, control::RECEIPTS), receipts);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_pending_validation_cost_depends_on_current_rows_not_history() {
    let mut counts = Vec::new();
    for historical in [1, 65] {
        let f = seeded(historical);
        publisher(&f).rebuild_all().await.unwrap();
        let storage = f.storage.write().unwrap();
        let (id, before) = storage.scan_all().unwrap().into_iter().next().unwrap();
        let ns = storage
            .get_namespace(NamespaceId::new(before.namespace_id))
            .unwrap()
            .unwrap();
        let result = storage
            .apply_legacy_decay_with_receipt(
                &MutationContext::unkeyed(ns.id),
                id,
                &before,
                &ns,
                before.phase,
                0.5,
                0.5,
            )
            .unwrap();
        let crate::storage::maintenance::DecayMutationResult::Committed(receipt) = result else {
            panic!("actual materialization required");
        };
        let admission = crate::storage::maintenance::ReceiptAdmission::new(8192).unwrap();
        admission.record(&receipt).unwrap();
        control::take_validation_work();
        storage
            .meta_store()
            .check_receipt_admission(Some(&admission))
            .unwrap();
        let work = control::take_validation_work();
        assert_eq!(work.full_validations, 0);
        assert_eq!(work.receipt_decodes, 1);
        println!("bounded_pending history_insertions={historical} current_pending=1 {work:?}");
        counts.push(work.receipt_decodes);
    }
    assert_eq!(counts, vec![1, 1]);
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AckWork {
    pub validation_calls: usize,
    pub effect_rows: usize,
    pub commit_attempts: usize,
    pub commit_successes: usize,
}

fn seeded(n: usize) -> Fixture {
    let fixture = Fixture::new();
    {
        let mut storage = fixture.storage.write().unwrap();
        for i in 0..n {
            storage
                .insert_memory_with_receipt(
                    &MutationContext::unkeyed(NamespaceId::new(1)),
                    &InsertMemory {
                        id: None,
                        summary: format!("batch ack {i}"),
                        full_text: None,
                        tags: vec![],
                        initial_stability: Some(1.),
                        created_at: Some(123),
                        embedding: None,
                        parent_id: None,
                        supersedes: None,
                    },
                    &[1., 0., 0., 0.],
                    123,
                )
                .unwrap();
        }
    }
    fixture
}

fn publisher(f: &Fixture) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}

fn rows(store: &MetadataStore, table: TableDefinition<&[u8], &[u8]>) -> Vec<(Vec<u8>, Vec<u8>)> {
    store
        .db
        .begin_read()
        .unwrap()
        .open_table(table)
        .unwrap()
        .iter()
        .unwrap()
        .map(|r| {
            let (k, v) = r.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}

fn probe(store: &MetadataStore) -> std::sync::mpsc::Receiver<AckWork> {
    let (send, receive) = std::sync::mpsc::channel();
    *store.ack_work_probe.lock().unwrap() = Some(send);
    receive
}

/// Fault tests restore only their original valid outbox after a real publisher
/// has completed the matching views. No ready stamps or receipts are invented.
async fn ready_pending(n: usize) -> Fixture {
    let f = seeded(n);
    let pending = rows(f.storage.read().unwrap().meta_store(), EFFECTS);
    publisher(&f).rebuild_all().await.unwrap();
    {
        let storage = f.storage.read().unwrap();
        let meta = storage.meta_store();
        let _guard = meta.mutation_guard().unwrap();
        let tx = meta.db.begin_write().unwrap();
        for (k, v) in pending {
            tx.open_table(EFFECTS)
                .unwrap()
                .insert(k.as_slice(), v.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();
        meta.derived_dirty.store(n != 0, Ordering::SeqCst);
    }
    f
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_empty_small_large_real_publish_preserves_receipts_and_generation() {
    for n in [0, 1, 256] {
        let f = seeded(n);
        let (at, receipts, records, receive) = {
            let storage = f.storage.read().unwrap();
            let meta = storage.meta_store();
            (
                storage.control_state().unwrap().generation_ref(),
                rows(meta, control::RECEIPTS),
                rows(meta, META_TABLE),
                probe(meta),
            )
        };
        publisher(&f).rebuild_all().await.unwrap();
        let work = receive
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        eprintln!("STARTUP_ACK_WORK backlog={n} {work:?}");
        assert_eq!(work.validation_calls, 1);
        assert_eq!(work.effect_rows, n);
        assert_eq!(work.commit_attempts, usize::from(n > 0));
        assert_eq!(work.commit_successes, usize::from(n > 0));
        {
            let storage = f.storage.read().unwrap();
            let meta = storage.meta_store();
            assert_eq!(storage.control_state().unwrap().generation_ref(), at);
            assert_eq!(rows(meta, control::RECEIPTS), receipts);
            assert_eq!(rows(meta, META_TABLE), records);
            assert!(meta.pending_effects().unwrap().is_empty());
            assert!(meta.publication_state().unwrap().matches_canonical());
        }
        publisher(&f).rebuild_all().await.unwrap();
        let root = f.close();
        let storage = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
        assert_eq!(storage.control_state().unwrap().generation_ref(), at);
        assert_eq!(rows(storage.meta_store(), control::RECEIPTS), receipts);
        assert_eq!(rows(storage.meta_store(), META_TABLE), records);
        assert!(storage.meta_store().pending_effects().unwrap().is_empty());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_corruption_and_unapplied_reject_without_partial_retirement() {
    for fault in 0..6 {
        let f = ready_pending(3).await;
        let storage = f.storage.read().unwrap();
        let meta = storage.meta_store();
        let at = storage.control_state().unwrap().generation_ref();
        let mut effect = meta.pending_effects().unwrap().pop().unwrap();
        let tx = meta.db.begin_write().unwrap();
        match fault {
            0 => effect.version = 999,
            1 => effect.canonical_applied = false,
            2 => effect.receipt.store_id = uuid::Uuid::new_v4(),
            3 => effect.invalidated_ids.clear(),
            4 => {
                let key = control::scoped_key(
                    at.store_id,
                    effect.scope,
                    effect.receipt.operation_id.as_bytes(),
                );
                tx.open_table(control::RECEIPTS)
                    .unwrap()
                    .remove(key.as_slice())
                    .unwrap();
            }
            5 => effect.scope = control::MutationScope::Namespace(NamespaceId::new(999)),
            _ => unreachable!(),
        }
        tx.open_table(EFFECTS)
            .unwrap()
            .insert(
                effect.receipt.operation_id.as_bytes().as_slice(),
                control::encode(&effect).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
        let before = rows(meta, EFFECTS);
        let receipts = rows(meta, control::RECEIPTS);
        assert!(
            meta.acknowledge_rebuilt_effects_at(at, None).is_err(),
            "fault={fault}"
        );
        assert_eq!(rows(meta, EFFECTS), before);
        assert_eq!(rows(meta, control::RECEIPTS), receipts);
        assert_eq!(storage.control_state().unwrap().generation_ref(), at);
        assert!(meta.needs_effects_rebuild());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_requires_current_generation_and_every_actual_view_even_when_empty() {
    for n in [0, 2] {
        let f = ready_pending(n).await;
        let storage = f.storage.read().unwrap();
        let meta = storage.meta_store();
        let at = storage.control_state().unwrap().generation_ref();
        let before = rows(meta, EFFECTS);
        let mut foreign = at;
        foreign.store_id = uuid::Uuid::new_v4();
        assert!(meta.acknowledge_rebuilt_effects_at(foreign, None).is_err());
        let mut future = at;
        future.generation = future.generation.next().unwrap();
        assert!(meta.acknowledge_rebuilt_effects_at(future, None).is_err());
        meta.invalidate_derived_view(crate::storage::publication::DerivedView::Vectors);
        assert!(meta.acknowledge_rebuilt_effects_at(at, None).is_err());
        assert_eq!(rows(meta, EFFECTS), before);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_same_generation_effect_change_and_leftovers_abort_atomically() {
    for mutate_existing in [true, false] {
        let f = ready_pending(3).await;
        let storage = f.storage.read().unwrap();
        let meta = storage.meta_store();
        let at = storage.control_state().unwrap().generation_ref();
        *meta.ack_test_hook.lock().unwrap() = Some(Box::new(move |meta| {
            // Deliberate private-table fault injection: no legitimate writer can
            // pass the held mutation guard at this boundary.
            let mut effect = meta.pending_effects().unwrap().pop().unwrap();
            if mutate_existing {
                effect.canonical_applied = false;
            } else {
                effect.receipt.operation_id = uuid::Uuid::new_v4();
            }
            let tx = meta.db.begin_write().unwrap();
            tx.open_table(EFFECTS)
                .unwrap()
                .insert(
                    effect.receipt.operation_id.as_bytes().as_slice(),
                    control::encode(&effect).unwrap().as_slice(),
                )
                .unwrap();
            tx.commit().unwrap();
        }));
        assert!(meta.acknowledge_rebuilt_effects_at(at, None).is_err());
        assert_eq!(
            rows(meta, EFFECTS).len(),
            if mutate_existing { 3 } else { 4 }
        );
        assert_eq!(storage.control_state().unwrap().generation_ref(), at);
        assert!(meta.needs_effects_rebuild());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_abort_and_lost_commit_result_preserve_recovery_and_unresolved_work() {
    for fault in [1, 2] {
        let f = ready_pending(3).await;
        let (at, receipts) = {
            let storage = f.storage.read().unwrap();
            let meta = storage.meta_store();
            meta.ack_test_fault.store(fault, Ordering::SeqCst);
            let at = storage.control_state().unwrap().generation_ref();
            let receipts = rows(meta, control::RECEIPTS);
            assert!(meta.acknowledge_rebuilt_effects_at(at, None).is_err());
            assert!(meta.needs_effects_rebuild());
            assert_eq!(rows(meta, EFFECTS).len(), if fault == 1 { 3 } else { 0 });
            if fault == 2 {
                assert!(meta.unavailable.load(Ordering::Acquire));
                assert!(meta.work_tracker.status().unresolved > 0);
            }
            (at, receipts)
        };
        let root = f.close();
        let storage = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
        assert_eq!(storage.control_state().unwrap().generation_ref(), at);
        assert_eq!(rows(storage.meta_store(), control::RECEIPTS), receipts);
        assert_eq!(
            storage.meta_store().pending_effects().unwrap().len(),
            if fault == 1 { 3 } else { 0 }
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_guard_excludes_same_generation_ack_writer() {
    let f = ready_pending(3).await;
    let storage = f.storage.read().unwrap();
    let meta = storage.meta_store();
    let at = storage.control_state().unwrap().generation_ref();
    let operation = meta.pending_effects().unwrap()[0].receipt.operation_id;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    *meta.ack_test_hook.lock().unwrap() = Some(Box::new(move |_| {
        entered_tx.send(()).unwrap();
        release_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    }));
    std::thread::scope(|scope| {
        let batch = scope.spawn(|| meta.acknowledge_rebuilt_effects_at(at, None));
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(matches!(
            meta.publication.try_write(),
            Err(std::sync::TryLockError::WouldBlock)
        ));
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let writer = scope.spawn(move || {
            started_tx.send(()).unwrap();
            let result = meta.acknowledge_derived_effects(operation);
            done_tx.send(()).unwrap();
            result
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(done_rx.try_recv().is_err());
        release_tx.send(()).unwrap();
        batch.join().unwrap().unwrap();
        writer.join().unwrap().unwrap();
    });
    assert!(meta.pending_effects().unwrap().is_empty());
    // A subsequent real canonical writer invalidates readiness at its new G.
    let mut namespace = NamespaceConfig::default_namespace(0);
    namespace.name = "after-ack".into();
    meta.create_namespace(&namespace).unwrap();
    assert!(meta.acknowledge_rebuilt_effects_at(at, None).is_err());
    assert!(!meta.publication_state().unwrap().matches_canonical());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_cancelled_waiter_cannot_cancel_owned_worker_or_claim_drained() {
    let f = seeded(3);
    let owner = publisher(&f);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let receive = {
        let storage = f.storage.read().unwrap();
        let meta = storage.meta_store();
        *meta.ack_test_hook.lock().unwrap() = Some(Box::new(move |_| {
            entered_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
        }));
        probe(meta)
    };
    let task = tokio::spawn(async move { owner.rebuild_all().await });
    tokio::task::spawn_blocking(move || {
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(tracker.status().pending > 0);
    release_tx.send(()).unwrap();
    let work = tokio::task::spawn_blocking(move || {
        receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(work.commit_successes, 1);
    // Acquire a worker-held index lease to await its real release, not a sleep.
    drop(f.graph.write().await);
    assert!(tracker.status().unresolved > 0);
    assert!(tracker.close_and_drain(std::time::Instant::now()).is_err());
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_accepted_completion_finishes_after_root_admission_closes() {
    let f = seeded(3);
    let owner = publisher(&f);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    *f.storage
        .read()
        .unwrap()
        .meta_store()
        .ack_test_hook
        .lock()
        .unwrap() = Some(Box::new(move |_| {
        entered_tx.send(()).unwrap();
        release_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
    }));
    let task = tokio::spawn(async move { owner.rebuild_all().await });
    tokio::task::spawn_blocking(move || {
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
    })
    .await
    .unwrap();
    let closing = tracker.clone();
    let drain = tokio::task::spawn_blocking(move || {
        closing.close_and_drain(std::time::Instant::now() + std::time::Duration::from_secs(10))
    });
    let waiting = tracker.clone();
    tokio::task::spawn_blocking(move || waiting.wait_until_closed())
        .await
        .unwrap();
    assert!(tracker.begin().is_err());
    assert!(!drain.is_finished());
    release_tx.send(()).unwrap();
    task.await.unwrap().unwrap();
    let freeze = drain.await.unwrap().unwrap();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    let storage = f.storage.read().unwrap();
    assert!(storage.meta_store().pending_effects().unwrap().is_empty());
    assert!(
        storage
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    drop(storage);
    drop(freeze);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_crash_child() {
    let Ok(path) = std::env::var("RECALLD_ACK_CRASH_STORE") else {
        return;
    };
    let storage = crate::storage::RedbStorageEngine::open(std::path::Path::new(&path)).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let f = Fixture::from_open_engine(temporary, storage);
    publisher(&f).rebuild_all().await.unwrap();
    panic!("ACK crash point not reached");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_crash_before_after_commit_preserves_canonical_history() {
    for committed in [false, true] {
        let f = seeded(3);
        let (at, receipts) = {
            let storage = f.storage.read().unwrap();
            (
                storage.control_state().unwrap().generation_ref(),
                rows(storage.meta_store(), control::RECEIPTS),
            )
        };
        let root = f.close();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::metadata::effects::ack_tests::startup_ack_crash_child",
                "--nocapture",
            ])
            .env("RECALLD_ACK_CRASH_STORE", root.path())
            .env(
                "RECALLD_TEST_EFFECT_CRASH",
                if committed {
                    "after_rebuilt_effects_ack_commit"
                } else {
                    "before_rebuilt_effects_ack_commit"
                },
            )
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(91),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let storage = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
        assert_eq!(storage.control_state().unwrap().generation_ref(), at);
        assert_eq!(rows(storage.meta_store(), control::RECEIPTS), receipts);
        assert_eq!(
            storage.meta_store().pending_effects().unwrap().len(),
            if committed { 0 } else { 3 }
        );
        let f = Fixture::from_open_engine(root, storage);
        publisher(&f).rebuild_all().await.unwrap();
        assert!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .pending_effects()
                .unwrap()
                .is_empty()
        );
    }
}

thread_local! {
    static WORK: Cell<Option<AckWork>> = const { Cell::new(None) };
}

pub(super) fn count(update: impl FnOnce(&mut AckWork)) {
    WORK.with(|slot| {
        if let Some(mut work) = slot.get() {
            update(&mut work);
            slot.set(Some(work));
        }
    });
}

pub(super) struct Probe(Option<Sender<AckWork>>);
impl Probe {
    pub fn start(store: &MetadataStore) -> Self {
        let sender = store.ack_work_probe.lock().unwrap().take();
        if sender.is_some() {
            WORK.with(|slot| {
                assert!(slot.replace(Some(AckWork::default())).is_none());
            });
        }
        Self(sender)
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let work = WORK.with(|slot| slot.replace(None).unwrap());
            let _ = sender.send(work);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_actual_rebuild_has_linear_validation_work() {
    use crate::storage::{StorageEngine, control::MutationContext, insertion::InsertMemory};
    let fixture = crate::test_support::Fixture::new();
    let n = 32;
    let (send, receive) = std::sync::mpsc::channel();
    let before = {
        let mut storage = fixture.storage.write().unwrap();
        for i in 0..n {
            let input = InsertMemory {
                id: None,
                summary: format!("ack fixture {i}"),
                full_text: None,
                tags: vec![],
                initial_stability: Some(1.),
                created_at: Some(123),
                embedding: None,
                parent_id: None,
                supersedes: None,
            };
            storage
                .insert_memory_with_receipt(
                    &MutationContext::unkeyed(NamespaceId::new(1)),
                    &input,
                    &[1., 0., 0., 0.],
                    123,
                )
                .unwrap();
        }
        let pending = storage.meta_store().pending_effects().unwrap();
        assert_eq!(pending.len(), n);
        assert!(pending.iter().all(|effect| effect.canonical_applied));
        *storage.meta_store().ack_work_probe.lock().unwrap() = Some(send);
        storage.control_state().unwrap().generation_ref()
    };
    crate::derived_completion::DerivedCompletion {
        storage: fixture.storage.clone(),
        cache: fixture.cache.clone(),
        graph: fixture.graph.clone(),
        vectors: fixture.vector_index.clone(),
        fts: fixture.fts_index.clone(),
        entities: fixture.entity_index.clone(),
    }
    .rebuild_all()
    .await
    .unwrap();
    let work = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    {
        let storage = fixture.storage.read().unwrap();
        assert_eq!(storage.control_state().unwrap().generation_ref(), before);
        assert!(storage.meta_store().pending_effects().unwrap().is_empty());
        assert!(
            storage
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
        assert_eq!(storage.scan_all().unwrap().len(), n);
    }
    eprintln!("STARTUP_ACK_WORK backlog={n} {work:?}");
    assert!(
        work.effect_rows <= 2 * n,
        "actual ACK repeats full backlog validation: {work:?}"
    );
    assert!(work.validation_calls <= 2, "{work:?}");
    assert_eq!(work.commit_successes, 1, "{work:?}");
}
