use super::*;
use crate::mcp::bridge::StorageEngine as _;
use crate::storage::batch::{BatchInput, BatchKind, BatchStatus};
use crate::test_support::Fixture;
use serde_json::json;

struct Provider {
    profile: Option<crate::model::schema::EmbeddingProfile>,
    profiles: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    entered: Option<Arc<tokio::sync::Notify>>,
    release: Option<Arc<tokio::sync::Notify>>,
    corrupt: Option<Arc<tokio::sync::RwLock<crate::search::FlatVectorIndex>>>,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for Provider {
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        self.profiles.fetch_add(1, Ordering::SeqCst);
        self.profile.clone()
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "synthetic-linked-batch"
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(entered) = &self.entered {
            entered.notify_one();
        }
        if let Some(release) = &self.release {
            release.notified().await;
        }
        if let Some(index) = &self.corrupt {
            *index.write().await = crate::search::FlatVectorIndex::new(8);
        }
        Ok(vec![2., 0., 0., 0.])
    }
}
fn provider(d: &Data) -> Arc<Provider> {
    Arc::new(Provider {
        profile: Some(d.profile.clone()),
        profiles: Arc::new(AtomicUsize::new(0)),
        calls: Arc::new(AtomicUsize::new(0)),
        entered: None,
        release: None,
        corrupt: None,
    })
}
fn request(f: &Fixture, key: &str) -> BatchInput {
    BatchInput {
        expected_generation: None,
        mutation_version: 3,
        result_version: 1,
        kind: BatchKind::Insert,
        items: vec![json!({"summary":"native producer","entities":["José"]})],
        default_namespace: "native".into(),
        store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
        idempotency_key: Some(key.into()),
        batch_operation_id: None,
    }
}
fn adapter(
    f: &Fixture,
    p: Arc<Provider>,
    clock: Arc<FixedClock>,
    config: crate::config::RecalldConfig,
) -> crate::mcp::bridge_adapters::McpStorageAdapter {
    crate::mcp::bridge_adapters::McpStorageAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        p,
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
        Arc::new(config),
        chrono_tz::UTC,
    )
    .with_clock(clock)
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
fn expected_replay(receipt: &WriteReceipt) -> WriteReceipt {
    let mut expected = receipt.clone();
    expected.replayed = true;
    expected
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linked_producer_completed_replay_skips_settings_provider_and_rediscovery() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let r = request(&f, "producer");
    assert!(
        matches!(
            a.mutate_batch(r.clone()).await,
            Err(crate::mcp::bridge::BridgeError::Unavailable(_))
        ),
        "public version3 remains gated"
    );
    let out = a.run_batch(r.clone()).await.unwrap();
    assert!(out.all_ready(), "{out:?}");
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    let receipt = out.items[0].receipt.as_ref().unwrap().clone();
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
        panic!()
    };
    assert!(!transition.edges.is_empty());
    assert_eq!(transition.advisory_matches(), Some(vec![(d.parent, 1.)]));
    assert_eq!(
        out.items[0].result.as_ref().unwrap()["advisory"]["requested"],
        true
    );
    assert!(out.items[0].result.as_ref().unwrap()["advisory"]["readGeneration"].is_object());
    {
        let mut engine = f.storage.write().unwrap();
        let policy = NativePolicyInputs::default();
        let intent = engine
            .meta_store()
            .capture_native_tombstone(d.parent, Some(d.ns), &policy)
            .unwrap();
        let key = MutationContext {
            contract_version: 2,
            store_id: Some(engine.meta_store().control_state().unwrap().store_id),
            idempotency_key: Some("remove-advisory".into()),
            ..MutationContext::unkeyed(d.ns)
        };
        engine
            .tombstone_native_record(&key, &intent, &policy, clock.as_ref(), None)
            .unwrap();
    }
    let mut unavailable = provider(&d);
    Arc::get_mut(&mut unavailable).unwrap().profile = None;
    let mut config = crate::config::RecalldConfig::default();
    config.graph.autolink_enabled = false;
    config.graph.max_auto_links = 0;
    config.graph.max_entity_links = 0;
    config.graph.temporal_window_ms = 0;
    config.graph.auto_link_threshold = f64::NAN;
    let replay_adapter = adapter(&f, unavailable.clone(), clock, config);
    let before = f.storage.read().unwrap().control_state().unwrap();
    let replay = replay_adapter.run_batch(r.clone()).await.unwrap();
    assert!(replay.all_ready(), "{replay:?}");
    assert_eq!(replay.items[0].receipt, Some(expected_replay(&receipt)));
    assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
    let result = replay.items[0].result.as_ref().unwrap();
    assert!(result.get("nearDuplicates").is_none());
    assert_eq!(result["advisory"]["requested"], true);
    assert_eq!(
        result["advisory"]["warnings"],
        json!(["ADVISORY_CONTENT_UNAVAILABLE"])
    );
    assert_eq!(
        result["advisory"]["readGeneration"],
        serde_json::to_value(before.generation_ref()).unwrap()
    );
    let mut changed = r;
    changed.items[0]["checkDuplicates"] = json!(false);
    assert!(replay_adapter.run_batch(changed).await.is_err());
    assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linked_producer_checked_empty_and_disabled_have_distinct_outcomes_and_read_generation() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock, Default::default());
    let mut r = request(&f, "empty");
    r.items[0]["embedding"] = json!([0., 1., 0., 0.]);
    r.items[0]["embeddingProfile"] = json!(d.profile);
    let out = a.run_batch(r.clone()).await.unwrap();
    assert!(out.all_ready(), "{out:?}");
    let result = out.items[0].result.as_ref().unwrap();
    assert!(result.get("nearDuplicates").is_none());
    assert_eq!(result["advisory"]["requested"], true);
    assert!(result["advisory"]["readGeneration"].is_object());
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } =
        &out.items[0].receipt.as_ref().unwrap().outcome
    else {
        panic!()
    };
    assert_eq!(transition.advisory, Some(vec![]));
    r.idempotency_key = Some("disabled".into());
    r.items[0]["checkDuplicates"] = json!(false);
    let out = a.run_batch(r).await.unwrap();
    assert!(out.all_ready(), "{out:?}");
    let result = out.items[0].result.as_ref().unwrap();
    assert_eq!(result["advisory"]["requested"], false);
    assert!(result["advisory"]["readGeneration"].is_null());
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } =
        &out.items[0].receipt.as_ref().unwrap().outcome
    else {
        panic!()
    };
    assert!(transition.advisory.is_none());
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linked_producer_publication_failure_keeps_actual_child_receipt_then_replays_once() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().corrupt = Some(f.vector_index.clone());
    let a = adapter(&f, p.clone(), clock, Default::default());
    let r = request(&f, "publish");
    let out = a.run_batch(r.clone()).await.unwrap();
    assert_eq!(out.items[0].status, BatchStatus::Ok);
    assert!(!out.items[0].ready);
    let receipt = out.items[0].receipt.as_ref().unwrap();
    assert_ne!(receipt.operation_id, out.batch_operation_id);
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
    let before = f.storage.read().unwrap().control_state().unwrap();
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    let replay = a.run_batch(r).await.unwrap();
    assert!(replay.all_ready(), "{replay:?}");
    assert_eq!(replay.items[0].receipt, Some(expected_replay(receipt)));
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linked_producer_root_before_provider_completes_graph_descendants_while_closing() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let mut p = provider(&d);
    let provider = Arc::get_mut(&mut p).unwrap();
    provider.entered = Some(entered.clone());
    provider.release = Some(release.clone());
    let a = adapter(&f, p, clock, Default::default());
    let r = request(&f, "close");
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let task = tokio::spawn(async move { a.run_batch(r).await });
    tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
        .await
        .unwrap();
    assert!(tracker.status().pending > 0);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        1
    );
    let graph = f.graph.clone().write_owned().await;
    let gate = crate::derived_completion::publication_gate(&f.storage)
        .await
        .unwrap();
    let closing = tracker.clone();
    let drain = tokio::task::spawn_blocking(move || {
        closing.close_and_drain(std::time::Instant::now() + std::time::Duration::from_secs(15))
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while tracker.status().admission_open {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while gate.try_write().is_ok() {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    assert!(
        f.storage.try_write().is_ok(),
        "graph wait must not hold storage"
    );
    drop(graph);
    let out = task.await.unwrap().unwrap();
    assert!(out.all_ready(), "{out:?}");
    let freeze = drain.await.unwrap().unwrap();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        0
    );
    drop(freeze);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linked_producer_unknown_child_reopens_exact_reserved_operation_without_provider() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let r = request(&f, "unknown");
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .fail_batch_commit
        .store(3, Ordering::SeqCst);
    let out = a.run_batch(r.clone()).await.unwrap();
    assert_eq!(out.items[0].status, BatchStatus::Unknown);
    assert!(out.items[0].receipt.is_none());
    let child = out.items[0].operation_id.unwrap();
    assert_ne!(child, out.batch_operation_id);
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    drop(a);
    let root = f.close();
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let replay = a.run_batch(r).await.unwrap();
    assert!(replay.all_ready(), "{replay:?}");
    let receipt = replay.items[0].receipt.as_ref().unwrap();
    assert_eq!(receipt.operation_id, child);
    assert_eq!(
        receipt.committed_generation,
        out.reservation_receipt
            .unwrap()
            .committed_generation
            .next()
            .unwrap()
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linked_producer_unavailable_provider_leaves_only_reservation_and_resumes_frozen_settings()
{
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let r = request(&f, "provider-pending");
    let before = f.storage.read().unwrap().control_state().unwrap();
    let out = a.run_batch(r.clone()).await.unwrap();
    assert_eq!(out.items[0].status, BatchStatus::NotAttempted);
    assert!(out.items[0].receipt.is_none());
    assert_eq!(out.items[0].error.as_ref().unwrap().code, "UNAVAILABLE");
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 1);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        before.committed_generation.next().unwrap()
    );
    let mut config = crate::config::RecalldConfig::default();
    config.graph.autolink_enabled = false;
    config.graph.auto_link_threshold = f64::NAN;
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock, config);
    let completed = a.run_batch(r).await.unwrap();
    assert!(completed.all_ready(), "{completed:?}");
    assert_eq!(completed.batch_operation_id, out.batch_operation_id);
    assert_eq!(completed.items[0].operation_id, out.items[0].operation_id);
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } =
        &completed.items[0].receipt.as_ref().unwrap().outcome
    else {
        panic!()
    };
    assert!(transition.settings.autolink_enabled);
    assert!(!transition.edges.is_empty());
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
}
