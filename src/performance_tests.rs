use super::*;
use crate::model::{CachedRecord, MemoryId, NamespaceId};
use crate::search::{adapters::*, *};
use crate::test_support::{Fixture, MemorySpec};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tracing::instrument::WithSubscriber;

#[derive(Clone, Default)]
pub(crate) struct Capture(pub(crate) Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}
pub(crate) fn subscriber(capture: Capture) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::fmt()
        .json()
        .without_time()
        .with_env_filter("off,recalld::performance=trace")
        .with_writer(capture)
        .finish()
}
fn events(capture: &Capture) -> Vec<serde_json::Value> {
    String::from_utf8(capture.0.lock().unwrap().clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()["fields"].clone())
        .collect()
}
struct NoCache;
impl RecordCache for NoCache {
    fn get(&self, _: &MemoryId) -> Option<CachedRecord> {
        panic!("read-only cache lookup")
    }
}
struct NoRif;
impl RifProcessor for NoRif {
    fn compute_suppressions(&self, _: &[MemoryId], _: &[MemoryId]) -> Vec<RifSuppression> {
        vec![]
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn performance_trace_reconciles_real_reads_and_never_changes_false_mode_state() {
    use crate::storage::StorageEngine;
    let fx = Fixture::new();
    let ns = NamespaceId::new(1);
    let other = fx.create_namespace("private-namespace");
    let mut target_bytes = 0;
    let mut ids = Vec::new();
    for i in 0..4 {
        let namespace = if i == 3 { other } else { ns };
        let id = fx
            .insert_memory_with(MemorySpec {
                namespace,
                summary: "private-payload-needle",
                tags: if i == 2 {
                    &["project/reject"]
                } else {
                    &["project/allowed"]
                },
                embedding: Some(&[1.0, 0.0, 0.0, 0.0]),
                ..MemorySpec::new(ns)
            })
            .await;
        ids.push(id);
        let record = fx.record(id).unwrap();
        if namespace == ns {
            target_bytes += record.encode().unwrap().len();
        }
        fx.vector_index
            .write()
            .await
            .add(
                id,
                &[1.0, 0.0, 0.0, 0.0],
                VectorMetadata {
                    namespace_id: namespace,
                    decay_phase: 1,
                    tags: vec![],
                },
            )
            .unwrap();
        fx.fts_index
            .lock()
            .await
            .add(namespace, id, &record.summary, None, &[])
            .unwrap();
    }
    fx.graph
        .write()
        .await
        .add_edge(
            ids[0],
            ids[1],
            crate::model::EdgeType::Associative,
            1.0,
            false,
        )
        .unwrap();
    let clock = Arc::new(crate::time::ManualClock::new(100));
    let engine = QueryEngine::new(
        Arc::new(StorageNamespaceResolver::new(fx.storage.clone())),
        Arc::new(SingleEmbeddingRegistry::new(Arc::new(
            crate::embedding::PassthroughProvider::new(4),
        ))),
        Arc::new(SharedVectorIndexRegistry::new(fx.vector_index.clone())),
        Arc::new(SharedFtsIndexRegistry::new(fx.fts_index.clone())),
        Arc::new(SharedEntityIndexReader::new(fx.entity_index.clone())),
        Arc::new(NoCache),
        Arc::new(StorageMetadataAdapter::new(fx.storage.clone())),
        Arc::new(SharedGraphReader::new(fx.graph.clone())),
        Arc::new(NoRif),
        Arc::new(
            StorageAccessRecorder::new(fx.storage.clone(), fx.cache.clone())
                .with_clock(clock.clone()),
        ),
    )
    .with_lifecycle_inputs(clock, Default::default(), 1.0);
    let query: SearchQuery = serde_json::from_value(serde_json::json!({
        "text":"private-payload-needle", "queryEmbedding":[1.0,0.0,0.0,0.0],
        "namespace":"default", "limit":2, "recordAccess":false, "graphDepth":1,
        "filter":{"requireTags":["project/allowed"]}
    }))
    .unwrap();
    fx.storage.write().unwrap().sync().unwrap();
    // Read live canonical bytes directly. Offline validation deliberately refuses
    // a copied redb header while its writer is still open.
    let files = [
        fx.path().join("meta.db"),
        fx.path().join("fulltext.dat"),
        fx.namespace_dir("default").join("vectors.dat"),
        fx.namespace_dir("private-namespace").join("vectors.dat"),
    ];
    let before: Vec<_> = files.iter().map(|p| std::fs::read(p).unwrap()).collect();
    let off = engine
        .search_with_semantics(query.clone(), RetrievalSemantics::V2(VersionMode::History))
        .with_subscriber(tracing::subscriber::NoSubscriber::default())
        .await
        .unwrap();
    let capture = Capture::default();
    let on = engine
        .search_with_semantics(query.clone(), RetrievalSemantics::V2(VersionMode::History))
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&off.response.results).unwrap(),
        serde_json::to_value(&on.response.results).unwrap()
    );
    assert_eq!(
        files
            .iter()
            .map(|p| std::fs::read(p).unwrap())
            .collect::<Vec<_>>(),
        before
    );
    let trace = events(&capture);
    let one = |kind: &str| trace.iter().find(|e| e["kind"] == kind).unwrap();
    assert_eq!(one("canonical_read")["examined"], 3);
    assert_eq!(one("canonical_read")["encoded_record_bytes"], target_bytes);
    assert_eq!(one("eligibility")["eligible"], 2);
    assert_eq!(one("eligibility")["rejected"], 1);
    assert_eq!(one("vector_scan")["examined"], 4);
    assert_eq!(one("vector_scan")["dot_products"], 2);
    assert_eq!(one("fts_rows")["examined"], 3);
    assert_eq!(one("fts_rows")["eligible"], 2);
    assert_eq!(one("graph_work")["examined_edges"], 2);
    assert_eq!(
        one("pipeline_accounting")["elapsed_ns"],
        one("pipeline_accounting")["accounted_ns"]
    );
    for stage in [
        "storage.read.queue",
        "storage.read.wait",
        "storage.read.hold",
        "vector.lock_wait",
        "fts.lock_hold",
    ] {
        assert!(trace.iter().any(|e| e["stage"] == stage), "{stage}");
    }
    let raw = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    for secret in [
        "private-payload-needle",
        "private-namespace",
        "project/allowed",
    ] {
        assert!(!raw.contains(secret));
    }
    for (id, _) in fx.storage.read().unwrap().scan_all().unwrap() {
        assert!(!raw.contains(&id.to_string()));
    }

