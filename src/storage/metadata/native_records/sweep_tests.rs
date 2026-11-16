//! Actual owning native engine, existing scheduler and fresh-generation barrier.
use super::*;
use crate::decay::sweep::{DecaySweepRunner, SweepConfig, SweepResult};
use crate::storage::work::{WorkStatus, WorkTracker};
use crate::test_support::Fixture;
use std::sync::atomic::AtomicI64;
use std::time::{Duration, Instant};

const DAY: i64 = 86_400_000;
struct CountingClock {
    now: AtomicI64,
    calls: AtomicUsize,
}
impl CountingClock {
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
impl crate::time::Clock for CountingClock {
    fn now_millis(&self) -> i64 {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.now.load(Ordering::SeqCst)
    }
}
fn completion(f: &Fixture) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}
fn target(f: &Fixture) -> crate::storage::control::GenerationRef {
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .publication_state()
        .unwrap()
        .canonical
}
fn row(f: &Fixture, id: MemoryId) -> DiskRecord {
    f.storage.read().unwrap().get_record(id).unwrap().unwrap()
}
fn setup() -> (Fixture, Data, MemoryId, Arc<CountingClock>) {
    setup_with_multiplier(1.)
}
fn setup_with_multiplier(multiplier: f64) -> (Fixture, Data, MemoryId, Arc<CountingClock>) {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let c = Arc::new(CountingClock::new(3000));
    let mut session =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c.clone()).unwrap();
    let mut original = input(&d);
    original.memory.initial_stability = Some(1.);
    let intent = session.capture_insert(scope(&d), &original).unwrap();
    let key = ctx(&session, &d, "sweep-target");
    let id = inserted(
        &session
            .insert_prepared(&key, &intent, &generated())
            .unwrap(),
    );
    if multiplier != 1. {
        drop(session);
        crate::test_support::explicitly_adopt_inherited_policy(&d.path, d.ns, multiplier, 3000);
        session = NativeLifecycleSession::open(
            &d.path,
            NativePolicyInputs {
                multiplier,
                ..Default::default()
            },
            c.clone(),
        )
        .unwrap();
    }
    (Fixture::from_open_engine(root, session.engine), d, id, c)
}
async fn wait(tracker: &WorkTracker, predicate: impl Fn(WorkStatus) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !predicate(tracker.status()) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
fn runner(f: &Fixture, c: Arc<CountingClock>, startup: bool) -> DecaySweepRunner {
    DecaySweepRunner::new(
        SweepConfig {
            sweep_on_startup: startup,
            ..Default::default()
        },
        Arc::new(crate::decay::DecayConfig::default()),
        Default::default(),
        f.storage.clone(),
        f.graph.clone(),
        f.cache.clone(),
        1.,
    )
    .unwrap()
    .with_clock(c)
    .with_completion(completion(f))
}
async fn pass(f: &Fixture, now: i64) -> SweepResult {
    DecaySweepRunner::execute_sweep_with_work(
        &SweepConfig::default(),
        &crate::decay::DecayConfig::default(),
        &Default::default(),
        &f.storage,
        &f.graph,
        &f.cache,
        1.,
        Some(now),
        None,
        Some(&completion(f)),
    )
    .await
}
async fn ready(f: &Fixture) {
    let generation = target(f);
    let report = completion(f)
        .readiness_barrier(generation, Instant::now() + Duration::from_secs(10))
        .await
        .unwrap();
    assert_eq!(report.committed_generation, generation.generation);
    assert!(report.ready);
    assert_eq!(report.pending_work, 0);
    assert_eq!(report.derived.len(), 4);
    assert!(
        report
            .derived
            .iter()
            .all(|v| v.ready && v.applied_generation == generation.generation)
    );
}
fn retained(before: &DiskRecord, after: &DiskRecord) {
    assert_eq!(before.created_at, after.created_at);
    assert_eq!(before.last_accessed_at, after.last_accessed_at);
    assert_eq!(before.stability, after.stability);
    assert_eq!(before.difficulty, after.difficulty);
    assert_eq!(before.is_permastore, after.is_permastore);
    assert_eq!(before.access_history, after.access_history);
    let a = &before.schema.as_ref().unwrap().lifecycle;
    let b = &after.schema.as_ref().unwrap().lifecycle;
    assert_eq!(a.decay_anchor_at, b.decay_anchor_at);
    assert_eq!(a.observed_at, b.observed_at);
    assert_eq!(a.retrieved_at, b.retrieved_at);
    assert_eq!(a.reinforced_at, b.reinforced_at);
    assert_eq!(a.timestamp_origin, b.timestamp_origin);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sweep_startup_and_manual_finish_under_closed_admission_with_fresh_barrier() {
    for startup in [true, false] {
        let (f, d, id, c) = setup();
        completion(&f).rebuild_all().await.unwrap(); // Initial private views only.
        ready(&f).await;
        let original = row(&f, id);
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        c.set(10_000 * DAY);
        let sweep = runner(&f, c.clone(), startup);
        let mut handle = None;
        for (index, phase) in [Some(DecayPhase::Summary), Some(DecayPhase::Ghost), None]
            .into_iter()
            .enumerate()
        {
            let before = target(&f);
            let calls = c.calls.load(Ordering::SeqCst);
            let held = f.graph.clone().write_owned().await;
            if index > 0 || !startup {
                sweep.trigger();
                if index == 0 {
                    sweep.trigger();
                    assert_eq!(
                        tracker.status().pending,
                        1,
                        "coalesced trigger must own one root before start"
                    );
                }
            }
            if index == 0 {
                handle = Some(sweep.start());
            }
            let publisher = completion(&f);
            let barrier = tokio::spawn(async move {
                publisher
                    .readiness_barrier(before, Instant::now() + Duration::from_secs(10))
                    .await
            });
            wait(&tracker, |s| !s.admission_open).await;
            assert!(!barrier.is_finished());
            drop(held);
            let report = barrier.await.unwrap().unwrap();
            assert_eq!(report.committed_generation, target(&f).generation);
            assert!(report.committed_generation > before.generation);
            assert!(
                report
                    .derived
                    .iter()
                    .all(|v| v.ready && v.applied_generation == report.committed_generation)
            );
            assert_eq!(
                c.calls.load(Ordering::SeqCst),
                calls + 1,
                "one sample per scheduled pass"
            );
            let current = f.storage.read().unwrap().get_record(id).unwrap();
            assert_eq!(current.as_ref().map(|r| r.phase), phase);
            if let Some(current) = current {
                retained(&original, &current);
                assert_eq!(
                    current.schema.as_ref().unwrap().lifecycle.phase_changed_at,
                    10_000 * DAY
                );
                if current.phase == DecayPhase::Ghost {
                    assert!(current.schema.as_ref().unwrap().labels.is_empty());
                    assert!(
                        f.storage
                            .read()
                            .unwrap()
                            .get_vector(d.ns, current.vector_slot)
                            .unwrap()
                            .is_some()
                    );
                }
            }
            assert_eq!(tracker.status().pending, 0);
            assert_eq!(tracker.status().unresolved, 0);
        }
        sweep.shutdown();
        handle.unwrap().await.unwrap();
        drop(sweep);
        ready(&f).await;
        let _root = f.close();
        inspect_schema(&d.path).unwrap();
        assert!(
            RedbStorageEngine::open(&d.path).is_err(),
            "ordinary v2 serving remains closed"
        );
        let reopened =
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c).unwrap();
        assert!(reopened.engine.get_record(id).unwrap().is_none());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sweep_partial_publication_failure_keeps_original_receipt_until_actual_repair() {
    let (f, _d, _id, _c) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let before = target(&f);
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    let result = pass(&f, 10_000 * DAY).await;
    let (receipt, _) = result
        .failed_commit
        .as_ref()
        .expect("full original committed receipt");
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativePhaseMaterialized { .. }
    ));
    assert_eq!(
        receipt.committed_generation,
        before.generation.next().unwrap()
    );
    assert_eq!(
        target(&f).generation,
        receipt.committed_generation,
        "stop after partial failure"
    );
    assert_eq!(result.full_to_summary, 1);
    assert!(!result.compaction_triggered);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    assert!(
        completion(&f)
            .readiness_barrier(target(&f), Instant::now() + Duration::from_secs(1))
            .await
            .is_err()
    );
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    completion(&f).complete().await.unwrap(); // Explicit repair test, not the successful-pass proof.
    ready(&f).await;
    assert_eq!(
        target(&f).generation,
        receipt.committed_generation,
        "repair cannot repeat unkeyed mutation"
    );
    let engine = f.storage.read().unwrap();
    let read = engine.meta_store().db.begin_read().unwrap();
    let key = control::scoped_key(
        receipt.store_id,
        MutationScope::Namespace(match &receipt.outcome {
            MutationOutcome::NativePhaseMaterialized { transition } => transition.namespace_id,
            _ => unreachable!(),
        }),
        receipt.operation_id.as_bytes(),
    );
    let stored = read.open_table(control::RECEIPTS).unwrap();
    assert_eq!(
        control::validate_receipt(stored.get(key.as_slice()).unwrap().unwrap().value())
            .unwrap()
            .receipt,
        *receipt
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sweep_rechecks_donor_and_fan_after_capture_without_resampling_clock() {
    for days in [2., 4.5] {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let created = 10_000 * DAY;
        let c = Arc::new(CountingClock::new(created));
        let mut s = NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c.clone())
            .unwrap();
        let mut original = input(&d);
        original.memory.created_at = Some(created);
        original.memory.initial_stability = Some(1.);
        let capture = s.capture_insert(scope(&d), &original).unwrap();
        let id = inserted(
            &s.insert_prepared(&ctx(&s, &d, "target"), &capture, &generated())
                .unwrap(),
        );
        let f = Fixture::from_open_engine(root, s.engine);
        completion(&f).rebuild_all().await.unwrap();
        let before = row(&f, id);
        let now = created + (days * DAY as f64) as i64;
        c.set(now);
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let held = f.graph.clone().write_owned().await;
        let storage = f.storage.clone();
        let graph = f.graph.clone();
        let cache = f.cache.clone();
        let publisher = completion(&f);
        let task = tokio::spawn(async move {
            DecaySweepRunner::execute_sweep_with_work(
                &SweepConfig::default(),
                &crate::decay::DecayConfig::default(),
                &Default::default(),
                &storage,
                &graph,
                &cache,
                1.,
                Some(now),
                None,
                Some(&publisher),
            )
            .await
        });
        wait(&tracker, |s| s.pending >= 2).await; // Per-record accepted child follows snapshot capture.
        {
            let mut engine = f.storage.write().unwrap();
            let policy = NativePolicyInputs::default();
            let mut context = MutationContext {
                store_id: Some(target_from_engine(&engine)),
                scope: MutationScope::Namespace(d.ns),
                idempotency_key: Some("donor-feedback".into()),
                expected_generation: None,
                contract_version: 2,
            };
            let intent = engine
                .meta_store()
                .capture_feedback(d.parent, None, 3, &policy)
                .unwrap();
            engine
                .meta_store()
                .apply_native_feedback(&context, &intent, &policy, c.as_ref())
                .unwrap();
            let mut donor = input(&d);
            donor.memory.created_at = Some(now);
            let intent = engine
                .meta_store()
                .capture_native_insert(scope(&d), &donor, &policy)
                .unwrap();
            context.idempotency_key = Some("new-fan".into());
            engine
                .insert_native_record(&context, &intent, &generated(), &policy, c.as_ref(), None)
                .unwrap();
            assert_eq!(
                engine
                    .get_record(id)
                    .unwrap()
                    .unwrap()
                    .schema
                    .as_ref()
                    .unwrap()
                    .lifecycle
                    .record_generation,
                before.schema.as_ref().unwrap().lifecycle.record_generation
            );
        }
        c.set(20_000 * DAY);
        let calls = c.calls.load(Ordering::SeqCst);
        drop(held);
        let result = task.await.unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(
            result.publication_error.is_none(),
            "{:?}",
            result.publication_error
        );
        assert!(!result.compaction_triggered);
        assert_eq!(c.calls.load(Ordering::SeqCst), calls);
        let after = row(&f, id);
        assert_eq!(after.phase, DecayPhase::Full);
        assert!(after.decay_strength > after.strength);
        assert_eq!(
            after
                .schema
                .as_ref()
                .unwrap()
                .lifecycle
                .lifecycle_updated_at,
            now
        );
        retained(&before, &after);
        ready(&f).await;
    }
}
fn target_from_engine(engine: &RedbStorageEngine) -> uuid::Uuid {
    engine.meta_store().control_state().unwrap().store_id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sweep_aborted_or_queued_shutdown_work_cannot_claim_readiness() {
    for queued in [false, true] {
        let (f, _d, id, c) = setup();
        completion(&f).rebuild_all().await.unwrap();
        let before = target(&f);
        let encoded = row(&f, id).encode().unwrap();
        c.set(10_000 * DAY);
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let sweep = runner(&f, c, !queued);
        let held = f.graph.clone().write_owned().await;
        if queued {
            sweep.trigger();
            sweep.shutdown();
        } else {
            let task = sweep.start();
            wait(&tracker, |s| s.pending >= 3).await;
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        }
        drop(held);
        assert!(tracker.status().unresolved > 0);
        assert_eq!(target(&f), before);
        assert_eq!(row(&f, id).encode().unwrap(), encoded);
        assert!(
            completion(&f)
                .readiness_barrier(before, Instant::now() + Duration::from_millis(50))
                .await
                .is_err()
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sweep_without_completion_refuses_before_canonical_mutation() {
    let (f, _d, id, _c) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let before = target(&f);
    let record = row(&f, id).encode().unwrap();
    let result = DecaySweepRunner::execute_sweep_at(
        &SweepConfig::default(),
        &crate::decay::DecayConfig::default(),
        &Default::default(),
        &f.storage,
        &f.graph,
        &f.cache,
        1.,
        Some(10_000 * DAY),
    )
    .await;
    assert!(result.publication_error.unwrap().contains("completion"));
    assert_eq!(target(&f), before);
    assert_eq!(row(&f, id).encode().unwrap(), record);
    ready(&f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sweep_preserves_unknown_migration_marker_and_clamps_regressed_clock() {
    let root = tempfile::tempdir().unwrap();
    let mut d = fixture(root.path());
    let source = root.path().join("legacy");
    let canonical = root.path().join("late-schema");
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
    let destination = root.path().join("late-v2");
    migrate_lifecycle(
        &canonical,
        &destination,
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(&canonical).unwrap().control.generation_ref(),
            namespace_id: d.ns,
            expected_policy_revision: Generation::default(),
            target: LifecycleVersion::ExplicitFeedbackV2,
            idempotency_key: "late-policy".into(),
        },
    )
    .unwrap();
    d.path = destination;
    // Explicit present-day policy adoption must not rewrite the unknown old
    // phase timestamp: the assertions below retain that original marker.
    crate::test_support::explicitly_adopt_inherited_policy(&d.path, d.ns, 1., 500 * DAY);
    let c = Arc::new(CountingClock::new(500 * DAY));
    let session =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c.clone()).unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    let before = row(&f, d.parent);
    assert_eq!(
        before.schema.as_ref().unwrap().lifecycle.phase_changed_at,
        10_000 * DAY
    );
    for (now, phase) in [
        (500 * DAY, DecayPhase::Summary),
        (100 * DAY, DecayPhase::Ghost),
    ] {
        let result = pass(&f, now).await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.publication_error.is_none());
        let after = row(&f, d.parent);
        assert_eq!(after.phase, phase);
        retained(&before, &after);
        assert_eq!(
            after
                .schema
                .as_ref()
                .unwrap()
                .lifecycle
                .lifecycle_updated_at,
            500 * DAY
        );
        assert_eq!(
            after.schema.as_ref().unwrap().lifecycle.phase_changed_at,
            10_000 * DAY
        );
        ready(&f).await;
    }
    let _root = f.close();
    inspect_schema(&d.path).unwrap();
    let reopened = NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c).unwrap();
    assert_eq!(
        reopened.engine.get_record(d.parent).unwrap().unwrap().phase,
        DecayPhase::Ghost
    );
}

#[tokio::test]
async fn native_sweep_actual_interval_registers_only_at_tick_and_publishes_fresh_generation() {
    let (f, _d, id, c) = setup();
    completion(&f).rebuild_all().await.unwrap();
    ready(&f).await;
    c.set(10_000 * DAY);
    let calls = c.calls.load(Ordering::SeqCst);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let before = target(&f);
    let sweep = DecaySweepRunner::new(
        SweepConfig {
            interval: Duration::from_secs(60),
            sweep_on_startup: false,
            ..Default::default()
        },
        Arc::new(crate::decay::DecayConfig::default()),
        Default::default(),
        f.storage.clone(),
        f.graph.clone(),
        f.cache.clone(),
        1.,
    )
    .unwrap()
    .with_clock(c.clone())
    .with_completion(completion(&f));
    let held = f.graph.clone().write_owned().await;
    // Existing tokio-test enables test-util. Only the runtime's scheduling clock
    // advances; retention still uses the independently injected CountingClock.
    tokio::time::pause();
    let task = sweep.start();
    tokio::task::yield_now().await;
    assert_eq!(
        tracker.status().pending,
        0,
        "idle scheduler is not accepted work"
    );
    assert_eq!(c.calls.load(Ordering::SeqCst), calls);
    tokio::time::advance(Duration::from_secs(60)).await;
    tokio::time::resume();
    wait(&tracker, |s| s.pending > 0).await;
    drop(held);
    let report = completion(&f)
        .readiness_barrier(
            before,
            tokio::time::Instant::now().into_std() + Duration::from_secs(10),
        )
        .await
        .unwrap();
    assert_eq!(report.committed_generation, target(&f).generation);
    assert!(report.committed_generation > before.generation);
    assert!(
        report
            .derived
            .iter()
            .all(|v| v.ready && v.applied_generation == report.committed_generation)
    );
    assert_eq!(row(&f, id).phase, DecayPhase::Summary);
    assert_eq!(c.calls.load(Ordering::SeqCst), calls + 1);
    sweep.shutdown();
    task.await.unwrap();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
}

#[path = "sweep_origin_tests.rs"]
mod origin;
