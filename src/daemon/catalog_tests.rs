//! Real Unix-socket remote MCP catalog regressions; isolated native fixtures only.
use super::*;
use crate::mcp::{
    bridge::McpBridge,
    protocol::{Era, JsonRpcMessage},
    server::{McpDispatcher, RequestContext},
};
use crate::native_catalog_adapter::tests::{FixedClock, api1_seeded_fixture};
use crate::serialization::catalog_api1::CatalogReplyAdmission;
use crate::storage::{StorageEngine as _, native_lifecycle::NativeLifecycleSession};
use serde_json::{Value, json};

async fn remote(
    namespaces: Arc<dyn NamespaceRegistry>,
    path: &Path,
) -> (
    Arc<crate::daemon::DaemonClient>,
    tokio::task::JoinHandle<()>,
) {
    let listener = UnixListener::bind(path).unwrap();
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let stub = Arc::new(crate::test_support::StubBridge::default());
        let (tx, rx) = watch::channel(false);
        handle_connection(
            stub.clone(),
            stub.clone(),
            namespaces,
            stub,
            stream,
            Arc::new(AtomicU32::new(1)),
            Arc::new(AtomicI64::new(0)),
            rx,
            tx,
        )
        .await;
    });
    let client = Arc::new(crate::daemon::DaemonClient::connect(path).await.unwrap());
    client.ping().await.unwrap();
    (client, task)
}

async fn dispatch(
    client: Arc<crate::daemon::DaemonClient>,
    name: &str,
    id: Value,
    era: Era,
) -> Value {
    let mut bridge: McpBridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.namespaces = Arc::new(crate::daemon::bridge_adapters::RemoteNamespaceAdapter::new(
        client,
    ));
    let dispatcher = McpDispatcher::new(Arc::new(bridge));
    let request: JsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc":"2.0", "id":id, "method":"tools/call",
        "params":{"name":"create_namespace", "arguments":{"name":name,"initialStability":3.7145}}
    }))
    .unwrap();
    let response = dispatcher
        .dispatch(
            &RequestContext {
                era,
                meta: None,
                legacy_version: None,
            },
            request,
        )
        .await
        .unwrap();
    serde_json::to_value(response).unwrap()
}