    // Actual engine and adapters nested under the shared transport wrapper:
    // selection, canonical bytes and redaction hold for sampled AND skipped roots.
    let sampled = Capture::default();
    let sampler = Sampler::new(NonZeroU64::new(2));
    for ordinal in 1..=8 {
        let result = measure_with_sampler(
            "mcp.search",
            false,
            engine
                .search_with_semantics(query.clone(), RetrievalSemantics::V2(VersionMode::History)),
            Some(&sampler),
        )
        .with_subscriber(subscriber(sampled.clone()))
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(&off.response.results).unwrap(),
            serde_json::to_value(&result.response.results).unwrap()
        );
        assert_eq!(sampler.roots.load(Ordering::Relaxed), ordinal);
    }
    assert_eq!(
        files
            .iter()
            .map(|p| std::fs::read(p).unwrap())
            .collect::<Vec<_>>(),
        before
    );
    let sampled_events = events(&sampled);
    let count = (1..=8).filter(|n| selected(*n, 2)).count();
    for kind in [
        "canonical_read",
        "eligibility",
        "vector_scan",
        "fts_rows",
        "graph_work",
        "pipeline_accounting",
    ] {
        assert_eq!(
            sampled_events.iter().filter(|e| e["kind"] == kind).count(),
            count,
            "{kind}"
        );
    }
    let sampled_raw = String::from_utf8(sampled.0.lock().unwrap().clone()).unwrap();
    for secret in [
        "private-payload-needle",
        "private-namespace",
        "project/allowed",
    ] {
        assert!(!sampled_raw.contains(secret));
    }
    for id in &ids {
        assert!(!sampled_raw.contains(&id.to_string()));
    }

    // Same real writer path emits commit/publication costs when explicitly true.
    let mut write_query = query;
    write_query.record_access = true;
    engine
        .search(write_query)
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    let trace = events(&capture);
    assert!(
        trace
            .iter()
            .any(|e| e["kind"] == "exposure" && e["receipt_count"] == 1)
    );
    assert!(
        trace
            .iter()
            .any(|e| e["stage"] == "exposure.cache_publication")
    );
}

