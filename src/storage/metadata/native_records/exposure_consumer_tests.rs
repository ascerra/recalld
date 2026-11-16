use super::*;
use crate::decay::observation::ObservationError;
use crate::search::{SearchError, SearchQuery};
use crate::storage::control::ExposureScope;

fn setup() -> (Fixture, Data, Arc<FixedClock>, MemoryId) {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let intent = session.capture_insert(scope(&d), &input(&d)).unwrap();
    let receipt = session
        .insert_prepared(&ctx(&session, &d, "create"), &intent, &generated())
        .unwrap();
    let id = inserted(&receipt);
    (
        Fixture::from_open_engine(root, session.engine),
        d,
        clock,
        id,
    )
}
fn observation(error: SearchError) -> ObservationError {
    match error {
        SearchError::Observation(error) => error,
        other => panic!("lost observation outcome: {other:?}"),
    }
}
fn scope_for(d: &Data) -> ExposureScope {
    ExposureScope {
        namespace: "native".into(),
        source: Some(d.parent),
    }
}
fn event_count(f: &Fixture, id: MemoryId) -> usize {
    f.record(id).unwrap().access_history.len()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_query_deduplicates_returned_ids_preserves_anchor_and_reopens() {
    let (f, d, clock, id) = setup();
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let (api, bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
    let before = f.record(id).unwrap();
    let source = f.record(d.parent).unwrap().encode().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    let receipts = engine
        .record_exposures_at_with_work(
            &[
                (id, crate::model::AccessKind::AssociativeRetrieval),
                (id, crate::model::AccessKind::DirectRetrieval),
                (id, crate::model::AccessKind::AssociativeRetrieval),
            ],
            scope_for(&d),
            123_456,
            None,
        )
        .await
        .unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(
        clock.0.load(Ordering::SeqCst),
        calls,
        "sampled time must not consult caller clock again"
    );
    let after = f.record(id).unwrap();
    let old = &before.schema.as_ref().unwrap().lifecycle;
    let new = &after.schema.as_ref().unwrap().lifecycle;
    assert_eq!(
        new.retrieved_at,
        Some(123_456.max(old.lifecycle_updated_at))
    );
    assert_eq!(new.decay_anchor_at, old.decay_anchor_at);
    assert_eq!(new.reinforced_at, old.reinforced_at);
    assert_eq!(new.observed_at, old.observed_at);
    assert_eq!(new.phase_changed_at, old.phase_changed_at);
    assert_eq!(
        (after.stability, after.phase, after.is_permastore),
        (before.stability, before.phase, before.is_permastore)
    );
    assert_eq!(after.access_history.len(), before.access_history.len() + 1);
    assert_eq!(
        after.access_history.last().unwrap().kind,
        crate::model::AccessKind::DirectRetrieval
    );
    assert_eq!(
        f.record(d.parent).unwrap().encode().unwrap(),
        source,
        "scope source is not an exposed result"
    );
    let query: SearchQuery = serde_json::from_value(
        json!({"namespace":"native","queryMode":"metadata_only","filter":{"minStrength":0.0},"recordAccess":false}),
    )
    .unwrap();
    let stable = bytes(&d.path);
    for _ in 0..2 {
        engine.search(query.clone()).await.unwrap();
    }
    assert_eq!(bytes(&d.path), stable);
    let mut query = query;
    query.record_access = true;
    let counts = [event_count(&f, id), event_count(&f, d.parent)];
    let calls = clock.0.load(Ordering::SeqCst);
    let result = engine.search(query).await.unwrap();
    assert_eq!(result.results.len(), 2);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls + 1);
    assert_eq!(
        [event_count(&f, id), event_count(&f, d.parent)],
        [counts[0] + 1, counts[1] + 1]
    );
    let state = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .publication_state()
        .unwrap();
    assert!(state.matches_canonical(), "{state:?}");
    drop((api, bridge, engine, owner));
    let root = f.close();
    let reopened = open(&d, clock);
    assert_eq!(
        reopened
            .engine
            .get_record(id)
            .unwrap()
            .unwrap()
            .access_history
            .len(),
        counts[0] + 1
    );
    drop((reopened, root));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_http_mcp_get_similar_true_and_false_use_real_observer() {
    let (f, d, clock, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let (api, bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
    let stable = bytes(&d.path);
    for access in [false, true] {
        let count = event_count(&f, id);
        let calls = clock.0.load(Ordering::SeqCst);
        let (status, body) = http(
            &api,
            "GET",
            &format!("/memories/{id}?recordAccess={access}"),
            Value::Null,
        )
        .await;
        assert!(status.is_success(), "{body}");
        assert_eq!(event_count(&f, id), count + usize::from(access));
        if access {
            origin::assert_origin(
                &f,
                d.ns,
                1,
                Some(id),
                &[(id, crate::model::AccessKind::DirectRetrieval)],
            );
        }
        assert_eq!(clock.0.load(Ordering::SeqCst), calls + 1);
        let count = event_count(&f, id);
        let calls = clock.0.load(Ordering::SeqCst);
        mcp(
            &bridge,
            "get_memory",
            json!({"id":id,"recordAccess":access}),
        )
        .await;
        assert_eq!(event_count(&f, id), count + usize::from(access));
        if access {
            origin::assert_origin(
                &f,
                d.ns,
                1,
                Some(id),
                &[(id, crate::model::AccessKind::DirectRetrieval)],
            );
        }
        assert_eq!(clock.0.load(Ordering::SeqCst), calls + 1);
        let source = event_count(&f, id);
        let target = event_count(&f, d.parent);
        let (status, body) = http(
            &api,
            "POST",
            &format!("/similar/{id}"),
            json!({"recordAccess":access}),
        )
        .await;
        assert!(status.is_success(), "{body}");
        assert!(!body["hits"].as_array().unwrap().is_empty());
        assert_eq!(event_count(&f, id), source);
        assert_eq!(event_count(&f, d.parent), target + usize::from(access));
        if access {
            origin::assert_origin(
                &f,
                d.ns,
                1,
                Some(id),
                &[(d.parent, crate::model::AccessKind::AssociativeRetrieval)],
            );
        }
        let target = event_count(&f, d.parent);
        let body = mcp(
            &bridge,
            "find_similar_memories",
            json!({"id":id,"recordAccess":access}),
        )
        .await;
        assert!(!body["memories"].as_array().unwrap().is_empty());
        assert_eq!(event_count(&f, id), source);
        assert_eq!(event_count(&f, d.parent), target + usize::from(access));
        if access {
            origin::assert_origin(
                &f,
                d.ns,
                1,
                Some(id),
                &[(d.parent, crate::model::AccessKind::AssociativeRetrieval)],
            );
        }
        if !access {
            assert_eq!(bytes(&d.path), stable);
        }
    }
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    drop((api, bridge, engine));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_partial_publication_keeps_receipt_and_repairs_without_reobservation() {
    let (f, d, clock, id) = setup();
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let (_api, _bridge, engine) = readers_with_exposure(&f, clock, true);
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    let before = event_count(&f, id);
    let error = observation(
        engine
            .record_exposures_at_with_work(
                &[(id, crate::model::AccessKind::DirectRetrieval)],
                scope_for(&d),
                5000,
                None,
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(error.commit_state, "committed");
    assert_eq!(error.receipts.len(), 1);
    assert!(!error.ready);
    assert!(error.unknown_operation_id.is_none());
    assert_eq!(event_count(&f, id), before + 1);
    assert!(
        !f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    owner.complete().await.unwrap();
    assert_eq!(event_count(&f, id), before + 1);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_unknown_commit_keeps_operation_and_requires_reopen() {
    let (f, d, clock, id) = setup();
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let (api, bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
    let before = event_count(&f, id);
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .observation_fault
        .store(3, Ordering::SeqCst);
    let error = observation(
        engine
            .record_exposures_at_with_work(
                &[(id, crate::model::AccessKind::DirectRetrieval)],
                scope_for(&d),
                5000,
                None,
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(error.commit_state, "unknown");
    assert!(!error.ready);
    assert!(error.receipts.is_empty());
    let operation = error.unknown_operation_id.unwrap();
    assert!(owner.complete().await.is_err());
    drop((api, bridge, engine, owner));
    let root = f.close();
    let reopened = open(&d, clock);
    assert_eq!(
        reopened
            .engine
            .get_record(id)
            .unwrap()
            .unwrap()
            .access_history
            .len(),
        before + 1
    );
    assert!(
        reopened
            .metadata()
            .lookup_operation(
                reopened.metadata().control_state().unwrap().store_id,
                MutationScope::Namespace(d.ns),
                operation
            )
            .unwrap()
            .is_some()
    );
    drop((reopened, root));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_deferred_selection_never_writes_and_unsupported_recorder_stays_closed() {
    let (f, d, clock, id) = setup();
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let (api, bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
    let before = bytes(&d.path);
    let prepared = engine
        .prepare_coherent_get(id, Some("native"), true, None)
        .await
        .unwrap()
        .unwrap();
    assert!(prepared.requested_access);
    assert_eq!(prepared.evaluated_at, 86_400_000);
    assert_eq!(
        prepared.read_generation,
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref()
    );
    assert!(prepared.payload.is_some());
    assert_eq!(
        bytes(&d.path),
        before,
        "deferred capability must not enable pinned writes"
    );
    let (_unsupported_api, _unsupported_bridge, unsupported) = readers(&f, clock.clone());
    assert!(unsupported.get_memory_with_access(id, true).await.is_err());
    assert_eq!(bytes(&d.path), before);
    let receipts = engine
        .record_exposures_at_with_work(
            &[(id, crate::model::AccessKind::DirectRetrieval)],
            scope_for(&d),
            prepared.evaluated_at,
            None,
        )
        .await
        .unwrap();
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].committed_generation > prepared.read_generation.generation);
    drop((api, bridge));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_per_namespace_failure_publishes_preceding_receipts_without_terminal_access()
 {
    let root = tempfile::tempdir().unwrap();
    let mut d = fixture(root.path());
    // Actual legacy creation and offline migration, never forged native rows.
    let source = root.path().join("legacy");
    let mut legacy = RedbStorageEngine::open(&source).unwrap();
    let mut ns = NamespaceConfig::default_namespace(0);
    ns.name = "second".into();
    ns.embedding_dim = 4;
    let second = legacy.create_namespace(&ns).unwrap();
    let mut input = input(&d).memory;
    input.parent_id = None;
    let inserted_receipt = legacy
        .insert_memory_with_receipt(&MutationContext::unkeyed(second), &input, &[], 3000)
        .unwrap();
    let dead = crate::storage::insertion::inserted_id(&inserted_receipt).unwrap();
    legacy.sync().unwrap();
    drop(legacy);
    let canonical = root.path().join("two-schema");
    migrate_schema(
        &source,
        &canonical,
        &MigrationOptions {
            migration_now: 4000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![
                ProfileAttestationInput {
                    namespace_id: d.ns,
                    namespace_name: "native".into(),
                    descriptor: d.profile.clone(),
                },
                ProfileAttestationInput {
                    namespace_id: second,
                    namespace_name: "second".into(),
                    descriptor: d.profile.clone(),
                },
            ],
        },
    )
    .unwrap();
    let mut current = canonical;
    for (i, ns) in [d.ns, second].into_iter().enumerate() {
        let dest = root.path().join(format!("two-v2-{i}"));
        migrate_lifecycle(
            &current,
            &dest,
            &LifecycleMigrationOptions {
                expected_source: inspect_schema(&current).unwrap().control.generation_ref(),
                namespace_id: ns,
                expected_policy_revision: Generation::default(),
                target: LifecycleVersion::ExplicitFeedbackV2,
                idempotency_key: format!("switch-{i}"),
            },
        )
        .unwrap();
        current = dest;
    }
    d.path = current;
    // Remigration created a fresh unknown binding. Adopt only the namespace
    // intended to commit first; the terminal namespace must stay unknown.
    let before = inspect_schema(&d.path).unwrap();
    assert!(
        before
            .namespaces
            .iter()
            .all(|n| n.schema.as_ref().unwrap().inherited_decay.is_none())
    );
    crate::test_support::explicitly_adopt_inherited_policy(&d.path, d.ns, 1., 4000);
    assert!(
        inspect_schema(&d.path)
            .unwrap()
            .namespaces
            .into_iter()
            .find(|n| n.id == second)
            .unwrap()
            .schema
            .unwrap()
            .inherited_decay
            .is_none()
    );
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let policy = NativePolicyInputs::default();
    let intent = session
        .metadata()
        .capture_native_tombstone(dead, Some(second), &policy)
        .unwrap();
    let mut context = ctx(&session, &d, "forget-second");
    context.scope = MutationScope::Namespace(second);
    session
        .engine
        .tombstone_native_record(&context, &intent, &policy, clock.as_ref(), None)
        .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let tombstone = f.record(dead).unwrap().encode().unwrap();
    let initial = event_count(&f, d.parent);
    let error = owner
        .record_native_exposures_with_work(
            vec![
                (dead, crate::model::AccessKind::AssociativeRetrieval),
                (d.parent, crate::model::AccessKind::DirectRetrieval),
                (d.parent, crate::model::AccessKind::AssociativeRetrieval),
            ],
            None,
            86_400_001,
            policy,
            None,
            2,
        )
        .await
        .unwrap_err();
    assert_eq!(error.commit_state, "committed");
    assert_eq!(error.receipts.len(), 1);
    assert!(error.unknown_operation_id.is_none());
    assert_eq!(event_count(&f, d.parent), initial + 1);
    assert_eq!(f.record(dead).unwrap().encode().unwrap(), tombstone);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    let target = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        owner
            .readiness_barrier(
                target,
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
            .await
            .unwrap()
            .ready
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_accepted_root_completes_closed_admission_and_cancelled_worker_keeps_leases()
 {
    use std::time::{Duration, Instant};
    for cancel in [false, true] {
        let (f, d, clock, id) = setup();
        let owner = completion(&f);
        owner.rebuild_all().await.unwrap();
        let (_api, _bridge, engine) = readers_with_exposure(&f, clock, true);
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let root = tracker.admit(None).unwrap();
        if !cancel {
            let t = tracker.clone();
            let drain = tokio::task::spawn_blocking(move || {
                t.close_and_drain(Instant::now() + Duration::from_secs(5))
            });
            let t = tracker.clone();
            tokio::task::spawn_blocking(move || t.wait_until_closed())
                .await
                .unwrap();
            let receipts = engine
                .record_exposures_at_with_work(
                    &[(id, crate::model::AccessKind::DirectRetrieval)],
                    scope_for(&d),
                    86_400_001,
                    Some(&root),
                )
                .await
                .unwrap();
            root.finish();
            let freeze = drain.await.unwrap().unwrap();
            let state = f
                .storage
                .read()
                .unwrap()
                .meta_store()
                .publication_state()
                .unwrap();
            assert!(state.matches_canonical());
            assert_eq!(state.canonical.generation, receipts[0].committed_generation);
            assert_eq!(tracker.status().pending, 0);
            drop(freeze);
        } else {
            let gate = f
                .storage
                .read()
                .unwrap()
                .meta_store()
                .derived_publication
                .clone();
            let graph = f.graph.clone().write_owned().await;
            let scope = scope_for(&d);
            let task = tokio::spawn(async move {
                let result = engine
                    .record_exposures_at_with_work(
                        &[(id, crate::model::AccessKind::DirectRetrieval)],
                        scope,
                        86_400_001,
                        Some(&root),
                    )
                    .await;
                root.finish();
                result
            });
            tokio::time::timeout(Duration::from_secs(5), async {
                while gate.try_write().is_ok() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let storage = f.storage.clone();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _lock = storage.write().unwrap();
                ready_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            });
            ready_rx.await.unwrap();
            drop(graph);
            tokio::time::timeout(Duration::from_secs(5), async {
                while tracker.status().pending < 4 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            assert!(gate.try_write().is_err());
            assert!(f.graph.try_write().is_err());
            assert!(tracker.status().unresolved > 0);
            release_tx.send(()).unwrap();
            blocker.await.unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while gate.try_write().is_err() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let target = f
                .storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .generation_ref();
            assert!(
                owner
                    .readiness_barrier(target, Instant::now() + Duration::from_millis(25))
                    .await
                    .is_err()
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_invalid_configured_policy_fails_without_writes_and_empty_has_no_work() {
    let (f, d, clock, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let (api, bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
    drop((api, bridge));
    let engine = Arc::try_unwrap(engine).ok().unwrap().with_lifecycle_inputs(
        clock.clone(),
        crate::decay::DecayConfig::default(),
        f64::NAN,
    );
    let before = bytes(&d.path);
    let calls = clock.0.load(Ordering::SeqCst);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    assert!(
        engine
            .record_exposures_at_with_work(&[], scope_for(&d), 123, None)
            .await
            .unwrap()
            .is_empty()
    );
    let error = observation(
        engine
            .record_exposures_at_with_work(
                &[(id, crate::model::AccessKind::DirectRetrieval)],
                scope_for(&d),
                123,
                None,
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(error.commit_state, "notCommitted");
    assert!(error.receipts.is_empty());
    assert_eq!(bytes(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
}

#[path = "exposure_origin_tests.rs"]
mod origin;