fn content(out: &Value) -> Value {
    serde_json::from_str(out["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_create_remote_mcp_eras_list_profile_and_strict_reopen() {
    for era in [Era::Legacy, Era::Modern] {
        let (f, path, provider) = api1_seeded_fixture();
        let adapter = f.namespace_adapter().with_native_catalog(
            provider,
            Default::default(),
            Arc::new(FixedClock),
        );
        let socket = tempfile::tempdir().unwrap();
        let (client, task) = remote(Arc::new(adapter), &socket.path().join("c.sock")).await;
        let id = json!("caller\n\"\\λ");
        let out = dispatch(client.clone(), "remote", id.clone(), era).await;
        assert_eq!(out["id"], id);
        assert!(out["result"].get("isError").is_none(), "{out}");
        let value = content(&out);
        assert_eq!(value["name"], "remote");
        assert_eq!(value["embeddingDim"], 4);
        if era == Era::Modern {
            assert_eq!(out["result"]["structuredContent"], value);
        } else {
            assert!(out["result"].get("structuredContent").is_none());
        }
        let listed = client.call("list_namespaces", Value::Null).await.unwrap();
        assert!(
            listed
                .as_array()
                .unwrap()
                .iter()
                .any(|ns| ns["name"] == "remote")
        );
        let before = f.storage.read().unwrap().control_state().unwrap();
        let duplicate = dispatch(client.clone(), "remote", json!(2), era).await;
        assert_eq!(content(&duplicate)["error"], "NAMESPACE_EXISTS");
        let invalid = dispatch(client.clone(), "../invalid", json!(3), era).await;
        assert_eq!(invalid["result"]["isError"], true);
        // Exercise adapter validation without relying on MCP argument validation.
        let invalid = client.call("create_namespace_bounded_v1", json!({
            "input":{"name":"../invalid"},
            "caller": CatalogReplyAdmission::mcp(crate::mcp::protocol::JsonRpcId::Number(3), era).caller(),
        })).await.unwrap_err();
        assert!(matches!(invalid, BridgeError::PartiallyApplied(_)));
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
        drop(client);
        task.await.unwrap();
        let dir = f.close();
        let reopened =
            NativeLifecycleSession::open(&path, Default::default(), Arc::new(FixedClock)).unwrap();
        let ns = reopened
            .engine
            .get_namespace_by_name("remote")
            .unwrap()
            .unwrap();
        assert_eq!(ns.id.get(), value["id"].as_u64().unwrap() as u32);
        assert_eq!(ns.embedding_dim, 4);
        assert_eq!(
            crate::storage::native_records::profile_id(&ns),
            Some(
                crate::embedding::runtime::tests::profile()
                    .profile_id()
                    .unwrap()
            )
        );
        assert_eq!(reopened.engine.list_namespaces().unwrap().len(), 2);
        drop(reopened);
        drop(dir);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_mcp_budget_and_huge_caller_id_never_create() {
    let (f, path, provider) = api1_seeded_fixture();
    let adapter = f.namespace_adapter().with_native_catalog(
        provider,
        Default::default(),
        Arc::new(FixedClock),
    );
    let socket = tempfile::tempdir().unwrap();
    let (client, task) = remote(Arc::new(adapter), &socket.path().join("c.sock")).await;
    let before = f.storage.read().unwrap().control_state().unwrap();
    for era in [Era::Legacy, Era::Modern] {
        let mut caller = serde_json::to_value(
            CatalogReplyAdmission::mcp(
                crate::mcp::protocol::JsonRpcId::String("\\\"\n".repeat(1000)),
                era,
            )
            .caller(),
        )
        .unwrap();
        // Admit the empty failure envelope exactly, then require the real
        // receipt candidate to fail before namespace staging/commit.
        let (mut low, mut high) = (0, MAX_MESSAGE_SIZE);
        while low < high {
            let middle = low + (high - low) / 2;
            caller["limit"] = json!(middle);
            let admission = CatalogReplyAdmission::from_caller(
                serde_json::from_value(caller.clone()).unwrap(),
                MAX_MESSAGE_SIZE,
            )
            .unwrap();
            if admission.admit_native().is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        caller["limit"] = json!(low);
        let error = client
            .call(
                "create_namespace_bounded_v1",
                json!({
                    "input":{"name":"budget"}, "caller":caller,
                }),
            )
            .await
            .unwrap_err();
        let BridgeError::PartiallyApplied(message) = error else {
            panic!("{error:?}")
        };
        let evidence: Value = serde_json::from_str(&message).unwrap();
        assert_eq!(evidence["error"], "RESPONSE_TOO_LARGE");
        assert_eq!(evidence["commitState"], "notCommitted");
        let out = dispatch(
            client.clone(),
            "huge",
            json!("x".repeat(MAX_MESSAGE_SIZE)),
            era,
        )
        .await;
        assert_eq!(out["result"]["isError"], true);
    }
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert!(
        f.storage
            .read()
            .unwrap()
            .get_namespace_by_name("budget")
            .unwrap()
            .is_none()
    );
    assert!(!path.join("budget").exists());
    assert!(!path.join("huge").exists());
    drop(client);
    task.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_mcp_committed_failure_preserves_full_receipt() {
    for era in [Era::Legacy, Era::Modern] {
        let (f, path, provider) = api1_seeded_fixture();
        let adapter = f
            .namespace_adapter()
            .with_native_catalog(provider, Default::default(), Arc::new(FixedClock))
            .with_catalog_fault("after_metadata_commit");
        let socket = tempfile::tempdir().unwrap();
        let (client, task) = remote(Arc::new(adapter), &socket.path().join("c.sock")).await;
        let out = dispatch(client.clone(), "committed", json!("id\n\""), era).await;
        assert_eq!(out["result"]["isError"], true);
        let value = content(&out);
        assert_eq!(value["commitState"], "committed");
        assert_eq!(value["error"], "SERVICE_UNAVAILABLE");
        assert!(value.get("ready").is_none());
        let receipt: crate::storage::control::WriteReceipt =
            serde_json::from_value(value["receipt"].clone()).unwrap();
        assert_eq!(value["operationId"], json!(receipt.operation_id));
        drop(client);
        task.await.unwrap();
        let dir = f.close();
        let reopened =
            NativeLifecycleSession::open(&path, Default::default(), Arc::new(FixedClock)).unwrap();
        let stored = reopened
            .engine
            .meta_store()
            .lookup_operation(
                receipt.store_id,
                crate::storage::control::MutationScope::Catalog,
                receipt.operation_id,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            crate::storage::control::encode(&receipt).unwrap(),
            crate::storage::control::encode(&stored).unwrap()
        );
        assert!(
            reopened
                .engine
                .get_namespace_by_name("committed")
                .unwrap()
                .is_some()
        );
        drop(reopened);
        drop(dir);
    }
}

#[tokio::test]
async fn old_daemon_unknown_method_is_final_without_legacy_fallback() {
    let socket = tempfile::tempdir().unwrap();
    let path = socket.path().join("old.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(request.method, "create_namespace_bounded_v1");
        write_framed_message(
            &mut stream,
            &DaemonResponse::error(
                request.id,
                DaemonRpcError {
                    code: -32601,
                    message: "unknown method".into(),
                },
            ),
            MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
        assert!(
            read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                .await
                .is_err()
        );
    });
    let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
    let out = dispatch(client.clone(), "old", json!(1), Era::Modern).await;
    assert_eq!(out["result"]["isError"], true);
    drop(client);
    task.await.unwrap();
}
