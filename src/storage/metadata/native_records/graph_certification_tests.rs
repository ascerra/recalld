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
                tags: vec![crate::model::Tag::new("entity/shared").unwrap()],
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

#[tokio::test]
async fn certified_standalone_certified_actual_semantic_edges_and_zero_cap_reopen() {
    for api in [1, 2] {
        for cap in [0, 1] {
            let (f, d, clock, ids) = retained(true);
            completion(&f).rebuild_all().await.unwrap();
            let adapter = owned(&f, clock.clone());
            let consumer: &dyn RelationshipGraph = &adapter;
            let reads = probe(&f, &d);
            let state = f.storage.read().unwrap().control_state().unwrap();
            let calls = clock.0.load(Ordering::SeqCst);
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
                .await
                .unwrap();
            assert_eq!(reads.reads(), 3);
            if cap == 0 {
                assert!(result.is_none());
                assert_eq!(state, f.storage.read().unwrap().control_state().unwrap());
                assert_eq!(clock.0.load(Ordering::SeqCst), calls);
            } else {
                let receipt = result.unwrap();
                let MutationOutcome::NativeGraphChanged { transition } = &receipt.outcome else {
                    panic!()
                };
                assert_eq!(transition.edges.len(), 1);
                let edge = &transition.edges[0].after;
                assert_eq!(edge.target, d.parent.min(ids[1]));
                assert_eq!(edge.edge_type, crate::model::EdgeType::Associative);
                assert_eq!(edge.weight_bits, 1f32.to_bits());
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
            drop(adapter);
            let _root = f.close();
            crate::storage::offline_snapshot::OfflineSnapshot::open(&d.path).unwrap();
        }
    }
}
#[tokio::test]
async fn certified_standalone_certification_does_not_mask_source_or_candidate_corruption() {
    for source in [true, false] {
        let (f, d, clock, ids) = retained(true);
        completion(&f).rebuild_all().await.unwrap();
        let corrupt = if source { ids[0] } else { ids[1] };
        {
            let engine = f.storage.write().unwrap();
            let mut row = engine.get_record(corrupt).unwrap().unwrap();
            row.schema.as_mut().unwrap().vector.as_mut().unwrap().sha256[0] ^= 1;
            let tx = engine.meta_store().db.begin_write().unwrap();
            tx.open_table(META_TABLE)
                .unwrap()
                .insert(
                    corrupt.as_bytes().as_slice(),
                    row.encode().unwrap().as_slice(),
                )
                .unwrap();
            tx.commit().unwrap();
        }
        let before = bytes(&d.path);
        let state = f.storage.read().unwrap().control_state().unwrap();
        let calls = clock.0.load(Ordering::SeqCst);
        let reads = probe(&f, &d);
        let result = owned(&f, clock.clone())
            .select_native_links(
                key(&f, &d, 2, None),
                Selection::Semantic {
                    source: ids[0],
                    threshold: 0.9,
                    max_links: 1,
                },
                None,
            )
            .await;
        assert!(
            matches!(result, Err(StorageError::OwnershipMismatch(_))),
            "{result:?}"
        );
        assert!(reads.reads() > 0);
        assert_eq!(state, f.storage.read().unwrap().control_state().unwrap());
        assert_eq!(before, bytes(&d.path));
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert!(
            f.storage
                .read()
                .unwrap()
                .load_all_edges()
                .unwrap()
                .is_empty()
        );
        let _root = f.close();
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err()
        );
    }
}
#[test]
fn certified_standalone_exact_namespace_and_generation_preflight_and_final_metadata_writer_race() {
    use crate::graph::native_producer::select;
    let (f, d, clock, ids) = retained(true);
    let mut engine = f.storage.write().unwrap();
    let generation = engine.control_state().unwrap().generation_ref();
    let ns = engine.get_namespace(d.ns).unwrap().unwrap();
    let mut stale_ns = ns.clone();
    stale_ns.name.push_str("-other");
    let reads = Probe::new(vec![ids[0], ids[1], d.parent]);
    assert!(matches!(
        engine
            .meta_store()
            .with_generation_read_at(generation, |view| view
                .check_native_vector_namespace(&stale_ns)),
        Err(StorageError::RecoveryRequired(_))
    ));
    assert_eq!(reads.reads(), 0);
    let (input, at) = select(
        &engine,
        d.ns,
        &Selection::Semantic {
            source: ids[0],
            threshold: 0.9,
            max_links: 1,
        },
    )
    .unwrap();
    let mut intent = engine
        .meta_store()
        .capture_native_graph(scope(&d), &input, &NativePolicyInputs::default(), None)
        .unwrap();
    intent.selection_generation = Some(at);
    // A real metadata-only writer can commit while this engine write guard is
    // held. Select an unrelated record so the final global G check is essential.
    let unrelated = [d.parent, ids[1]]
        .into_iter()
        .find(|id| {
            !input
                .edges
                .iter()
                .any(|e| e.source == *id || e.target == *id)
        })
        .unwrap();
    let mut context = MutationContext::unkeyed(d.ns);
    context.store_id = Some(generation.store_id);
    context.contract_version = 2;
    std::thread::scope(|scope| {
        let meta = engine.meta_store();
        let clock = clock.clone();
        let context = context.clone();
        scope
            .spawn(move || {
                let intent = meta
                    .capture_feedback(unrelated, None, 3, &NativePolicyInputs::default())
                    .unwrap();
                meta.apply_native_feedback(
                    &context,
                    &intent,
                    &NativePolicyInputs::default(),
                    clock.as_ref(),
                )
                .unwrap();
            })
            .join()
            .unwrap();
    });
    assert!(matches!(
        engine
            .meta_store()
            .with_generation_read_at(at, |view| view.check_native_vector_namespace(&ns)),
        Err(StorageError::GenerationConflict { .. })
    ));
    let before = bytes(&d.path);
    let state = engine.control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    assert!(matches!(
        engine.apply_native_graph(
            &context,
            &intent,
            &NativePolicyInputs::default(),
            clock.as_ref(),
            None
        ),
        Err(StorageError::LifecycleConflict(_))
    ));
    assert_eq!(state, engine.control_state().unwrap());
    assert_eq!(before, bytes(&d.path));
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert!(engine.load_all_edges().unwrap().is_empty());
}
#[tokio::test]
async fn certified_standalone_missing_owner_is_recovery_not_absence_and_never_reads_vectors() {
    let (f, d, clock, ids) = retained(true);
    completion(&f).rebuild_all().await.unwrap();
    let context = key(&f, &d, 2, None);
    let before = bytes(&d.path);
    let calls = clock.0.load(Ordering::SeqCst);
    let reads = probe(&f, &d);
    *f.storage
        .read()
        .unwrap()
        .meta_store()
        .vector_validation
        .write()
        .unwrap() = None;
    let result = owned(&f, clock.clone())
        .select_native_links(
            context,
            Selection::Semantic {
                source: ids[0],
                threshold: 0.9,
                max_links: 1,
            },
            None,
        )
        .await;
    assert!(
        matches!(result, Err(StorageError::RecoveryRequired(_))),
        "{result:?}"
    );
    assert_eq!(reads.reads(), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(before, bytes(&d.path));
}
