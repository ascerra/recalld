use super::*;
use std::sync::atomic::AtomicI64;
use std::time::{Duration, Instant};

struct CountingClock {
    now: AtomicI64,
    calls: AtomicUsize,
}
impl crate::time::Clock for CountingClock {
    fn now_millis(&self) -> i64 {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.now.load(Ordering::SeqCst)
    }
}
fn setup() -> (Fixture, Data, Arc<CountingClock>, MemoryId, MutationContext) {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(CountingClock {
        now: AtomicI64::new(5000),
        calls: AtomicUsize::new(0),
    });
    let mut s = NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
        .unwrap();
    let intent = s.capture_insert(scope(&d), &input(&d)).unwrap();
    let created = s
        .insert_prepared(&ctx(&s, &d, "create"), &intent, &generated())
        .unwrap();
    let id = inserted(&created);
    let mut context = ctx(&s, &d, "forget");
    context.expected_generation = Some(created.committed_generation);
    (
        Fixture::from_open_engine(root, s.engine),
        d,
        clock,
        id,
        context,
    )
}
async fn until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_forget_concurrent_replay_finishes_accepted_descendants_and_fresh_barrier() {
    let (f, d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let before = f.record(id).unwrap();
    let target = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let root = tracker.begin().unwrap();
    let held = f.graph.clone().write_owned().await;
    let calls = clock.calls.load(Ordering::SeqCst);
    clock.now.store(1000, Ordering::SeqCst);
    let worker = publisher.clone();
    let worker_clock = clock.clone();
    let task = tokio::spawn(async move {
        let (a, b) = tokio::join!(
            worker.forget_native_with_work(
                context.clone(),
                id,
                Some("native".into()),
                NativePolicyInputs::default(),
                worker_clock.clone(),
                Some(&root)
            ),
            worker.forget_native_with_work(
                context,
                id,
                Some("native".into()),
                NativePolicyInputs::default(),
                worker_clock,
                Some(&root)
            )
        );
        root.finish();
        (a.unwrap(), b.unwrap())
    });
    until(|| tracker.status().pending >= 3).await;
    let barrier = publisher.clone();
    let ready = tokio::spawn(async move {
        barrier
            .readiness_barrier(target, Instant::now() + Duration::from_secs(10))
            .await
    });
    until(|| !tracker.status().admission_open).await;
    drop(held);
    let (a, b) = task.await.unwrap();
    assert_eq!(a.operation_id, b.operation_id);
    assert_ne!(a.replayed, b.replayed);
    assert_eq!(a.committed_generation, target.generation.next().unwrap());
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);
    let ready = ready.await.unwrap().unwrap();
    assert!(ready.ready);
    assert_eq!(ready.committed_generation, a.committed_generation);
    let after = f.record(id).unwrap();
    assert_eq!(after.phase, DecayPhase::Tombstone);
    assert_eq!(after.access_history, before.access_history);
    assert_eq!(
        after.schema.as_ref().unwrap().lifecycle.decay_anchor_at,
        before.schema.as_ref().unwrap().lifecycle.decay_anchor_at
    );
    assert_eq!(
        after.schema.as_ref().unwrap().lifecycle.phase_changed_at,
        before
            .schema
            .as_ref()
            .unwrap()
            .lifecycle
            .phase_changed_at
            .max(
                before
                    .schema
                    .as_ref()
                    .unwrap()
                    .lifecycle
                    .lifecycle_updated_at
                    .max(1000)
            )
    );
    assert!(after.schema.as_ref().unwrap().vector.is_none());
    assert!(after.schema.as_ref().unwrap().labels.entities.is_empty());
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    drop(publisher);
    let _root = f.close();
    inspect_schema(&d.path).unwrap();
    let reopened =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).unwrap();
    assert_eq!(
        reopened.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_forget_partial_publication_replays_before_policy_clock_and_cas() {
    let (f, d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    let result = publisher
        .forget_native_with_work(
            context.clone(),
            id,
            None,
            NativePolicyInputs::default(),
            clock.clone(),
            None,
        )
        .await;
    let Err(StorageError::CommittedMutation { receipt, .. }) = result else {
        panic!("missing full receipt")
    };
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativeMemoryTombstoned { .. }
    ));
    let calls = clock.calls.load(Ordering::SeqCst);
    let current = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        publisher
            .readiness_barrier(current, Instant::now() + Duration::from_secs(5))
            .await
            .is_err()
    );
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    let invalid_policy = NativePolicyInputs {
        multiplier: f64::NAN,
        ..Default::default()
    };
    clock.now.store(i64::MIN, Ordering::SeqCst);
    let replay = publisher
        .forget_native_with_work(
            context.clone(),
            id,
            None,
            invalid_policy.clone(),
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(replay.committed_generation, receipt.committed_generation);
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls);
    assert!(
        publisher
            .readiness_barrier(current, Instant::now() + Duration::from_secs(5))
            .await
            .unwrap()
            .ready
    );
    let before = bytes(&d.path);
    let error = publisher
        .forget_native_with_work(
            context,
            id,
            Some("another-name".into()),
            invalid_policy,
            clock.clone(),
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error,StorageError::CommittedMutation{receipt:r,source} if r.operation_id==receipt.operation_id && matches!(*source,StorageError::NamespaceMismatch{..}))
    );
    assert_eq!(bytes(&d.path), before);
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_forget_scope_contract_and_stale_cas_reject_before_clock_or_bytes() {
    let (f, d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let before = bytes(&d.path);
    let calls = clock.calls.load(Ordering::SeqCst);
    for case in ["contract", "scope", "assertion", "cas"] {
        let mut ctx = context.clone();
        let mut name = None;
        match case {
            "contract" => ctx.contract_version = 3,
            "scope" => ctx.scope = MutationScope::Namespace(NamespaceId::new(999)),
            "assertion" => name = Some("another-name".into()),
            "cas" => ctx.expected_generation = Some(Generation::default()),
            _ => unreachable!(),
        }
        let error = publisher
            .forget_native_with_work(
                ctx,
                id,
                name,
                NativePolicyInputs::default(),
                clock.clone(),
                None,
            )
            .await
            .unwrap_err();
        if case == "assertion" {
            assert!(matches!(error, StorageError::NamespaceMismatch { .. }));
        } else if case == "contract" {
            assert!(matches!(error, StorageError::InvalidMutationContext(_)));
        }
        assert_eq!(bytes(&d.path), before, "{case}");
        assert_eq!(clock.calls.load(Ordering::SeqCst), calls, "{case}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_forget_terminal_new_key_is_receipted_and_replay_never_rebinds_id() {
    let (f, d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let first = publisher
        .forget_native_with_work(
            context.clone(),
            id,
            None,
            NativePolicyInputs::default(),
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    let before = f.record(id).unwrap().encode().unwrap();
    let mut next = context.clone();
    next.idempotency_key = Some("terminal-new-key".into());
    next.expected_generation = None;
    let second = publisher
        .forget_native_with_work(
            next.clone(),
            id,
            None,
            NativePolicyInputs::default(),
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert_ne!(first.operation_id, second.operation_id);
    assert_eq!(
        second.committed_generation,
        first.committed_generation.next().unwrap()
    );
    assert_eq!(f.record(id).unwrap().encode().unwrap(), before);
    let calls = clock.calls.load(Ordering::SeqCst);
    let invalid_policy = NativePolicyInputs {
        multiplier: f64::NAN,
        ..Default::default()
    };
    let replay = publisher
        .forget_native_with_work(next, id, None, invalid_policy.clone(), clock.clone(), None)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, second.operation_id);
    assert!(matches!(
        publisher
            .forget_native_with_work(context, d.parent, None, invalid_policy, clock.clone(), None)
            .await,
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls);
    assert_eq!(f.record(d.parent).unwrap().phase, DecayPhase::Full);
    let target = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    assert_eq!(target.generation, second.committed_generation);
    assert!(
        publisher
            .readiness_barrier(target, Instant::now() + Duration::from_secs(5))
            .await
            .unwrap()
            .ready
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_forget_cancelled_waiter_retains_worker_lease_and_unresolved_work() {
    let (f, _d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let root = tracker.begin().unwrap();
    let gate = crate::derived_completion::publication_gate(&f.storage)
        .await
        .unwrap();
    let held = f.graph.clone().write_owned().await;
    let task = tokio::spawn(async move {
        publisher
            .forget_native_with_work(
                context,
                id,
                None,
                NativePolicyInputs::default(),
                clock,
                Some(&root),
            )
            .await
    });
    until(|| tracker.status().pending == 2 && gate.try_write().is_err()).await;
    let storage = f.storage.clone();
    let (held_tx, held_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        let _guard = storage.write().unwrap();
        held_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    held_rx.await.unwrap();
    drop(held);
    until(|| tracker.status().pending == 3).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let publication_held = gate.try_write().is_err();
    let graph_held = f.graph.try_write().is_err();
    release_tx.send(()).unwrap();
    blocker.await.unwrap();
    until(|| tracker.status().pending <= 2).await;
    assert!(publication_held && graph_held);
    assert!(tracker.status().unresolved >= 1);
    let target = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        completion(&f)
            .readiness_barrier(target, Instant::now() + Duration::from_millis(50))
            .await
            .is_err()
    );
}
