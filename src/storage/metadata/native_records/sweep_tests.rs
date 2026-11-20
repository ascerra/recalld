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

fn setup_memories(count: usize) -> (Fixture, Vec<MemoryId>) {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let c = Arc::new(CountingClock::new(3000));
    let mut session =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c).unwrap();
    let mut ids = Vec::new();
    for n in 0..count {
        let mut memory = input(&d);
        memory.memory.initial_stability = Some(1.);
        let intent = session.capture_insert(scope(&d), &memory).unwrap();
        let key = ctx(&session, &d, &format!("pass-batch-{n}"));
        ids.push(inserted(
            &session
                .insert_prepared(&key, &intent, &generated())
                .unwrap(),
        ));
    }
    (Fixture::from_open_engine(root, session.engine), ids)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_timer_pass_publishes_per_batch_and_checks_each_write_against_its_own_record() {
    let (f, ids) = setup_memories(5);
    completion(&f).rebuild_all().await.unwrap();
    ready(&f).await;
    let scans = || {
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .recovery_scans
            .load(Ordering::SeqCst)
    };
    let edge_loads = || {
        f.storage
            .read()
            .unwrap()
            .edge_store()
            .full_loads
            .load(Ordering::SeqCst)
    };
    let before = scans();
    let loads_before = edge_loads();
    // One hour on, every strength moves and no phase changes, so the pass
    // commits every memory: the five here plus the fixture's parent.
    let result = DecaySweepRunner::execute_sweep_with_work(
        &SweepConfig {
            write_batch_size: 2,
            ..Default::default()
        },
        &crate::decay::DecayConfig::default(),
        &Default::default(),
        &f.storage,
        &f.graph,
        &f.cache,
        1.,
        Some(3000 + DAY / 24),
        None,
        Some(&completion(&f)),
    )
    .await;
    let scanned = scans() - before;
    assert!(
        result.errors.is_empty() && result.publication_error.is_none(),
        "{result:?}"
    );
    assert_eq!(result.memories_scanned, ids.len() as u64 + 1);
    assert_eq!(result.unchanged_skipped, 0);
    assert_eq!(result.full_to_summary, 0);
    // A publication reads every record once. Six commits in batches of two
    // publish after the first (on its own), the third, the fifth and the
    // last. Applying each write reads only that write's record.
    assert_eq!(scanned, 4);
    // Every edge is read once per publication too, and never by a commit:
    // each reads only its memory's neighbourhood (#215).
    assert_eq!(edge_loads() - loads_before, 4);
    ready(&f).await;
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    for id in &ids {
        let record = row(&f, *id);
        assert_eq!(record.phase, DecayPhase::Full);
        assert!(record.strength < 1., "strength was not materialized");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_phase_effect_checked_against_its_own_record_matches_a_full_scan() {
    use crate::storage::metadata::effects::{validate_plan, validation_record_ids};
    let (f, ids) = setup_memories(2);
    completion(&f).rebuild_all().await.unwrap();
    let id = ids[0];
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let work = tracker.begin().unwrap();
    let policy = NativePolicyInputs::default();
    let capture = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .capture_native_decay(id, scope_id(&f, id), &policy, 3000 + DAY / 24)
        .unwrap();
    let context = MutationContext {
        store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
        scope: MutationScope::Namespace(scope_id(&f, id)),
        idempotency_key: None,
        expected_generation: None,
        contract_version: 2,
    };
    let committed = completion(&f)
        .apply_native_decay_in_pass(context, capture, policy, &work)
        .await
        .unwrap();
    assert!(matches!(
        committed,
        crate::storage::native_maintenance::NativeDecayResult::Committed(_)
    ));
    let (effect, all, own) = {
        let storage = f.storage.read().unwrap();
        let effect = storage
            .meta_store()
            .pending_effects()
            .unwrap()
            .pop()
            .unwrap();
        let ids = validation_record_ids(&effect).unwrap();
        assert_eq!(ids, vec![id]);
        let own = storage.meta_store().records_for_recovery(&ids).unwrap();
        (
            effect,
            storage.meta_store().scan_all_for_recovery().unwrap(),
            own,
        )
    };
    validate_plan(&effect, &all).unwrap();
    validate_plan(&effect, &own).unwrap();
    let mut wrong = own.clone();
    wrong[0].1.strength = 1.;
    assert!(validate_plan(&effect, &wrong).is_err());
    assert!(validate_plan(&effect, &[]).is_err());
    let mut released = effect.clone();
    released
        .releases
        .push(crate::storage::metadata::effects::VectorRelease {
            namespace_id: scope_id(&f, id),
            slot: own[0].1.vector_slot,
            memory_id: id,
        });
    assert!(
        validation_record_ids(&released).is_none(),
        "a release is checked against every record"
    );
    completion(&f).complete().await.unwrap();
    f.storage.read().unwrap().meta_store().finish_work(work);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_timer_pass_publication_failure_after_the_first_batch_names_its_first_waiting_commit()
 {
    // Seven commits in batches of two publish after the 1st, the 3rd, the 5th
    // and, at the final flush, the 7th. Fail the 2nd publication (commits 2
    // and 3 waiting) or the 4th (commits 6 and 7 waiting).
    for (nth, first_waiting, scanned) in [(2, 2, 3), (4, 6, 7)] {
        let (f, _) = setup_memories(6);
        completion(&f).rebuild_all().await.unwrap();
        ready(&f).await;
        let before = target(&f).generation;
        let generation = |n| (0..n).fold(before, |g, _| g.next().unwrap());
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .derived_publication_failure_nth
            .store(nth, Ordering::SeqCst);
        let result = DecaySweepRunner::execute_sweep_with_work(
            &SweepConfig {
                write_batch_size: 2,
                ..Default::default()
            },
            &crate::decay::DecayConfig::default(),
            &Default::default(),
            &f.storage,
            &f.graph,
            &f.cache,
            1.,
            Some(10_000 * DAY),
            None,
            Some(&completion(&f)),
        )
        .await;
        assert_eq!(result.memories_scanned, scanned, "{result:?}");
        assert_eq!(result.full_to_summary, scanned, "{result:?}");
        assert_eq!(
            target(&f).generation,
            generation(scanned),
            "no write after the failure"
        );
        let (receipt, _) = result.failed_commit.as_ref().expect("first waiting commit");
        assert_eq!(receipt.committed_generation, generation(first_waiting));
        let MutationOutcome::NativePhaseMaterialized { transition } = &receipt.outcome else {
            panic!("not a native phase commit: {receipt:?}");
        };
        assert_eq!(result.errors.len(), 1, "{result:?}");
        assert_eq!(result.errors[0].memory_id, transition.memory_id);
        assert!(
            result.errors[0]
                .error
                .contains("injected derived publication failure"),
            "{result:?}"
        );
        assert!(result.publication_error.is_some());
        assert!(!result.compaction_triggered);
        assert!(result.compaction_receipt.is_none());
        // The repair publishes every commit of the pass.
        completion(&f).complete().await.unwrap();
        ready(&f).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_deletion_recounts_a_peer_that_keeps_other_edges() {
    // The fixture's parent links to all three; deleting one child leaves it
    // two outgoing edges, which the commit reads per node, not from all edges.
    let (f, ids) = setup_memories(3);
    completion(&f).rebuild_all().await.unwrap();
    let id = ids[0];
    let parent = f
        .storage
        .read()
        .unwrap()
        .load_all_edges()
        .unwrap()
        .into_iter()
        .find(|e| e.target == id)
        .unwrap()
        .source;
    let ns = scope_id(&f, id);
    let policy = NativePolicyInputs::default();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    for phase in [Some(DecayPhase::Summary), Some(DecayPhase::Ghost), None] {
        let work = tracker.begin().unwrap();
        let capture = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .capture_native_decay(id, ns, &policy, 10_000 * DAY)
            .unwrap();
        let context = MutationContext {
            store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
            scope: MutationScope::Namespace(ns),
            idempotency_key: None,
            expected_generation: None,
            contract_version: 2,
        };
        assert!(matches!(
            completion(&f)
                .apply_native_decay_in_pass(context, capture, policy.clone(), &work)
                .await
                .unwrap(),
            crate::storage::native_maintenance::NativeDecayResult::Committed(_)
        ));
        completion(&f).complete().await.unwrap();
        f.storage.read().unwrap().meta_store().finish_work(work);
        let current = f.storage.read().unwrap().get_record(id).unwrap();
        assert_eq!(current.map(|r| r.phase), phase);
    }
    ready(&f).await;
    let edges = f.storage.read().unwrap().load_all_edges().unwrap();
    assert!(edges.iter().all(|e| e.source != id && e.target != id));
    for other in [parent, ids[1], ids[2]] {
        let outgoing = edges.iter().filter(|e| e.source == other).count();
        assert_eq!(usize::from(row(&f, other).edge_count), outgoing);
    }
    assert_eq!(row(&f, parent).edge_count, 2);
}

/// The bonus as a native commit computed it before #215, from every row of the
/// namespace and every edge: the oracle for the neighbourhood read.
fn full_graph_bonus(
    tx: &redb::WriteTransaction,
    edges: &[crate::storage::PersistedEdge],
    ns: &NamespaceConfig,
    policy: &crate::decay::lifecycle::EffectiveLifecyclePolicy,
    id: MemoryId,
    now: i64,
) -> f32 {
    use redb::ReadableTable;
    let mut graph = crate::graph::RelationshipGraph::new();
    let mut donors = HashMap::new();
    let mut ids = std::collections::HashSet::new();
    for row in tx.open_table(META_TABLE).unwrap().iter().unwrap() {
        let (_, bytes) = row.unwrap();
        let r = DiskRecord::from_bytes(bytes.value()).unwrap();
        if r.namespace_id != ns.id.get() {
            continue;
        }
        let key = MemoryId::from_bytes(r.id);
        ids.insert(key);
        graph
            .add_node(key, ns.id, r.phase, r.strength, r.vector_slot)
            .unwrap();
        let state = crate::storage::native_lifecycle::LifecycleState::capture(&r).unwrap();
        if state.eligible() {
            let p = crate::decay::lifecycle::project(
                r.stability,
                state.lifecycle.decay_anchor_at,
                policy,
                now.max(state.lifecycle.lifecycle_updated_at),
                0.,
            )
            .unwrap();
            donors.insert(key, p.strength);
        }
    }
    for e in edges {
        if ids.contains(&e.source) && ids.contains(&e.target) {
            graph
                .add_edge(e.source, e.target, e.edge_type, e.weight, e.auto_created)
                .unwrap();
        }
    }
    crate::graph::activation::connection_bonus_with_strengths(
        id,
        &graph,
        &Default::default(),
        &donors,
    )
}

#[test]
fn neighbourhood_bonus_equals_the_full_namespace_graph_bonus() {
    use crate::model::EdgeType::*;
    let root = tempfile::tempdir().unwrap();
    let mut foreign = MemoryId::nil();
    let d = certified_fixture_with_seed(root.path(), |engine| {
        let mut ns = NamespaceConfig::default_namespace(0);
        ns.name = "foreign".into();
        ns.embedding_dim = 4;
        let ns = engine.create_namespace(&ns).unwrap();
        let input = crate::storage::insertion::InsertMemory {
            id: None,
            summary: "foreign".into(),
            full_text: None,
            tags: vec![],
            initial_stability: None,
            created_at: Some(1000),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        foreign = crate::storage::insertion::inserted_id(
            &engine
                .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &input, &[], 1000)
                .unwrap(),
        )
        .unwrap();
    });
    let c = Arc::new(CountingClock::new(3000));
    let mut session =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), c).unwrap();
    let mut m = Vec::new();
    for n in 0..16 {
        let mut memory = input(&d);
        memory.memory.initial_stability = Some(1. + n as f32 * 0.37);
        let intent = session.capture_insert(scope(&d), &memory).unwrap();
        let key = ctx(&session, &d, &format!("bonus-{n}"));
        m.push(inserted(
            &session
                .insert_prepared(&key, &intent, &generated())
                .unwrap(),
        ));
    }
    // Every memory is already the child of the fixture's parent, whose fan is
    // too high to contribute. On top: chains for two-hop paths, shared
    // neighbours, two edge types between one pair, zero-factor edges, a self
    // edge, a hub at the fan threshold, an edge into another namespace and
    // one to a missing row.
    let missing = MemoryId::from_bytes([0xab; 16]);
    let edge =
        |source: MemoryId, target: MemoryId, edge_type, weight| crate::storage::PersistedEdge {
            source,
            target,
            edge_type,
            weight,
            auto_created: false,
            created_at: 3000,
        };
    let mut synthetic = vec![
        edge(m[0], m[1], Associative, 0.9),
        edge(m[0], m[1], Entity, 0.2),
        edge(m[1], m[2], Entity, 0.8),
        edge(m[2], m[3], Causal, 0.7),
        edge(m[3], m[4], Temporal, 0.6),
        edge(m[0], m[5], Associative, 0.5),
        edge(m[5], m[2], Associative, 0.4),
        edge(m[1], m[5], ParentChild, 0.3),
        edge(m[1], m[7], Temporal, 0.75),
        edge(m[6], m[0], Contradicts, 1.),
        edge(m[7], m[6], Supersedes, 1.),
        edge(m[3], foreign, Associative, 1.),
        edge(foreign, m[4], Entity, 1.),
        edge(m[4], missing, Associative, 1.),
        edge(m[8], m[8], Associative, 0.5),
        edge(m[8], m[0], Associative, 0.6),
        edge(m[0], m[9], Associative, 0.95),
        edge(m[10], m[11], Entity, 0.85),
    ];
    for hub_child in &m[10..16] {
        synthetic.push(edge(m[9], *hub_child, Associative, 0.9));
    }
    session
        .engine
        .edge_store()
        .batch_add_edges(&synthetic)
        .unwrap();
    let edges = session.engine.load_all_edges().unwrap();
    let snapshot = session.engine.edge_store().read_transaction().unwrap();
    let tx = session.metadata().db.begin_write().unwrap();
    // Mixed phases and eligibility, inside a transaction that is never
    // committed: two ghosts, a summary without one (no donor), a summary.
    {
        let mut rows = tx.open_table(META_TABLE).unwrap();
        for (index, phase, keep_summary) in [
            (2, DecayPhase::Ghost, false),
            (5, DecayPhase::Summary, false),
            (4, DecayPhase::Summary, true),
            (11, DecayPhase::Ghost, false),
        ] {
            let mut r = DiskRecord::from_bytes(
                rows.get(m[index].as_bytes().as_slice())
                    .unwrap()
                    .unwrap()
                    .value(),
            )
            .unwrap();
            r.phase = phase;
            if !keep_summary {
                r.summary.clear();
            }
            if phase == DecayPhase::Ghost {
                // As the native ghost patch removes them.
                r.text_offset = 0;
                r.text_length = 0;
                r.summary.clear();
                r.tags.clear();
                let labels = &mut r.schema.as_mut().unwrap().labels;
                labels.entities.clear();
                labels.topics.clear();
                labels.emotions.clear();
            }
            rows.insert(
                m[index].as_bytes().as_slice(),
                r.encode().unwrap().as_slice(),
            )
            .unwrap();
        }
    }
    let ns = NamespaceConfig::decode(
        tx.open_table(NAMESPACE_TABLE)
            .unwrap()
            .get(d.ns.get())
            .unwrap()
            .unwrap()
            .value(),
    )
    .unwrap();
    let policy =
        crate::decay::lifecycle::resolve_policy(&ns, &crate::decay::DecayConfig::default(), 1.)
            .unwrap();
    let mut nonzero = 0;
    for now in [3000 + DAY / 2, 3000 + 3 * DAY] {
        for id in m.iter().copied().chain([d.parent, foreign, missing]) {
            let expected = full_graph_bonus(&tx, &edges, &ns, &policy, id, now);
            let actual = crate::storage::metadata::native_maintenance::canonical_bonus(
                &tx, &snapshot, &ns, &policy, id, now,
            )
            .unwrap();
            assert_eq!(actual.to_bits(), expected.to_bits(), "{id} at {now}");
            nonzero += usize::from(expected > 0.);
        }
    }
    assert!(nonzero >= 12, "fixture too sparse: {nonzero}");
}

fn scope_id(f: &Fixture, id: MemoryId) -> crate::model::NamespaceId {
    crate::model::NamespaceId::new(row(f, id).namespace_id)
}

#[path = "sweep_origin_tests.rs"]
mod origin;

// Issue #239: every swept memory adds a retained receipt. Reopen still runs
// every full validation, over its disposable copies and the real file, but
// decodes each receipt once. Native reads rely on that owned validation.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_reopen_decodes_each_receipt_once_and_reads_skip_history() {
    use crate::serialization::catalog_page::{CatalogKind, CatalogPageRequest};
    use crate::serialization::list::ListPageRequest;
    use crate::storage::control;
    let (f, _) = setup_memories(4);
    for hour in 1..=2 {
        completion(&f).rebuild_all().await.unwrap();
        let r = pass(&f, 3000 + hour * DAY / 24).await;
        assert!(
            r.errors.is_empty() && r.publication_error.is_none(),
            "{r:?}"
        );
    }
    let path = f.storage.read().unwrap().db_path().to_path_buf();
    let root = f.close();
    control::take_validation_work();
    let engine = RedbStorageEngine::open_native(&path).unwrap();
    let work = control::take_validation_work();
    let receipts = {
        let read = engine.meta_store().db.begin_read().unwrap();
        read.open_table(control::RECEIPTS).unwrap().len().unwrap() as usize
    };
    assert!(receipts > 12, "{receipts}");
    // Once on the disposable preflight copy, once on the opened real file.
    assert_eq!(work.full_validations, 2, "{work:?}");
    assert_eq!(work.receipt_decodes, receipts, "{work:?}");
    drop(engine);

    // Daemon startup: advisory classification, then the owned upgrade-or-open.
    use crate::storage::automatic_upgrade::{StoreFormat, classify, prepare_upgrade};
    assert_eq!(classify(&path).unwrap(), StoreFormat::Current);
    let (engine, _upgrade) =
        prepare_upgrade(&path, 3000, Box::new(crate::storage::ids::SystemIds)).unwrap();
    let work = control::take_validation_work();
    // Once on the disposable preflight copy, once on the opened real file.
    assert_eq!(work.full_validations, 2, "{work:?}");
    assert_eq!(work.receipt_decodes, receipts, "{work:?}");

    let f = Fixture::from_open_engine(root, engine);
    let engine = f.storage.read().unwrap();
    let meta = engine.meta_store();
    let list = ListPageRequest::from_mcp(&serde_json::json!({"contractVersion": 2})).unwrap();
    let catalog = CatalogPageRequest::from_mcp(
        &serde_json::json!({"contractVersion": 2}),
        CatalogKind::Namespaces,
    )
    .unwrap();
    let ticket = meta.work_tracker().admit(None).unwrap();
    control::take_validation_work();
    let page = meta
        .canonical_list_page("native", &list, None, &ticket)
        .unwrap();
    assert!(!page.data.memories.is_empty());
    meta.canonical_catalog_page(&catalog, None, &ticket)
        .unwrap();
    assert_eq!(
        control::take_validation_work(),
        control::ValidationWork::default()
    );
    meta.finish_work(ticket);
}
