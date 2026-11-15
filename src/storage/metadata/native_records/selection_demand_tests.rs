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
        let peer_id = inserted(
            &s.insert_prepared(&ctx(&s, &d, "peer"), &intent, &generated())
                .unwrap(),
        );
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
            a.store_native_single(
                key.clone(),
                input.clone(),
                NativePolicyInputs::default(),
                None,
            )
            .await
            .unwrap()
            .receipt
        };
        let reads = take_reads();
        let expected = match mode {
            "off" => (0, 0),
            "entity" | "temporal" => (1, 0),
            _ => (1, 2),
        };
        if reads != expected {
            mismatches.push(format!("{mode}: actual={reads:?} expected={expected:?}"));
        }
        let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
            panic!("{receipt:?}")
        };
        let expected_kind = match mode {
            "entity" => Some(EdgeType::Entity),
            "temporal" => Some(EdgeType::Temporal),
            "semantic" => Some(EdgeType::Associative),
            _ => None,
        };
        assert_eq!(
            transition.edges.len(),
            usize::from(expected_kind.is_some()),
            "{mode}"
        );
        if let Some(kind) = expected_kind {
            let edge = &transition.edges[0];
            assert_eq!(edge.edge_type, kind);
            let target = if mode == "semantic" {
                d.parent.min(peer_id)
            } else {
                peer_id
            };
            assert_eq!(edge.target, target, "{mode}");
            assert_eq!(edge.weight_bits, 1f32.to_bits(), "{mode}");
        }
        assert_eq!(
            transition.advisory.as_ref().map(Vec::len),
            (mode == "advisory").then_some(2)
        );
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
            a.store_native_single(key, input, NativePolicyInputs::default(), None)
                .await
                .unwrap()
                .receipt
        };
        assert_eq!(replay, expected_replay(&receipt));
        assert_eq!(take_reads(), (0, 0), "replay {mode}");
        assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
        assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    }
    assert!(
        mismatches.is_empty(),
        "selection work excess: {mismatches:?}"
    );
}

#[test]
fn selection_demand_uncomputed_scores_cannot_be_used_and_off_still_validates_input() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let mut input = single_input(&d, false);
    input.check_duplicates = false;
    let mut config = crate::config::GraphConfig::default();
    config.max_auto_links = 0;
    let mut intent = s
        .metadata()
        .capture_native_store(
            scope(&d),
            &input,
            &crate::graph::native_selection::freeze_settings(&config).unwrap(),
            &NativePolicyInputs::default(),
        )
        .unwrap();
    let candidates =
        crate::graph::native_selection::capture(&s.engine, &intent, &generated()).unwrap();
    assert!(!candidates.candidates.is_empty());
    assert!(candidates.candidates.iter().all(|c| c.similarity.is_none()));
    assert!(candidates.advisory(true).is_err());
    assert!(candidates.static_links(&Default::default()).is_err());
    intent.check_duplicates = true;
    assert!(candidates.frozen_plan(&intent).is_err());
    intent.check_duplicates = false;
    intent.settings.autolink_enabled = false;
    let before = s.metadata().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    for vector in [vec![0.; 4], vec![f32::NAN; 4], vec![1.; 2]] {
        intent.insert.original.memory.embedding = Some(vector);
        take_reads();
        assert!(matches!(
            crate::graph::native_selection::capture(&s.engine, &intent, &generated()),
            Err(StorageError::CallerVectorInvalid(_))
        ));
        assert_eq!(take_reads(), (0, 0));
    }
    intent.insert.original.memory.embedding = None;
    intent.insert.original.embedding_profile = None;
    assert!(matches!(
        crate::graph::native_selection::capture(&s.engine, &intent, &generated()),
        Err(StorageError::ProviderVectorUnavailable(_))
    ));
    assert_eq!(s.metadata().control_state().unwrap(), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
}

// Inject a genuine candidate-reference/payload mismatch only after the actual
// consumer's provider begins (after reservation in batch mode). This negative
// corruption probe is not a fixture-generation shortcut or normal writer.
struct CorruptCandidate {
    storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
    id: MemoryId,
    profile: crate::model::schema::EmbeddingProfile,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for CorruptCandidate {
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "synthetic-candidate-corruption"
    }
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        Some(self.profile.clone())
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let engine = self.storage.write().unwrap();
        let tx = engine.meta_store().db.begin_write().unwrap();
        {
            let mut table = tx.open_table(META_TABLE).unwrap();
            let mut row = DiskRecord::from_bytes(
                table
                    .get(self.id.as_bytes().as_slice())
                    .unwrap()
                    .unwrap()
                    .value(),
            )
            .unwrap();
            row.schema.as_mut().unwrap().vector.as_mut().unwrap().sha256 = [0; 32];
            table
                .insert(
                    self.id.as_bytes().as_slice(),
                    row.encode().unwrap().as_slice(),
                )
                .unwrap();
        }
        tx.commit().unwrap();
        Ok(vec![1., 0., 0., 0.])
    }
}

#[tokio::test]
async fn selection_demand_actual_consumers_keep_semantic_and_advisory_corruption_checks() {
    for batch in [false, true] {
        for advisory in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let d = fixture(root.path());
            let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
            let s = open(&d, clock.clone());
            let f = Fixture::from_open_engine(tempfile::tempdir().unwrap(), s.engine);
            completion(&f).rebuild_all().await.unwrap();
            let p = Arc::new(CorruptCandidate {
                storage: f.storage.clone(),
                id: d.parent,
                profile: d.profile.clone(),
                calls: AtomicUsize::new(0),
            });
            let mut config = crate::config::RecalldConfig::default();
            config.graph.autolink_enabled = !advisory;
            let a = crate::mcp::bridge_adapters::McpStorageAdapter::new(
                f.storage.clone(),
                f.cache.clone(),
                p.clone(),
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
                f.graph.clone(),
                Arc::new(config),
                chrono_tz::UTC,
            )
            .with_clock(clock.clone());
            let mut input = single_input(&d, true);
            input.insert.memory.parent_id = None;
            input.check_duplicates = advisory;
            let before = f.storage.read().unwrap().control_state().unwrap();
            let calls = clock.0.load(Ordering::SeqCst);
            take_reads();
            if batch {
                let mut r = request(&f, "corrupt");
                r.items[0]["checkDuplicates"] = json!(advisory);
                let out = a.run_batch(r).await.unwrap();
                assert!(!out.all_ready(), "{out:?}");
                assert!(out.items[0].receipt.is_none(), "{out:?}");
                assert!(
                    format!("{out:?}").contains("native candidate vector hash mismatch"),
                    "{out:?}"
                );
            } else {
                let err = a
                    .store_native_single(
                        single_context(&f, &d, "corrupt"),
                        input,
                        NativePolicyInputs::default(),
                        None,
                    )
                    .await
                    .unwrap_err();
                assert!(
                    format!("{err:?}").contains("native candidate vector hash mismatch"),
                    "{err:?}"
                );
                assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
            }
            assert_eq!(take_reads(), (1, 1));
            assert_eq!(p.calls.load(Ordering::SeqCst), 1);
            assert_eq!(clock.0.load(Ordering::SeqCst), calls);
            assert_eq!(
                f.storage
                    .read()
                    .unwrap()
                    .meta_store()
                    .records_in_namespace(d.ns)
                    .unwrap()
                    .len(),
                1
            );
            drop(a);
            drop(p);
            drop(f);
            assert!(
                NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock)
                    .is_err()
            );
        }
    }
}
