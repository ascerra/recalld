//! Actual producer/close races on isolated tempstores; no readiness watermark claim.
use crate::{
    model::{DecayPhase, NamespaceId},
    storage::{StorageEngine, work::WorkTracker},
    test_support::{Fixture, MemorySpec},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn completion(f: &Fixture) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        graph: f.graph.clone(),
        cache: f.cache.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}
async fn wait(tracker: &WorkTracker, predicate: impl Fn(crate::storage::work::WorkStatus) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !predicate(tracker.status()) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
fn close(
    tracker: &WorkTracker,
) -> tokio::task::JoinHandle<
    Result<crate::storage::work::WorkFreeze, crate::storage::work::WorkError>,
> {
    let tracker = tracker.clone();
    tokio::task::spawn_blocking(move || {
        tracker.close_and_drain(Instant::now() + Duration::from_secs(10))
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_graph_discovery_and_explicit_edges_finish_after_close() {
    for explicit in [false, true] {
        let f = Fixture::new();
        let first = f.insert_memory_in(NamespaceId::new(1), &[]).await;
        let second = f.insert_memory_in(NamespaceId::new(1), &[]).await;
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let held = f.graph.clone().write_owned().await;
        let graph = f.graph.clone();
        let storage = f.storage.clone();
        let cache = f.cache.clone();
        let task = tokio::spawn(async move {
            if explicit {
                crate::graph::apply_parent_edge(first, second, &graph, &storage, &cache).await;
            } else {
                assert_eq!(
                    crate::graph::perform_temporal_link(
                        second,
                        NamespaceId::new(1),
                        100,
                        1000,
                        2,
                        &[(first, 99)],
                        &graph,
                        &storage,
                        &cache
                    )
                    .await
                    .unwrap(),
                    1
                );
            }
        });
        wait(&tracker, |s| s.pending > 0).await;
        let drain = close(&tracker);
        wait(&tracker, |s| !s.admission_open).await;
        assert!(!drain.is_finished());
        drop(held);
        task.await.unwrap();
        let freeze = drain.await.unwrap().unwrap();
        assert_eq!(tracker.status().unresolved, 0);
        let expected = if explicit {
            crate::model::EdgeType::ParentChild
        } else {
            crate::model::EdgeType::Temporal
        };
        assert!(
            f.graph
                .read()
                .await
                .has_typed_edge_between_ids(&first, &second, expected)
        );
        let storage = f.storage.read().unwrap();
        let edges = storage.load_all_edges().unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_type, expected);
        assert!(storage.meta_store().pending_effects().unwrap().is_empty());
        assert!(storage.meta_store().freeze_metadata(freeze).is_ok());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_graph_discovery_retains_unresolved_root_without_mutation() {
    let f = Fixture::new();
    let first = f.insert_memory_in(NamespaceId::new(1), &[]).await;
    let second = f.insert_memory_in(NamespaceId::new(1), &[]).await;
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let held = f.graph.clone().write_owned().await;
    let graph = f.graph.clone();
    let storage = f.storage.clone();
    let cache = f.cache.clone();
    let task = tokio::spawn(async move {
        crate::graph::perform_temporal_link(
            second,
            NamespaceId::new(1),
            100,
            1000,
            2,
            &[(first, 99)],
            &graph,
            &storage,
            &cache,
        )
        .await
    });
    wait(&tracker, |s| s.pending > 0).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(held);
    assert_eq!(tracker.status().unresolved, 1);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        before.committed_generation
    );
    assert_eq!(f.record(first).unwrap().edge_count, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_catalog_adapters_keep_children_admitted_until_handle_publication() {
    use crate::api::NamespaceRegistry as _;
    use crate::mcp::bridge::NamespaceRegistry as _;
    for mcp in [false, true] {
        let f = Fixture::new();
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let (ready, held) = std::sync::mpsc::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let storage = f.storage.clone();
        let holder = std::thread::spawn(move || {
            let _read = storage.read().unwrap();
            ready.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        held.recv_timeout(Duration::from_secs(10)).unwrap();
        let api = crate::api::adapters::NamespaceRegistryAdapter::new(
            f.storage.clone(),
            std::sync::Arc::new(crate::embedding::PassthroughProvider::new(
                crate::test_support::DIM,
            )),
        )
        .with_completion(f.completion());
        let bridge = f.namespace_adapter();
        let task = tokio::spawn(async move {
            if mcp {
                bridge
                    .create_namespace(crate::mcp::bridge::CreateNamespaceInput {
                        name: "accepted".into(),
                        embedding_dim: Some(4),
                        initial_stability: None,
                        desired_retention: None,
                        decay_rate_multiplier: None,
                    })
                    .await
                    .unwrap();
            } else {
                api.create("accepted", Some(4), 1., 0.9, None)
                    .await
                    .unwrap();
            }
        });
        wait(&tracker, |s| s.pending >= 2).await;
        let drain = close(&tracker);
        wait(&tracker, |s| !s.admission_open).await;
        assert!(!drain.is_finished());
        release.send(()).unwrap();
        holder.join().unwrap();
        task.await.unwrap();
        let freeze = drain.await.unwrap().unwrap();
        assert!(
            f.storage
                .read()
                .unwrap()
                .get_namespace_by_name("accepted")
                .unwrap()
                .is_some()
        );
        assert_eq!(tracker.status().pending, 0);
        drop(freeze);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_manual_and_startup_sweeps_drain_transitive_delete_compaction_and_views() {
    use crate::decay::sweep::{DecaySweepRunner, SweepConfig};
    for startup in [false, true] {
        let f = Fixture::new();
        let dead = f
            .insert_memory_with(MemorySpec {
                phase: DecayPhase::Ghost,
                decay_strength: 0.,
                ..MemorySpec::new(NamespaceId::new(1))
            })
            .await;
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let runner = DecaySweepRunner::new(
            SweepConfig {
                sweep_on_startup: startup,
                ..Default::default()
            },
            Arc::new(crate::decay::DecayConfig::default()),
            crate::graph::ActivationConfig::default(),
            f.storage.clone(),
            f.graph.clone(),
            f.cache.clone(),
            1.,
        )
        .unwrap()
        .with_completion(completion(&f));
        let held = f.graph.clone().write_owned().await;
        if !startup {
            runner.trigger();
            runner.trigger();
            assert_eq!(
                tracker.status().pending,
                1,
                "Notify coalescing owns one accepted root"
            );
        }
        let handle = runner.start();
        wait(&tracker, |s| s.pending > 0).await;
        let drain = close(&tracker);
        wait(&tracker, |s| !s.admission_open).await;
        assert!(!drain.is_finished());
        drop(held);
        let freeze = drain.await.unwrap().unwrap();
        assert!(f.record(dead).is_none());
        assert!(!f.graph.read().await.contains(&dead));
        assert!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .pending_effects()
                .unwrap()
                .is_empty()
        );
        assert_eq!(tracker.status().unresolved, 0);
        // A rejected new trigger cannot leave a ticket behind or enqueue writes.
        runner.trigger();
        assert_eq!(tracker.status().pending, 0);
        runner.shutdown();
        handle.await.unwrap();
        drop(freeze);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_sweep_keeps_accepted_work_unresolved() {
    use crate::decay::sweep::{DecaySweepRunner, SweepConfig};
    let f = Fixture::new();
    let id = f
        .insert_memory_with(MemorySpec {
            phase: DecayPhase::Ghost,
            decay_strength: 0.,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let held = f.graph.clone().write_owned().await;
    let runner = DecaySweepRunner::new(
        SweepConfig::default(),
        Arc::new(crate::decay::DecayConfig::default()),
        crate::graph::ActivationConfig::default(),
        f.storage.clone(),
        f.graph.clone(),
        f.cache.clone(),
        1.,
    )
    .unwrap()
    .with_completion(completion(&f));
    let task = runner.start();
    wait(&tracker, |s| s.pending >= 2).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(held);
    assert!(tracker.status().unresolved > 0);
    assert!(f.record(id).is_some());
}

#[tokio::test]
async fn shutdown_abandons_queued_trigger_instead_of_claiming_completion() {
    use crate::decay::sweep::{DecaySweepRunner, SweepConfig};
    let f = Fixture::new();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let runner = DecaySweepRunner::new(
        SweepConfig {
            sweep_on_startup: false,
            ..Default::default()
        },
        Arc::new(crate::decay::DecayConfig::default()),
        crate::graph::ActivationConfig::default(),
        f.storage.clone(),
        f.graph.clone(),
        f.cache.clone(),
        1.,
    )
    .unwrap();
    runner.trigger();
    assert_eq!(tracker.status().pending, 1);
    runner.shutdown();
    assert_eq!(tracker.status().unresolved, 1);
    runner.trigger();
    assert_eq!(tracker.status().pending, 1);
    runner.start().await.unwrap();
    assert_eq!(tracker.status().unresolved, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_graph_node_publication_is_registered_before_lock_wait() {
    use crate::api::RelationshipGraph as _;
    let f = Fixture::new();
    let id = f.insert_memory_in(NamespaceId::new(1), &[]).await;
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let adapter = Arc::new(crate::api::adapters::RelationshipGraphAdapter::new(
        f.graph.clone(),
        f.vector_index.clone(),
        f.entity_index.clone(),
        f.storage.clone(),
        f.cache.clone(),
        Arc::new(crate::config::RecalldConfig::default()),
    ));
    let held = f.graph.clone().write_owned().await;
    let worker = adapter.clone();
    let task = tokio::spawn(async move { worker.tombstone_node(id).await });
    wait(&tracker, |s| s.pending == 1).await;
    let drain = close(&tracker);
    wait(&tracker, |s| !s.admission_open).await;
    assert!(matches!(
        adapter
            .add_node(
                crate::model::MemoryId::new(),
                NamespaceId::new(1),
                DecayPhase::Full,
                1.,
                0
            )
            .await,
        Err(crate::graph::GraphError::Unavailable(_))
    ));
    assert!(!drain.is_finished());
    drop(held);
    task.await.unwrap().unwrap();
    let freeze = drain.await.unwrap().unwrap();
    assert_eq!(
        f.graph.read().await.get_node(&id).unwrap().decay_phase,
        DecayPhase::Tombstone
    );
    assert_eq!(tracker.status().pending, 0);
    drop(freeze);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sweep_publication_failure_preserves_receipt_and_can_repair_without_reopen() {
    use crate::api::FsrsEngine as _;
    let f = Fixture::new();
    let dead = f
        .insert_memory_with(MemorySpec {
            phase: DecayPhase::Ghost,
            decay_strength: 0.,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    let live = f
        .insert_memory_with(MemorySpec {
            is_permastore: true,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    *f.vector_index.write().await =
        crate::search::FlatVectorIndex::new(crate::test_support::DIM + 1);
    let adapter = crate::api::adapters::FsrsEngineAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        f.graph.clone(),
        Arc::new(crate::config::RecalldConfig::default()),
        false,
    )
    .with_indexes(
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
    );
    let error = adapter.trigger_sweep(None).await.unwrap_err();
    let pending = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .pending_effects()
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert!(
        matches!(error.downcast_ref::<crate::storage::StorageError>(),Some(crate::storage::StorageError::CommittedMutation{receipt,..}) if receipt.operation_id==pending[0].receipt.operation_id)
    );
    assert!(f.record(dead).is_none());
    assert!(f.record(live).is_some());
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    assert_eq!(
        tracker.status().pending,
        0,
        "known failure is durable pending work, not an abandoned task"
    );
    let freeze = close(&tracker).await.unwrap().unwrap();
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .freeze_metadata(freeze)
            .is_err()
    );
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(crate::test_support::DIM);
    completion(&f).complete().await.unwrap();
    let freeze = close(&tracker).await.unwrap().unwrap();
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .freeze_metadata(freeze)
            .is_ok()
    );
    assert_eq!(tracker.status().unresolved, 0);
}
