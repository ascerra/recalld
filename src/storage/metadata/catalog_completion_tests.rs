//! Actual public legacy catalog writes, receipts and owned publication.
use super::*;
use crate::api::adapters::*;
use crate::mcp::bridge_adapters::direct_bridge;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tower::ServiceExt;

struct Harness {
    system: crate::Recalld,
    calls: Arc<AtomicUsize>,
    provider: tokio::task::JoinHandle<()>,
    dir: tempfile::TempDir,
}
impl Harness {
    async fn new() -> Self {
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let app = axum::Router::new().route(
            "/api/embed",
            axum::routing::post(move || {
                let calls = counted.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    axum::Json(json!({"embeddings":[[1.,0.,0.,0.]]}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let provider = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let mut config = crate::config::RecalldConfig::default();
        config.storage.data_dir = dir.path().to_string_lossy().into_owned();
        config.embedding.provider = crate::config::EmbeddingProvider::Ollama;
        config.embedding.base_url = format!("http://{addr}");
        config.embedding.model_name = "isolated-catalog-test".into();
        config.embedding.dimensions = 4;
        config.decay.disable_sweep = true;
        Self {
            system: crate::Recalld::new(config).await.unwrap(),
            calls,
            provider,
            dir,
        }
    }
    fn app(&self) -> axum::Router {
        let s = &self.system;
        let config = Arc::new(s.config().clone());
        let state = crate::api::AppState::new(
            Arc::new(SearchPipelineAdapter::new(
                s.query_engine().clone(),
                s.embedding().clone(),
                s.vector_index().clone(),
                s.fts_index().clone(),
                s.entity_index().clone(),
            )),
            Arc::new(StorageEngineAdapter::new(
                s.storage().clone(),
                s.cache().clone(),
                s.vector_index().clone(),
                s.fts_index().clone(),
                s.entity_index().clone(),
                s.graph().clone(),
            )),
            Arc::new(RecordCacheAdapter::new(s.cache().clone())),
            Arc::new(RelationshipGraphAdapter::new(
                s.graph().clone(),
                s.vector_index().clone(),
                s.entity_index().clone(),
                s.storage().clone(),
                s.cache().clone(),
                config.clone(),
            )),
            Arc::new(FsrsEngineAdapter::new(
                s.storage().clone(),
                s.cache().clone(),
                s.graph().clone(),
                config,
                false,
            )),
            Arc::new(NamespaceRegistryAdapter::from_system(s)),
            Arc::new(NoopMetricsCollector),
        );
        crate::api::router(state, &Default::default())
    }
    fn raw(&self) -> Value {
        let e = self
            .system
            .storage()
            .read()
            .unwrap_or_else(|p| p.into_inner());
        let m = e.meta_store();
        let tx = m.db.begin_read().unwrap();
        let control = tx.open_table(control::CONTROL).unwrap();
        let canonical = ControlState::decode(control.get("state").unwrap().unwrap().value())
            .unwrap()
            .generation_ref();
        let receipts = tx
            .open_table(control::RECEIPTS)
            .unwrap()
            .iter()
            .unwrap()
            .map(|row| {
                let (_, v) = row.unwrap();
                serde_json::to_value(control::validate_receipt(v.value()).unwrap().receipt).unwrap()
            })
            .collect::<Vec<_>>();
        json!({"canonical":canonical,"receipts":receipts})
    }
    fn publication(&self) -> crate::storage::publication::PublicationState {
        self.system
            .storage()
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
    }
    fn receipt(&self, name: &str) -> WriteReceipt {
        use crate::storage::StorageEngine as _;
        let e = self
            .system
            .storage()
            .read()
            .unwrap_or_else(|p| p.into_inner());
        let id = e.get_namespace_by_name(name).unwrap().unwrap().id;
        let tx = e.meta_store().db.begin_read().unwrap();
        for row in tx.open_table(control::RECEIPTS).unwrap().iter().unwrap() {
            let (_, v) = row.unwrap();
            let r = control::validate_receipt(v.value()).unwrap().receipt;
            if matches!(r.outcome, MutationOutcome::NamespaceCreated { namespace_id, .. } if namespace_id == id)
            {
                return r;
            }
        }
        panic!("missing actual receipt")
    }
    fn fault(&self, mode: u8) {
        self.system
            .storage()
            .read()
            .unwrap()
            .meta_store()
            .catalog_test_fault
            .store(mode, Ordering::SeqCst);
    }
    fn tracker(&self) -> crate::storage::work::WorkTracker {
        self.system
            .storage()
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.provider.abort();
    }
}
async fn post(app: axum::Router, path: &str, body: Value) -> (u16, Value) {
    let response = app
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
fn mcp_body(result: &crate::mcp::protocol::ToolCallResult) -> Value {
    let value = serde_json::to_value(result).unwrap();
    let parsed: Value =
        serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap();
    if let Some(structured) = value.get("structuredContent") {
        assert_eq!(structured, &parsed);
    }
    parsed
}
fn files(path: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        path: &std::path::Path,
        out: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) {
        for item in std::fs::read_dir(path).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out)
            } else {
                out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = Default::default();
    walk(path, path, &mut out);
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_completion_public_empty_create_then_false_read() {
    for http in [false, true] {
        let h = Harness::new().await;
        let bridge = direct_bridge(&h.system, "default".into());
        let before = h.publication();
        assert!(before.matches_canonical());
        let args = json!({"name":"foreign","embeddingDim":4});
        if http {
            let (s, v) = post(h.app(), "/namespaces", args).await;
            assert_eq!(s, 201, "{v}");
        } else {
            let r = crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", args).await;
            assert_ne!(r.is_error, Some(true), "{r:?}");
            assert_eq!(mcp_body(&r)["id"], 2);
        }
        let receipt = h.receipt("foreign");
        let after = h.publication();
        assert!(after.matches_canonical());
        assert_eq!(
            after.canonical.generation,
            before.canonical.generation.next().unwrap()
        );
        assert_eq!(receipt.committed_generation, after.canonical.generation);
        assert_eq!(h.tracker().status().pending, 0);
        assert_eq!(h.calls.load(Ordering::SeqCst), 0);
        let raw = h.raw();
        let bytes = files(h.dir.path());
        let fts_before = h.system.fts_index().lock().await.rebuild_statistics();
        assert_eq!(
            fts_before["sqlite"], "3.49.1",
            "review WAL read-mark offsets if SQLite changes"
        );
        let query =
            json!({"query":"amber","namespace":"default","recordAccess":false,"depth":0,"limit":3});
        if http {
            let (s, v) = post(h.app(), "/search", query).await;
            assert_eq!(s, 200, "{v}");
        } else {
            let r = crate::mcp::tools::dispatch_tool(&bridge, "recall_memories", query).await;
            assert_ne!(r.is_error, Some(true), "{r:?}");
        }
        assert!(
            h.calls.load(Ordering::SeqCst) > 0,
            "actual semantic provider must be usable"
        );
        assert_eq!(h.raw(), raw);
        assert_eq!(h.publication(), after);
        assert_eq!(
            h.system.fts_index().lock().await.rebuild_statistics(),
            fts_before
        );
        let after_bytes = files(h.dir.path());
        assert_eq!(
            after_bytes.keys().collect::<Vec<_>>(),
            bytes.keys().collect::<Vec<_>>()
        );
        for (name, before) in &bytes {
            let after = &after_bytes[name];
            if name == "fts.db-shm" {
                // Bundled SQLite3.49.1: two48-byte WalIndexHdrs + nBackfill
                // + aReadMark[0] place reader mark1 at104..108. A read may
                // select mxFrame here; no database/WAL or other SHM byte changes.
                assert_eq!(before.len(), after.len());
                assert!(before[..104] == after[..104], "unexpected SHM header write");
                assert!(
                    before[108..] == after[108..],
                    "unexpected SHM write outside reader mark1"
                );
                assert_eq!(
                    u32::from_ne_bytes(after[104..108].try_into().unwrap()),
                    u32::from_ne_bytes(after[16..20].try_into().unwrap())
                );
            } else {
                assert!(before == after, "false read changed {name}");
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_completion_public_full_receipt_survives_failure_and_daemon() {
    for surface in [0, 1, 2] {
        let h = Harness::new().await;
        let before = h.publication();
        h.fault(1);
        let mut bridge = direct_bridge(&h.system, "default".into());
        let mut server = None;
        if surface == 2 {
            let (remote, task) = crate::daemon::server::catalog_test_remote(
                bridge.namespaces.clone(),
                &h.dir.path().join("probe.sock"),
            )
            .await;
            bridge.namespaces = remote;
            server = Some(task);
        }
        let args = json!({"name":"known","embeddingDim":4});
        let body = if surface == 1 {
            let (s, v) = post(h.app(), "/namespaces", args).await;
            assert_eq!(s, 503, "{v}");
            v
        } else {
            let r = crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", args).await;
            assert_eq!(r.is_error, Some(true));
            mcp_body(&r)
        };
        let receipt = h.receipt("known");
        assert_eq!(body["receipt"], serde_json::to_value(&receipt).unwrap());
        assert_eq!(body["commitState"], "committed");
        assert_eq!(body["ready"], false);
        assert_eq!(body["error"], "INDEX_NOT_READY");
        let after = h.publication();
        assert_eq!(after.applied, before.applied);
        assert_eq!(after.canonical.generation, receipt.committed_generation);
        assert!(!after.matches_canonical());
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("catalog derived publication failure")
        );
        drop(bridge);
        if let Some(task) = server {
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_completion_actual_lost_ack_has_identity_without_fabricated_receipt() {
    for http in [false, true] {
        let h = Harness::new().await;
        h.fault(2);
        let bridge = direct_bridge(&h.system, "default".into());
        let args = json!({"name":"unknown","embeddingDim":4});
        let body = if http {
            let (s, v) = post(h.app(), "/namespaces", args).await;
            assert_eq!(s, 503);
            v
        } else {
            let r = crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", args).await;
            assert_eq!(r.is_error, Some(true));
            mcp_body(&r)
        };
        let raw = h.raw();
        let receipt = raw["receipts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["outcome"]["namespaceId"] == 2)
            .unwrap();
        assert_eq!(body["commitState"], "unknown");
        assert_eq!(body["operationId"], receipt["operationId"]);
        assert!(body.get("receipt").is_none());
        assert!(h.tracker().status().unresolved > 0);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_completion_invalid_duplicate_unbound_finish_only_known_work() {
    use crate::api::NamespaceRegistry as _;
    let h = Harness::new().await;
    let before = h.raw();
    let published = h.publication();
    let unbound =
        NamespaceRegistryAdapter::new(h.system.storage().clone(), h.system.embedding().clone());
    let error = unbound
        .create("unbound", Some(4), 1., 0.9, None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("owner-bound"));
    assert_eq!(h.raw(), before);
    assert_eq!(h.tracker().status().pending, 0);
    let (s, _) = post(h.app(), "/namespaces", json!({"name":"bad/name"})).await;
    assert_eq!(s, 400);
    let (s, _) = post(h.app(), "/namespaces", json!({"name":"default"})).await;
    assert_eq!(s, 409);
    let bridge = direct_bridge(&h.system, "default".into());
    for name in ["bad/name", "default"] {
        let r = crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", json!({"name":name}))
            .await;
        assert_eq!(r.is_error, Some(true));
    }
    assert_eq!(h.raw(), before);
    assert_eq!(h.publication(), published);
    assert_eq!(h.tracker().status().pending, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_completion_cancelled_queued_worker_keeps_unknown_root_and_lease() {
    for http in [false, true] {
        let h = Harness::new().await;
        let tracker = h.tracker();
        let before = h.publication();
        let gate = h
            .system
            .storage()
            .read()
            .unwrap()
            .meta_store()
            .derived_publication
            .clone();
        let (held_tx, held_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let storage = h.system.storage().clone();
        let holder = std::thread::spawn(move || {
            let _read = storage.read().unwrap();
            held_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let app = h.app();
        let bridge = direct_bridge(&h.system, "default".into());
        let task = tokio::spawn(async move {
            let args = json!({"name":"cancelled","embeddingDim":4});
            if http {
                let _ = post(app, "/namespaces", args).await;
            } else {
                let _ = crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", args).await;
            }
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while tracker.status().pending < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        task.abort();
        let _ = task.await;
        assert!(tracker.status().pending >= 2);
        assert!(tracker.status().unresolved >= 1);
        assert!(
            gate.try_write().is_err(),
            "worker must own its lease after waiter cancellation"
        );
        release_tx.send(()).unwrap();
        holder.join().unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while tracker.status().pending > 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(tracker.status().pending, 1);
        assert_eq!(tracker.status().unresolved, 1);
        let receipt = h.receipt("cancelled");
        assert_eq!(
            receipt.committed_generation,
            before.canonical.generation.next().unwrap()
        );
        let after = h.publication();
        assert_eq!(after.applied, before.applied);
        assert!(!after.matches_canonical());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_completion_worker_panic_is_unknown_without_invented_identity() {
    for http in [false, true] {
        let h = Harness::new().await;
        let before = h.raw();
        let tracker = h.tracker();
        h.fault(3);
        let bridge = direct_bridge(&h.system, "default".into());
        let args = json!({"name":"panic","embeddingDim":4});
        let body = if http {
            let (s, v) = post(h.app(), "/namespaces", args).await;
            assert_eq!(s, 503);
            v
        } else {
            let r = crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", args).await;
            assert_eq!(r.is_error, Some(true));
            mcp_body(&r)
        };
        assert_eq!(body["commitState"], "unknown");
        assert!(body["operationId"].is_null());
        assert!(body.get("receipt").is_none());
        assert_eq!(h.raw(), before);
        assert!(tracker.status().unresolved > 0);
    }
}
