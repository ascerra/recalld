use super::*;
use crate::derived_completion::DerivedCompletion;
use crate::model::EdgeType;
use crate::storage::native_graph::{NativeEdgeInput, NativeGraphInput};
use crate::test_support::Fixture;

fn completion(f: &Fixture) -> DerivedCompletion {
    DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}
fn prepared() -> (
    Fixture,
    Data,
    MutationContext,
    NativeGraphInput,
    Arc<FixedClock>,
) {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let mut original = input(&d);
    original.memory.parent_id = None;
    let intent = s.capture_insert(scope(&d), &original).unwrap();
    let id = inserted(
        &s.insert_prepared(&ctx(&s, &d, "producer-record"), &intent, &generated())
            .unwrap(),
    );
    let context = ctx(&s, &d, "producer-graph");
    let input = NativeGraphInput {
        edges: vec![NativeEdgeInput {
            source: id,
            target: d.parent,
            edge_type: EdgeType::Entity,
            weight: 0.75,
            auto_created: true,
        }],
    };
    (
        Fixture::from_open_engine(root, s.engine),
        d,
        context,
        input,
        clock,
    )
}
struct NeverClock;
impl Clock for NeverClock {
    fn now_millis(&self) -> i64 {
        panic!("replay sampled clock")
    }
}

#[tokio::test]
async fn native_graph_producer_publication_failure_preserves_exact_receipt_then_replays_without_clock_policy()
 {
    let (f, d, context, input, clock) = prepared();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let generation = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(5);
    let error = publisher
        .native_graph(
            context.clone(),
            input.clone(),
            NativePolicyInputs::default(),
            clock.clone(),
            None,
        )
        .await
        .err()
        .expect("bad derived width accepted");
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("{error}")
    };
    assert_eq!(receipt.committed_generation, generation.next().unwrap());
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()[0]
            .receipt,
        *receipt
    );
    let calls = clock.0.load(Ordering::SeqCst);
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    let policy = NativePolicyInputs {
        multiplier: f64::NAN,
        ..Default::default()
    };
    let replay = publisher
        .native_graph(context, input.clone(), policy, Arc::new(NeverClock), None)
        .await
        .unwrap()
        .unwrap();
    let mut expected = (*receipt).clone();
    expected.replayed = true;
    assert_eq!(replay.receipt, expected);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    assert!(f.graph.read().await.has_typed_edge_between_ids(
        &input.edges[0].source,
        &d.parent,
        EdgeType::Entity
    ));
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        receipt.committed_generation
    );
    drop(publisher);
    let root = f.close();
    let s = open(&d, clock);
    assert_eq!(s.engine.edge_store().load_all_edges().unwrap().len(), 1);
    drop(s);
    drop(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_graph_producer_root_precedes_graph_wait_and_completes_descendants_during_close() {
    use std::time::{Duration, Instant};
    let (f, _d, context, input, clock) = prepared();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let gate = crate::derived_completion::publication_gate(&f.storage)
        .await
        .unwrap();
    let graph = f.graph.clone().write_owned().await;
    let job = tokio::spawn(async move {
        publisher
            .native_graph(context, input, NativePolicyInputs::default(), clock, None)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        // Root admission precedes a short storage read to get the gate.
        // Wait for the actual lease before asserting the graph-wait lock order.
        while tracker.status().pending == 0 || gate.try_write().is_ok() {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    assert!(
        f.storage.try_write().is_ok(),
        "producer awaited graph while holding storage"
    );
    let closing = tracker.clone();
    let drain = tokio::task::spawn_blocking(move || {
        closing.close_and_drain(Instant::now() + Duration::from_secs(10))
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while tracker.status().admission_open {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    assert!(!drain.is_finished());
    drop(graph);
    let result = job.await.unwrap().unwrap().unwrap();
    assert!(!result.receipt.replayed);
    let frozen = drain.await.unwrap().unwrap();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    drop(frozen);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_graph_producer_cancelled_before_graph_ownership_stays_unresolved_without_commit() {
    use std::time::Duration;
    let (f, _d, context, input, clock) = prepared();
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let gate = crate::derived_completion::publication_gate(&f.storage)
        .await
        .unwrap();
    let graph = f.graph.clone().write_owned().await;
    let job = tokio::spawn(async move {
        publisher
            .native_graph(context, input, NativePolicyInputs::default(), clock, None)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while tracker.status().pending == 0 || gate.try_write().is_ok() {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    job.abort();
    assert!(job.await.unwrap_err().is_cancelled());
    drop(graph);
    assert_eq!(tracker.status().pending, 1);
    assert_eq!(tracker.status().unresolved, 1);
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
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