#[tokio::test]
async fn performance_errors_cancellation_and_disabled_collection_are_explicit() {
    let capture = Capture::default();
    async {
        measure("test.outer", false, async {
            measure::<(), ()>("test.inner", false, async { Ok(()) }).await
        })
        .await
        .unwrap();
        let error: Result<(), ()> = measure("test.error", false, async { Err(()) }).await;
        assert!(error.is_err());
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(1),
                measure::<(), ()>("test.cancel", false, std::future::pending())
            )
            .await
            .is_err()
        );
    }
    .with_subscriber(subscriber(capture.clone()))
    .await;
    let raw: Vec<serde_json::Value> = String::from_utf8(capture.0.lock().unwrap().clone())
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let inner = raw
        .iter()
        .find(|e| e["fields"]["stage"] == "test.inner")
        .unwrap();
    let spans = inner["spans"].as_array().unwrap();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0]["operation"], "test.outer");
    assert_eq!(spans[1]["operation"], "test.inner");
    assert_ne!(spans[0]["trace_id"], spans[1]["trace_id"]);
    let trace = events(&capture);
    assert!(
        trace
            .iter()
            .any(|e| e["stage"] == "test.error" && e["outcome"] == "error")
    );
    assert!(
        trace
            .iter()
            .any(|e| e["stage"] == "test.cancel" && e["outcome"] == "cancelled_error_or_unwound")
    );
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        assert!(!enabled());
        assert!(Timer::new("disabled").start.is_none());
        assert!(Timeline::new().start.is_none());
        assert!(Worker::new("disabled").context.is_none());
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn performance_sink_io_failure_preserves_actual_committed_not_ready_receipts() {
    struct FailedSink;
    impl Write for FailedSink {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("fixture sink failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let fx = Fixture::new();
    let id = fx.insert_memory().await;
    let receipts = crate::decay::observation::record_exposures(
        fx.storage.clone(),
        fx.cache.clone(),
        vec![(id, crate::model::AccessKind::DirectRetrieval)],
        100,
    )
    .await
    .unwrap();
    assert_eq!(receipts.len(), 1);
    let operation = receipts[0].operation_id;
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("off,recalld::performance=trace")
        .with_writer(|| FailedSink)
        .finish();
    let error = tracing::subscriber::with_default(subscriber, || {
        crate::decay::observation::Observed {
            value: (),
            receipts,
        }
        .finish(Err::<(), _>(SearchError::Unavailable(
            "fixture final admission failure".into(),
        )))
        .unwrap_err()
    });
    let SearchError::Observation(error) = error else {
        panic!("lost committed classification")
    };
    assert_eq!(error.commit_state, "committed");
    assert!(!error.ready);
    assert_eq!(error.receipts[0].operation_id, operation);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampling_pins_one_root_decision_across_nested_async_and_blocking_work() {
    let sampler = Sampler::new(NonZeroU64::new(7));
    let capture = Capture::default();
    let mut expected = Vec::new();
    async {
        for ordinal in 1..=100 {
            let select = selected(ordinal, 7);
            if select {
                expected.push(ordinal);
            }
            measure_with_sampler(
                "rest.search",
                false,
                async {
                    assert_eq!(enabled(), select);
                    tokio::task::yield_now().await;
                    measure("engine.search", false, async {
                        assert_eq!(enabled(), select);
                        let worker = Worker::new("test.queue");
                        assert_eq!(worker.context.is_some(), select);
                        assert_eq!(worker.queued.start.is_some(), select);
                        tokio::task::spawn_blocking(move || {
                            worker.run(|| {
                                assert_eq!(enabled(), select);
                                let mut timeline = Timeline::new();
                                assert_eq!(timeline.start.is_some(), select);
                                timeline.mark("test.stage");
                                crate::performance::event!(kind = "test_counter", count = 1);
                            })
                        })
                        .await
                        .unwrap();
                        Ok::<_, ()>(())
                    })
                    .await
                },
                Some(&sampler),
            )
            .await
            .unwrap();
        }
    }
    .with_subscriber(subscriber(capture.clone()))
    .await;
    assert!(!expected.is_empty() && expected.len() < 100);
    assert_eq!(sampler.roots.load(Ordering::Relaxed), 100);
    assert_eq!(
        sampler.selected.load(Ordering::Relaxed),
        expected.len() as u64
    );
    let mut roots = Vec::new();
    for line in String::from_utf8(capture.0.lock().unwrap().clone())
        .unwrap()
        .lines()
    {
        let event: serde_json::Value = serde_json::from_str(line).unwrap();
        let spans = event["spans"].as_array().unwrap();
        let root = &spans[0];
        let ordinal = root["root_ordinal"].as_u64().unwrap();
        assert!(expected.contains(&ordinal));
        assert_eq!(
            root["sampled_roots_seen"],
            (expected.iter().position(|n| *n == ordinal).unwrap() + 1) as u64
        );
        for span in spans {
            assert_eq!(span["root_ordinal"], ordinal);
            assert_eq!(span["sample_every"], 7);
            assert_eq!(span["event_version"], 2);
        }
        if event["fields"]["stage"] == "rest.search" {
            roots.push(ordinal);
        } else {
            assert_eq!(spans.len(), 2, "worker lost transport/engine ancestry");
            assert_ne!(spans[0]["trace_id"], spans[1]["trace_id"]);
        }
    }
    assert_eq!(roots, expected);
}

#[tokio::test]
async fn sampling_preserves_errors_cancellation_and_restores_outside_context() {
    let sampler = Sampler::new(NonZeroU64::new(2));
    let capture = Capture::default();
    async {
        for ordinal in 1..=8 {
            let error: Result<(), &'static str> = measure_with_sampler(
                "mcp.search",
                false,
                async { Err("private-error-payload") },
                Some(&sampler),
            )
            .await;
            assert_eq!(error, Err("private-error-payload"));
            assert!(REQUEST.try_with(|_| ()).is_err());
            assert!(enabled()); // full tracing outside a request stays compatible
            assert_eq!(sampler.roots.load(Ordering::Relaxed), ordinal);
        }
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(1),
                measure_with_sampler::<(), ()>(
                    "test.cancel",
                    false,
                    std::future::pending(),
                    Some(&sampler)
                )
            )
            .await
            .is_err()
        );
        assert!(REQUEST.try_with(|_| ()).is_err());
    }
    .with_subscriber(subscriber(capture.clone()))
    .await;
    let raw = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(!raw.contains("private-error-payload"));
    let trace = events(&capture);
    let expected_errors = (1..=8).filter(|n| selected(*n, 2)).count();
    assert_eq!(
        trace.iter().filter(|v| v["outcome"] == "error").count(),
        expected_errors
    );
    assert_eq!(
        trace
            .iter()
            .filter(|v| v["outcome"] == "cancelled_error_or_unwound")
            .count(),
        usize::from(selected(9, 2))
    );
}

