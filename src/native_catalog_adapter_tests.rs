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

pub(crate) struct Provider {
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
pub(crate) struct FixedClock;
struct UnknownProvider;
#[async_trait::async_trait]
impl EmbeddingProvider for UnknownProvider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        panic!("catalog embedded text")
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        panic!("display label is not profile evidence")
    }
}
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
pub(crate) fn fixture() -> (Fixture, std::path::PathBuf, Arc<Provider>) {
    fixture_with_record(false)
}
fn fixture_with_record(seed: bool) -> (Fixture, std::path::PathBuf, Arc<Provider>) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("legacy");
    let mut old = RedbStorageEngine::open(&source).unwrap();
    let mut ns = crate::model::NamespaceConfig::default_namespace(100);
    ns.embedding_dim = 4;
    let namespace_id = old.create_namespace(&ns).unwrap();
    if seed {
        let input = crate::storage::insertion::InsertMemory {
            id: None,
            summary: "canonical publication witness".into(),
            full_text: None,
            tags: vec![],
            initial_stability: None,
            created_at: Some(100),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        old.insert_memory_with_receipt(
            &crate::storage::control::MutationContext::unkeyed(namespace_id),
            &input,
            &[],
            100,
        )
        .unwrap();
    }
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
        fixture.cache.clone(),
        fixture.graph.clone(),
        fixture.vector_index.clone(),
        fixture.fts_index.clone(),
        fixture.entity_index.clone(),
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
    let mut replay_receipt = replay["receipt"].clone();
    replay_receipt["replayed"] = json!(false);
    assert_eq!(
        replay_receipt, first["receipt"],
        "strict MCP/REST receipt parity"
    );
    assert_eq!(replay["data"], first["data"]);
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
        fixture.cache.clone(),
        fixture.graph.clone(),
        fixture.vector_index.clone(),
        fixture.fts_index.clone(),
        fixture.entity_index.clone(),
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

    let unknown = NamespaceRegistryAdapter::new(fixture.storage.clone()).with_native_catalog(
        Arc::new(UnknownProvider),
        NativePolicyInputs::default(),
        Arc::new(FixedClock),
        fixture.cache.clone(),
        fixture.graph.clone(),
        fixture.vector_index.clone(),
        fixture.fts_index.clone(),
        fixture.entity_index.clone(),
    );
    let (status, out) = rest(&unknown, &json!({"name":"unbound"}), None).await;
    assert_eq!(status, 503);
    assert_eq!(out["error"]["code"], "EMBEDDING_UNAVAILABLE");
    assert_eq!(out["error"]["commitState"], "notCommitted");
    assert_eq!(bytes(&path), before);
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
                    fixture.cache.clone(),
                    fixture.graph.clone(),
                    fixture.vector_index.clone(),
                    fixture.fts_index.clone(),
                    fixture.entity_index.clone(),
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
            let encoded = crate::storage::control::encode(&stored).unwrap();
            let expected: Value = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(expected, envelope["receipt"]);
            let decoded: WriteReceipt =
                serde_json::from_value(envelope["receipt"].clone()).unwrap();
            assert_eq!(decoded, stored);
            assert_eq!(crate::storage::control::encode(&decoded).unwrap(), encoded);
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

struct NoPolicyClock;
impl Clock for NoPolicyClock {
    fn now_millis(&self) -> i64 {
        panic!("policy/replay sampled a clock")
    }
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

async fn api1_dispatch(
    bridge: crate::mcp::bridge::McpBridge,
    name: &str,
    id: Value,
    era: crate::mcp::protocol::Era,
) -> Value {
    use crate::mcp::{server::{McpDispatcher, RequestContext}, protocol::JsonRpcMessage};
    let dispatcher = McpDispatcher::new(Arc::new(bridge));
    let context = RequestContext { era, meta: None, legacy_version: None };
    let request: JsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc":"2.0", "id":id, "method":"tools/call",
        "params":{"name":"create_namespace", "arguments":{"name":name,"initialStability":3.7145}}
    })).unwrap();
    let response = dispatcher.dispatch(&context, request).await.unwrap();
    serde_json::to_value(response).unwrap()
}

