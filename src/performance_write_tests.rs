//! Actual online legacy producers. No native-serving or overhead evidence.
use super::*;
use crate::embedding::{EmbeddingError, EmbeddingProvider};
use crate::mcp::{McpServer, bridge_adapters::McpStorageAdapter, protocol::*};
use crate::storage::StorageEngine as _;
use std::sync::atomic::AtomicUsize;

struct LocalProvider(Arc<AtomicUsize>);
#[async_trait::async_trait]
impl EmbeddingProvider for LocalProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.starts_with("private-input-canary") {
            assert_eq!(text, "private-input-canary\n\nprivate-text-canary");
        }
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![1.0, 0.0, 0.0, 0.0])
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "offline-deterministic"
    }
}

fn server(fx: &Fixture, calls: Arc<AtomicUsize>) -> Arc<tokio::sync::Mutex<McpServer>> {
    let mut config = crate::config::RecalldConfig::default();
    config.graph.autolink_enabled = false;
    let storage = McpStorageAdapter::new(
        fx.storage.clone(),
        fx.cache.clone(),
        Arc::new(crate::embedding::runtime::CheckedProvider(Box::new(
            LocalProvider(calls),
        ))),
        fx.vector_index.clone(),
        fx.fts_index.clone(),
        fx.entity_index.clone(),
        fx.graph.clone(),
        Arc::new(config),
        chrono_tz::UTC,
    );
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = Arc::new(storage);
    Arc::new(tokio::sync::Mutex::new(McpServer::new(Arc::new(bridge))))
}
fn request() -> String {
    serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "_meta":{META_PROTOCOL_VERSION:PROTOCOL_VERSION, META_CLIENT_CAPABILITIES:{}},
        "name":"store_memory","arguments":{"summary":"private-input-canary", "fullText":"private-text-canary", "checkDuplicates":false}
    }}).to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_producer_actual_mcp_store_exposes_provider_and_completion_work() {
    let fx = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let server = server(&fx, calls.clone());
    let capture = Capture::default();
    let response = crate::mcp::transport::process_line(&server, &request())
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap()
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert!(value.get("error").is_none(), "{value}");
    assert_ne!(value["result"]["isError"], true, "{value}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(fx.storage.read().unwrap().scan_all().unwrap().len(), 1);
    assert!(
        fx.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    let trace = events(&capture);
    assert!(
        trace
            .iter()
            .any(|e| e["kind"] == "provider_call" && e["requested_vectors"] == 1),
        "missing actual provider counters: {trace:?}"
    );
    assert!(
        trace
            .iter()
            .any(|e| e["kind"] == "derived_rebuild" && e["records"] == 1),
        "missing actual complete-publication counters"
    );
}

#[derive(Clone, Default)]
struct Output {
    bytes: Arc<Mutex<Vec<u8>>>,
    flushes: Arc<AtomicUsize>,
    fail_flush: bool,
}
impl tokio::io::AsyncWrite for Output {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.bytes.lock().unwrap().extend_from_slice(buf);
        std::task::Poll::Ready(Ok(buf.len()))
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        std::task::Poll::Ready(if self.fail_flush {
            Err(std::io::Error::other("private-delivery-error-canary"))
        } else {
            Ok(())
        })
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

fn raw_events(capture: &Capture) -> Vec<serde_json::Value> {
    String::from_utf8(capture.0.lock().unwrap().clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_producer_stdio_flush_failure_preserves_actual_commit_and_context() {
    for fail_flush in [false, true] {
        let fx = Fixture::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let server = server(&fx, calls.clone());
        let output = Output {
            fail_flush,
            ..Default::default()
        };
        let captured = output.clone();
        let capture = Capture::default();
        let line = request();
        let input = format!("{line}\n");
        let result = crate::mcp::transport::serve_io(server, input.as_bytes(), output)
            .with_subscriber(subscriber(capture.clone()))
            .await;
        assert_eq!(result.is_err(), fail_flush);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(captured.flushes.load(Ordering::SeqCst), 1);
        assert_eq!(fx.storage.read().unwrap().scan_all().unwrap().len(), 1);
        assert!(
            fx.storage
                .read()
                .unwrap()
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
        let trace = events(&capture);
        let output_bytes = captured.bytes.lock().unwrap().len();
        assert!(
            trace
                .iter()
                .any(|e| e["kind"] == "transport_input" && e["source_bytes"] == line.len())
        );
        assert!(trace.iter().any(|e| e["kind"] == "transport_output"
            && e["phase"] == "attempt"
            && e["source_bytes"] == output_bytes));
        assert_eq!(
            trace
                .iter()
                .filter(|e| e["kind"] == "transport_output" && e["phase"] == "complete")
                .count(),
            usize::from(!fail_flush)
        );
        assert!(
            trace
                .iter()
                .any(|e| e["kind"] == "transport_delivery" && e["success"] == !fail_flush)
        );
        assert!(trace.iter().any(|e| e["stage"] == "mcp.stdio"
            && e["outcome"] == if fail_flush { "error" } else { "ok" }));
        for resource in ["vector", "text"] {
            assert_eq!(
                trace
                    .iter()
                    .filter(|e| e["kind"] == "sync_call"
                        && e["resource"] == resource
                        && e["phase"] == "attempt")
                    .count(),
                1
            );
            assert_eq!(
                trace
                    .iter()
                    .filter(|e| e["kind"] == "sync_call"
                        && e["resource"] == resource
                        && e["phase"] == "return"
                        && e["success"] == true)
                    .count(),
                1
            );
        }
        assert!(
            trace
                .iter()
                .any(|e| e["kind"] == "commit_call" && e["sync_coverage"] == "opaque_backend")
        );
        assert!(trace.iter().all(|e| e.get("backend_sync_count").is_none()));
        let rows = raw_events(&capture);
        let worker = rows
            .iter()
            .find(|e| e["fields"]["stage"] == "insert.worker_queue")
            .unwrap();
        let spans = worker["spans"].as_array().unwrap();
        assert_eq!(spans[0]["operation"], "mcp.stdio");
        assert_eq!(spans[0]["intent"], "unknown");
        assert!(spans[0].get("record_access").is_none());
        assert_eq!(spans[0]["event_version"], 3);
        let root_id = spans[0]["trace_id"].clone();
        for row in rows.iter().filter(|e| {
            e["fields"]["kind"] == "sync_call" || e["fields"]["kind"] == "derived_rebuild"
        }) {
            assert_eq!(row["spans"][0]["trace_id"], root_id);
        }
        for row in rows
            .iter()
            .filter(|e| e["fields"]["kind"] == "pipeline_accounting")
        {
            assert_eq!(row["fields"]["accounted_ns"], row["fields"]["elapsed_ns"]);
        }
        let raw = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
        for secret in [
            "private-input-canary",
            "private-text-canary",
            "private-delivery-error-canary",
        ] {
            assert!(!raw.contains(secret));
        }
        for (id, _) in fx.storage.read().unwrap().scan_all().unwrap() {
            assert!(!raw.contains(&id.to_string()));
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_producer_unsampled_stdio_keeps_success_without_any_trace() {
    let fx = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let server = server(&fx, calls.clone());
    let input = format!("{}\n", request());
    let capture = Capture::default();
    let decision = Decision {
        selected: false,
        every: 100,
        root_ordinal: 2,
        sampled_roots_seen: 0,
    };
    REQUEST
        .scope(
            decision,
            crate::mcp::transport::serve_io(server, input.as_bytes(), Output::default()),
        )
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    assert!(events(&capture).is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(fx.storage.read().unwrap().scan_all().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_producer_cancelled_admitted_store_remains_unresolved_not_acknowledged() {
    let fx = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let server = server(&fx, calls.clone());
    let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
    let gate = crate::derived_completion::publication_gate(&fx.storage)
        .await
        .unwrap();
    let held = gate.write_owned().await;
    let before = fx.storage.read().unwrap().control_state().unwrap();
    let capture = Capture::default();
    let captured = Output::default();
    let output = captured.clone();
    let task = tokio::spawn(
        async move {
            let input = format!("{}\n", request());
            crate::mcp::transport::serve_io(server, input.as_bytes(), output).await
        }
        .with_subscriber(subscriber(capture.clone())),
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while calls.load(Ordering::SeqCst) == 0 || tracker.status().pending < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(held);
    assert!(tracker.status().unresolved > 0);
    assert_eq!(fx.storage.read().unwrap().control_state().unwrap(), before);
    assert_eq!(captured.flushes.load(Ordering::SeqCst), 0);
    let trace = events(&capture);
    assert!(
        trace
            .iter()
            .any(|e| e["stage"] == "mcp.stdio" && e["outcome"] == "cancelled_error_or_unwound")
    );
    assert!(
        !trace
            .iter()
            .any(|e| e["kind"] == "insert_receipt" || e["kind"] == "transport_delivery")
    );
}

#[tokio::test]
async fn online_producer_unknown_root_and_sync_skip_allocate_nothing() {
    use std::task::{Context, Poll, Waker};
    let capture = Capture::default();
    let decision = Decision {
        selected: false,
        every: 100,
        root_ordinal: 2,
        sampled_roots_seen: 0,
    };
    REQUEST
        .scope(decision, async {
            let mut future = std::pin::pin!(measure_transport("test.transport", async {
                sync_io("test", "sync_data", || Ok::<_, std::io::Error>(()))?;
                Ok::<_, std::io::Error>(())
            }));
            let mut cx = Context::from_waker(Waker::noop());
            assert_eq!(
                allocation_probe::count(|| assert!(matches!(
                    future.as_mut().poll(&mut cx),
                    Poll::Ready(Ok(()))
                ))),
                0
            );
        })
        .with_subscriber(subscriber(capture.clone()))
        .await;
    assert!(events(&capture).is_empty());
}

#[tokio::test]
async fn online_producer_cache_hits_and_provider_errors_are_actual_not_extrapolated() {
    use crate::embedding::{CachedProvider, runtime::CheckedProvider};
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = CachedProvider::new(
        Box::new(CheckedProvider(Box::new(LocalProvider(calls.clone())))),
        4,
    );
    let capture = Capture::default();
    measure_transport("test.provider", async {
        provider.embed("private-cache-canary").await?;
        provider.embed("private-cache-canary").await?;
        Ok::<_, EmbeddingError>(())
    })
    .with_subscriber(subscriber(capture.clone()))
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let trace = events(&capture);
    assert_eq!(
        trace
            .iter()
            .filter(|e| e["kind"] == "provider_call")
            .count(),
        1
    );
    assert_eq!(
        trace
            .iter()
            .filter(|e| e["kind"] == "embedding_cache" && e["hit"] == true)
            .count(),
        1
    );
    assert_eq!(
        trace
            .iter()
            .filter(|e| e["kind"] == "embedding_cache" && e["hit"] == false)
            .count(),
        1
    );
    assert!(
        trace
            .iter()
            .all(|e| e.get("provider_retries").is_none() && e.get("provider_queue_ns").is_none())
    );
    let failed = CheckedProvider(Box::new(crate::embedding::PassthroughProvider::new(4)));
    let error = measure_transport("test.provider_error", failed.embed("private-error-canary"))
        .with_subscriber(subscriber(capture.clone()))
        .await;
    assert!(error.is_err());
    assert!(
        events(&capture)
            .iter()
            .any(|e| e["kind"] == "provider_return" && e["success"] == false)
    );
    let raw = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(!raw.contains("private-cache-canary") && !raw.contains("private-error-canary"));
}

#[tokio::test]
async fn online_producer_sync_fault_counts_attempt_without_success_or_backend_guess() {
    let capture = Capture::default();
    let result = measure_transport("test.sync", async {
        // Controlled sync-call fault; not a claimed device fault or durability test.
        sync_io("test", "sync_data", || {
            Err::<(), _>(std::io::Error::other("private-io-canary"))
        })
    })
    .with_subscriber(subscriber(capture.clone()))
    .await;
    assert!(result.is_err());
    let trace = events(&capture);
    assert_eq!(
        trace
            .iter()
            .filter(|e| e["kind"] == "sync_call" && e["phase"] == "attempt")
            .count(),
        1
    );
    assert_eq!(
        trace
            .iter()
            .filter(|e| e["kind"] == "sync_call" && e["phase"] == "return" && e["success"] == false)
            .count(),
        1
    );
    assert!(
        !trace
            .iter()
            .any(|e| e["kind"] == "sync_call" && e["success"] == true)
    );
    assert!(trace.iter().all(|e| e.get("backend_sync_count").is_none()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_producer_actual_committed_publication_failure_and_keyed_replay_keep_receipt() {
    use crate::storage::{StorageError, control::MutationContext, insertion::InsertMemory};
    let fx = Fixture::new();
    let completion = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    let input = InsertMemory {
        id: None,
        summary: "private-replay-canary".into(),
        full_text: Some("private-body-canary".into()),
        tags: vec![],
        initial_stability: None,
        created_at: None,
        embedding: Some(vec![1., 0., 0., 0.]),
        parent_id: None,
        supersedes: None,
    };
    let ctx = MutationContext {
        store_id: Some(fx.storage.read().unwrap().control_state().unwrap().store_id),
        idempotency_key: Some("private-key-canary".into()),
        ..MutationContext::unkeyed(NamespaceId::new(1))
    };
    *fx.vector_index.write().await = FlatVectorIndex::new(5);
    let capture = Capture::default();
    let error = completion
        .insert(ctx.clone(), input.clone(), vec![].into(), 123)
        .with_subscriber(subscriber(capture.clone()))
        .await
        .err()
        .unwrap();
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("lost committed outcome")
    };
    assert_eq!(
        fx.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()[0]
            .receipt,
        *receipt
    );
    let failed = events(&capture);
    assert!(
        failed
            .iter()
            .any(|e| e["kind"] == "insert_receipt" && e["replayed"] == false)
    );
    assert!(
        !failed
            .iter()
            .any(|e| e["kind"] == "derived_rebuild" || e["kind"] == "derived_ack")
    );
    assert!(
        failed
            .iter()
            .any(|e| e["stage"] == "insert.complete" && e["outcome"] == "error")
    );
    *fx.vector_index.write().await = FlatVectorIndex::new(4);
    let replay_capture = Capture::default();
    let replay = completion
        .insert(ctx, input, vec![f32::NAN].into(), i64::MAX)
        .with_subscriber(subscriber(replay_capture.clone()))
        .await
        .unwrap();
    assert!(replay.receipt.replayed);
    assert_eq!(replay.receipt.operation_id, receipt.operation_id);
    assert_eq!(
        replay.receipt.committed_generation,
        receipt.committed_generation
    );
    assert!(
        fx.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    let trace = events(&replay_capture);
    assert!(
        !trace
            .iter()
            .any(|e| e["kind"] == "payload_append" || e["kind"] == "sync_call")
    );
    assert!(
        trace
            .iter()
            .any(|e| e["kind"] == "insert_receipt" && e["replayed"] == true)
    );
    assert_eq!(
        trace
            .iter()
            .filter(|e| e["kind"] == "derived_view_published")
            .count(),
        4
    );
    assert!(
        trace
            .iter()
            .any(|e| e["kind"] == "derived_ack" && e["success"] == true)
    );
}
