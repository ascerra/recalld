//! Move protocol boundary tests. Fake peers verify transport behavior, not mutation proof.
use super::*;
use crate::mcp::{
    protocol::{Era, JsonRpcMessage},
    server::{McpDispatcher, RequestContext},
};
use crate::serialization::move_memories::{Admission, Receipt, Reply};
use serde_json::{Value, json};

fn arguments() -> Value {
    json!({"storeId":Uuid::from_u128(1),"idempotencyKey":"stable-move",
        "sourceNamespace":"source","destinationNamespace":"target",
        "ids":[Uuid::from_u128(3), Uuid::from_u128(2)],"embeddingMode":"regenerate"})
}

fn receipt() -> Receipt {
    Receipt {
        store_id: Uuid::from_u128(1),
        operation_id: Uuid::from_u128(9),
        ids: vec![
            crate::model::MemoryId::from_uuid(Uuid::from_u128(3)),
            crate::model::MemoryId::from_uuid(Uuid::from_u128(2)),
        ],
        source_namespace: "source".into(),
        destination_namespace: "target".into(),
        source_namespace_id: 1,
        destination_namespace_id: 2,
        destination_profile_id: crate::embedding::runtime::tests::profile()
            .profile_id()
            .unwrap(),
        committed_generation: "42".into(),
        replayed: false,
    }
}

async fn call(storage: Arc<dyn BridgeStorageEngine>, input: Value, id: Value, era: Era) -> Value {
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = storage;
    let dispatcher = McpDispatcher::new(Arc::new(bridge));
    let request: JsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc":"2.0","id":id,"method":"tools/call",
        "params":{"name":"move_memories","arguments":input}
    }))
    .unwrap();
    serde_json::to_value(
        dispatcher
            .dispatch(
                &RequestContext {
                    era,
                    meta: None,
                    legacy_version: None,
                },
                request,
            )
            .await
            .unwrap(),
    )
    .unwrap()
}

fn content(response: &Value) -> Value {
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

async fn joined(task: tokio::task::JoinHandle<()>) {
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn move_remote_mcp_both_eras_keep_order_identity_and_compact_failure_evidence() {
    for era in [Era::Legacy, Era::Modern] {
        for state in ["committed", "unknown"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("move.sock");
            let listener = UnixListener::bind(&path).unwrap();
            let expected = arguments();
            let mut reply = Reply::error(
                "SERVICE_UNAVAILABLE",
                state,
                Some(Uuid::from_u128(9)),
                "move publication unavailable",
            );
            reply.receipt = Some(receipt());
            let expected_reply = json!(&reply);
            let task = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
                assert_eq!(request.method, "move_memories_v1");
                assert_eq!(request.params["input"], expected);
                assert_eq!(request.params["caller"]["id"], json!("id\n\"\\λ"));
                assert_eq!(request.params["caller"]["modern"], era == Era::Modern);
                write_framed_message(
                    &mut stream,
                    &DaemonResponse::success(request.id, json!(reply)),
                    MAX_MESSAGE_SIZE,
                )
                .await
                .unwrap();
                assert!(
                    read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                        .await
                        .is_err(),
                    "no automatic replay"
                );
            });
            let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
            let storage = Arc::new(crate::daemon::bridge_adapters::RemoteStorageAdapter::new(
                client,
            ));
            let out = call(storage, arguments(), json!("id\n\"\\λ"), era).await;
            assert_eq!(out["id"], json!("id\n\"\\λ"));
            assert_eq!(out["result"]["isError"], true);
            assert_eq!(content(&out), expected_reply);
            assert_eq!(out["result"]["content"].as_array().unwrap().len(), 2);
            if era == Era::Modern {
                assert_eq!(out["result"]["structuredContent"], expected_reply);
            } else {
                assert!(out["result"].get("structuredContent").is_none());
            }
            joined(task).await;
        }
    }
}

#[tokio::test]
async fn move_unknown_daemon_method_fails_closed_without_old_route_or_replay() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(request.method, "move_memories_v1");
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
    let storage = Arc::new(crate::daemon::bridge_adapters::RemoteStorageAdapter::new(
        client,
    ));
    let out = call(storage, arguments(), json!(1), Era::Modern).await;
    assert_eq!(out["result"]["isError"], true);
    assert!(
        content(&out)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("updated and restarted")
    );
    joined(task).await;
}