fn api1_bridge(f: &Fixture, provider: Arc<Provider>, fault: Option<&'static str>) -> crate::mcp::bridge::McpBridge {
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    let mut adapter = f.namespace_adapter().with_native_catalog(
        provider, NativePolicyInputs::default(), Arc::new(FixedClock));
    if let Some(fault) = fault { adapter = adapter.with_catalog_fault(fault); }
    bridge.namespaces = Arc::new(adapter);
    bridge
}

fn api1_seeded_fixture() -> (Fixture, std::path::PathBuf, Arc<Provider>) {
    let (f, path, provider) = fixture();
    let dir = f.close();
    let engine = RedbStorageEngine::open_with_ids(&path, true,
        Box::new(crate::storage::ids::SeededIds::new([81; 32]))).unwrap();
    (Fixture::from_open_engine(dir, engine), path, provider)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_api1_real_dispatcher_eras_original_context_and_alias_guard() {
    use crate::mcp::{bridge::NamespaceRegistry, protocol::Era};
    for era in [Era::Legacy, Era::Modern] {
        let (f, path, provider) = fixture();
        let before = f.storage.read().unwrap().control_state().unwrap();
        let adapter = f.namespace_adapter().with_native_catalog(provider.clone(), NativePolicyInputs::default(), Arc::new(FixedClock));
        let input = crate::mcp::bridge::CreateNamespaceInput {
            name: "unbounded".into(), embedding_dim: None, initial_stability: None,
            desired_retention: None, decay_rate_multiplier: None,
        };
        let original = bytes(&path);
        assert!(adapter.create_namespace(input).await.is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        assert_eq!(bytes(&path), original);
        drop(adapter);
        let out = api1_dispatch(api1_bridge(&f, provider.clone(), None), "local", json!("id\n\"\\"), era).await;
        assert_eq!(out["id"], "id\n\"\\");
        assert!(out["result"].get("isError").is_none(), "{out}");
        let content: Value = serde_json::from_str(out["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(content["name"], "local");
        assert_eq!(content["embeddingDim"], 4);
        if era == Era::Modern { assert_eq!(out["result"]["structuredContent"], content); }
        else { assert!(out["result"].get("structuredContent").is_none()); }
        assert!(serde_json::to_vec(&out).unwrap().len() + 1 <= 8_388_608);
        assert_eq!(f.storage.read().unwrap().control_state().unwrap().committed_generation,
            before.committed_generation.next().unwrap());
        let (_copy, db) = crate::storage::validation::inspect_database_copy(&path.join("meta.db"), true).unwrap();
        use redb::ReadableTable;
        let tx = db.begin_read().unwrap();
        let table = tx.open_table(crate::storage::control::RECEIPTS).unwrap();
        let found = table.iter().unwrap().filter_map(|row| {
            let (_, value) = row.unwrap();
            let stored = crate::storage::control::validate_receipt(value.value()).unwrap();
            (stored.receipt.committed_generation == before.committed_generation.next().unwrap()).then_some(stored)
        }).collect::<Vec<_>>();
        assert_eq!(found.len(), 1);
        let input = crate::storage::native_catalog::NativeNamespaceInput {
            name: "local".into(), embedding_dim: None, embedding_profile_id: None,
            initial_stability: None, desired_retention: None, decay_rate_multiplier: None,
        };
        let options = crate::serialization::contract::WriteOptions::default();
        assert_eq!(found[0].request_hash, input.hash(&options.context(MutationScope::Catalog, ContractVersion::V1)).unwrap());
        assert_ne!(found[0].request_hash, input.hash(&options.context(MutationScope::Catalog, ContractVersion::V2)).unwrap());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_api1_actual_known_and_unknown_frames_reopen() {
    use crate::mcp::protocol::Era;
    for point in ["after_metadata_commit", "before_handle_publish", "after_handle_publish", "commit_unknown"] {
        let (f, path, provider) = fixture();
        let before = f.storage.read().unwrap().control_state().unwrap();
        let out = api1_dispatch(api1_bridge(&f, provider, Some(point)), "faulted", json!(17), Era::Modern).await;
        assert_eq!(out["result"]["isError"], true, "{point}: {out}");
        let content: Value = serde_json::from_str(out["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(content, out["result"]["structuredContent"]);
        assert_eq!(content["error"], "SERVICE_UNAVAILABLE");
        assert!(content.get("ready").is_none());
        assert_eq!(content["commitState"], if point == "commit_unknown" { "unknown" } else { "committed" });
        let operation = uuid::Uuid::parse_str(content["operationId"].as_str().unwrap()).unwrap();
        if point == "commit_unknown" { assert!(content.get("receipt").is_none()); }
        else { assert_eq!(content["receipt"]["operationId"], content["operationId"]); }
        assert!(serde_json::to_vec(&out).unwrap().len() + 1 <= 8_388_608);
        let dir = f.close();
        let reopened = NativeLifecycleSession::open(&path, NativePolicyInputs::default(), Arc::new(FixedClock)).unwrap();
        let receipt = reopened.engine.meta_store().lookup_operation(before.store_id, MutationScope::Catalog, operation).unwrap().unwrap();
        assert_eq!(receipt.committed_generation, before.committed_generation.next().unwrap());
        if point != "commit_unknown" {
            // Preserve f32 bits independently of the Value decimal parses in
            // the actual PartiallyApplied -> tool -> structured-content path.
            let decoded: WriteReceipt = serde_json::from_value(content["receipt"].clone()).unwrap();
            assert_eq!(crate::storage::control::encode(&decoded).unwrap(), crate::storage::control::encode(&receipt).unwrap());
            let mut original = content.clone();
            original["receipt"] = serde_json::to_value(&receipt).unwrap();
            let admission = crate::serialization::catalog_api1::CatalogReplyAdmission::mcp(
                crate::mcp::protocol::JsonRpcId::Number(17), Era::Modern);
            let expected = admission.encoded_for_test(&original, true);
            let mut actual = serde_json::to_vec(&out).unwrap(); actual.push(b'\n');
            assert_eq!(expected, actual, "actual 3.7145 receipt full MCP emission");
        }
        drop(reopened); drop(dir);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_api1_oversized_outer_id_refuses_before_provider_legacy_unchanged() {
    use crate::mcp::protocol::Era;
    let (f, path, provider) = api1_seeded_fixture();
    let before = bytes(&path);
    let ids = f.storage.read().unwrap().meta_store().id_source().checkpoint().unwrap();
    assert!(ids.is_some());
    let out = api1_dispatch(api1_bridge(&f, provider.clone(), None), "refused", json!("x".repeat(8_388_608)), Era::Legacy).await;
    assert_eq!(out["result"]["isError"], true);
    let error: Value = serde_json::from_str(out["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(error["error"], "RESPONSE_TOO_LARGE");
    assert_eq!(error["commitState"], "notCommitted");
    assert!(error.get("receipt").is_none());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(bytes(&path), before);
    assert_eq!(f.storage.read().unwrap().meta_store().id_source().checkpoint().unwrap(), ids);
    // The already supplied outer ID cannot itself be made to fit the native
    // policy. The proof is refusal before canonical work, not a small echo.
    let legacy = Fixture::new();
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.namespaces = Arc::new(legacy.namespace_adapter());
    let out = api1_dispatch(bridge, "legacy", json!("x".repeat(8_388_608)), Era::Legacy).await;
    assert!(out["result"].get("isError").is_none());
    assert!(serde_json::to_vec(&out).unwrap().len() > 8_388_608);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_api1_foreign_pending_receipt_is_not_drained_or_returned() {
    use crate::serialization::catalog_api1::{self, CatalogReplyAdmission};
    let (f, path, provider) = api1_seeded_fixture();
    let adapter = NativeCatalogAdapter::new(f.completion(), provider.clone(), Default::default(), Arc::new(FixedClock));
    let input = |name: &str| crate::storage::native_catalog::NativeNamespaceInput {
        name: name.into(), embedding_dim: None, embedding_profile_id: None,
        initial_stability: None, desired_retention: None, decay_rate_multiplier: None,
    };
    let first = adapter.create_api1_bounded(input("prior"), CatalogReplyAdmission::http()).await.unwrap();
    let ns = catalog_api1::namespace(&first).unwrap();
    let prior = {
        let mut engine = f.storage.write().unwrap();
        let supplied = crate::storage::native_records::NativeInsert {
            memory: crate::storage::insertion::InsertMemory {
                id: None, summary: "retained foreign outbox witness".into(), full_text: None,
                tags: vec![], initial_stability: None, created_at: None,
                embedding: Some(vec![1., 0., 0., 0.]), parent_id: None, supersedes: None,
            }, observed_at: None, entities: vec![], topics: vec![], emotions: vec![],
            embedding_profile: Some(crate::embedding::runtime::tests::profile()),
        };
        let policy = NativePolicyInputs::default();
        let intent = engine.meta_store().capture_native_insert(
            crate::storage::native_lifecycle::LifecycleScope { namespace_id: ns.id, namespace_name: ns.name, source: None },
            &supplied, &policy).unwrap();
        let ctx = crate::storage::control::MutationContext::unkeyed(ns.id);
        engine.insert_native_record(&ctx, &intent,
            &crate::embedding::runtime::GeneratedEmbedding::unverified(vec![], Some(crate::embedding::runtime::tests::profile())),
            &policy, &FixedClock, None).unwrap()
    };
    let before = f.storage.read().unwrap().control_state().unwrap();
    let original = bytes(&path);
    let ids = f.storage.read().unwrap().meta_store().id_source().checkpoint().unwrap();
    assert!(ids.is_some());
    let error = adapter.create_api1_bounded(input("blocked"), CatalogReplyAdmission::http()).await.unwrap_err();
    assert!(matches!(&error, CatalogError::Storage(StorageError::MaintenanceDependencyUnavailable)));
    let (_, value) = catalog_api1::failure(&error);
    assert_eq!(value["commitState"], "notCommitted");
    assert!(value.get("receipt").is_none());
    assert!(value.get("ready").is_none());
    assert_eq!(bytes(&path), original);
    let engine = f.storage.read().unwrap();
    assert_eq!(engine.control_state().unwrap(), before);
    assert_eq!(engine.meta_store().id_source().checkpoint().unwrap(), ids);
    assert_eq!(engine.meta_store().lookup_operation(prior.store_id, MutationScope::Namespace(ns.id), prior.operation_id).unwrap().unwrap(), prior);
    assert!(!engine.meta_store().publication_state().unwrap().matches_canonical());
}

struct PausedCatalogClock {
    entered: tokio::sync::Notify,
    released: std::sync::Mutex<bool>,
    wake: std::sync::Condvar,
}
impl Clock for PausedCatalogClock {
    fn now_millis(&self) -> i64 {
        self.entered.notify_one();
        let mut released = self.released.lock().unwrap();
        while !*released { released = self.wake.wait(released).unwrap(); }
        2000
    }
}
impl PausedCatalogClock {
    fn release(&self) { *self.released.lock().unwrap() = true; self.wake.notify_all(); }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_api1_cancel_keeps_worker_publication_lease() {
    use crate::serialization::catalog_api1::CatalogReplyAdmission;
    let (f, path, provider) = fixture();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let owner_count = Arc::strong_count(&f.storage);
    let gate = crate::derived_completion::publication_gate(&f.storage).await.unwrap();
    let clock = Arc::new(PausedCatalogClock {
        entered: tokio::sync::Notify::new(), released: std::sync::Mutex::new(false), wake: std::sync::Condvar::new(),
    });
    let adapter = NativeCatalogAdapter::new(f.completion(), provider, Default::default(), clock.clone());
    let worker = tokio::spawn(async move {
        adapter.create_api1_bounded(crate::storage::native_catalog::NativeNamespaceInput {
            name: "cancelled".into(), embedding_dim: None, embedding_profile_id: None,
            initial_stability: None, desired_retention: None, decay_rate_multiplier: None,
        }, CatalogReplyAdmission::http()).await
    });
    // Always release the deterministic pause before asserting, so a failed
    // oracle cannot strand the runtime's blocking worker during test shutdown.
    let entered = tokio::time::timeout(std::time::Duration::from_secs(10), clock.entered.notified()).await;
    if entered.is_err() { clock.release(); panic!("catalog worker did not reach sampled clock"); }
    worker.abort();
    let cancelled = worker.await;
    let held = gate.try_write().is_err();
    clock.release();
    assert!(cancelled.unwrap_err().is_cancelled());
    assert!(held, "caller cancellation released the worker's publication lease");
    let completed = tokio::time::timeout(std::time::Duration::from_secs(10), gate.write()).await.unwrap();
    {
        let engine = f.storage.read().unwrap();
        assert!(engine.get_namespace_by_name("cancelled").unwrap().is_some());
        assert_eq!(engine.control_state().unwrap().committed_generation, before.committed_generation.next().unwrap());
        assert!(!engine.meta_store().publication_state().unwrap().matches_canonical());
    }
    drop(completed); drop(gate);
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while Arc::strong_count(&f.storage) != owner_count { tokio::task::yield_now().await; }
    }).await.unwrap();
    let dir = f.close();
    let reopened = NativeLifecycleSession::open(&path, NativePolicyInputs::default(), Arc::new(FixedClock)).unwrap();
    assert!(reopened.engine.get_namespace_by_name("cancelled").unwrap().is_some());
    assert_eq!(reopened.engine.control_state().unwrap().committed_generation, before.committed_generation.next().unwrap());
    drop(reopened); drop(dir);
}
fn rest_adapter(
    f: &Fixture,
    provider: Arc<Provider>,
    clock: Arc<dyn Clock>,
) -> NamespaceRegistryAdapter {
    NamespaceRegistryAdapter::new(f.storage.clone()).with_native_catalog(
        provider,
        NativePolicyInputs::default(),
        clock,
        f.cache.clone(),
        f.graph.clone(),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
    )
}
async fn ready(f: &Fixture) -> crate::storage::control::GenerationRef {
    let actual = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    let report = completion(f)
        .readiness_barrier(
            actual,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(report.ready);
    assert_eq!(report.pending_work, 0);
    assert_eq!(report.committed_generation, actual.generation);
    assert_eq!(report.derived.len(), 4);
    assert!(
        report
            .derived
            .iter()
            .all(|v| v.ready && v.applied_generation == actual.generation)
    );
    let status = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .work_tracker()
        .status();
    assert_eq!(status.pending, 0);
    assert_eq!(status.unresolved, 0);
    actual
}
async fn policy_rest(adapter: &NamespaceRegistryAdapter, id: &str, body: &Value) -> (u16, Value) {
    let response = crate::api::update_namespace_policy_v2(adapter, id, body, None).await;
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(
        response.into_body(),
        crate::daemon::protocol::MAX_MESSAGE_SIZE,
    )
    .await
    .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
fn policy_body(f: &Fixture, key: Option<&str>) -> Value {
    let engine = f.storage.read().unwrap();
    let ns = engine
        .get_namespace(crate::model::NamespaceId::new(1))
        .unwrap()
        .unwrap();
    let values = crate::serialization::policy::PolicyValues::retained(
        &crate::storage::native_catalog::NamespacePolicyState::capture(&ns).unwrap(),
    );
    let mut body = json!({"policy":values,"storeId":engine.control_state().unwrap().store_id});
    if let Some(key) = key {
        body["idempotencyKey"] = json!(key)
    }
    body
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_policy_wire_catalog_noop_change_and_replay_publish_current_generation() {
    let (f, path, provider) = fixture_with_record(true);
    let create = rest_adapter(&f, provider.clone(), Arc::new(FixedClock));
    let before = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        completion(&f)
            .readiness_barrier(
                before,
                std::time::Instant::now() + std::time::Duration::from_secs(2)
            )
            .await
            .is_err()
    );
    let create_body =
        json!({"name":"created","storeId":before.store_id,"idempotencyKey":"catalog-ready"});
    let (status, created) = rest(&create, &create_body, None).await;
    assert_eq!(status, 200, "{created}");
    let created_at = ready(&f).await;
    assert_eq!(created_at.generation.get(), before.generation.get() + 1);
    provider.available.store(false, Ordering::SeqCst);
    let adapter = rest_adapter(&f, provider.clone(), Arc::new(NoPolicyClock));
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.namespaces = Arc::new(f.namespace_adapter().with_native_catalog(
        provider.clone(),
        NativePolicyInputs::default(),
        Arc::new(NoPolicyClock),
    ));
    let mut original = policy_body(&f, Some("noop-A"));
    original["expectedGeneration"] = json!(created_at.generation);
    let before_row = f.storage.read().unwrap().scan_all().unwrap()[0]
        .1
        .to_bytes();
    let (status, noop) = policy_rest(&adapter, "1", &original).await;
    assert_eq!(status, 200, "{noop}");
    assert_eq!(noop["data"]["changed"], false);
    let noop_at = ready(&f).await;
    assert_eq!(noop_at.generation.get(), created_at.generation.get() + 1);
    assert_eq!(
        f.storage.read().unwrap().scan_all().unwrap()[0]
            .1
            .to_bytes(),
        before_row
    );
    let mut change = original.clone();
    change["idempotencyKey"] = json!("change-B");
    change["expectedGeneration"] = json!(noop_at.generation);
    change["policy"]["initialStability"] = json!(7.0);
    let (status, changed) = policy_rest(&adapter, "1", &change).await;
    assert_eq!(status, 200, "{changed}");
    assert_eq!(changed["data"]["changed"], true);
    let changed_at = ready(&f).await;
    assert_eq!(changed_at.generation.get(), noop_at.generation.get() + 1);
    assert_ne!(
        f.storage.read().unwrap().scan_all().unwrap()[0]
            .1
            .to_bytes(),
        before_row
    );
    let before_replay = bytes(&path);
    let mut mcp = original.clone();
    mcp["namespaceId"] = json!(1);
    mcp["contractVersion"] = json!(2);
    let response =
        crate::mcp::tools::dispatch_v2(&bridge, "update_namespace_policy", mcp.clone()).await;
    assert_ne!(response.is_error, Some(true));
    let response = serde_json::to_value(response).unwrap();
    let text: Value =
        serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, response["structuredContent"]);
    assert_eq!(text["receipt"]["replayed"], true);
    assert_eq!(
        text["receipt"]["operationId"],
        noop["receipt"]["operationId"]
    );
    assert_eq!(
        text["receipt"]["committedGeneration"],
        noop["receipt"]["committedGeneration"]
    );
    assert_eq!(text["data"], noop["data"]);
    let (status, rest_replay) = policy_rest(&adapter, "1", &original).await;
    assert_eq!(status, 200);
    assert_eq!(text, rest_replay);
    assert_eq!(ready(&f).await, changed_at);
    assert_eq!(bytes(&path), before_replay);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .get_namespace(crate::model::NamespaceId::new(1))
            .unwrap()
            .unwrap()
            .initial_stability,
        7.0
    );
    let no_key = policy_body(&f, None);
    let (status, unchanged) = policy_rest(&adapter, "1", &no_key).await;
    assert_eq!(status, 200);
    assert!(unchanged.get("receipt").is_none());
    assert_eq!(ready(&f).await, changed_at);
    // Catalog replay also repairs current-G publication rather than rewinding to
    // the original creation receipt. No provider or clock is sampled.
    let (status, replayed) = rest(&adapter, &create_body, None).await;
    assert_eq!(status, 200, "{replayed}");
    assert_eq!(
        replayed["receipt"]["operationId"],
        created["receipt"]["operationId"]
    );
    assert_eq!(ready(&f).await, changed_at);
    drop(bridge);
    drop(create);
    drop(adapter);
    let dir = f.close();
    let reopened =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), Arc::new(FixedClock))
            .unwrap();
    let f = Fixture::from_open_engine(dir, reopened.engine);
    let adapter = rest_adapter(&f, provider, Arc::new(NoPolicyClock));
    let before_reopen_replay = bytes(&path);
    let (status, reopened_replay) = policy_rest(&adapter, "1", &original).await;
    assert_eq!(status, 200, "{reopened_replay}");
    assert_eq!(reopened_replay, text);
    assert_eq!(ready(&f).await, changed_at);
    assert_eq!(bytes(&path), before_reopen_replay);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_policy_wire_real_completion_failures_keep_original_receipt_and_replay_repairs() {
    for kind in ["catalog", "noop", "changed"] {
        let catalog = kind == "catalog";
        let (f, _path, provider) = fixture_with_record(true);
        completion(&f).complete().await.unwrap();
        let before = ready(&f).await;
        let adapter = rest_adapter(&f, provider.clone(), Arc::new(FixedClock));
        let mut body = if catalog {
            json!({"name":"failed-publication","storeId":before.store_id,"idempotencyKey":"failure"})
        } else {
            policy_body(&f, Some("failure"))
        };
        if kind == "changed" {
            body["policy"]["initialStability"] = json!(9.0);
        }
        if !catalog {
            provider.available.store(false, Ordering::SeqCst);
        }
        // Real coordinator dimension failure, not injected success stamps.
        *f.vector_index.write().await =
            crate::search::VectorIndexOwner::uniform(crate::test_support::DIM + 1);
        let (status, error) = if catalog {
            rest(&adapter, &body, None).await
        } else {
            policy_rest(&adapter, "1", &body).await
        };
        assert_eq!(status, 503, "{error}");
        assert_eq!(error["error"]["commitState"], "committed");
        assert_eq!(error["error"]["code"], "INDEX_NOT_READY");
        assert!(error["receipt"].is_object());
        let after = f
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
        assert_eq!(after.generation.get(), before.generation.get() + 1);
        assert!(
            completion(&f)
                .readiness_barrier(
                    after,
                    std::time::Instant::now() + std::time::Duration::from_secs(2)
                )
                .await
                .is_err()
        );
        let scope = if catalog {
            MutationScope::Catalog
        } else {
            MutationScope::Namespace(crate::model::NamespaceId::new(1))
        };
        let operation = serde_json::from_value(error["receipt"]["operationId"].clone()).unwrap();
        let stored = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .lookup_operation(after.store_id, scope, operation)
            .unwrap()
            .unwrap();
        let encoded = crate::storage::control::encode(&stored).unwrap();
        assert_eq!(
            error["receipt"],
            serde_json::from_slice::<Value>(&encoded).unwrap()
        );
        assert_eq!(
            serde_json::from_value::<WriteReceipt>(error["receipt"].clone()).unwrap(),
            stored
        );
        // Retrying a known receipt must still fail while actual completion fails.
        let (status, failed_replay) = if catalog {
            rest(&adapter, &body, None).await
        } else {
            policy_rest(&adapter, "1", &body).await
        };
        assert_eq!(status, 503, "{kind}: {failed_replay}");
        assert_eq!(failed_replay["error"]["commitState"], "committed");
        let mut retained_replay = stored.clone();
        retained_replay.replayed = true;
        assert_eq!(
            failed_replay["receipt"],
            serde_json::from_slice::<Value>(
                &crate::storage::control::encode(&retained_replay).unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .generation_ref(),
            after
        );
        *f.vector_index.write().await =
            crate::search::VectorIndexOwner::uniform(crate::test_support::DIM);
        provider.available.store(false, Ordering::SeqCst);
        let (status, replayed) = if catalog {
            rest(&adapter, &body, None).await
        } else {
            policy_rest(&adapter, "1", &body).await
        };
        assert_eq!(status, 200, "{replayed}");
        assert_eq!(
            replayed["receipt"]["operationId"],
            error["receipt"]["operationId"]
        );
        assert_eq!(
            replayed["receipt"]["committedGeneration"],
            error["receipt"]["committedGeneration"]
        );
        assert_eq!(replayed["receipt"]["replayed"], true);
        assert_eq!(ready(&f).await, after);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_policy_wire_actual_completion_uses_root_accepted_before_close() {
    for catalog in [false, true] {
        let (f, _, provider) = fixture_with_record(true);
        let owner = completion(&f);
        let adapter = NativeCatalogAdapter::new(
            owner.clone(),
            provider,
            NativePolicyInputs::default(),
            Arc::new(FixedClock),
        );
        let root = owner.accept_work(None).unwrap();
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let other = tracker.clone();
        let drain = std::thread::spawn(move || {
            other
                .close_and_drain(std::time::Instant::now() + std::time::Duration::from_secs(10))
                .unwrap()
        });
        tracker.wait_until_closed();
        assert!(owner.accept_work(None).is_err());
        if catalog {
            let request =
                CreateNamespaceRequest::from_http(&json!({"name":"accepted"}), None).unwrap();
            adapter.create_accepted(request, &root).await.unwrap();
        } else {
            let request = crate::serialization::policy::PolicyRequest::from_http(
                &policy_body(&f, Some("accepted")),
                "1",
                None,
            )
            .unwrap();
            adapter.policy_accepted(request, &root).await.unwrap();
        }
        owner.finish_known_work(root);
        drop(drain.join().unwrap());
        ready(&f).await;
    }
}

struct BlockingClock {
    entered: std::sync::mpsc::Sender<()>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}
impl Clock for BlockingClock {
    fn now_millis(&self) -> i64 {
        self.entered.send(()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        2000
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_policy_wire_cancelled_spawn_retains_unknown_root_and_executing_child() {
    let (f, _, provider) = fixture();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let gate = crate::derived_completion::publication_gate(&f.storage)
        .await
        .unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let adapter = NativeCatalogAdapter::new(
        completion(&f),
        provider,
        NativePolicyInputs::default(),
        Arc::new(BlockingClock {
            entered: entered_tx,
            release: std::sync::Mutex::new(release_rx),
        }),
    );
    let request = CreateNamespaceRequest::from_http(&json!({"name":"cancelled"}), None).unwrap();
    let task = tokio::spawn(async move { adapter.create(request).await });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    assert!(tracker.status().pending >= 2);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let status = tracker.status();
    assert!(status.pending >= 2);
    assert_eq!(status.unresolved, 1);
    assert!(
        gate.try_write().is_err(),
        "detached worker still owns publication lease"
    );
    release_tx.send(()).unwrap();
    // Engine lock acquisition waits for the real detached worker to leave its
    // canonical commit. Its known child can finish; the cancelled root cannot.
    tokio::task::spawn_blocking({
        let storage = f.storage.clone();
        move || {
            assert_eq!(storage.read().unwrap().list_namespaces().unwrap().len(), 2);
        }
    })
    .await
    .unwrap();
    let status = tracker.status();
    assert_eq!(status.pending, 1);
    assert_eq!(status.unresolved, 1);
    assert!(
        tracker
            .close_and_drain(std::time::Instant::now() + std::time::Duration::from_millis(10))
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_policy_wire_unknown_worker_keeps_root_unresolved() {
    let (f, _, provider) = fixture();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let adapter = NativeCatalogAdapter::new(
        completion(&f),
        provider,
        NativePolicyInputs::default(),
        Arc::new(NoPolicyClock),
    );
    let request = CreateNamespaceRequest::from_http(&json!({"name":"panic"}), None).unwrap();
    assert!(matches!(
        adapter.create(request).await,
        Err(CatalogError::WorkerUnknown)
    ));
    let status = tracker.status();
    assert!(status.pending >= 2);
    assert_eq!(status.pending, status.unresolved);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_policy_wire_rejects_invalid_scope_preconditions_and_policy_without_writes() {
    let (f, path, provider) = fixture_with_record(true);
    provider.available.store(false, Ordering::SeqCst);
    let adapter = rest_adapter(&f, provider.clone(), Arc::new(NoPolicyClock));
    let original = policy_body(&f, Some("unchanged"));
    let before = bytes(&path);
    for (field, value) in [
        ("initialStability", Value::Null),
        ("desiredRetention", json!(2)),
        ("lifecycleVersion", json!(2)),
        ("embeddingProfileId", json!("unknown")),
    ] {
        let mut body = original.clone();
        body["policy"][field] = value;
        let (status, error) = policy_rest(&adapter, "1", &body).await;
        assert_eq!(status, 400, "{error}");
        assert_eq!(error["error"]["commitState"], "notCommitted");
        assert_eq!(bytes(&path), before);
    }
    for id in ["default", "0", "01", "4294967296"] {
        let (status, error) = policy_rest(&adapter, id, &original).await;
        assert_eq!(status, 400, "{error}");
        assert_eq!(bytes(&path), before);
    }
    let (status, error) = policy_rest(&adapter, "2", &original).await;
    assert_eq!(status, 404);
    assert_eq!(error["error"]["code"], "NAMESPACE_NOT_FOUND");
    assert_eq!(bytes(&path), before);
    let mut stale = original.clone();
    stale["expectedGeneration"] = json!("0");
    let (status, error) = policy_rest(&adapter, "1", &stale).await;
    assert_eq!(status, 409);
    assert_eq!(error["error"]["code"], "GENERATION_CONFLICT");
    assert_eq!(bytes(&path), before);
    let mut foreign = original.clone();
    foreign["storeId"] = json!(uuid::Uuid::new_v4());
    let (status, error) = policy_rest(&adapter, "1", &foreign).await;
    assert_eq!(status, 409);
    assert_eq!(error["error"]["code"], "STORE_LINEAGE_MISMATCH");
    assert_eq!(bytes(&path), before);
    let response =
        crate::api::update_namespace_policy_v2(&adapter, "1", &original, Some("different")).await;
    assert_eq!(response.status().as_u16(), 400);
    assert_eq!(bytes(&path), before);
    let (status, first) = policy_rest(&adapter, "1", &original).await;
    assert_eq!(status, 200, "{first}");
    let after = bytes(&path);
    let mut conflict = original;
    conflict["policy"]["initialStability"] = json!(8);
    let (status, error) = policy_rest(&adapter, "1", &conflict).await;
    assert_eq!(status, 409);
    assert_eq!(error["error"]["code"], "IDEMPOTENCY_CONFLICT");
    assert_eq!(bytes(&path), after);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    ready(&f).await;
}
