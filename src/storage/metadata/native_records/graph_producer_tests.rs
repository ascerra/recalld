use super::*;
use crate::storage::native_graph::{NativeEdgeInput, NativeGraphInput};

fn setup() -> (Fixture, Data, Arc<FixedClock>, MemoryId) {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let mut original = input(&d);
    original.memory.parent_id = None;
    let intent = s.capture_insert(scope(&d), &original).unwrap();
    let r = s
        .insert_prepared(&ctx(&s, &d, "graph-source"), &intent, &generated())
        .unwrap();
    let id = inserted(&r);
    (Fixture::from_open_engine(root, s.engine), d, clock, id)
}
fn plan(id: MemoryId, target: MemoryId) -> NativeGraphInput {
    NativeGraphInput {
        edges: vec![NativeEdgeInput {
            source: id,
            target,
            edge_type: crate::model::EdgeType::Temporal,
            weight: 0.5,
            auto_created: true,
        }],
    }
}
fn adapter(f: &Fixture) -> RelationshipGraphAdapter {
    RelationshipGraphAdapter::new(
        f.graph.clone(),
        f.vector_index.clone(),
        f.entity_index.clone(),
        f.storage.clone(),
        f.cache.clone(),
        Arc::new(Default::default()),
    )
}
#[tokio::test]
async fn native_graph_producer_legacy_temporal_rejects_native_owner() {
    let (f, d, _, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let count = crate::graph::perform_temporal_link(
        id,
        d.ns,
        3000,
        10000,
        2,
        &[(d.parent, 1000)],
        &f.graph,
        &f.storage,
        &f.cache,
    )
    .await
    .unwrap_err();
    assert!(matches!(count, crate::graph::AutoLinkError::Storage(_)));
}
#[tokio::test]
async fn native_graph_producer_legacy_adapter_rejects_native_owner() {
    let (f, d, _, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let owner: &dyn crate::api::RelationshipGraph = &adapter(&f);
    let outcome = owner.apply_supersedes_edge(id, d.parent).await;
    assert_eq!(outcome.status, crate::model::SupersedesStatus::Failed);
}
#[test]
fn native_graph_producer_original_api1_core_guard() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock);
    let mut original = input(&d);
    original.memory.parent_id = None;
    let intent = s.capture_insert(scope(&d), &original).unwrap();
    let id = inserted(
        &s.insert_prepared(&ctx(&s, &d, "graph-source"), &intent, &generated())
            .unwrap(),
    );
    let mut k = ctx(&s, &d, "api1-graph");
    k.contract_version = 1;
    s.put_graph(&k, scope(&d), &plan(id, d.parent))
        .unwrap()
        .unwrap();
}