#[tokio::test]
async fn move_actual_daemon_rejects_invalid_input_and_unbounded_caller_before_adapter() {
    let storage = Arc::new(crate::test_support::StubBridge::default());
    let admission = Admission::mcp(
        crate::mcp::protocol::JsonRpcId::String("x".repeat(MAX_MESSAGE_SIZE - 1024)),
        Era::Modern,
    );
    let response = native_method_test_rpc(
        storage.clone(),
        json!({
            "input":arguments(), "caller":admission.caller(),
        }),
        "move_memories_v1",
    )
    .await;
    assert_eq!(
        response.result.unwrap()["error"]["code"],
        "RESPONSE_TOO_LARGE"
    );
    let mut invalid = arguments();
    invalid["ids"] = json!([]);
    let response = native_method_test_rpc(storage, json!({
        "input":invalid, "caller":Admission::mcp(crate::mcp::protocol::JsonRpcId::Number(1), Era::Modern).caller(),
    }), "move_memories_v1").await;
    assert_eq!(response.result.unwrap()["error"]["code"], "INVALID_INPUT");
}

#[tokio::test]
async fn move_remote_mcp_explicit_replay_keeps_original_compact_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("replay.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        for replayed in [false, true] {
            let request = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                .await
                .unwrap();
            assert_eq!(request.method, "move_memories_v1");
            assert_eq!(request.params["input"], arguments());
            let mut retained = receipt();
            retained.replayed = replayed;
            write_framed_message(
                &mut stream,
                &DaemonResponse::success(
                    request.id,
                    json!(Reply {
                        receipt: Some(retained),
                        error: None
                    }),
                ),
                MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap();
        }
        assert!(
            read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                .await
                .is_err()
        );
    });
    let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
    let storage = Arc::new(crate::daemon::bridge_adapters::RemoteStorageAdapter::new(
        client,
    ));
    let first = call(storage.clone(), arguments(), json!(1), Era::Modern).await;
    let replay = call(storage.clone(), arguments(), json!(2), Era::Modern).await;
    assert!(first["result"].get("isError").is_none());
    assert!(replay["result"].get("isError").is_none());
    let first = content(&first);
    let mut replay = content(&replay);
    assert_eq!(replay["receipt"]["replayed"], true);
    replay["receipt"]["replayed"] = json!(false);
    assert_eq!(first, replay);
    assert_eq!(first["receipt"]["ids"], arguments()["ids"]);
    assert_eq!(
        first["receipt"]["destinationProfileId"],
        json!(receipt().destination_profile_id)
    );
    drop(storage);
    joined(task).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn move_native_actual_socket_mcp_preserves_history_replay_and_strict_reopen() {
    use crate::storage::StorageEngine as _;
    use crate::storage::native_move::tests::{fixture, request};
    for era in [Era::Legacy, Era::Modern] {
        let root = tempfile::tempdir().unwrap();
        let (path, session, provider, ids) = fixture(root.path());
        let f = crate::test_support::Fixture::from_open_engine(root, session.engine);
        f.completion().rebuild_all().await.unwrap();
        let original: Vec<_> = ids.iter().map(|id| f.record(*id).unwrap()).collect();
        let edges = f.persisted_edges();
        let before = f.storage.read().unwrap().control_state().unwrap();
        let input = request(
            &f.storage.read().unwrap(),
            ids.clone(),
            "socket-move",
            "default",
            "project-mtx",
        );
        let owned = Arc::new(crate::mcp::bridge_adapters::McpStorageAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            provider.clone(),
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
            f.graph.clone(),
            Arc::new(crate::config::RecalldConfig::default()),
            chrono_tz::UTC,
        ));
        let socket = tempfile::tempdir().unwrap();
        let (remote, task) =
            native_lifecycle_test_remote(owned.clone(), &socket.path().join("native.sock")).await;
        let remote = Arc::new(remote);
        // Failure after generating one embedding must not move any member.
        provider
            .fail_at
            .store(2, std::sync::atomic::Ordering::SeqCst);
        let failed = call(remote.clone(), json!(&input), json!("before"), era).await;
        assert_eq!(content(&failed)["error"]["commitState"], "notCommitted");
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
        for (id, original) in ids.iter().zip(&original) {
            assert_eq!(
                f.record(*id).unwrap().encode().unwrap(),
                original.encode().unwrap()
            );
        }
        provider
            .fail_at
            .store(0, std::sync::atomic::Ordering::SeqCst);
        let out = call(remote.clone(), json!(&input), json!("native\n\"\\λ"), era).await;
        assert!(out["result"].get("isError").is_none(), "{out}");
        let first = content(&out);
        assert_eq!(first["receipt"]["ids"], json!(ids));
        assert_eq!(first["receipt"]["destinationNamespace"], "project-mtx");
        assert_eq!(
            first["receipt"]["destinationProfileId"],
            json!(
                crate::embedding::runtime::tests::profile()
                    .profile_id()
                    .unwrap()
            )
        );
        assert_eq!(
            first["receipt"]["sourceNamespaceId"],
            original[0].namespace_id
        );
        let source = crate::model::NamespaceId::new(original[0].namespace_id);
        let destination = crate::model::NamespaceId::new(
            first["receipt"]["destinationNamespaceId"].as_u64().unwrap() as u32,
        );
        {
            let fts = f.fts_index.lock().await;
            assert!(fts.search(source, "move", 10).unwrap().is_empty());
            assert_eq!(
                fts.search(destination, "move", 10).unwrap().len(),
                ids.len()
            );
            let entities = f.entity_index.read().await;
            assert!(
                entities
                    .find_by_entities(source, &["Alice".into()], MemoryId::nil())
                    .is_empty()
            );
            assert_eq!(
                entities
                    .find_by_entities(destination, &["Alice".into()], MemoryId::nil())
                    .len(),
                ids.len()
            );
        }
        assert!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
        let calls = provider.calls.load(std::sync::atomic::Ordering::SeqCst);
        let replay = call(remote.clone(), json!(&input), json!(2), era).await;
        assert!(replay["result"].get("isError").is_none(), "{replay}");
        let mut replay = content(&replay);
        assert_eq!(replay["receipt"]["replayed"], true);
        replay["receipt"]["replayed"] = json!(false);
        assert_eq!(replay, first);
        assert_eq!(
            provider.calls.load(std::sync::atomic::Ordering::SeqCst),
            calls
        );
        let mut conflict = input.clone();
        conflict.ids.reverse();
        let rejected = call(remote.clone(), json!(conflict), json!(3), era).await;
        assert_eq!(content(&rejected)["error"]["code"], "IDEMPOTENCY_CONFLICT");
        assert_eq!(f.persisted_edges(), edges);
        for (id, original) in ids.iter().zip(&original) {
            let after = f.record(*id).unwrap();
            assert_eq!(
                json!(after.namespace_id),
                first["receipt"]["destinationNamespaceId"]
            );
            let mut expected = original.clone();
            expected.namespace_id = after.namespace_id;
            expected.vector_slot = after.vector_slot;
            let schema = expected.schema.as_mut().unwrap();
            schema.vector = after.schema.as_ref().unwrap().vector.clone();
            schema.embedding_origin = None;
            schema.lifecycle.record_generation = schema.lifecycle.record_generation.next().unwrap();
            assert_eq!(expected.encode().unwrap(), after.encode().unwrap());
        }
        // Check the real retained receipt's compact committed/unknown projection.
        let operation = Uuid::parse_str(first["receipt"]["operationId"].as_str().unwrap()).unwrap();
        let retained = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .lookup_operation(
                before.store_id,
                crate::storage::control::MutationScope::Catalog,
                operation,
            )
            .unwrap()
            .unwrap();
        let failure = Reply::from_result(Err(crate::storage::StorageError::CommittedMutation {
            receipt: Box::new(retained),
            source: Box::new(crate::storage::StorageError::RecoveryRequired(
                "derived index unavailable".into(),
            )),
        }));
        assert_eq!(json!(failure.receipt), first["receipt"]);
        assert_eq!(failure.error.unwrap().commit_state, "committed");
        drop(remote);
        joined(task).await;
        drop(owned);
        let root = f.close();
        let reopened = crate::storage::RedbStorageEngine::open_native(&path).unwrap();
        assert_eq!(reopened.load_all_edges().unwrap(), edges);
        for id in ids {
            let record = reopened.get_record(id).unwrap().unwrap();
            assert_eq!(
                json!(record.namespace_id),
                first["receipt"]["destinationNamespaceId"]
            );
        }
        drop(reopened);
        drop(root);
    }
}
