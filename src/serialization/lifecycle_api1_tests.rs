//! Real Remote/framing with emulated old handler shapes, not an old daemon binary.
use super::*;
use crate::daemon::bridge_adapters::RemoteStorageAdapter;
use crate::daemon::protocol::{
    LEGACY_MAX_MESSAGE_SIZE, MAX_MESSAGE_SIZE, read_framed_message, write_framed_message,
};
use crate::daemon::{DaemonClient, DaemonRequest, DaemonResponse};
use crate::mcp::bridge::StorageEngine as _;
use std::sync::Arc;
use tokio::io::BufReader;

#[tokio::test]
async fn api1_public_old_peer_selects_exact_scoped_unscoped_before_write() {
    for operation in [Operation::Feedback, Operation::Forget] {
        for namespace in [None, Some("asserted")] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("old.sock");
            let listener = tokio::net::UnixListener::bind(&path).unwrap();
            let request = request(MemoryId::new(), 4, namespace);
            let expected = request.clone();
            let task = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut stream = BufReader::new(stream);
                let ping: DaemonRequest = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
                assert_eq!(ping.method, "ping");
                write_framed_message(
                    stream.get_mut(),
                    &DaemonResponse::success(ping.id, json!({})),
                    MAX_MESSAGE_SIZE,
                )
                .await
                .unwrap();
                let call: DaemonRequest = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
                let method = if operation == Operation::Feedback {
                    "reinforce_memory"
                } else {
                    "delete_memory"
                };
                assert_eq!(
                    call.method,
                    if namespace.is_some() {
                        format!("{method}_scoped")
                    } else {
                        method.into()
                    }
                );
                let mut params = json!({"id":expected.id});
                if operation == Operation::Feedback {
                    params["quality"] = json!(4);
                }
                if let Some(ns) = namespace {
                    params["namespace"] = json!(ns);
                }
                assert_eq!(call.params, params);
                let value = if operation == Operation::Forget {
                    json!(true)
                } else {
                    json!({"id":expected.id,"strength":1.,"stability":7.,"phase":"Full","isPermastore":false})
                };
                write_framed_message(
                    stream.get_mut(),
                    &DaemonResponse::success(call.id, value),
                    MAX_MESSAGE_SIZE,
                )
                .await
                .unwrap();
                assert!(
                    tokio::time::timeout(
                        std::time::Duration::from_millis(100),
                        read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    )
                    .await
                    .is_err(),
                    "no second mutation"
                );
            });
            let remote =
                RemoteStorageAdapter::new(Arc::new(DaemonClient::connect(&path).await.unwrap()));
            let result = remote.api1_lifecycle(request, operation).await;
            assert!(result.value.is_some(), "{result:?}");
            assert!(
                result.lifecycle.is_none(),
                "old peer has no invented receipt"
            );
            task.await.unwrap();
        }
    }
}

#[tokio::test]
async fn api1_public_advertised_unknown_method_never_falls_back_and_frame_rejects_prewrite() {
    for limit in [MAX_MESSAGE_SIZE, LEGACY_MAX_MESSAGE_SIZE] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peer.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            let ping: DaemonRequest = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                .await
                .unwrap();
            assert_eq!(ping.method, "ping");
            write_framed_message(
                stream.get_mut(),
                &DaemonResponse::success(
                    ping.id,
                    json!({"protocolVersion":2,"maxMessageSize":limit,"api1LifecycleV1":true}),
                ),
                MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap();
            if limit == MAX_MESSAGE_SIZE {
                let call: DaemonRequest = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
                assert_eq!(call.method, "forget_api1_v1");
                write_framed_message(
                    stream.get_mut(),
                    &DaemonResponse::error(
                        call.id,
                        crate::daemon::protocol::DaemonRpcError {
                            code: crate::daemon::protocol::ERR_INVALID_INPUT,
                            message: "unknown method: forget_api1_v1".into(),
                        },
                    ),
                    MAX_MESSAGE_SIZE,
                )
                .await
                .unwrap();
            }
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                )
                .await
                .is_err()
            );
        });
        let remote =
            RemoteStorageAdapter::new(Arc::new(DaemonClient::connect(&path).await.unwrap()));
        let reply = remote
            .api1_lifecycle(request(MemoryId::new(), 3, None), Operation::Forget)
            .await;
        assert!(reply.value.is_none());
        assert!(reply.lifecycle.unwrap().receipt.is_none());
        task.await.unwrap();
    }
}

#[tokio::test]
async fn api1_public_reconnect_resets_capability_without_retrying_lost_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reconnect.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        for new in [true, false] {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            let ping: DaemonRequest = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                .await
                .unwrap();
            assert_eq!(ping.method, "ping");
            write_framed_message(stream.get_mut(),&DaemonResponse::success(ping.id,if new {json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"api1LifecycleV1":true})}else{json!({})}),MAX_MESSAGE_SIZE).await.unwrap();
            let call: DaemonRequest = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                .await
                .unwrap();
            assert_eq!(
                call.method,
                if new {
                    "forget_api1_v1"
                } else {
                    "delete_memory"
                }
            );
            if !new {
                write_framed_message(
                    stream.get_mut(),
                    &DaemonResponse::success(call.id, json!(true)),
                    MAX_MESSAGE_SIZE,
                )
                .await
                .unwrap();
            }
            // First call is deliberately lost after receipt by peer. No automatic retry.
        }
    });
    let remote = RemoteStorageAdapter::new(Arc::new(DaemonClient::connect(&path).await.unwrap()));
    let input = request(MemoryId::new(), 3, None);
    let unknown = remote
        .api1_lifecycle(input.clone(), Operation::Forget)
        .await;
    assert!(unknown.value.is_none());
    let unknown = unknown.lifecycle.unwrap();
    assert!(unknown.receipt.is_none());
    assert_eq!(unknown.error.unwrap().commit_state, CommitState::Unknown);
    let next = remote.api1_lifecycle(input, Operation::Forget).await;
    assert!(matches!(next.value, Some(Value::Forget { deleted: true })));
    assert!(next.lifecycle.is_none());
    task.await.unwrap();
}

#[test]
fn api1_public_private_selector_validation_preserves_missing_nil_behavior() {
    let op = Operation::Feedback;
    let good = json!({"contractVersion":1,"id":uuid::Uuid::nil()});
    assert!(parse(&good, op).is_ok());
    for value in [Json::Null, json!(true), json!(2), json!(3), json!("1")] {
        let mut bad = good.clone();
        bad["contractVersion"] = value;
        assert!(parse(&bad, op).is_err());
    }
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("contractVersion");
    assert!(parse(&missing, op).is_err());
    let mut quality = good.clone();
    quality["quality"] = Json::Null;
    assert!(parse(&quality, op).is_err());
    assert!(
        native::parse_private(&good, op).is_err(),
        "native-only strict nil guard unchanged"
    );
}