fn owned(f: &Fixture, clock: Arc<dyn Clock>) -> RelationshipGraphAdapter {
    adapter(f).with_native_completion(f.fts_index.clone(), clock, NativePolicyInputs::default())
}
fn key(f: &Fixture, d: &Data, api: u32, name: Option<&str>) -> MutationContext {
    let state = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap();
    MutationContext {
        store_id: Some(state.store_id),
        scope: MutationScope::Namespace(d.ns),
        idempotency_key: name.map(str::to_owned),
        expected_generation: Some(state.committed_generation),
        contract_version: api,
    }
}
fn bytes(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(p: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(p).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                visit(&p, out)
            } else {
                out.insert(p.clone(), std::fs::read(p).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    visit(root, &mut out);
    out
}
async fn until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !condition() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn native_graph_producer_actual_trait_api_replay_nochange_and_offline_history() {
    use crate::api::RelationshipGraph as _;
    for api in [1, 2] {
        let (f, d, clock, id) = setup();
        completion(&f).rebuild_all().await.unwrap();
        let adapter = owned(&f, clock.clone());
        let consumer: &dyn crate::api::RelationshipGraph = &adapter;
        let k = key(&f, &d, api, Some("explicit"));
        let input = plan(id, d.parent);
        let receipt = consumer
            .put_native_edges(k.clone(), input.clone(), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            receipt.committed_generation,
            k.expected_generation.unwrap().next().unwrap()
        );
        assert!(matches!(
            receipt.outcome,
            MutationOutcome::NativeGraphChanged { .. }
        ));
        assert!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
        let calls = clock.0.load(Ordering::SeqCst);
        let before = bytes(&d.path);
        let mut replay = receipt.clone();
        replay.replayed = true;
        assert_eq!(
            consumer
                .put_native_edges(k.clone(), input.clone(), None)
                .await
                .unwrap(),
            Some(replay)
        );
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert_eq!(bytes(&d.path), before);
        let mut changed = k.clone();
        changed.contract_version = 3 - api;
        assert!(matches!(
            consumer
                .put_native_edges(changed, input.clone(), None)
                .await,
            Err(StorageError::IdempotencyConflict)
        ));
        let noop = consumer
            .put_native_edges(key(&f, &d, api, Some("unchanged")), input.clone(), None)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            noop.outcome,
            MutationOutcome::NativeGraphUnchanged { .. }
        ));
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        // Actual owner completion repairs stale derived views even for unkeyed None.
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .invalidate_derived_view(crate::storage::publication::DerivedView::Graph);
        assert!(
            consumer
                .put_native_edges(key(&f, &d, api, None), input.clone(), None)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
        drop(adapter);
        let root = f.close();
        let snapshot = crate::storage::offline_snapshot::OfflineSnapshot::open(&d.path).unwrap();
        assert_eq!(snapshot.record_count().unwrap(), 2);
        drop(snapshot);
        let export = root.path().join("graph.jsonl");
        crate::storage::logical_export::export_store(&d.path, &export).unwrap();
        let imported = root.path().join("imported");
        crate::storage::logical_export::import_store(&export, &imported).unwrap();
        let mut session =
            NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), clock.clone())
                .unwrap();
        let mut expected = receipt.clone();
        expected.replayed = true;
        assert_eq!(
            session.put_graph(&k, scope(&d), &input).unwrap(),
            Some(expected)
        );
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    }
}
#[tokio::test]
async fn native_graph_producer_automatic_families_use_canonical_payloads_and_reject_keys() {
    use crate::api::RelationshipGraph as _;
    use crate::graph::native_producer::NativeLinkSelection as S;
    for api in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock.clone());
        let mut original = input(&d);
        original.memory.parent_id = None;
        let capture = session.capture_insert(scope(&d), &original).unwrap();
        let id = inserted(
            &session
                .insert_prepared(&ctx(&session, &d, "source"), &capture, &generated())
                .unwrap(),
        );
        original.memory.created_at = Some(4000);
        let capture = session.capture_insert(scope(&d), &original).unwrap();
        let peer = inserted(
            &session
                .insert_prepared(&ctx(&session, &d, "peer"), &capture, &generated())
                .unwrap(),
        );
        let f = Fixture::from_open_engine(root, session.engine);
        completion(&f).rebuild_all().await.unwrap();
        let adapter = owned(&f, clock.clone());
        f.cache.batch_invalidate(&[id, peer, d.parent]).await;
        for selection in [
            S::Semantic {
                source: id,
                threshold: 0.9,
                max_links: 1,
            },
            S::Entity {
                source: id,
                max_links: 1,
            },
            S::Temporal {
                source: id,
                window_ms: 10000,
                max_links: 1,
            },
        ] {
            let k = key(&f, &d, api, Some("unsupported-auto-key"));
            let before = bytes(&d.path);
            let calls = clock.0.load(Ordering::SeqCst);
            assert!(matches!(
                adapter
                    .select_native_links(k, selection.clone(), None)
                    .await,
                Err(StorageError::InvalidMutationContext(_))
            ));
            assert_eq!(bytes(&d.path), before);
            assert_eq!(clock.0.load(Ordering::SeqCst), calls);
            let receipt = adapter
                .select_native_links(key(&f, &d, api, None), selection.clone(), None)
                .await
                .unwrap()
                .unwrap();
            let MutationOutcome::NativeGraphChanged { transition } = receipt.outcome else {
                panic!()
            };
            assert_eq!(transition.edges.len(), 1);
            let edge = &transition.edges[0].after;
            match selection {
                S::Semantic { .. } => assert_eq!(edge.target, peer.min(d.parent)),
                S::Entity { .. } | S::Temporal { .. } => assert_eq!(edge.target, peer),
            }
            assert_eq!(edge.source, id);
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
        assert_eq!(f.storage.read().unwrap().load_all_edges().unwrap().len(), 3);
        drop(adapter);
        let _root = f.close();
        inspect_schema(&d.path).unwrap();
    }
}
#[tokio::test]
async fn native_graph_producer_missing_owner_scope_and_unsupported_contract_do_not_mutate() {
    use crate::api::RelationshipGraph as _;
    let (f, d, clock, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let before = bytes(&d.path);
    let calls = clock.0.load(Ordering::SeqCst);
    assert!(matches!(
        adapter(&f)
            .put_native_edges(key(&f, &d, 1, Some("missing")), plan(id, d.parent), None)
            .await,
        Err(StorageError::RecoveryRequired(_))
    ));
    let owner = owned(&f, clock.clone());
    for api in [0, 3] {
        assert!(matches!(
            owner
                .put_native_edges(key(&f, &d, api, None), plan(id, d.parent), None)
                .await,
            Err(StorageError::InvalidMutationContext(_))
        ));
    }
    let mut wrong = key(&f, &d, 1, None);
    wrong.scope = MutationScope::Catalog;
    assert!(matches!(
        owner
            .put_native_edges(wrong, plan(id, d.parent), None)
            .await,
        Err(StorageError::MutationNamespaceMismatch)
    ));
    assert_eq!(bytes(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert!(
        owner
            .post_creation_links_with_work(id, d.ns, &[1., 0., 0., 0.], &[], &[], 3000, None)
            .await
            .is_err()
    );
    assert!(
        owner
            .add_node(id, d.ns, DecayPhase::Full, 1., 0)
            .await
            .is_err()
    );
    assert!(owner.tombstone_node(id).await.is_err());
    assert_eq!(bytes(&d.path), before);
}
#[test]
fn native_graph_producer_final_selection_generation_rejects_phantom_candidate() {
    use crate::graph::native_producer::{NativeLinkSelection, select};
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock);
    let mut original = input(&d);
    original.memory.parent_id = None;
    let capture = session.capture_insert(scope(&d), &original).unwrap();
    let id = inserted(
        &session
            .insert_prepared(&ctx(&session, &d, "source"), &capture, &generated())
            .unwrap(),
    );
    let (input, at) = select(
        &session.engine,
        d.ns,
        &NativeLinkSelection::Temporal {
            source: id,
            window_ms: 10000,
            max_links: 1,
        },
    )
    .unwrap();
    let mut capture = session.capture_graph(scope(&d), &input).unwrap();
    capture.selection_generation = Some(at);
    let added = session.capture_insert(scope(&d), &original).unwrap();
    session
        .insert_prepared(&ctx(&session, &d, "phantom"), &added, &generated())
        .unwrap();
    let before = bytes(&d.path);
    assert!(matches!(
        session.put_graph_prepared(&ctx(&session, &d, "selected"), &capture),
        Err(StorageError::LifecycleConflict(_))
    ));
    assert_eq!(bytes(&d.path), before);
}
#[tokio::test]
async fn native_graph_producer_publication_failure_preserves_exact_receipt_and_replay_repairs() {
    use crate::api::RelationshipGraph as _;
    let (f, d, clock, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let owner = owned(&f, clock.clone());
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
    let k = key(&f, &d, 1, Some("partial"));
    let input = plan(id, d.parent);
    let error = owner
        .put_native_edges(k.clone(), input.clone(), None)
        .await
        .unwrap_err();
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("lost full receipt: {error:?}")
    };
    let expected = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .lookup_native_graph(&k, &input, None)
        .unwrap()
        .unwrap();
    let mut replay = (*receipt).clone();
    replay.replayed = true;
    assert_eq!(expected, replay);
    let calls = clock.0.load(Ordering::SeqCst);
    assert!(
        !f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    let invalid = adapter(&f).with_native_completion(
        f.fts_index.clone(),
        clock.clone(),
        NativePolicyInputs {
            multiplier: f64::NAN,
            ..Default::default()
        },
    );
    assert_eq!(
        invalid.put_native_edges(k, input, None).await.unwrap(),
        Some(replay)
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    let at = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        completion(&f)
            .readiness_barrier(
                at,
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
            .await
            .unwrap()
            .ready
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_graph_producer_cancelled_waiter_keeps_worker_guards_and_unresolved_root() {
    use crate::api::RelationshipGraph as _;
    let (f, d, clock, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let owner = owned(&f, clock.clone());
    let k = key(&f, &d, 1, Some("cancel"));
    let input = plan(id, d.parent);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let root = tracker.begin().unwrap();
    let gate = crate::derived_completion::publication_gate(&f.storage)
        .await
        .unwrap();
    let held = f.graph.clone().write_owned().await;
    let saved_key = k.clone();
    let task = tokio::spawn(async move { owner.put_native_edges(k, input, Some(&root)).await });
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
    let at = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        completion(&f)
            .readiness_barrier(
                at,
                std::time::Instant::now() + std::time::Duration::from_millis(50)
            )
            .await
            .is_err()
    );
    let root = f.close();
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, session.engine);
    let receipt = owned(&f, clock)
        .put_native_edges(saved_key, plan(id, d.parent), None)
        .await
        .unwrap()
        .unwrap();
    assert!(receipt.replayed);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .status()
            .pending,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_graph_producer_accepted_descendants_complete_during_barrier_close() {
    use crate::api::RelationshipGraph as _;
    let (f, d, clock, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let owner = owned(&f, clock.clone());
    let k = key(&f, &d, 1, Some("closing"));
    let input = plan(id, d.parent);
    let at = f
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
    let task = tokio::spawn(async move {
        let result = owner.put_native_edges(k, input, Some(&root)).await;
        root.finish();
        result
    });
    until(|| tracker.status().pending >= 2).await;
    let publisher = completion(&f);
    let barrier = tokio::spawn(async move {
        publisher
            .readiness_barrier(
                at,
                std::time::Instant::now() + std::time::Duration::from_secs(10),
            )
            .await
    });
    until(|| !tracker.status().admission_open).await;
    assert!(
        owned(&f, clock)
            .put_native_edges(key(&f, &d, 1, None), plan(id, d.parent), None)
            .await
            .is_err()
    );
    drop(held);
    let receipt = task.await.unwrap().unwrap().unwrap();
    let ready = barrier.await.unwrap().unwrap();
    assert!(ready.ready);
    assert_eq!(ready.committed_generation, receipt.committed_generation);
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
}
struct UnknownClock(std::sync::atomic::AtomicBool);
impl Clock for UnknownClock {
    fn now_millis(&self) -> i64 {
        if self.0.swap(false, Ordering::SeqCst) {
            effects::FAIL_AFTER_RECORD_COMMIT.with(|v| v.set(true));
        }
        86_400_000
    }
}
#[tokio::test]
async fn native_graph_producer_unknown_commit_preserves_actual_identity_until_reopen() {
    use crate::api::RelationshipGraph as _;
    let (f, d, clock, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let owner = owned(
        &f,
        Arc::new(UnknownClock(std::sync::atomic::AtomicBool::new(true))),
    );
    let k = key(&f, &d, 1, Some("unknown"));
    let input = plan(id, d.parent);
    let error = owner
        .put_native_edges(k.clone(), input.clone(), None)
        .await
        .unwrap_err();
    let StorageError::CommitOutcomeUnknown { operation_id, .. } = error else {
        panic!("lost unknown identity: {error:?}")
    };
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .status()
            .unresolved
            > 0
    );
    drop(owner);
    let root = f.close();
    let mut session = open(&d, clock.clone());
    let receipt = session.put_graph(&k, scope(&d), &input).unwrap().unwrap();
    assert_eq!(receipt.operation_id, operation_id);
    assert!(receipt.replayed);
    let f = Fixture::from_open_engine(root, session.engine);
    let owner = owned(&f, clock);
    assert_eq!(
        owner.put_native_edges(k, input, None).await.unwrap(),
        Some(receipt)
    );
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
#[tokio::test]
async fn native_graph_producer_process_child() {
    use crate::api::RelationshipGraph as _;
    let Ok(path) = std::env::var("RECALLD_GRAPH_PRODUCER_PATH") else {
        return;
    };
    let d = Data {
        path: path.into(),
        ns: NamespaceId::new(
            std::env::var("RECALLD_GRAPH_PRODUCER_NS")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        parent: MemoryId::from_uuid(
            std::env::var("RECALLD_GRAPH_PRODUCER_TARGET")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        profile: crate::embedding::runtime::tests::profile(),
    };
    let id = MemoryId::from_uuid(
        std::env::var("RECALLD_GRAPH_PRODUCER_SOURCE")
            .unwrap()
            .parse()
            .unwrap(),
    );
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(tempfile::tempdir().unwrap(), session.engine);
    completion(&f).rebuild_all().await.unwrap();
    owned(&f, clock)
        .put_native_edges(key(&f, &d, 1, Some("process")), plan(id, d.parent), None)
        .await
        .unwrap();
    panic!("crash point not reached");
}
#[tokio::test]
async fn native_graph_producer_process_exit_reopens_and_replays_exactly_once() {
    use crate::api::RelationshipGraph as _;
    for point in [
        "before_metadata_commit",
        "after_metadata_commit",
        "after_edge_commit",
        "after_canonical_ack",
    ] {
        let (f, d, clock, id) = setup();
        completion(&f).rebuild_all().await.unwrap();
        let k = key(&f, &d, 1, Some("process"));
        let root = f.close();
        let child=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","storage::metadata::native_records::tests::readers::graph_producer::native_graph_producer_process_child","--nocapture"])
            .env("RECALLD_GRAPH_PRODUCER_PATH",&d.path).env("RECALLD_GRAPH_PRODUCER_NS",d.ns.get().to_string()).env("RECALLD_GRAPH_PRODUCER_TARGET",d.parent.to_string()).env("RECALLD_GRAPH_PRODUCER_SOURCE",id.to_string()).env("RECALLD_TEST_EFFECT_CRASH",point).output().unwrap();
        assert_eq!(
            child.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        let session = open(&d, clock.clone());
        let f = Fixture::from_open_engine(root, session.engine);
        let owner = owned(&f, clock.clone());
        let receipt = owner
            .put_native_edges(k.clone(), plan(id, d.parent), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.replayed, point != "before_metadata_commit");
        assert_eq!(
            receipt.committed_generation,
            k.expected_generation.unwrap().next().unwrap()
        );
        let mut expected = receipt;
        expected.replayed = true;
        assert_eq!(
            owner
                .put_native_edges(k, plan(id, d.parent), None)
                .await
                .unwrap(),
            Some(expected)
        );
        assert_eq!(f.storage.read().unwrap().load_all_edges().unwrap().len(), 1);
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
}

#[tokio::test]
async fn native_graph_producer_filters_namespace_and_terminal_candidates_before_caps() {
    use crate::api::RelationshipGraph as _;
    use crate::graph::native_producer::NativeLinkSelection as S;
    let root = tempfile::tempdir().unwrap();
    let mut foreign = MemoryId::nil();
    let d = fixture_with_seed(root.path(), |engine| {
        let mut ns = NamespaceConfig::default_namespace(0);
        ns.name = "foreign".into();
        ns.embedding_dim = 4;
        let ns = engine.create_namespace(&ns).unwrap();
        let input = crate::storage::insertion::InsertMemory {
            id: None,
            summary: "foreign close candidate".into(),
            full_text: None,
            tags: vec![],
            initial_stability: None,
            created_at: Some(2999),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        foreign = crate::storage::insertion::inserted_id(
            &engine
                .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &input, &[], 2999)
                .unwrap(),
        )
        .unwrap();
    });
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let mut ids = Vec::new();
    for (key, created_at) in [("source", 3000), ("terminal", 2999), ("eligible", 4000)] {
        let mut original = input(&d);
        original.memory.parent_id = None;
        original.memory.created_at = Some(created_at);
        let intent = session.capture_insert(scope(&d), &original).unwrap();
        ids.push(inserted(
            &session
                .insert_prepared(&ctx(&session, &d, key), &intent, &generated())
                .unwrap(),
        ));
    }
    let terminal = session.capture_tombstone(ids[1], None).unwrap();
    session
        .tombstone(&ctx(&session, &d, "remove"), &terminal)
        .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    let owner = owned(&f, clock);
    for selected in [
        S::Semantic {
            source: ids[0],
            threshold: 0.9,
            max_links: 1,
        },
        S::Entity {
            source: ids[0],
            max_links: 1,
        },
        S::Temporal {
            source: ids[0],
            window_ms: 10000,
            max_links: 1,
        },
    ] {
        let receipt = owner
            .select_native_links(key(&f, &d, 1, None), selected.clone(), None)
            .await
            .unwrap()
            .unwrap();
        let MutationOutcome::NativeGraphChanged { transition } = receipt.outcome else {
            panic!()
        };
        assert_eq!(transition.edges.len(), 1);
        let edge = &transition.edges[0].after;
        assert_ne!(edge.target, foreign);
        assert_ne!(edge.target, ids[1]);
        match selected {
            S::Semantic { .. } => assert_eq!(edge.target, ids[2].min(d.parent)),
            _ => assert_eq!(edge.target, ids[2]),
        }
    }
}
