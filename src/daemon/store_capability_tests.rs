//! Same-connection capability selection; no core producer stand-in claims.
use super::super::protocol::{DaemonResponse, read_framed_message, write_framed_message};
use super::*;
use serde_json::{Value, json};
use std::sync::Arc;

async fn receive(stream: &mut UnixStream) -> DaemonRequest {
    read_framed_message(stream, MAX_MESSAGE_SIZE).await.unwrap()
}
async fn answer(stream: &mut UnixStream, request: &DaemonRequest, result: Value) {
    write_framed_message(
        stream,
        &DaemonResponse::success(request.id, result),
        MAX_MESSAGE_SIZE,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn api1_store_capability_cross_product_preserves_lifecycle_selection() {
    for (store, lifecycle) in [(false, false), (true, false), (false, true), (true, true)] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let ping = receive(&mut stream).await;
            assert_eq!(ping.method, "ping");
            answer(&mut stream,&ping,json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"api1StoreV1":store,"api1LifecycleV1":lifecycle})).await;
            let write = receive(&mut stream).await;
            assert_eq!(
                write.method,
                if store {
                    "store_api1_v1"
                } else {
                    "store_memory"
                }
            );
            assert_eq!(
                write.params,
                json!({"namespace":"Exact Namespace","route":if store {"new"} else {"old"}})
            );
            answer(&mut stream, &write, json!({"ok":true})).await;
            let feedback = receive(&mut stream).await;
            assert_eq!(
                feedback.method,
                if lifecycle {
                    "feedback_api1_v1"
                } else {
                    "reinforce_memory_scoped"
                }
            );
            answer(&mut stream, &feedback, json!(true)).await;
        });
        let client = DaemonClient::connect(&path).await.unwrap();
        let (selected, _) = client
            .call_api1_store(
                json!({"namespace":"Exact Namespace","route":"new"}),
                Some(json!({"namespace":"Exact Namespace","route":"old"})),
            )
            .await
            .unwrap();
        assert_eq!(selected, store);
        let (selected, _) = client
            .call_api1_lifecycle(
                "feedback_api1_v1",
                json!({}),
                "reinforce_memory_scoped",
                json!({}),
            )
            .await
            .unwrap();
        assert_eq!(selected, lifecycle);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn api1_store_reconnect_downgrade_resets_both_without_mutation_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let ping = receive(&mut first).await;
        answer(&mut first,&ping,json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"api1StoreV1":true,"api1LifecycleV1":true})).await;
        let write = receive(&mut first).await;
        assert_eq!(write.method, "store_api1_v1");
        drop(first);
        let (mut second, _) = listener.accept().await.unwrap();
        let ping = receive(&mut second).await;
        assert_eq!(ping.method, "ping");
        answer(&mut second, &ping, json!({})).await;
        let write = receive(&mut second).await;
        assert_eq!(write.method, "store_memory");
        assert_eq!(write.params, json!({"attempt":2}));
        answer(&mut second, &write, json!(true)).await;
        let feedback = receive(&mut second).await;
        assert_eq!(feedback.method, "reinforce_memory");
        answer(&mut second, &feedback, json!(true)).await;
    });
    let client = DaemonClient::connect(&path).await.unwrap();
    assert!(
        client
            .call_api1_store(json!({"attempt":1}), Some(json!({"attempt":1})))
            .await
            .is_err()
    );
    let (new, _) = client
        .call_api1_store(json!({"attempt":2}), Some(json!({"attempt":2})))
        .await
        .unwrap();
    assert!(!new);
    let (new, _) = client
        .call_api1_lifecycle("feedback_api1_v1", json!({}), "reinforce_memory", json!({}))
        .await
        .unwrap();
    assert!(!new);
    task.await.unwrap();
}

