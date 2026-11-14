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
async fn native_graph_producer_baseline_existing_temporal() {
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
    .unwrap();
    assert_eq!(count, 1);
}
#[tokio::test]
async fn native_graph_producer_baseline_actual_adapter() {
    let (f, d, _, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let owner: &dyn crate::api::RelationshipGraph = &adapter(&f);
    let outcome = owner.apply_supersedes_edge(id, d.parent).await;
    assert_eq!(outcome.status, crate::model::SupersedesStatus::Applied);
}
#[test]
fn native_graph_producer_baseline_original_api1() {
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
