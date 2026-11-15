//! Actual online legacy producers. No native-serving or overhead evidence.
use super::*;
use crate::embedding::{EmbeddingError, EmbeddingProvider};
use crate::mcp::{McpServer, bridge_adapters::McpStorageAdapter, protocol::*};
use crate::storage::StorageEngine as _;
use std::sync::atomic::AtomicUsize;

struct LocalProvider(Arc<AtomicUsize>);
#[async_trait::async_trait]
impl EmbeddingProvider for LocalProvider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, EmbeddingError> {
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
