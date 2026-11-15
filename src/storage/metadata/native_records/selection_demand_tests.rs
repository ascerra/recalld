use super::*;
use crate::graph::native_selection::take_reads;
use crate::model::EdgeType;

// Run the actual async consumer on one thread so instrumentation is isolated
// from other tests and cannot count their independent stores.
#[tokio::test]
async fn selection_demand_actual_single_reads_edges_and_reopen_replay() {
    actual(false).await;
}

#[tokio::test]
async fn selection_demand_actual_batch_reads_edges_and_reopen_replay() {
    actual(true).await;
}

async fn actual(batch: bool) {
    let mut mismatches = Vec::new();
    for mode in ["off", "entity", "temporal", "semantic", "advisory"] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut s = open(&d, clock.clone());
        let mut peer = input(&d);
        peer.memory.parent_id = None;
        peer.entities = vec!["shared".into()];
        let intent = s.capture_insert(scope(&d), &peer).unwrap();
        let peer_id = inserted(&s.insert_prepared(&ctx(&s, &d, "peer"), &intent, &generated()).unwrap());
        let f = Fixture::from_open_engine(tempfile::tempdir().unwrap(), s.engine);
        completion(&f).rebuild_all().await.unwrap();
        let mut config = crate::config::RecalldConfig::default();
        config.graph.autolink_enabled = mode != "off" && mode != "advisory";
        config.graph.max_auto_links = usize::from(mode == "semantic");
        config.graph.max_entity_links = usize::from(mode == "entity");
        config.graph.max_temporal_links = usize::from(mode == "temporal");
        config.graph.temporal_window_ms = 10;
        let p = provider(&d);
        let a = adapter(&f, p.clone(), clock.clone(), config);
        let mut input = single_input(&d, true);
        input.insert.memory.parent_id = None;
        input.insert.entities = vec!["shared".into()];
        input.check_duplicates = mode == "advisory";
        let key = single_context(&f, &d, mode);
        let mut request = request(&f, mode);
        request.items[0] = json!({"summary":"native demand", "entities":["shared"],
            "createdAt":3000,"checkDuplicates":mode == "advisory"});
        take_reads();
        let receipt = if batch {
            let out = a.run_batch(request.clone()).await.unwrap();
            assert!(out.all_ready(), "{mode}: {out:?}");
            out.items[0].receipt.clone().unwrap()
        } else {
            a.store_native_single(key.clone(), input.clone(), NativePolicyInputs::default(), None)
                .await.unwrap().receipt
        };
        let reads = take_reads();
        let expected = match mode {
            "off" => (0, 0),
            "entity" | "temporal" => (1, 0),
            _ => (1, 2),
        };
        if reads != expected { mismatches.push(format!("{mode}: actual={reads:?} expected={expected:?}")); }
        let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else { panic!("{receipt:?}") };
        let expected_kind = match mode {
            "entity" => Some(EdgeType::Entity),
            "temporal" => Some(EdgeType::Temporal),
            "semantic" => Some(EdgeType::Associative),
            _ => None,
        };
        assert_eq!(transition.edges.len(), usize::from(expected_kind.is_some()), "{mode}");
        if let Some(kind) = expected_kind {
            let edge = &transition.edges[0];
            assert_eq!(edge.edge_type, kind);
            let target = if mode == "semantic" { d.parent.min(peer_id) } else { peer_id };
            assert_eq!(edge.target, target, "{mode}");
            assert_eq!(edge.weight_bits, 1f32.to_bits(), "{mode}");
        }
        assert_eq!(transition.advisory.as_ref().map(Vec::len), (mode == "advisory").then_some(2));
        assert_eq!(p.calls.load(Ordering::SeqCst), 1);
        let before = f.storage.read().unwrap().control_state().unwrap();
        drop(a);
        drop(f);
        // Strict owned reopen validates the actual receipt/edges and payloads.
        let s = open(&d, clock.clone());
        let f = Fixture::from_open_engine(tempfile::tempdir().unwrap(), s.engine);
        completion(&f).rebuild_all().await.unwrap();
        let mut unavailable = provider(&d);
        Arc::get_mut(&mut unavailable).unwrap().profile = None;
        let mut changed = crate::config::RecalldConfig::default();
        changed.graph.auto_link_threshold = f64::NAN;
        let a = adapter(&f, unavailable.clone(), clock.clone(), changed);
        let calls = clock.0.load(Ordering::SeqCst);
        take_reads();
        let replay = if batch {
            let out = a.run_batch(request).await.unwrap();
            assert!(out.all_ready(), "{out:?}");
            out.items[0].receipt.clone().unwrap()
        } else {
            a.store_native_single(key, input, NativePolicyInputs::default(), None).await.unwrap().receipt
        };
        assert_eq!(replay, expected_replay(&receipt));
        assert_eq!(take_reads(), (0, 0), "replay {mode}");
        assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
        assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    }
    assert!(mismatches.is_empty(), "selection work excess: {mismatches:?}");
}
