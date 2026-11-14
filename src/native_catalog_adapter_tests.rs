use super::*;
use crate::{
    api::adapters::NamespaceRegistryAdapter,
    storage::{
        StorageEngine,
        native_lifecycle::NativeLifecycleSession,
        schema_migration::{MigrationOptions, inspect_schema, migrate_schema},
    },
    test_support::Fixture,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Provider {
    available: AtomicBool,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl EmbeddingProvider for Provider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        panic!("catalog embedded text")
    }
    fn dimensions(&self) -> usize {
        assert!(
            self.available.load(Ordering::SeqCst),
            "replay sampled provider"
        );
        4
    }
    fn model_name(&self) -> &str {
        panic!("catalog inferred identity from label")
    }
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        assert!(
            self.available.load(Ordering::SeqCst),
            "replay sampled provider"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        Some(crate::embedding::runtime::tests::profile())
    }
}
struct FixedClock;
pub(super) struct Fault(Option<&'static str>);
impl Fault {
    pub(super) fn enter(point: Option<&'static str>) -> Self {
        Self(crate::storage::catalog::FAILURE.with(|f| f.replace(point)))
    }
}
impl Drop for Fault {
    fn drop(&mut self) {
        crate::storage::catalog::FAILURE.with(|f| f.set(self.0));
    }
}
impl Clock for FixedClock {
    fn now_millis(&self) -> i64 {
        2000
    }
}
fn fixture() -> (Fixture, std::path::PathBuf, Arc<Provider>) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("legacy");
    let mut old = RedbStorageEngine::open(&source).unwrap();
    let mut ns = crate::model::NamespaceConfig::default_namespace(100);
    ns.embedding_dim = 4;
    old.create_namespace(&ns).unwrap();
    old.sync().unwrap();
    drop(old);
    let target = dir.path().join("native");
    migrate_schema(
        &source,
        &target,
        &MigrationOptions {
            migration_now: 1000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let session =
        NativeLifecycleSession::open(&target, NativePolicyInputs::default(), Arc::new(FixedClock))
            .unwrap();
    (
        Fixture::from_open_engine(dir, session.engine),
        target,
        Arc::new(Provider {
            available: AtomicBool::new(true),
            calls: AtomicUsize::new(0),
        }),
    )
}
fn bytes(path: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    fn walk(
        path: &std::path::Path,
        root: &std::path::Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                walk(&p, root, out)
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().into(),
                    std::fs::read(p).unwrap(),
                );
            }
        }
    }
    walk(path, path, &mut out);
    out
}
async fn rest(adapter: &NamespaceRegistryAdapter, body: &Value, key: Option<&str>) -> (u16, Value) {
    let response = crate::api::create_namespace_v2(adapter, body, key).await;
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_wire_real_cross_transport_replay_and_reopen() {
    let (fixture, path, provider) = fixture();
    let state = fixture.storage.read().unwrap().control_state().unwrap();
    let adapter = NamespaceRegistryAdapter::new(fixture.storage.clone()).with_native_catalog(
        provider.clone(),
        NativePolicyInputs::default(),
        Arc::new(FixedClock),
    );
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.namespaces = Arc::new(fixture.namespace_adapter().with_native_catalog(
        provider.clone(),
        NativePolicyInputs::default(),
        Arc::new(FixedClock),
    ));
    let request = json!({"name":"created","storeId":state.store_id,"idempotencyKey":"same-key","expectedGeneration":state.committed_generation});
    let (status, first) = rest(&adapter, &request, Some("same-key")).await;
    assert_eq!(status, 200, "{first}");
    assert_eq!(first["receipt"]["replayed"], false);
    assert!(first.get("readGeneration").is_none());
    assert!(first.get("readiness").is_none());
    let mut mcp = request.clone();
    mcp["contractVersion"] = json!(2);
    provider.available.store(false, Ordering::SeqCst);
    let before = bytes(&path);
    let result = crate::mcp::tools::dispatch_v2(&bridge, "create_namespace", mcp.clone()).await;
    assert_ne!(result.is_error, Some(true));
    let result = serde_json::to_value(&result).unwrap();
    let replay = &result["structuredContent"];
    assert_eq!(replay["receipt"]["replayed"], true);
    assert_eq!(
        replay["receipt"]["operationId"],
        first["receipt"]["operationId"]
    );
    assert_eq!(
        replay["receipt"]["committedGeneration"],
        first["receipt"]["committedGeneration"]
    );
    assert_eq!(
        serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
        *replay
    );
    assert_eq!(bytes(&path), before);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", mcp)
            .await
            .is_error,
        Some(true)
    );
    let mut changed = request.clone();
    changed["initialStability"] = json!(5);
    let (status, out) = rest(&adapter, &changed, None).await;
    assert_eq!(status, 409);
    assert_eq!(out["error"]["code"], "IDEMPOTENCY_CONFLICT");
    changed = request.clone();
    changed["storeId"] = json!(uuid::Uuid::new_v4());
    let (status, out) = rest(&adapter, &changed, None).await;
    assert_eq!(status, 409);
    assert_eq!(out["error"]["code"], "STORE_LINEAGE_MISMATCH");
    assert_eq!(bytes(&path), before);
    drop(bridge);
    drop(adapter);
    let dir = fixture.close();
    let reopened =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), Arc::new(FixedClock))
            .unwrap();
    let request = CreateNamespaceRequest::from_http(&request, None).unwrap();
    let mut engine = reopened.engine;
    let receipt = engine
        .create_native_namespace_with_provider(
            &request
                .write
                .context(MutationScope::Catalog, ContractVersion::V2),
            &request.input,
            provider.as_ref(),
            &NativePolicyInputs::default(),
            &FixedClock,
            None,
        )
        .unwrap();
    assert!(receipt.replayed);
    assert_eq!(
        serde_json::to_value(receipt).unwrap()["operationId"],
        first["receipt"]["operationId"]
    );
    drop(engine);
    drop(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_wire_invalid_before_provider_and_typed_profile_conflict() {
    let (fixture, path, provider) = fixture();
    let adapter = NamespaceRegistryAdapter::new(fixture.storage.clone()).with_native_catalog(
        provider.clone(),
        NativePolicyInputs::default(),
        Arc::new(FixedClock),
    );
    let before = bytes(&path);
    for request in [
        json!({"name":"created","embeddingDim":8193}),
        json!({"name":"created","embeddingProfileId":null}),
        json!({"name":"created","bogus":1}),
        json!({"name":"created","idempotencyKey":"missing-lineage"}),
    ] {
        let (status, out) = rest(&adapter, &request, None).await;
        assert_eq!(status, 400, "{out}");
        assert_eq!(bytes(&path), before);
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let (status, out) = rest(&adapter, &json!({"name":"created","embeddingDim":5}), None).await;
    assert_eq!(status, 409);
    assert_eq!(out["error"]["code"], "EMBEDDING_PROFILE_MISMATCH");
    assert_eq!(out["error"]["field"], "/embeddingDim");
    assert_eq!(bytes(&path), before);
    assert_eq!(
        fixture
            .storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .status()
            .pending,
        0
    );
}

#[test]
fn native_catalog_wire_accepted_root_survives_closed_admission_for_lookup_and_commit() {
    let (fixture, _, provider) = fixture();
    let mut engine = fixture.storage.write().unwrap();
    let tracker = engine.meta_store().work_tracker();
    let root = tracker.begin().unwrap();
    let other = tracker.clone();
    let drain = std::thread::spawn(move || {
        other
            .close_and_drain(std::time::Instant::now() + std::time::Duration::from_secs(10))
            .unwrap()
    });
    tracker.wait_until_closed();
    let state = engine.control_state().unwrap();
    let request = CreateNamespaceRequest::from_http(
        &json!({"name":"created","storeId":state.store_id,"idempotencyKey":"accepted"}),
        None,
    )
    .unwrap();
    let ctx = request
        .write
        .context(MutationScope::Catalog, ContractVersion::V2);
    engine
        .create_native_namespace_with_provider(
            &ctx,
            &request.input,
            provider.as_ref(),
            &NativePolicyInputs::default(),
            &FixedClock,
            Some(&root),
        )
        .unwrap();
    provider.available.store(false, Ordering::SeqCst);
    assert!(
        engine
            .create_native_namespace_with_provider(
                &ctx,
                &request.input,
                provider.as_ref(),
                &NativePolicyInputs::default(),
                &FixedClock,
                Some(&root)
            )
            .unwrap()
            .replayed
    );
    engine.meta_store().finish_work(root);
    drop(drain.join().unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_wire_actual_fault_receipt_and_unknown_reconcile() {
    for point in [
        "after_metadata_commit",
        "before_handle_publish",
        "after_handle_publish",
        "commit_unknown",
    ] {
        let (fixture, path, provider) = fixture();
        let state = fixture.storage.read().unwrap().control_state().unwrap();
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.namespaces = Arc::new(
            fixture
                .namespace_adapter()
                .with_native_catalog(
                    provider.clone(),
                    NativePolicyInputs::default(),
                    Arc::new(FixedClock),
                )
                .with_catalog_fault(point),
        );
        let request = json!({"contractVersion":2,"name":"created","storeId":state.store_id,"idempotencyKey":"fault","expectedGeneration":state.committed_generation});
        let envelope = if matches!(point, "before_handle_publish" | "commit_unknown") {
            let adapter = NamespaceRegistryAdapter::new(fixture.storage.clone())
                .with_native_catalog(
                    provider.clone(),
                    NativePolicyInputs::default(),
                    Arc::new(FixedClock),
                )
                .with_catalog_fault(point);
            let mut body = request.clone();
            body.as_object_mut().unwrap().remove("contractVersion");
            let (status, envelope) = rest(&adapter, &body, None).await;
            assert_eq!(status, 503);
            envelope
        } else {
            let result =
                crate::mcp::tools::dispatch_v2(&bridge, "create_namespace", request.clone()).await;
            assert_eq!(result.is_error, Some(true), "{point}");
            let wire = serde_json::to_value(result).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(wire["content"][0]["text"].as_str().unwrap())
                    .unwrap(),
                wire["structuredContent"]
            );
            wire["structuredContent"].clone()
        };
        assert_eq!(
            envelope["error"]["commitState"],
            if point == "commit_unknown" {
                "unknown"
            } else {
                "committed"
            },
            "{point}"
        );
        assert_eq!(envelope["error"]["retryable"], false);
        assert!(
            envelope.get("readiness").is_none(),
            "no fabricated global readiness"
        );
        let operation = serde_json::from_value(envelope["error"]["operationId"].clone()).unwrap();
        assert!(fixture.storage.read().unwrap().control_state().is_err());
        assert!(
            fixture
                .storage
                .read()
                .unwrap()
                .meta_store()
                .lookup_operation(state.store_id, MutationScope::Catalog, operation)
                .is_err(),
            "poisoned lookup must remain closed"
        );
        if point == "commit_unknown" {
            assert!(envelope.get("receipt").is_none());
        } else {
            assert_eq!(
                envelope["receipt"]["operationId"],
                envelope["error"]["operationId"]
            );
        }
        drop(bridge);
        let dir = fixture.close();
        let mut reopened = NativeLifecycleSession::open(
            &path,
            NativePolicyInputs::default(),
            Arc::new(FixedClock),
        )
        .unwrap();
        let stored = reopened
            .engine
            .meta_store()
            .lookup_operation(state.store_id, MutationScope::Catalog, operation)
            .unwrap()
            .unwrap();
        if point != "commit_unknown" {
            assert_eq!(serde_json::to_value(&stored).unwrap(), envelope["receipt"]);
        }
        provider.available.store(false, Ordering::SeqCst);
        let request = CreateNamespaceRequest::from_mcp(&request).unwrap();
        let replay = reopened
            .create_namespace(
                &request
                    .write
                    .context(MutationScope::Catalog, ContractVersion::V2),
                &request.input,
                provider.as_ref(),
            )
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.operation_id, stored.operation_id);
        assert_eq!(reopened.engine.list_namespaces().unwrap().len(), 2);
        drop(reopened);
        drop(dir);
    }
}
