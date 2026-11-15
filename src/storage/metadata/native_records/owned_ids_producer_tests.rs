//! Actual native producer consumers of the same MetadataStore allocation owner.
//! These tests do not activate public native serving or claim whole-state reset.
use super::*;
use crate::storage::ids::{IdKind, IdSource, IdState, SeededIds};

async fn seeded() -> (Fixture, Data, Arc<FixedClock>) {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let engine =
        RedbStorageEngine::open_with_ids(&d.path, true, Box::new(SeededIds::new([7; 32]))).unwrap();
    let f = Fixture::from_open_engine(root, engine);
    completion(&f).rebuild_all().await.unwrap();
    (f, d, clock)
}
fn cursor(f: &Fixture) -> IdState {
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .id_source()
        .checkpoint()
        .unwrap()
        .unwrap()
}
fn memory(receipt: &WriteReceipt) -> MemoryId {
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
        panic!("wrong linked native receipt")
    };
    assert_eq!(transition.sampled_at, 86_400_000);
    transition.memory_id
}
async fn ready(f: &Fixture) {
    let at = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    let report = completion(f)
        .readiness_barrier(
            at,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        )
        .await
        .unwrap();
    assert!(report.ready);
    assert_eq!(report.pending_work, 0);
    assert!(
        report
            .derived
            .iter()
            .all(|v| v.ready && v.applied_generation == at.generation)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_native_single_generates_one_sampled_identity_and_receipt_then_replays_without_allocation()
 {
    let (f, d, clock) = seeded().await;
    let expected = SeededIds::from_state(cursor(&f)).unwrap();
    let id = expected.memory(86_400_000).unwrap();
    let operation = expected.uuid(IdKind::Operation).unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let key = single_context(&f, &d, "owned-single");
    let input = single_input(&d, true);
    let result = a
        .store_native_single(
            key.clone(),
            input.clone(),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(memory(&result.receipt), id);
    assert_eq!(result.receipt.operation_id, operation);
    assert_eq!(&id.as_bytes()[..6], &86_400_000_u64.to_be_bytes()[2..]);
    assert_eq!(Some(cursor(&f)), expected.checkpoint().unwrap());
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    ready(&f).await;
    let before = cursor(&f);
    let replay = a
        .store_native_single(
            key.clone(),
            input.clone(),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(replay.receipt, expected_replay(&result.receipt));
    assert_eq!(cursor(&f), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    let mut changed = input;
    changed.insert.memory.summary.push_str(" different");
    assert!(matches!(
        a.store_native_single(key, changed, NativePolicyInputs::default(), None)
            .await,
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(cursor(&f), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    ready(&f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_native_batch_reservations_and_children_share_source_and_closed_owner_replay_keeps_cursor()
 {
    let (f, d, clock) = seeded().await;
    let expected = SeededIds::from_state(cursor(&f)).unwrap();
    let children = [
        expected.uuid(IdKind::Operation).unwrap(),
        expected.uuid(IdKind::Operation).unwrap(),
    ];
    let parent = expected.uuid(IdKind::Operation).unwrap();
    let memories = [
        expected.memory(86_400_000).unwrap(),
        expected.memory(86_400_000).unwrap(),
    ];
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let mut r = request(&f, "owned-batch");
    r.items
        .push(json!({"summary":"second owned child","entities":["José"]}));
    let out = a.run_batch(r.clone()).await.unwrap();
    assert!(out.all_ready(), "{out:?}");
    assert_eq!(out.batch_operation_id, parent);
    assert_eq!(
        out.reservation_receipt.as_ref().unwrap().operation_id,
        parent
    );
    for (i, item) in out.items.iter().enumerate() {
        let receipt = item.receipt.as_ref().unwrap();
        assert_eq!(receipt.operation_id, children[i]);
        assert_eq!(memory(receipt), memories[i]);
    }
    assert_eq!(Some(cursor(&f)), expected.checkpoint().unwrap());
    assert_eq!(clock.0.load(Ordering::SeqCst), 2);
    assert_eq!(p.calls.load(Ordering::SeqCst), 2);
    ready(&f).await;
    let before = cursor(&f);
    drop(a);
    let root = f.close();
    // The previous owner is actually closed. This is an allocation-source
    // constructor/replay proof, not an unchecked-file snapshot implementation.
    let engine = RedbStorageEngine::open_with_ids(
        &d.path,
        true,
        Box::new(SeededIds::from_state(before.clone()).unwrap()),
    )
    .unwrap();
    let f = Fixture::from_open_engine(root, engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut unavailable = provider(&d);
    Arc::get_mut(&mut unavailable).unwrap().profile = None;
    let a = adapter(&f, unavailable.clone(), clock.clone(), Default::default());
    let replay = a.run_batch(r).await.unwrap();
    assert!(replay.all_ready());
    assert_eq!(replay.batch_operation_id, parent);
    for (actual, prior) in replay.items.iter().zip(&out.items) {
        assert_eq!(
            actual.receipt.as_ref().unwrap(),
            &expected_replay(prior.receipt.as_ref().unwrap())
        );
    }
    assert_eq!(cursor(&f), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 2);
    assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
    assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
    ready(&f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_native_postcommit_repair_preserves_reserved_receipt_and_allocation_cursor() {
    let (f, d, clock) = seeded().await;
    let expected = SeededIds::from_state(cursor(&f)).unwrap();
    let child = expected.uuid(IdKind::Operation).unwrap();
    let parent = expected.uuid(IdKind::Operation).unwrap();
    let id = expected.memory(86_400_000).unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().corrupt = Some(f.vector_index.clone());
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let r = request(&f, "owned-postcommit");
    let out = a.run_batch(r.clone()).await.unwrap();
    assert!(!out.all_ready());
    assert_eq!(out.batch_operation_id, parent);
    let receipt = out.items[0]
        .receipt
        .as_ref()
        .expect("committed child receipt must survive");
    assert_eq!(receipt.operation_id, child);
    assert_eq!(memory(receipt), id);
    assert_eq!(Some(cursor(&f)), expected.checkpoint().unwrap());
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let before = cursor(&f);
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    let replay = a.run_batch(r).await.unwrap();
    assert!(replay.all_ready(), "{replay:?}");
    assert_eq!(
        replay.items[0].receipt.as_ref().unwrap(),
        &expected_replay(receipt)
    );
    assert_eq!(cursor(&f), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    ready(&f).await;
}
