use super::*;
use crate::mcp::bridge::StorageEngine as _;
use crate::storage::batch::{BatchInput, BatchKind, BatchStatus};
use crate::test_support::Fixture;
use serde_json::json;

#[path = "insertion_origin_tests.rs"]
mod insertion_origin_tests;

#[path = "selection_demand_tests.rs"]
mod selection_demand;

struct Provider {
    texts: std::sync::Mutex<Vec<String>>,
    profile: Option<crate::model::schema::EmbeddingProfile>,
    profiles: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    entered: Option<Arc<tokio::sync::Notify>>,
    release: Option<Arc<tokio::sync::Notify>>,
    corrupt: Option<Arc<tokio::sync::RwLock<crate::search::VectorIndexOwner>>>,
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
    async fn embed(&self, text: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.texts.lock().unwrap().push(text.to_owned());
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(entered) = &self.entered {
            entered.notify_one();
        }
        if let Some(release) = &self.release {
            release.notified().await;
        }
        if let Some(index) = &self.corrupt {
            *index.write().await = crate::search::VectorIndexOwner::uniform(8);
        }
        Ok(vec![2., 0., 0., 0.])
    }
}
fn provider(d: &Data) -> Arc<Provider> {
    Arc::new(Provider {
        texts: std::sync::Mutex::new(Vec::new()),
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
        request_context: None,
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
    let d = certified_fixture(root.path());
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
    let (out, trace) = {
        let mut pending = std::pin::pin!(a.run_batch(r.clone()));
        crate::performance::tests::capture_full(pending.as_mut()).await
    };
    let out = out.unwrap();
    crate::performance::tests::assert_no_legacy_batch_events(&trace);
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
    let d = certified_fixture(root.path());
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
    batch_publication_origin(2).await;
}
async fn batch_publication_origin(contract: u32) {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().corrupt = Some(f.vector_index.clone());
    let a = adapter(&f, p.clone(), clock, Default::default());
    let mut r = request(&f, "publish");
    r.request_context =
        crate::storage::request_context::RetainedRequestContext::from_contract(contract).unwrap();
    let out = a.run_batch(r.clone()).await.unwrap();
    assert_eq!(out.items[0].status, BatchStatus::Ok);
    assert!(!out.items[0].ready);
    let receipt = out.items[0].receipt.as_ref().unwrap();
    assert_ne!(receipt.operation_id, out.batch_operation_id);
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
    let before = f.storage.read().unwrap().control_state().unwrap();
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    let replay = a.run_batch(r).await.unwrap();
    assert!(replay.all_ready(), "{replay:?}");
    assert_eq!(replay.items[0].receipt, Some(expected_replay(receipt)));
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linked_producer_root_before_provider_completes_graph_descendants_while_closing() {
    batch_close_origin(2).await;
}
async fn batch_close_origin(contract: u32) {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
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
    let mut r = request(&f, "close");
    r.request_context =
        crate::storage::request_context::RetainedRequestContext::from_contract(contract).unwrap();
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
    batch_unknown_origin(2).await;
}
async fn batch_unknown_origin(contract: u32) {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let mut r = request(&f, "unknown");
    r.request_context =
        crate::storage::request_context::RetainedRequestContext::from_contract(contract).unwrap();
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
    let d = certified_fixture(root.path());
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

fn single_input(d: &Data, generated: bool) -> crate::storage::native_store::NativeStoreInput {
    let mut insert = input(d);
    if generated {
        insert.memory.embedding = None;
        insert.embedding_profile = None;
    }
    crate::storage::native_store::NativeStoreInput {
        insert,
        check_duplicates: true,
    }
}
fn single_context(f: &Fixture, d: &Data, key: &str) -> MutationContext {
    MutationContext {
        contract_version: 2,
        store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
        idempotency_key: Some(key.into()),
        ..MutationContext::unkeyed(d.ns)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_replay_preserves_receipt_without_provider_settings_clock_or_batch() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let key = single_context(&f, &d, "single");
    let input = single_input(&d, true);
    let before = f.storage.read().unwrap().control_state().unwrap();
    let out = a
        .store_native_single(
            key.clone(),
            input.clone(),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        out.receipt.committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .lookup_operation(
                out.receipt.store_id,
                MutationScope::Namespace(d.ns),
                out.receipt.operation_id
            )
            .unwrap(),
        Some(out.receipt.clone())
    );
    assert!(out.advisory_requested);
    assert!(out.advisory.read_generation.is_some());
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    let mut config = crate::config::RecalldConfig::default();
    config.graph.auto_link_threshold = f64::NAN;
    let mut unavailable = provider(&d);
    Arc::get_mut(&mut unavailable).unwrap().profile = None;
    let replay_adapter = adapter(&f, unavailable.clone(), clock.clone(), config);
    let calls = clock.0.load(Ordering::SeqCst);
    let state = f.storage.read().unwrap().control_state().unwrap();
    let replay = replay_adapter
        .store_native_single(
            key.clone(),
            input.clone(),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(replay.receipt, expected_replay(&out.receipt));
    assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), state);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        0
    );
    let mut changed = input;
    changed.check_duplicates = false;
    assert!(matches!(
        replay_adapter
            .store_native_single(key, changed, NativePolicyInputs::default(), None)
            .await,
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), state);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_publication_failure_carries_exact_receipt_and_reopens_without_provider() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().corrupt = Some(f.vector_index.clone());
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let key = single_context(&f, &d, "single-fault");
    let input = single_input(&d, true);
    let error = a
        .store_native_single(
            key.clone(),
            input.clone(),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .err()
        .unwrap();
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("{error:?}")
    };
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .lookup_operation(
                receipt.store_id,
                MutationScope::Namespace(d.ns),
                receipt.operation_id
            )
            .unwrap()
            .unwrap(),
        *receipt
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    drop(a);
    let root = f.close();
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let a = adapter(&f, p.clone(), clock, Default::default());
    let replay = a
        .store_native_single(key, input, NativePolicyInputs::default(), None)
        .await
        .unwrap();
    assert_eq!(replay.receipt, expected_replay(&receipt));
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
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
async fn native_single_close_waits_provider_then_graph_without_storage_inversion() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().entered = Some(entered.clone());
    Arc::get_mut(&mut p).unwrap().release = Some(release.clone());
    let a = adapter(&f, p, clock, Default::default());
    let key = single_context(&f, &d, "single-close");
    let input = single_input(&d, true);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let task = tokio::spawn(async move {
        a.store_native_single(key, input, NativePolicyInputs::default(), None)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
        .await
        .unwrap();
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
    assert!(f.storage.try_write().is_ok());
    drop(graph);
    task.await.unwrap().unwrap();
    let freeze = drain.await.unwrap().unwrap();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    drop(freeze);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_provider_race_rejects_stale_plan_before_clock_or_allocation() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().entered = Some(entered.clone());
    Arc::get_mut(&mut p).unwrap().release = Some(release.clone());
    let a = adapter(&f, p, clock.clone(), Default::default());
    let key = single_context(&f, &d, "stale-single");
    let input = single_input(&d, true);
    let task = tokio::spawn(async move {
        a.store_native_single(key, input, NativePolicyInputs::default(), None)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
        .await
        .unwrap();
    let other = adapter(&f, provider(&d), clock.clone(), Default::default());
    other
        .store_native_single(
            single_context(&f, &d, "other-single"),
            single_input(&d, false),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    let state = f.storage.read().unwrap().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    release.notify_one();
    assert!(matches!(
        task.await.unwrap(),
        Err(StorageError::LifecycleConflict(_))
    ));
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), state);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_original_vector_bits_and_checked_empty_identity() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let a = adapter(&f, p.clone(), clock, Default::default());
    let mut original = single_input(&d, false);
    original.insert.memory.embedding = Some(vec![0., 2., 0., 0.]);
    let key = single_context(&f, &d, "bits");
    let out = a
        .store_native_single(
            key.clone(),
            original.clone(),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert!(out.advisory_requested);
    assert!(out.advisory.report.is_none());
    assert!(out.advisory.read_generation.is_some());
    let state = f.storage.read().unwrap().control_state().unwrap();
    for values in [vec![0., 1., 0., 0.], vec![-0., 2., 0., 0.]] {
        let mut changed = original.clone();
        changed.insert.memory.embedding = Some(values);
        assert!(matches!(
            a.store_native_single(key.clone(), changed, NativePolicyInputs::default(), None)
                .await,
            Err(StorageError::IdempotencyConflict)
        ));
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), state);
    }
    let mut disabled = original;
    disabled.check_duplicates = false;
    let out = a
        .store_native_single(
            single_context(&f, &d, "disabled"),
            disabled,
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert!(!out.advisory_requested);
    assert!(out.advisory.read_generation.is_none());
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_process_child() {
    let Ok(path) = std::env::var("RECALLD_SINGLE_PATH") else {
        return;
    };
    let d = Data {
        path: path.into(),
        ns: NamespaceId::new(std::env::var("RECALLD_SINGLE_NS").unwrap().parse().unwrap()),
        parent: MemoryId::from_uuid(
            std::env::var("RECALLD_SINGLE_PARENT")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        profile: crate::embedding::runtime::tests::profile(),
    };
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(tempfile::tempdir().unwrap(), s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let a = adapter(&f, provider(&d), clock, Default::default());
    a.store_native_single(
        single_context(&f, &d, "process"),
        single_input(&d, true),
        NativePolicyInputs::default(),
        None,
    )
    .await
    .unwrap();
    panic!("process failed to exit at requested commit boundary");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_process_exit_reopens_one_receipt_and_frozen_links() {
    for point in [
        "before_metadata_commit",
        "after_metadata_commit",
        "after_edge_commit",
        "after_canonical_ack",
    ] {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let s = open(&d, clock.clone());
        let before = s.metadata().control_state().unwrap();
        drop(s);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "storage::metadata::native_records::tests::store_producers::native_single_process_child", "--nocapture"])
            .env("RECALLD_SINGLE_PATH", &d.path)
            .env("RECALLD_SINGLE_NS", d.ns.get().to_string())
            .env("RECALLD_SINGLE_PARENT", d.parent.to_string())
            .env("RECALLD_TEST_EFFECT_CRASH", point)
            .output().unwrap();
        assert_eq!(
            child.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        let s = open(&d, clock.clone());
        let f = Fixture::from_open_engine(root, s.engine);
        let mut p = provider(&d);
        if point != "before_metadata_commit" {
            Arc::get_mut(&mut p).unwrap().profile = None;
        }
        let a = adapter(&f, p.clone(), clock, Default::default());
        let key = single_context(&f, &d, "process");
        let first = a
            .store_native_single(
                key.clone(),
                single_input(&d, true),
                NativePolicyInputs::default(),
                None,
            )
            .await
            .unwrap();
        let replay = a
            .store_native_single(
                key,
                single_input(&d, true),
                NativePolicyInputs::default(),
                None,
            )
            .await
            .unwrap();
        assert_eq!(replay.receipt, expected_replay(&first.receipt));
        assert_eq!(
            first.receipt.committed_generation,
            before.committed_generation.next().unwrap()
        );
        assert_eq!(
            p.calls.load(Ordering::SeqCst),
            usize::from(point == "before_metadata_commit")
        );
        let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &first.receipt.outcome
        else {
            panic!()
        };
        assert!(!transition.edges.is_empty());
        assert_eq!(
            f.storage.read().unwrap().load_all_edges().unwrap().len(),
            transition.edges.len()
        );
        assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_unknown_commit_retains_identity_and_unresolved_work_until_reopen() {
    single_unknown_origin(2).await;
}
async fn single_unknown_origin(contract: u32) {
    struct UnknownClock;
    impl Clock for UnknownClock {
        fn now_millis(&self) -> i64 {
            crate::storage::metadata::effects::FAIL_AFTER_RECORD_COMMIT.with(|flag| flag.set(true));
            86_400_000
        }
    }
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock.clone(), Default::default())
        .with_clock(Arc::new(UnknownClock));
    let mut key = single_context(&f, &d, "unknown-single");
    key.contract_version = contract;
    let original = single_input(&d, true);
    let error = a
        .store_native_single(
            key.clone(),
            original.clone(),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .err()
        .unwrap();
    let StorageError::CommitOutcomeUnknown { operation_id, .. } = error else {
        panic!("{error:?}")
    };
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    assert!(tracker.status().unresolved > 0);
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    drop(a);
    let root = f.close();
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let a = adapter(&f, p.clone(), clock, Default::default());
    let result = a
        .store_native_single(key, original, NativePolicyInputs::default(), None)
        .await
        .unwrap();
    assert_eq!(result.receipt.operation_id, operation_id);
    assert!(result.receipt.replayed);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_scope_lineage_controls_do_not_replay_other_namespace_or_call_provider() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock, Default::default());
    let key = single_context(&f, &d, "same-key");
    let original = single_input(&d, false);
    a.store_native_single(
        key.clone(),
        original.clone(),
        NativePolicyInputs::default(),
        None,
    )
    .await
    .unwrap();
    let before = f.storage.read().unwrap().control_state().unwrap();
    for case in 0..3 {
        let mut bad = key.clone();
        match case {
            0 => bad.scope = MutationScope::Namespace(NamespaceId::new(u32::MAX)),
            1 => bad.scope = MutationScope::Catalog,
            _ => bad.store_id = Some(uuid::Uuid::new_v4()),
        }
        assert!(
            a.store_native_single(bad, original.clone(), NativePolicyInputs::default(), None)
                .await
                .is_err()
        );
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    }
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
}

// Run against the pre-profile-resolver runtime to retain a genuine historical
// descriptor-input receipt. The later resolver must replay these exact bytes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_profile_before_fixture() {
    let Ok(output) = std::env::var("RECALLD_WRITE_PROFILE_BEFORE") else {
        return;
    };
    let output = std::path::PathBuf::from(output);
    std::fs::create_dir_all(&output).unwrap();
    let root = tempfile::tempdir_in(&output).unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let a = adapter(&f, provider(&d), clock, Default::default());
    let out = a
        .store_native_single(
            single_context(&f, &d, "profile-before"),
            single_input(&d, false),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    let manifest = serde_json::json!({"path":d.path,"namespace":d.ns.get(),"parent":d.parent.to_string(),"profile":d.profile,"receipt":out.receipt});
    std::fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    drop(a);
    let _retained = f.close().keep();
}

fn profile_wire_input(d: &Data, generated: bool) -> crate::storage::native_store::NativeStoreInput {
    let mut input = single_input(d, generated);
    input.insert.embedding_profile = None;
    input
}
fn receipt_rows(f: &Fixture) -> Vec<(Vec<u8>, Vec<u8>)> {
    use redb::ReadableTable;
    let engine = f.storage.read().unwrap();
    let tx = engine.meta_store().db.begin_read().unwrap();
    tx.open_table(crate::storage::control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|row| {
            let (k, v) = row.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_profile_id_fresh_binding_replay_and_original_bits() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let mut key = single_context(&f, &d, "wire-profile");
    key.expected_generation = Some(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
    );
    let input = profile_wire_input(&d, false);
    let profile_id = d.profile.profile_id().unwrap();
    let out = a
        .store_native_single_by_profile_id(
            key.clone(),
            input.clone(),
            Some(profile_id.clone()),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &out.receipt.outcome else {
        panic!()
    };
    let engine = f.storage.read().unwrap();
    let row = engine.get_record(transition.memory_id).unwrap().unwrap();
    assert_eq!(
        engine.get_vector(d.ns, row.vector_slot).unwrap().unwrap(),
        vec![1., 0., 0., 0.]
    );
    drop(engine);
    let rows = receipt_rows(&f);
    let calls = clock.0.load(Ordering::SeqCst);
    let mut config = crate::config::RecalldConfig::default();
    config.graph.auto_link_threshold = f64::NAN;
    let replay_adapter = adapter(&f, p.clone(), clock.clone(), config);
    let replay = replay_adapter
        .store_native_single_by_profile_id(
            key.clone(),
            input.clone(),
            Some(profile_id.clone()),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(replay.receipt, expected_replay(&out.receipt));
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(receipt_rows(&f), rows);
    for case in 0..4 {
        let mut changed = input.clone();
        let mut changed_id = profile_id.clone();
        match case {
            0 => changed.insert.memory.embedding = Some(vec![1., 0., 0., 0.]),
            1 => changed.insert.memory.embedding = Some(vec![2., -0., 0., 0.]),
            2 => changed_id = format!("ep1:{}", "0".repeat(64)),
            _ => changed.check_duplicates = false,
        }
        assert!(matches!(
            replay_adapter
                .store_native_single_by_profile_id(
                    key.clone(),
                    changed,
                    Some(changed_id),
                    NativePolicyInputs::default(),
                    None
                )
                .await,
            Err(StorageError::IdempotencyConflict)
        ));
        assert_eq!(receipt_rows(&f), rows);
    }
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_profile_id_invalid_pair_or_binding_has_no_provider_or_mutation() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let state = f.storage.read().unwrap().control_state().unwrap();
    let rows = receipt_rows(&f);
    let id = d.profile.profile_id().unwrap();
    for case in 0..7 {
        let mut input = profile_wire_input(&d, false);
        let mut selector = Some(id.clone());
        match case {
            0 => selector = None,
            1 => input.insert.memory.embedding = None,
            2 => input.insert.embedding_profile = Some(d.profile.clone()),
            3 => selector = Some(format!("ep1:{}", "0".repeat(64))),
            4 => selector = Some("not-a-profile".into()),
            5 => input.insert.memory.embedding = Some(vec![0.; 4]),
            _ => input.insert.memory.embedding = Some(vec![1.; 3]),
        }
        let error = a
            .store_native_single_by_profile_id(
                single_context(&f, &d, "bad-profile"),
                input,
                selector,
                NativePolicyInputs::default(),
                None,
            )
            .await
            .err()
            .unwrap();
        if case == 3 {
            assert!(matches!(error, StorageError::EmbeddingProfileMismatch));
            assert_eq!(error.to_string(), "EMBEDDING_PROFILE_MISMATCH");
        } else if case >= 5 {
            assert!(
                matches!(error, StorageError::CallerVectorInvalid(_)),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(error, StorageError::InvalidMutation(_)),
                "{error:?}"
            );
        }
        assert_eq!(state, f.storage.read().unwrap().control_state().unwrap());
        assert_eq!(receipt_rows(&f), rows);
        assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 1);
    }
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    let generated = a
        .store_native_single_by_profile_id(
            single_context(&f, &d, "generated-profile"),
            profile_wire_input(&d, true),
            None,
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert!(!generated.receipt.replayed);
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_profile_id_replays_genuine_before_fixture_bytes() {
    let Ok(manifest) = std::env::var("RECALLD_READ_PROFILE_BEFORE") else {
        return;
    };
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("copied-native");
    fn copy_dir(source: &std::path::Path, target: &std::path::Path) {
        std::fs::create_dir(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let to = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &to);
            } else {
                std::fs::copy(entry.path(), to).unwrap();
            }
        }
    }
    copy_dir(
        std::path::Path::new(saved["path"].as_str().unwrap()),
        &target,
    );
    let d = Data {
        path: target,
        ns: NamespaceId::new(saved["namespace"].as_u64().unwrap() as u32),
        parent: MemoryId::from_uuid(saved["parent"].as_str().unwrap().parse().unwrap()),
        profile: serde_json::from_value(saved["profile"].clone()).unwrap(),
    };
    let old: WriteReceipt = serde_json::from_value(saved["receipt"].clone()).unwrap();
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let mut config = crate::config::RecalldConfig::default();
    config.graph.auto_link_threshold = f64::NAN;
    let a = adapter(&f, p.clone(), clock.clone(), config);
    let rows = receipt_rows(&f);
    let state = f.storage.read().unwrap().control_state().unwrap();
    let replay = a
        .store_native_single_by_profile_id(
            single_context(&f, &d, "profile-before"),
            profile_wire_input(&d, false),
            Some(d.profile.profile_id().unwrap()),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(replay.receipt, expected_replay(&old));
    assert_eq!(
        receipt_rows(&f),
        rows,
        "old receipt bytes must not be rewritten"
    );
    assert_eq!(state, f.storage.read().unwrap().control_state().unwrap());
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_profile_id_publication_error_preserves_exact_committed_receipt() {
    single_publication_origin(2).await;
}
async fn single_publication_origin(contract: u32) {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock, Default::default());
    let mut key = single_context(&f, &d, "profile-publication");
    key.contract_version = contract;
    let input = profile_wire_input(&d, false);
    let id = Some(d.profile.profile_id().unwrap());
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    let error = a
        .store_native_single_by_profile_id(
            key.clone(),
            input.clone(),
            id.clone(),
            NativePolicyInputs::default(),
            None,
        )
        .await
        .err()
        .unwrap();
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("{error:?}")
    };
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .lookup_operation(
                receipt.store_id,
                MutationScope::Namespace(d.ns),
                receipt.operation_id
            )
            .unwrap(),
        Some(*receipt.clone())
    );
    let rows = receipt_rows(&f);
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    let replay = a
        .store_native_single_by_profile_id(key, input, id, NativePolicyInputs::default(), None)
        .await
        .unwrap();
    assert_eq!(replay.receipt, expected_replay(&receipt));
    assert_eq!(receipt_rows(&f), rows);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_profile_id_actual_wire_preserves_identity_and_current_generation_readiness() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let a = adapter(&f, p.clone(), clock.clone(), Default::default());
    let mut r = request(&f, "batch-id-selector");
    r.expected_generation = Some(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
    );
    r.items = vec![
        json!({"summary":"profile ID supplied", "embedding":[2.,0.,0.,0.], "embeddingProfileId":d.profile.profile_id().unwrap()}),
    ];
    let original = r.items.clone();
    let prepared = crate::mcp::batches::prepare_native_store(&r).unwrap();
    let mut old = r.clone();
    old.mutation_version = 2;
    let historical = crate::mcp::batches::prepare(&old).unwrap();
    assert!(matches!(
        historical[0].plan,
        crate::storage::batch::BatchItemPlan::Invalid { .. }
    ));
    assert_eq!(
        prepared[0].plan.request_hash(),
        crate::storage::control::request_hash(
            &r.context(),
            "batch_item_intent_v3",
            &[&historical[0].plan.request_hash()]
        )
    );
    let out = a.mutate_native_batch(r.clone()).await.unwrap();
    assert!(out.outcome.all_ready(), "{out:?}");
    let receipt = out.outcome.items[0].receipt.as_ref().unwrap().clone();
    assert_eq!(r.items, original);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    let current = completion(&f)
        .observe_readiness(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
    assert!(current.derived.iter().all(|v| v.matches_canonical));
    let rows = receipt_rows(&f);
    let mut config = crate::config::RecalldConfig::default();
    config.graph.auto_link_threshold = f64::NAN;
    config.graph.max_auto_links = usize::MAX;
    let replay_adapter = adapter(&f, p.clone(), clock.clone(), config);
    let replay = replay_adapter.mutate_native_batch(r.clone()).await.unwrap();
    assert!(replay.outcome.all_ready(), "{replay:?}");
    let replay_receipt = replay.outcome.items[0].receipt.as_ref().unwrap();
    assert_eq!(replay_receipt.operation_id, receipt.operation_id);
    assert_eq!(replay_receipt.outcome, receipt.outcome);
    assert_eq!(receipt_rows(&f), rows);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    for replacement in [json!([4., 0., 0., 0.]), json!([2., -0.0, 0., 0.])] {
        let mut changed = r.clone();
        changed.items[0]["embedding"] = replacement;
        let err = replay_adapter
            .mutate_native_batch(changed)
            .await
            .unwrap_err();
        assert_eq!(err.error.code, "IDEMPOTENCY_CONFLICT");
        assert_eq!(receipt_rows(&f), rows);
    }
    let mut changed = r;
    changed.items[0]["embeddingProfileId"] = json!(format!("ep1:{}", "0".repeat(64)));
    assert_eq!(
        replay_adapter
            .mutate_native_batch(changed)
            .await
            .unwrap_err()
            .error
            .code,
        "IDEMPOTENCY_CONFLICT"
    );
    assert_eq!(receipt_rows(&f), rows);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_profile_id_mismatch_and_invalid_pair_are_retained_without_provider() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock, Default::default());
    let mut r = request(&f, "batch-id-errors");
    r.items = vec![
        json!({"summary":"mismatch", "embedding":[1.,0.,0.,0.],"embeddingProfileId":format!("ep1:{}", "0".repeat(64))}),
        json!({"summary":"null", "embedding":[1.,0.,0.,0.],"embeddingProfileId":null}),
        json!({"summary":"unpaired", "embeddingProfileId":d.profile.profile_id().unwrap()}),
    ];
    let out = a.mutate_native_batch(r.clone()).await.unwrap();
    assert_eq!(out.outcome.failed, 3);
    assert_eq!(
        out.outcome.items[0].error.as_ref().unwrap().code,
        "CONFLICT"
    );
    assert_eq!(
        out.outcome.items[0]
            .error
            .as_ref()
            .unwrap()
            .field
            .as_deref(),
        Some("embeddingProfileId")
    );
    let (status, envelope) = crate::serialization::native_batch::response(Ok(out.clone()));
    assert_eq!(status, 422);
    let value = serde_json::to_value(envelope).unwrap();
    assert_eq!(
        value["data"]["outcomes"][0]["error"]["code"],
        "EMBEDDING_PROFILE_MISMATCH"
    );
    let rows = receipt_rows(&f);
    let replay = a.mutate_native_batch(r).await.unwrap();
    assert_eq!(replay.outcome.items[0].error, out.outcome.items[0].error);
    assert_eq!(receipt_rows(&f), rows);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_profile_id_pending_reopen_uses_retained_binding_and_original_cas() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    let mut r = request(&f, "batch-id-pending");
    r.expected_generation = Some(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
    );
    r.items = vec![
        json!({"summary":"retained profile", "embedding":[2.,0.,0.,0.],"embeddingProfileId":d.profile.profile_id().unwrap()}),
    ];
    let plans = crate::mcp::batches::prepare_native_store(&r)
        .unwrap()
        .into_iter()
        .map(|p| p.plan)
        .collect::<Vec<_>>();
    let retained = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .reserve_native_store_batch_with_work(&r.context(), &plans, None, None, || {
            crate::graph::native_selection::freeze_settings(&Default::default())
        })
        .unwrap();
    let child = retained.child(0).unwrap();
    let resolved = match &plans[0] {
        crate::storage::batch::BatchItemPlan::NativeStore {
            input,
            supplied_profile_id,
            ..
        } => crate::storage::batch::native::resolve_store_profile(
            input,
            supplied_profile_id.as_deref(),
            &child.defaults,
        )
        .unwrap(),
        _ => panic!(),
    };
    assert_eq!(resolved.insert.embedding_profile, Some(d.profile.clone()));
    let root = f.close();
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let mut config = crate::config::RecalldConfig::default();
    config.graph.auto_link_threshold = f64::NAN;
    config.graph.max_auto_links = usize::MAX;
    let a = adapter(&f, p.clone(), clock, config);
    let out = a.mutate_native_batch(r).await.unwrap();
    assert!(out.outcome.all_ready(), "{out:?}");
    assert_eq!(
        out.outcome.batch_operation_id,
        retained.reservation_receipt.operation_id
    );
    assert_eq!(out.outcome.items[0].operation_id, Some(child.operation_id));
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    let current = completion(&f)
        .observe_readiness(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
    assert!(current.derived.iter().all(|v| v.matches_canonical));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_document_recipe_actual_single_provider_input() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock, Default::default());
    let input = document_recipe_input(&d);
    let result = a
        .store_native_single_by_profile_id(
            single_context(&f, &d, "document-single"),
            input,
            None,
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert!(matches!(
        result.receipt.outcome,
        MutationOutcome::NativeMemoryInsertedWithLinks { .. }
    ));
    assert_eq!(
        *p.texts.lock().unwrap(),
        vec!["header\n\nbody raw entity/alice raw entity/bob topic/rust emotion/calm"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_document_recipe_actual_batch_provider_input() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock, Default::default());
    let mut input = request(&f, "document-batch");
    input.items = vec![json!({"summary":"header", "fullText":"body",
        "tags":["raw", "entity/alice", "raw"], "entities":["Alice", "Bob"],
        "topics":["Rust"], "emotions":["Calm"]})];
    let result = a.run_batch(input).await.unwrap();
    assert!(result.all_ready(), "{result:?}");
    assert_eq!(
        *p.texts.lock().unwrap(),
        vec!["header\n\nbody raw entity/alice raw entity/bob topic/rust emotion/calm"]
    );
}

fn document_recipe_input(d: &Data) -> crate::storage::native_store::NativeStoreInput {
    let mut input = profile_wire_input(d, true);
    input.insert.memory.summary = "header".into();
    input.insert.memory.full_text = Some("body".into());
    input.insert.memory.tags = ["raw", "entity/alice", "raw"]
        .into_iter()
        .map(|v| crate::model::Tag::new(v).unwrap())
        .collect();
    input.insert.entities = vec!["Alice".into(), "Bob".into()];
    input.insert.topics = vec!["Rust".into()];
    input.insert.emotions = vec!["Calm".into()];
    input
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_document_recipe_export_old_fixture() {
    let Some(output) = std::env::var_os("RECALLD_DOCUMENT_FIXTURE_OUT") else {
        return;
    };
    let output = PathBuf::from(output);
    std::fs::create_dir_all(&output).unwrap();
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = adapter(&f, p.clone(), clock, Default::default());
    let single = a
        .store_native_single_by_profile_id(
            single_context(&f, &d, "document-single"),
            document_recipe_input(&d),
            None,
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    let mut batch = request(&f, "document-batch");
    batch.items = vec![json!({"summary":"header", "fullText":"body",
        "tags":["raw", "entity/alice", "raw"], "entities":["Alice", "Bob"],
        "topics":["Rust"], "emotions":["Calm"]})];
    let result = a.run_batch(batch).await.unwrap();
    assert!(result.all_ready(), "{result:?}");
    // This artifact documents the actual old bug; it is not certified recipe output.
    assert_eq!(*p.texts.lock().unwrap(), vec!["body", "body"]);
    let receipts = json!({"single":single.receipt, "batch":result,
        "namespaceId":d.ns, "parent":d.parent, "profile":d.profile});
    std::fs::write(
        output.join("receipts.json"),
        serde_json::to_vec(&receipts).unwrap(),
    )
    .unwrap();
    drop(a);
    let _root = f.close();
    crate::storage::logical_export::export_store(&d.path, &output.join("store.jsonl")).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_document_recipe_old_receipts_replay_without_provider_or_rewriting() {
    let root = tempfile::tempdir().unwrap();
    let export = root.path().join("old.jsonl");
    std::fs::write(
        &export,
        include_bytes!("fixtures/document-recipe-legacy.jsonl"),
    )
    .unwrap();
    let path = root.path().join("imported");
    crate::storage::logical_export::import_store(&export, &path).unwrap();
    let historical: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/document-recipe-legacy-receipts.json"
    ))
    .unwrap();
    let d = Data {
        path,
        ns: serde_json::from_value(historical["namespaceId"].clone()).unwrap(),
        parent: serde_json::from_value(historical["parent"].clone()).unwrap(),
        profile: serde_json::from_value(historical["profile"].clone()).unwrap(),
    };
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let mut config = crate::config::RecalldConfig::default();
    config.graph.auto_link_threshold = f64::NAN;
    let a = adapter(&f, p.clone(), clock.clone(), config);
    let before = f.storage.read().unwrap().control_state().unwrap();
    let rows = receipt_rows(&f);
    let original: WriteReceipt = serde_json::from_value(historical["single"].clone()).unwrap();
    let result = a
        .store_native_single_by_profile_id(
            single_context(&f, &d, "document-single"),
            document_recipe_input(&d),
            None,
            NativePolicyInputs::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.receipt, expected_replay(&original));
    let mut batch = request(&f, "document-batch");
    batch.items = vec![json!({"summary":"header", "fullText":"body",
        "tags":["raw", "entity/alice", "raw"], "entities":["Alice", "Bob"],
        "topics":["Rust"], "emotions":["Calm"]})];
    let result = a.run_batch(batch).await.unwrap();
    assert!(result.all_ready(), "{result:?}");
    let child: WriteReceipt =
        serde_json::from_value(historical["batch"]["items"][0]["receipt"].clone()).unwrap();
    assert_eq!(result.items[0].receipt, Some(expected_replay(&child)));
    assert_eq!(receipt_rows(&f), rows);
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert!(p.texts.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_document_recipe_offline_actual_api1_and_api2() {
    for api in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock);
        let p = provider(&d);
        let mut key = ctx(&session, &d, "offline-document");
        key.contract_version = api;
        let mut input = document_recipe_input(&d).insert;
        input.memory.full_text = None;
        input.entities.push("José".into());
        let receipt = session
            .insert_generated(&key, scope(&d), &input, p.as_ref())
            .await
            .unwrap();
        assert!(matches!(
            receipt.outcome,
            MutationOutcome::NativeMemoryInserted { .. }
        ));
        assert_eq!(
            *p.texts.lock().unwrap(),
            vec!["header raw entity/alice raw entity/bob entity/josé topic/rust emotion/calm"]
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_document_recipe_plain_batch_with_explicit_auxiliary_disable() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let mut config = crate::config::RecalldConfig::default();
    config.graph.autolink_enabled = false;
    let a = adapter(&f, p.clone(), clock, config);
    let mut input = request(&f, "plain-document-batch");
    input.mutation_version = 2;
    input.items = vec![json!({"summary":"header", "fullText":"body",
        "tags":["raw", "entity/alice", "raw"], "entities":["Alice", "Bob"],
        "topics":["Rust"], "emotions":["Calm"], "checkDuplicates":false})];
    let result = a.run_batch(input).await.unwrap();
    assert!(result.all_ready(), "{result:?}");
    assert_eq!(
        *p.texts.lock().unwrap(),
        vec!["header\n\nbody raw entity/alice raw entity/bob topic/rust emotion/calm"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_single_wire_actual_unknown_commit_keeps_original_operation_on_reopen() {
    struct UnknownClock;
    impl Clock for UnknownClock {
        fn now_millis(&self) -> i64 {
            crate::storage::metadata::effects::FAIL_AFTER_RECORD_COMMIT.with(|flag| flag.set(true));
            86_400_000
        }
    }
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let p = provider(&d);
    let a = Arc::new(
        adapter(&f, p.clone(), clock.clone(), Default::default())
            .with_clock(Arc::new(UnknownClock)),
    );
    let body = serde_json::json!({"namespaceId":d.ns,"storeId":f.storage.read().unwrap().control_state().unwrap().store_id,"idempotencyKey":"wire-unknown-semantic","summary":"actual unknown wire outcome","checkDuplicates":false});
    let request =
        crate::serialization::native_single::Request::parse(&body, true, None, "native").unwrap();
    let rpc = crate::daemon::server::native_method_test_rpc(
        a.clone(),
        serde_json::to_value(&request).unwrap(),
        "store_native_single_observed_v1",
    )
    .await;
    assert!(rpc.error.is_none(), "{rpc:?}");
    let reply: crate::serialization::native_single::Reply =
        serde_json::from_value(rpc.result.unwrap()).unwrap();
    assert!(reply.receipt.is_none());
    let error = reply.error.unwrap();
    assert_eq!(error.code, "COMMIT_OUTCOME_UNKNOWN");
    assert_eq!(
        error.commit_state,
        crate::serialization::contract::CommitState::Unknown
    );
    let operation = error.operation_id.unwrap();
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
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    drop(a);
    let root = f.close();
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    let mut p = provider(&d);
    Arc::get_mut(&mut p).unwrap().profile = None;
    let a = Arc::new(adapter(&f, p.clone(), clock.clone(), Default::default()));
    let rpc = crate::daemon::server::native_method_test_rpc(
        a,
        serde_json::to_value(request).unwrap(),
        "store_native_single_observed_v1",
    )
    .await;
    assert!(rpc.error.is_none(), "{rpc:?}");
    let reply: crate::serialization::native_single::Reply =
        serde_json::from_value(rpc.result.unwrap()).unwrap();
    assert!(reply.error.is_none(), "{:?}", reply.error);
    let mut receipt = reply.receipt.unwrap();
    assert!(receipt.replayed);
    assert_eq!(receipt.operation_id, operation);
    receipt.replayed = false;
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .lookup_operation(receipt.store_id, MutationScope::Namespace(d.ns), operation)
            .unwrap(),
        Some(receipt)
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
}

#[path = "owned_ids_producer_tests.rs"]
mod owned_ids;