#[test]
fn sampling_rate_validation_never_silently_enables_full_trace() {
    assert_eq!(parse_rate(None).unwrap().get(), 1);
    assert_eq!(parse_rate(Some("100")).unwrap().get(), 100);
    for invalid in ["", "0", "-1", "1.5", " 10", "18446744073709551616"] {
        assert!(parse_rate(Some(invalid)).is_none());
    }
    let invalid = Sampler::new(None);
    assert!(!invalid.decide().selected);
    let full = Sampler::new(NonZeroU64::new(1));
    assert!((0..100).all(|_| full.decide().selected));
    // Known deterministic sequence, independent of trace span IDs and nesting.
    assert_eq!(
        (1..=16).filter(|n| selected(*n, 7)).collect::<Vec<_>>(),
        vec![15]
    );
}

// Counts only allocations made on the checking thread while enabled. This
// allocator is test-only; normal parallel test threads are unaffected.
mod allocation_probe {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! { static COUNT: Cell<Option<usize>> = const { Cell::new(None) }; }
    struct Counting;
    #[global_allocator]
    static ALLOCATOR: Counting = Counting;
    fn allocated() {
        let _ = COUNT.try_with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
    }
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            allocated();
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
            unsafe { System.dealloc(p, layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            allocated();
            unsafe { System.alloc_zeroed(layout) }
        }
        unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            allocated();
            unsafe { System.realloc(p, layout, size) }
        }
    }
    pub fn count(f: impl FnOnce()) -> usize {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                COUNT.with(|c| c.set(None));
            }
        }
        COUNT.with(|c| c.set(Some(0)));
        let reset = Reset;
        f();
        let n = COUNT.with(|c| c.get().unwrap());
        drop(reset);
        n
    }
}

