//! Actual new private route; fake peers below test decoder boundaries, not commits.
use super::*;
use crate::daemon::protocol::{
    DaemonResponse, MAX_MESSAGE_SIZE, read_framed_message, write_framed_message,
};
use crate::serialization::contract::CommitState;
use crate::test_support::{Fixture, MemorySpec};
use std::sync::Arc;

async fn legacy(phase: DecayPhase) -> (Fixture, crate::model::MemoryId) {
    let f = Fixture::new();
    let id = f
        .insert_memory_with(MemorySpec {
            summary: "retained ghost summary",
            tags: &["topic/retained"],
            phase,
            embedding: Some(&[1., 0., 0., 0.]),
            ..MemorySpec::new(crate::model::NamespaceId::new(1))
        })
        .await;
    f.completion().rebuild_all().await.unwrap();
    (f, id)
}
fn input(id: crate::model::MemoryId, access: bool) -> Request {
    Request {
        id,
        namespace: None,
        record_access: access,
    }
}
pub(crate) async fn fake_peer(value: Value, request: Request, capability: Value) -> Reply {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("point-peer.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(ping.method, "ping");
        write_framed_message(&mut stream, &DaemonResponse::success(ping.id,
            json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"api1GetV1":capability})), MAX_MESSAGE_SIZE).await.unwrap();
        if let Ok(get) = read_framed_message(&mut stream, MAX_MESSAGE_SIZE).await {
            assert_eq!(
                get.method, GET,
                "never fall back to an old method after negotiation"
            );
            write_framed_message(
                &mut stream,
                &DaemonResponse::success(get.id, value),
                MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap();
        }
    });
    let client = super::super::DaemonClient::connect(&path).await.unwrap();
    let reply = remote(&client, request, response_limit()).await;
    drop(client);
    tokio::time::timeout(std::time::Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    reply
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_point_legacy_ghost_direct_rpc_preserve_payload_and_one_event() {
    for remote in [false, true] {
        let (f, id) = legacy(DecayPhase::Ghost).await;
        let adapter = Arc::new(
            f.storage_adapter()
                .with_clock(Arc::new(crate::time::ManualClock::new(123456))),
        );
        let old = adapter.get_memory(id, false, None).await.unwrap().unwrap();
        let before = f.storage.read().unwrap().control_state().unwrap();
        let root = tempfile::tempdir().unwrap();
        let (target, daemon): (Arc<dyn StorageEngine>, _) = if remote {
            let (target, daemon) = super::super::server::native_lifecycle_test_remote(
                adapter.clone(),
                &root.path().join("point.sock"),
            )
            .await;
            (Arc::new(target), Some(daemon))
        } else {
            (adapter.clone(), None)
        };
        let readonly = target.get_api1(input(id, false), response_limit()).await;
        let public = readonly.mcp(&id.to_string(), false);
        assert_ne!(public.is_error, Some(true), "{public:?}");
        assert_eq!(
            public.structured_content,
            crate::mcp::protocol::ToolCallResult::json(&old)
                .unwrap()
                .structured_content
        );
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
        let n = f.record(id).unwrap().access_history.len();
        let reply = target.get_api1(input(id, true), response_limit()).await;
        let wire::Content::Point { reply, native, .. } = reply.inner else {
            panic!("actual legacy point reply")
        };
        assert!(!native);
        assert_eq!(reply.http_status, 200, "{reply:?}");
        assert_eq!(
            reply.envelope.data.as_ref().unwrap().memory.phase,
            DecayPhase::Ghost
        );
        assert_eq!(
            reply
                .envelope
                .data
                .as_ref()
                .unwrap()
                .memory
                .summary
                .as_deref(),
            Some(old.summary.as_str())
        );
        assert_eq!(f.record(id).unwrap().access_history.len(), n + 1);
        let receipt = reply.envelope.receipt.as_ref().unwrap();
        assert_eq!(reply.observation_receipts, vec![receipt.clone()]);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .lookup_operation(
                    receipt.store_id,
                    crate::storage::control::MutationScope::Namespace(
                        crate::model::NamespaceId::new(1)
                    ),
                    receipt.operation_id
                )
                .unwrap(),
            Some(receipt.clone())
        );
        assert!(reply.envelope.readiness.as_ref().unwrap().ready);
        drop(target);
        drop(adapter);
        if let Some(daemon) = daemon {
            tokio::time::timeout(std::time::Duration::from_secs(3), daemon)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_point_legacy_final_admission_keeps_exact_receipt_and_old_diagnostic() {
    for route in 0..3 {
        let (f, id) = legacy(DecayPhase::Full).await;
        let adapter = Arc::new(
            f.storage_adapter()
                .with_clock(Arc::new(crate::time::ManualClock::new(123456))),
        );
        let count = f.record(id).unwrap().access_history.len();
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .observation_fault
            .store(2, std::sync::atomic::Ordering::SeqCst);
        let receipt = if route == 0 {
            let error = adapter.get_memory(id, true, None).await.unwrap_err();
            let BridgeError::PartiallyApplied(text) = error else {
                panic!("old full receipt error")
            };
            let value: Value =
                serde_json::from_str(text.strip_prefix("observation failed: ").unwrap()).unwrap();
            assert!(
                value["message"]
                    .as_str()
                    .unwrap()
                    .starts_with("INDEX_NOT_READY: Storage("),
                "{value}"
            );
            assert!(!value["message"].as_str().unwrap().contains("Bridge("));
            serde_json::from_value(value["receipts"][0].clone()).unwrap()
        } else {
            let root = tempfile::tempdir().unwrap();
            let (target, daemon): (Arc<dyn StorageEngine>, _) = if route == 2 {
                let (target, daemon) = super::super::server::native_lifecycle_test_remote(
                    adapter.clone(),
                    &root.path().join("failure.sock"),
                )
                .await;
                (Arc::new(target), Some(daemon))
            } else {
                (adapter.clone(), None)
            };
            let mut bridge = crate::test_support::StubBridge::default().into_bridge();
            bridge.storage = target;
            let public = crate::mcp::tools::dispatch_tool(
                &bridge,
                "get_memory",
                json!({"id":id.to_string()}),
            )
            .await;
            assert_eq!(public.is_error, Some(true), "{public:?}");
            let value = public.structured_content.unwrap();
            assert_eq!(value["commitState"], "committed");
            assert!(!value.contains_key("data") && !value.contains_key("contractVersion"));
            let receipt: crate::storage::control::WriteReceipt =
                serde_json::from_value(value["receipt"].clone()).unwrap();
            let aliases: Vec<crate::storage::control::WriteReceipt> =
                serde_json::from_value(value["observationReceipts"].clone()).unwrap();
            assert_eq!(aliases, vec![receipt.clone()]);
            drop(bridge);
            if let Some(daemon) = daemon {
                tokio::time::timeout(std::time::Duration::from_secs(3), daemon)
                    .await
                    .unwrap()
                    .unwrap();
            }
            receipt
        };
        assert_eq!(f.record(id).unwrap().access_history.len(), count + 1);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .lookup_operation(
                    receipt.store_id,
                    crate::storage::control::MutationScope::Namespace(
                        crate::model::NamespaceId::new(1)
                    ),
                    receipt.operation_id
                )
                .unwrap(),
            Some(receipt)
        );
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .status()
                .pending,
            0
        );
    }
}

async fn legacy_malformed_payload(full_text: bool) {
    let (f, id) = legacy(DecayPhase::Ghost).await;
    let adapter = f.storage_adapter();
    let wire::Content::Point {
        mut reply,
        timezone,
        ..
    } = adapter
        .get_api1(input(id, false), response_limit())
        .await
        .inner
    else {
        panic!()
    };
    if full_text {
        reply.envelope.data.as_mut().unwrap().memory.full_text = Some("forged removed text".into());
    } else {
        reply.envelope.data.as_mut().unwrap().memory.summary = None;
    }
    let reply = fake_peer(
        wire::tagged(&reply, timezone, Mode::Legacy),
        input(id, false),
        json!(true),
    )
    .await;
    assert!(
        matches!(reply.inner, wire::Content::Unverified { .. }),
        "malformed legacy payload was accepted: {reply:?}"
    );
}
#[tokio::test]
async fn ordinary_point_before_peer_ghost_full_text_is_rejected() {
    legacy_malformed_payload(true).await;
}
#[tokio::test]
async fn ordinary_point_before_peer_legacy_missing_summary_is_rejected() {
    legacy_malformed_payload(false).await;
}

#[tokio::test]
async fn ordinary_point_bad_capability_does_not_send_a_get() {
    let result = fake_peer(
        Value::Null,
        input(crate::model::MemoryId::new(), true),
        json!("true"),
    )
    .await;
    let wire::Content::Unverified {
        reply,
        receipts_json,
    } = result.inner
    else {
        panic!()
    };
    assert_eq!(
        reply.envelope.error.unwrap().commit_state,
        CommitState::NotCommitted
    );
    assert!(receipts_json.is_empty());
}

#[test]
fn ordinary_point_production_raw_count_duplicates_and_overflow() {
    let raw = r#"{"observationReceipts":[null]}"#;
    assert!(acquire(raw, response_limit()).unwrap().is_empty());
    for n in [MAX_FRAGMENTS, MAX_FRAGMENTS + 1] {
        let raw = format!(
            "{{\"observationReceipts\":[{}]}}",
            vec!["null"; n].join(",")
        );
        assert_eq!(acquire(&raw, response_limit()).is_ok(), n <= MAX_FRAGMENTS);
    }
    for raw in [
        r#"{"receipt":null,"receipt":null}"#,
        r#"{"receipt":null,"rece\u0069pt":null}"#,
        r#"{"receivedEvidence":{}}"#,
    ] {
        assert!(acquire(raw, response_limit()).is_err());
    }
    let null: &RawValue = serde_json::from_str("null").unwrap();
    let mut candidates = Candidates {
        raw: vec![],
        bytes: usize::MAX,
        limit: usize::MAX,
    };
    assert!(candidates.push(null).is_err());
    assert!(candidates.raw.is_empty());
}

async fn legacy_wrong_mode(failure: bool) {
    let (f, id) = legacy(DecayPhase::Full).await;
    let adapter = f.storage_adapter();
    if failure {
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .observation_fault
            .store(2, std::sync::atomic::Ordering::SeqCst);
    }
    let result = adapter.get_api1(input(id, true), response_limit()).await;
    let wire::Content::Point {
        reply,
        timezone,
        native,
    } = result.inner
    else {
        panic!()
    };
    assert!(!native);
    assert_eq!(reply.http_status, if failure { 503 } else { 200 });
    let raw = serde_json::to_string(reply.envelope.receipt.as_ref().unwrap()).unwrap();
    let result = fake_peer(
        wire::tagged(&reply, timezone, Mode::Native),
        input(id, true),
        json!(true),
    )
    .await;
    assert!(
        matches!(result.inner, wire::Content::Unverified { .. }),
        "legacy receipt accepted under Native tag: {result:?}"
    );
    let value = result
        .mcp(&id.to_string(), false)
        .structured_content
        .unwrap();
    assert_eq!(value["commitState"], "unknown");
    assert!(
        !value.contains_key("receipt")
            && !value.contains_key("operationId")
            && !value.contains_key("readiness")
    );
    assert_eq!(value["receivedEvidence"]["receiptsJson"], json!([raw]));
}
#[tokio::test]
async fn ordinary_point_mode_before_legacy_success_cannot_claim_native() {
    legacy_wrong_mode(false).await;
}
#[tokio::test]
async fn ordinary_point_mode_before_legacy_failure_cannot_claim_native() {
    legacy_wrong_mode(true).await;
}
