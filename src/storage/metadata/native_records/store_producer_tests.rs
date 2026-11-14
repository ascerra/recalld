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
    let d = fixture(root.path());
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
    let d = fixture(root.path());
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
    let d = fixture(root.path());
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
    let d = fixture(root.path());
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
    let d = fixture(root.path());
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
        let d = fixture(root.path());
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
    let a = adapter(&f, p.clone(), clock.clone(), Default::default())
        .with_clock(Arc::new(UnknownClock));
    let key = single_context(&f, &d, "unknown-single");
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
    let d = fixture(root.path());
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