#[test]
fn sampling_disabled_and_skipped_paths_allocate_no_instrumentation_state() {
    use std::task::{Context, Poll, Waker};
    fn check(sampler: &Sampler) {
        let mut future = std::pin::pin!(measure_with_sampler(
            "rest.search",
            false,
            async {
                measure("engine.search", false, async {
                    assert!(!enabled());
                    assert!(Timer::new("test").start.is_none());
                    let mut timeline = Timeline::new();
                    assert!(timeline.start.is_none());
                    timeline.mark("test");
                    let worker = Worker::new("test");
                    assert!(worker.context.is_none() && worker.queued.start.is_none());
                    worker.run(|| {
                        assert!(!enabled());
                        crate::performance::event!(kind = "must_not_evaluate", value = forbidden());
                    });
                    Ok::<_, ()>(())
                })
                .await
            },
            Some(sampler)
        ));
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(
            allocation_probe::count(|| {
                assert!(matches!(
                    future.as_mut().poll(&mut context),
                    Poll::Ready(Ok(()))
                ));
            }),
            0
        );
    }
    fn forbidden() -> usize {
        panic!("unsampled event arguments evaluated")
    }
    // Subscriber installation/callsite registration and one-time config parsing
    // are outside this request-path assertion. No process environment mutation.
    tracing::subscriber::with_default(subscriber(Capture::default()), || {
        assert!(target_enabled());
        let skip = Sampler::new(NonZeroU64::new(7)); // ordinal1 is not selected
        check(&skip);
        assert_eq!(skip.roots.load(Ordering::Relaxed), 1);
    });
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        assert!(!target_enabled());
        let full = Sampler::new(NonZeroU64::new(1));
        check(&full);
        assert_eq!(full.roots.load(Ordering::Relaxed), 0);
    });
}

#[test]
fn trace_provenance_cold_disabled_paths_keep_zero_allocations_and_clocks() {
    const CHILD: &str = "RECALLD_COLD_TRACE_COST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "performance::tests::trace_provenance_cold_disabled_paths_keep_zero_allocations_and_clocks", "--nocapture"])
            .env(CHILD, "1").output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    fn forbidden() -> u64 {
        panic!("disabled field evaluated")
    }
    fn disabled() {
        assert_eq!(
            allocation_probe::count(|| {
                assert!(!enabled());
                assert!(Timer::new("cold").start.is_none());
                let timeline = Timeline::new();
                assert!(timeline.start.is_none() && timeline.last.is_none());
                let worker = Worker::new("cold");
                assert!(worker.context.is_none() && worker.queued.start.is_none());
                worker.run(|| {
                    crate::performance::event!(kind = "must_not_evaluate", value = forbidden())
                });
            }),
            0
        );
    }
    // No subscriber at all, then a live scoped subscriber on an untraced thread.
    disabled();
    let capture = Capture::default();
    let dispatch = tracing::Dispatch::new(subscriber(capture.clone()));
    disabled();
    for _ in 0..3 {
        tracing::dispatcher::with_default(&dispatch, || {
            assert!(target_enabled());
            crate::performance::event!(kind = "cold_scope_control");
        });
        disabled();
    }
    assert_eq!(events(&capture).len(), 3);
    drop(dispatch);
    // TRACE remains globally possible but this target must still be filtered out.
    let filtered = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .json()
            .without_time()
            .with_env_filter("off,trace_control=trace")
            .with_writer(capture)
            .finish(),
    );
    tracing::dispatcher::with_default(&filtered, || {
        assert_eq!(
            tracing::level_filters::LevelFilter::current(),
            tracing::level_filters::LevelFilter::TRACE
        );
        assert!(!target_enabled()); // Registration is outside the measured request path.
        disabled();
    });
}

// Reuse actual HTTP/MCP fixture clients without exposing a production sampler API.
pub(crate) async fn capture_sampled<T>(
    ordinal: u64,
    future: impl Future<Output = T>,
) -> (T, Vec<serde_json::Value>) {
    let sampler = Sampler::new(NonZeroU64::new(2));
    sampler.roots.store(ordinal - 1, Ordering::Relaxed);
    let capture = Capture::default();
    let result = measure_with_sampler(
        "test.request",
        false,
        async { Ok::<_, ()>(future.await) },
        Some(&sampler),
    )
    .with_subscriber(subscriber(capture.clone()))
    .await
    .unwrap();
    assert_eq!(sampler.roots.load(Ordering::Relaxed), ordinal);
    let rows = String::from_utf8(capture.0.lock().unwrap().clone())
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    (result, rows)
}

#[path = "performance_write_tests.rs"]
mod writes;

/// Actual producer tests capture without inventing an outer request intent.
pub(crate) async fn capture_full<T>(
    future: impl Future<Output = T>,
) -> (T, Vec<serde_json::Value>) {
    let capture = Capture::default();
    let result = future.with_subscriber(subscriber(capture.clone())).await;
    let rows = String::from_utf8(capture.0.lock().unwrap().clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    (result, rows)
}
