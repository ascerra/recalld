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
    setup_with_multiplier(1.)
}
fn setup_with_multiplier(
    multiplier: f64,
) -> (Fixture, Data, Arc<CountingClock>, MemoryId, MutationContext) {
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
    if multiplier != 1. {
        drop(s);
        crate::test_support::explicitly_adopt_inherited_policy(&d.path, d.ns, multiplier, 5000);
        s = NativeLifecycleSession::open(
            &d.path,
            NativePolicyInputs {
                multiplier,
                ..Default::default()
            },
            clock.clone(),
        )
        .unwrap();
    }
    let mut context = ctx(&s, &d, "feedback");
    context.expected_generation = Some(s.metadata().control_state().unwrap().committed_generation);
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

const DAY: i64 = 86_400_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_feedback_replay_survives_actual_policy_rollback_but_not_restore_lineage() {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    let (f, mut d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let receipt = publisher
        .feedback_native_with_work(
            context.clone(),
            id,
            3,
            None,
            NativePolicyInputs::default(),
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    let calls = clock.calls.load(Ordering::SeqCst);
    drop(publisher);
    let root = f.close();
    let destination = root.path().join("rollback");
    migrate_lifecycle(
        &d.path,
        &destination,
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(&d.path).unwrap().control.generation_ref(),
            namespace_id: d.ns,
            expected_policy_revision: policy_revision(&d),
            target: LifecycleVersion::LegacyAccessV1,
            idempotency_key: "rollback".into(),
        },
    )
    .unwrap();
    d.path = destination;
    clock.now.store(i64::MIN, Ordering::SeqCst);
    let session =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    let publisher = completion(&f);
    let before = f.record(id).unwrap().encode().unwrap();
    let current = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    let replay = publisher
        .feedback_native_with_work(
            context.clone(),
            id,
            3,
            Some("native".into()),
            NativePolicyInputs {
                multiplier: f64::NAN,
                ..Default::default()
            },
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls);
    assert_eq!(f.record(id).unwrap().encode().unwrap(), before);
    let ready = publisher
        .readiness_barrier(current, Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(ready.committed_generation, current.generation);
    assert!(ready.committed_generation > receipt.committed_generation);
    drop(publisher);
    let root = f.close();
    let backup = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("feedback.zip"),
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
    assert_ne!(
        inspect_schema(&d.path).unwrap().control.store_id,
        receipt.store_id
    );
    let session =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let before = bytes(&d.path);
    assert!(matches!(
        publisher
            .feedback_native_with_work(
                context,
                id,
                3,
                None,
                NativePolicyInputs::default(),
                clock.clone(),
                None
            )
            .await,
        Err(StorageError::StoreLineageMismatch)
    ));
    assert_eq!(bytes(&d.path), before);
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_feedback_uses_anchor_and_configured_policy_and_replays_before_clock() {
    use crate::decay::lifecycle::{FeedbackState, plan_feedback_state};
    let (f, d, clock, id, mut context) = setup_with_multiplier(0.25);
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let policy = NativePolicyInputs {
        multiplier: 0.25,
        ..Default::default()
    };
    clock.now.store(DAY, Ordering::SeqCst);
    {
        let engine = f.storage.read().unwrap();
        let intent = engine
            .meta_store()
            .capture_exposure(
                scope(&d),
                &[(id, crate::model::AccessKind::DirectRetrieval)],
                &policy,
            )
            .unwrap();
        let mut exposure = context.clone();
        exposure.idempotency_key = Some("exposure".into());
        engine
            .meta_store()
            .record_native_exposure(&exposure, &intent, &policy, clock.as_ref())
            .unwrap();
    }
    publisher.complete().await.unwrap();
    let before = f.record(id).unwrap();
    let fields = &before.schema.as_ref().unwrap().lifecycle;
    assert_ne!(fields.decay_anchor_at, before.last_accessed_at);
    let ns = f
        .storage
        .read()
        .unwrap()
        .get_namespace(d.ns)
        .unwrap()
        .unwrap();
    let resolved = policy.resolve(&ns).unwrap();
    let state = FeedbackState {
        stability: before.stability,
        difficulty: before.difficulty,
        phase: before.phase,
        is_permastore: before.is_permastore != 0,
        has_summary: !before.summary.is_empty(),
        anchor: fields.decay_anchor_at,
        updated_at: fields.lifecycle_updated_at,
    };
    let expected = plan_feedback_state(&state, &resolved, 2 * DAY, 3).unwrap();
    let wrong = plan_feedback_state(
        &FeedbackState {
            anchor: before.last_accessed_at,
            ..state
        },
        &resolved,
        2 * DAY,
        3,
    )
    .unwrap();
    assert_ne!(expected.stability, wrong.stability);
    let prior = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    context.expected_generation = Some(prior.generation);
    clock.now.store(2 * DAY, Ordering::SeqCst);
    let calls = clock.calls.load(Ordering::SeqCst);
    let receipt = publisher
        .feedback_native_with_work(
            context.clone(),
            id,
            3,
            Some("native".into()),
            policy,
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);
    assert_eq!(
        receipt.committed_generation,
        prior.generation.next().unwrap()
    );
    let after = f.record(id).unwrap();
    assert_eq!(after.stability, expected.stability);
    assert_eq!(after.phase, before.phase);
    assert_eq!(
        after.schema.as_ref().unwrap().lifecycle.decay_anchor_at,
        2 * DAY
    );
    assert_eq!(
        after.schema.as_ref().unwrap().lifecycle.retrieved_at,
        Some(DAY)
    );
    assert_eq!(
        after.schema.as_ref().unwrap().lifecycle.reinforced_at,
        Some(2 * DAY)
    );
    assert_eq!(after.access_history.len(), before.access_history.len() + 1);
    let target = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        publisher
            .readiness_barrier(target, Instant::now() + Duration::from_secs(5))
            .await
            .unwrap()
            .ready
    );
    clock.now.store(i64::MIN, Ordering::SeqCst);
    let invalid = NativePolicyInputs {
        multiplier: f64::NAN,
        ..Default::default()
    };
    let replay = publisher
        .feedback_native_with_work(
            context.clone(),
            id,
            3,
            None,
            invalid.clone(),
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(receipt.operation_id, replay.operation_id);
    assert!(matches!(
        publisher
            .feedback_native_with_work(context, id, 4, None, invalid, clock.clone(), None)
            .await,
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);
    assert_eq!(
        f.record(id).unwrap().encode().unwrap(),
        after.encode().unwrap()
    );
    drop(publisher);
    let _root = f.close();
    inspect_schema(&d.path).unwrap();
    let reopened =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).unwrap();
    assert_eq!(
        reopened
            .engine
            .get_record(id)
            .unwrap()
            .unwrap()
            .encode()
            .unwrap(),
        after.encode().unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_feedback_partial_publication_retains_receipt_without_outbox_and_repairs_on_replay()
{
    let (f, _d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    let error = publisher
        .feedback_native_with_work(
            context.clone(),
            id,
            3,
            None,
            NativePolicyInputs::default(),
            clock.clone(),
            None,
        )
        .await
        .unwrap_err();
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("missing full receipt")
    };
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::LifecycleFeedbackApplied { .. }
    ));
    let calls = clock.calls.load(Ordering::SeqCst);
    let before = f.record(id).unwrap().encode().unwrap();
    let target = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    assert!(
        publisher
            .readiness_barrier(target, Instant::now() + Duration::from_secs(5))
            .await
            .is_err()
    );
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    clock.now.store(i64::MIN, Ordering::SeqCst);
    let replay = publisher
        .feedback_native_with_work(
            context,
            id,
            3,
            None,
            NativePolicyInputs {
                multiplier: f64::NAN,
                ..Default::default()
            },
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(receipt.operation_id, replay.operation_id);
    assert_eq!(receipt.committed_generation, replay.committed_generation);
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls);
    assert_eq!(f.record(id).unwrap().encode().unwrap(), before);
    assert!(
        publisher
            .readiness_barrier(target, Instant::now() + Duration::from_secs(5))
            .await
            .unwrap()
            .ready
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_feedback_historical_replay_after_forget_cannot_resurrect_or_append_history() {
    let (f, d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let receipt = publisher
        .feedback_native_with_work(
            context.clone(),
            id,
            3,
            None,
            NativePolicyInputs::default(),
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    {
        let mut storage = f.storage.write().unwrap();
        let intent = storage
            .meta_store()
            .capture_native_tombstone(id, Some(d.ns), &NativePolicyInputs::default())
            .unwrap();
        let mut forget = context.clone();
        forget.idempotency_key = Some("forget".into());
        forget.expected_generation = None;
        storage
            .tombstone_native_record(
                &forget,
                &intent,
                &NativePolicyInputs::default(),
                clock.as_ref(),
                None,
            )
            .unwrap();
    }
    publisher.complete().await.unwrap();
    let before = f.record(id).unwrap().encode().unwrap();
    let calls = clock.calls.load(Ordering::SeqCst);
    let replay = publisher
        .feedback_native_with_work(
            context.clone(),
            id,
            3,
            None,
            NativePolicyInputs {
                multiplier: f64::NAN,
                ..Default::default()
            },
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    let mut fresh = context;
    fresh.idempotency_key = Some("new-feedback".into());
    fresh.expected_generation = None;
    assert!(matches!(
        publisher
            .feedback_native_with_work(
                fresh,
                id,
                3,
                None,
                NativePolicyInputs::default(),
                clock.clone(),
                None
            )
            .await,
        Err(StorageError::LifecycleContentUnavailable)
    ));
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls);
    assert_eq!(f.record(id).unwrap().encode().unwrap(), before);
    assert_eq!(f.record(id).unwrap().phase, DecayPhase::Tombstone);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_feedback_rejects_quality_contract_scope_name_and_cas_without_time_or_writes() {
    let (f, d, clock, id, context) = setup();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let before = bytes(&d.path);
    let calls = clock.calls.load(Ordering::SeqCst);
    for case in [
        "quality-zero",
        "quality-five",
        "contract",
        "scope",
        "name",
        "cas",
    ] {
        let mut key = context.clone();
        let mut name = None;
        let mut quality = 3;
        match case {
            "quality-zero" => quality = 0,
            "quality-five" => quality = 5,
            "contract" => key.contract_version = 3,
            "scope" => key.scope = MutationScope::Namespace(NamespaceId::new(999)),
            "name" => name = Some("another-name".into()),
            "cas" => key.expected_generation = Some(Generation::default()),
            _ => unreachable!(),
        }
        let error = publisher
            .feedback_native_with_work(
                key,
                id,
                quality,
                name,
                NativePolicyInputs::default(),
                clock.clone(),
                None,
            )
            .await
            .unwrap_err();
        if case == "name" {
            assert!(matches!(error, StorageError::NamespaceMismatch { .. }));
        }
        assert_eq!(clock.calls.load(Ordering::SeqCst), calls, "{case}");
        assert_eq!(bytes(&d.path), before, "{case}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_feedback_accepted_children_finish_closed_admission_and_cancelled_worker_retains_lease()
 {
    for cancel in [false, true] {
        let (f, _d, clock, id, context) = setup();
        let publisher = completion(&f);
        publisher.rebuild_all().await.unwrap();
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
        let gate = crate::derived_completion::publication_gate(&f.storage)
            .await
            .unwrap();
        let graph = f.graph.clone().write_owned().await;
        let worker = publisher.clone();
        let task = tokio::spawn(async move {
            let r = worker
                .feedback_native_with_work(
                    context,
                    id,
                    3,
                    None,
                    NativePolicyInputs::default(),
                    clock,
                    Some(&root),
                )
                .await;
            root.finish();
            r
        });
        until(|| tracker.status().pending == 2 && gate.try_write().is_err()).await;
        if !cancel {
            let barrier = publisher.clone();
            let ready = tokio::spawn(async move {
                barrier
                    .readiness_barrier(target, Instant::now() + Duration::from_secs(10))
                    .await
            });
            until(|| !tracker.status().admission_open).await;
            drop(graph);
            let receipt = task.await.unwrap().unwrap();
            let ready = ready.await.unwrap().unwrap();
            assert!(ready.ready);
            assert_eq!(ready.committed_generation, receipt.committed_generation);
            assert_eq!(tracker.status().pending, 0);
        } else {
            let storage = f.storage.clone();
            let (held_tx, held_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _held = storage.write().unwrap();
                held_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            });
            held_rx.await.unwrap();
            drop(graph);
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
            assert!(
                publisher
                    .readiness_barrier(target, Instant::now() + Duration::from_millis(50))
                    .await
                    .is_err()
            );
        }
    }
}