#[tokio::test]
async fn api1_store_unsupported_constraint_and_small_capable_peer_send_no_mutation() {
    for capable in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let ping = receive(&mut stream).await;
            answer(&mut stream, &ping, json!({"api1StoreV1":capable})).await;
            assert!(matches!(
                read_framed_message(&mut stream, MAX_MESSAGE_SIZE).await,
                Err(FrameError::Closed)
            ));
        });
        let client = DaemonClient::connect(&path).await.unwrap();
        assert!(
            client
                .call_api1_store(json!({"native":true}), None)
                .await
                .is_err()
        );
        drop(client);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn api1_store_remote_malformed_reply_is_unknown_without_same_connection_fallback() {
    use crate::mcp::bridge::StorageEngine as _;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let ping = receive(&mut stream).await;
        answer(
            &mut stream,
            &ping,
            json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"api1StoreV1":true}),
        )
        .await;
        let request = receive(&mut stream).await;
        assert_eq!(request.method, "store_api1_v1");
        answer(
            &mut stream,
            &request,
            json!({"route":"owned","result":{"not":"a reply"}}),
        )
        .await;
        let ping = receive(&mut stream).await;
        assert_eq!(ping.method, "ping", "no legacy mutation fallback");
        answer(
            &mut stream,
            &ping,
            json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"api1StoreV1":true}),
        )
        .await;
    });
    let client = Arc::new(DaemonClient::connect(&path).await.unwrap());
    let remote = super::super::bridge_adapters::RemoteStorageAdapter::new(client.clone());
    let request = crate::serialization::store_api1::Request {
        input: crate::mcp::args::parse_store_input_fields(
            &json!({"summary":"unacknowledged"}),
            "default",
        )
        .unwrap(),
        profile_id: None,
        created_at: None,
        require_native: false,
    };
    let crate::serialization::store_api1::Reply::Owned(reply) = remote.store_api1(request).await
    else {
        panic!("no compatibility success");
    };
    assert!(reply.receipt.is_none());
    assert_eq!(
        reply.error.unwrap().commit_state,
        crate::serialization::contract::CommitState::Unknown
    );
    client.ping().await.unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn api1_batch_capability_independent_selection_and_no_fallback() {
    for (batch, store, lifecycle) in [
        (false, true, true),
        (true, false, false),
        (true, true, true),
        (false, false, false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("batch.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let ping = receive(&mut stream).await;
            assert_eq!(ping.method, "ping");
            answer(&mut stream,&ping,json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"api1StoreBatchV1":batch,"api1StoreV1":store,"api1LifecycleV1":lifecycle})).await;
            let request = receive(&mut stream).await;
            assert_eq!(
                request.method,
                if batch {
                    "store_batch_api1_v1"
                } else {
                    "mutate_batch"
                }
            );
            assert_eq!(request.params, json!({"route":if batch{"new"}else{"old"}}));
            // A malformed reply is not permission to send a second mutation.
            answer(&mut stream, &request, json!({"malformed":true})).await;
            let next = receive(&mut stream).await;
            assert_eq!(next.method, "ping");
            answer(&mut stream, &next, json!({})).await;
        });
        let client = DaemonClient::connect(&path).await.unwrap();
        let (selected, value) = client
            .call_api1_store_batch(json!({"route":"new"}), Some(json!({"route":"old"})))
            .await
            .unwrap();
        assert_eq!(selected, batch);
        assert_eq!(value, json!({"malformed":true}));
        client.ping().await.unwrap();
        task.await.unwrap();
    }
}

#[tokio::test]
async fn api1_batch_lost_reply_reconnect_clears_independent_capability() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("batch.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut one, _) = listener.accept().await.unwrap();
        let ping = receive(&mut one).await;
        answer(
            &mut one,
            &ping,
            json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"api1StoreBatchV1":true}),
        )
        .await;
        assert_eq!(receive(&mut one).await.method, "store_batch_api1_v1");
        drop(one);
        let (mut two, _) = listener.accept().await.unwrap();
        let ping = receive(&mut two).await;
        assert_eq!(ping.method, "ping");
        answer(&mut two, &ping, json!({})).await;
        let req = receive(&mut two).await;
        assert_eq!(req.method, "mutate_batch");
        assert_eq!(req.params, json!({"explicitSecond":true}));
        answer(&mut two, &req, json!({})).await;
    });
    let client = DaemonClient::connect(&path).await.unwrap();
    assert!(
        client
            .call_api1_store_batch(json!({}), Some(json!({})))
            .await
            .is_err()
    );
    assert!(
        !client
            .call_api1_store_batch(json!({}), Some(json!({"explicitSecond":true})))
            .await
            .unwrap()
            .0
    );
    task.await.unwrap();
}
