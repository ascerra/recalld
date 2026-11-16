use super::*;
use crate::api::RelationshipGraph;
use crate::graph::native_producer::{NativeLinkSelection as Selection, vector_probe::Probe};

// Real legacy insertion -> explicit schema/lifecycle migration. Intentionally
// lacks the separate recipe certification; no native canonical row is authored.
fn retained(certified: bool) -> (Fixture, Data, Arc<FixedClock>, Vec<MemoryId>) {
    let root = tempfile::tempdir().unwrap();
    let mut ids = Vec::new();
    let seed = |engine: &mut RedbStorageEngine| {
        let ns = engine
            .list_namespaces()
            .unwrap()
            .into_iter()
            .find(|ns| ns.name == "native")
            .unwrap()
            .id;
        for i in 0..2 {
            let input = crate::storage::insertion::InsertMemory {
                id: None,
                summary: format!("shared graph peer {i}"),
                full_text: None,
                tags: vec!["entity/shared".into()],
                initial_stability: None,
                created_at: Some(2000 + i),
                embedding: Some(vec![1., 0., 0., 0.]),
                parent_id: None,
                supersedes: None,
            };
            let r = engine
                .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &input, &[], 2000)
                .unwrap();
            ids.push(crate::storage::insertion::inserted_id(&r).unwrap());
        }
    };
    let d = if certified {
        certified_fixture_with_seed(root.path(), seed)
    } else {
        fixture_with_seed(root.path(), seed)
    };
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    (
        Fixture::from_open_engine(root, session.engine),
        d,
        clock,
        ids,
    )
}
fn probe(f: &Fixture, d: &Data) -> Probe {
    Probe::new(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .records_in_namespace(d.ns)
            .unwrap()
            .into_iter()
            .map(|r| MemoryId::from_bytes(r.id))
            .collect(),
    )
}
#[tokio::test]
async fn certified_standalone_uncertified_semantic_actual_trait_rejects_before_reads_even_zero_cap()
{
    let mut failures = Vec::new();
    for api in [1, 2] {
        for cap in [0, 1] {
            let (f, d, clock, ids) = retained(false);
            completion(&f).rebuild_all().await.unwrap();
            let adapter = owned(&f, clock.clone());
            let consumer: &dyn RelationshipGraph = &adapter;
            let before = bytes(&d.path);
            let state = f.storage.read().unwrap().control_state().unwrap();
            let calls = clock.0.load(Ordering::SeqCst);
            let reads = probe(&f, &d);
            let result = consumer
                .select_native_links(
                    key(&f, &d, api, None),
                    Selection::Semantic {
                        source: ids[0],
                        threshold: 0.9,
                        max_links: cap,
                    },
                    None,
                )
                .await;
            let unchanged = state == f.storage.read().unwrap().control_state().unwrap()
                && before == bytes(&d.path);
            let clock_delta = clock.0.load(Ordering::SeqCst) - calls;
            println!(
                "actual Semantic api={api} cap={cap} outcome={result:?} reads={} clock={clock_delta} unchanged={unchanged}",
                reads.reads()
            );
            if !matches!(result, Err(StorageError::VectorCertificationRequired { namespace_id }) if namespace_id == d.ns.get())
                || reads.reads() != 0
                || clock_delta != 0
                || !unchanged
            {
                failures.push((api, cap));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "uncertified Semantic bypass: {failures:?}"
    );
}
#[tokio::test]
async fn certified_standalone_uncertified_structural_actual_traits_remain_available() {
    for api in [1, 2] {
        for mode in [0, 1, 2] {
            let (f, d, clock, ids) = retained(false);
            completion(&f).rebuild_all().await.unwrap();
            let adapter = owned(&f, clock.clone());
            let consumer: &dyn RelationshipGraph = &adapter;
            let reads = probe(&f, &d);
            let explicit_context = key(&f, &d, api, Some("explicit-uncert"));
            let receipt = match mode {
                0 => {
                    consumer
                        .select_native_links(
                            key(&f, &d, api, None),
                            Selection::Entity {
                                source: ids[0],
                                max_links: 1,
                            },
                            None,
                        )
                        .await
                }
                1 => {
                    consumer
                        .select_native_links(
                            key(&f, &d, api, None),
                            Selection::Temporal {
                                source: ids[0],
                                window_ms: 10000,
                                max_links: 1,
                            },
                            None,
                        )
                        .await
                }
                _ => {
                    consumer
                        .put_native_edges(explicit_context.clone(), plan(ids[0], ids[1]), None)
                        .await
                }
            }
            .unwrap()
            .unwrap();
            let MutationOutcome::NativeGraphChanged { transition } = &receipt.outcome else {
                panic!("{receipt:?}")
            };
            assert_eq!(transition.edges.len(), 1);
            assert_eq!(reads.reads(), 0);
            if mode == 2 {
                let calls = clock.0.load(Ordering::SeqCst);
                let before = bytes(&d.path);
                let context = explicit_context;
                let replay = consumer
                    .put_native_edges(context, plan(ids[0], ids[1]), None)
                    .await
                    .unwrap()
                    .unwrap();
                let mut expected = receipt.clone();
                expected.replayed = true;
                assert_eq!(replay, expected);
                assert_eq!(clock.0.load(Ordering::SeqCst), calls);
                assert_eq!(before, bytes(&d.path));
            }
            drop(adapter);
            let _root = f.close();
            crate::storage::offline_snapshot::OfflineSnapshot::open(&d.path).unwrap();
        }
    }
}
