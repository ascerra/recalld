use super::*;
use crate::daemon::bridge_adapters::RemoteSearchAdapter;
use crate::daemon::protocol::{
    DaemonResponse, MAX_MESSAGE_SIZE, read_framed_message, write_framed_message,
};
use crate::daemon::server::prepared_query_tests::{PreparedDaemon, search};
use crate::mcp::bridge::SearchPipeline;
use crate::test_support::{Fixture, MemorySpec};
use std::sync::Arc;

fn input(id: crate::model::MemoryId, access: bool) -> GetRequest {
    GetRequest::from_mcp(&json!({"contractVersion":2,"id":id.to_string(),"recordAccess":access}))
        .unwrap()
}
async fn fixture() -> (Fixture, crate::model::MemoryId) {
    let f = Fixture::new();
    let id = f
        .insert_memory_with(MemorySpec {
            summary: "point consumer",
            embedding: Some(&[1., 0., 0., 0.]),
            ..MemorySpec::new(crate::model::NamespaceId::new(1))
        })
        .await;
    f.completion().rebuild_all().await.unwrap();
    (f, id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_cold_get_matches_owner_false_and_observes_true_once() {
    let (f, id) = fixture().await;
    let owned = search(&f);
    let local = owned.get_contract(input(id, false), MAX_MESSAGE_SIZE).await;
    assert_eq!(local.http_status, 200);
    let before = f.storage.read().unwrap().control_state().unwrap();
    let daemon = PreparedDaemon::start(owned.clone()).await;
    let remote = RemoteSearchAdapter::new(daemon.client.clone());
    // Deliberately no explicit ping: the same locked exchange negotiates.
    let result = remote
        .get_contract(input(id, false), MAX_MESSAGE_SIZE)
        .await;
    assert_eq!(result.http_status, 200, "{result:?}");
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(local).unwrap()
    );
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    let accesses = f.record(id).unwrap().access_history.len();
    let result = remote.get_contract(input(id, true), MAX_MESSAGE_SIZE).await;
    assert_eq!(result.http_status, 200, "{result:?}");
    assert_eq!(result.observation_receipts.len(), 1);
    assert_eq!(
        result.envelope.receipt.as_ref(),
        result.observation_receipts.first()
    );
    assert!(result.envelope.readiness.as_ref().unwrap().ready);
    assert_eq!(f.record(id).unwrap().access_history.len(), accesses + 1);
    let actual = owned.get_contract(input(id, false), MAX_MESSAGE_SIZE).await;
    assert_eq!(
        actual.envelope.data.unwrap().memory.access_count,
        accesses + 1
    );
    daemon.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_private_origin_and_capacity_reject_before_observation() {
    let (f, id) = fixture().await;
    let daemon = PreparedDaemon::start(search(&f)).await;
    daemon.client.ping().await.unwrap();
    let args = json!({"contractVersion":2,"id":id.to_string(),"recordAccess":true});
    let before = f.storage.read().unwrap().control_state().unwrap();
    let history = f.record(id).unwrap().access_history;
    let base =
        json!({"arguments":args,"responseLimit":response_limit(),"originalContractVersion":2});
    let mut cases = vec![];
    for v in [Value::Null, json!(3), json!(1.5), json!(true), json!("1")] {
        let mut x = base.clone();
        x["originalContractVersion"] = v;
        cases.push(x);
    }
    let mut missing = base.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("originalContractVersion");
    cases.push(missing);
    let mut extra = base.clone();
    extra["extra"] = json!(0);
    cases.push(extra);
    for bound in [response_limit() - 1, response_limit() + 1] {
        let mut x = base.clone();
        x["responseLimit"] = json!(bound);
        cases.push(x);
    }
    for params in cases {
        assert!(daemon.client.call(GET, params).await.is_err());
    }
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert_eq!(f.record(id).unwrap().access_history, history);
    daemon.finish().await;
}

async fn peer(
    reply: Value,
    access: bool,
    caps: Value,
    wrong_id: bool,
    id: crate::model::MemoryId,
) -> GetReply {
    use tokio::net::UnixListener;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("get.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let admitted = caps.get("preparedGetV1") == Some(&Value::Bool(true));
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(ping.method, "ping");
        write_framed_message(
            &mut stream,
            &DaemonResponse::success(ping.id, caps),
            MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
        if admitted {
            let request = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                .await
                .unwrap();
            assert_eq!(request.method, GET);
            write_framed_message(
                &mut stream,
                &DaemonResponse::success(if wrong_id { request.id + 1 } else { request.id }, reply),
                MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap();
        } else {
            let next = tokio::time::timeout(
                std::time::Duration::from_millis(75),
                read_framed_message(&mut stream, MAX_MESSAGE_SIZE),
            )
            .await;
            assert!(next.is_err(), "unsupported peer must receive no get");
        }
    });
    let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
    let request = input(id, access);
    let result = RemoteSearchAdapter::new(client)
        .get_contract(request, MAX_MESSAGE_SIZE)
        .await;
    task.await.unwrap();
    result
}
fn capabilities(get: bool) -> Value {
    json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"preparedQueriesV1":false,"preparedGetV1":get})
}

#[tokio::test]
async fn old_capability_and_sent_failure_remain_distinct() {
    for access in [false, true] {
        let before = peer(
            json!({}),
            access,
            capabilities(false),
            false,
            crate::model::MemoryId::from_uuid(uuid::Uuid::from_u128(1)),
        )
        .await;
        assert_eq!(
            before.envelope.error.unwrap().commit_state,
            CommitState::NotCommitted
        );
        let sent = peer(
            json!({}),
            access,
            capabilities(true),
            false,
            crate::model::MemoryId::from_uuid(uuid::Uuid::from_u128(1)),
        )
        .await;
        assert_eq!(
            sent.envelope.error.unwrap().commit_state,
            if access {
                CommitState::Unknown
            } else {
                CommitState::NotCommitted
            }
        );
        assert!(sent.envelope.data.is_none());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_receipt_malformed_reply_keeps_only_received_evidence() {
    let (f, id) = fixture().await;
    let owned = search(&f);
    let valid = owned.get_contract(input(id, true), MAX_MESSAGE_SIZE).await;
    assert_eq!(valid.http_status, 200);
    let receipt = serde_json::to_value(valid.envelope.receipt.as_ref().unwrap()).unwrap();
    let original = framed_reply(valid);
    let control = peer(original.clone(), true, capabilities(true), false, id).await;
    assert_eq!(
        control.http_status, 200,
        "get-only capable peer: {control:?}"
    );
    for (label, mutated) in [
        ("wrong UUID", {
            let mut v = original.clone();
            v["reply"]["data"]["memory"]["id"] = json!(uuid::Uuid::from_u128(7));
            v
        }),
        ("unequal readiness", {
            let mut v = original.clone();
            v["reply"]["readiness"]["derived"][0]["appliedGeneration"] = json!(999999);
            v
        }),
        ("extra field", {
            let mut v = original.clone();
            v["reply"]["data"]["memory"]["fabricated"] = json!(true);
            v
        }),
    ] {
        // The request stays fixed: each single mutation must independently reject.
        let result = peer(mutated, true, capabilities(true), false, id).await;
        assert_eq!(result.http_status, 503, "{label}");
        assert_eq!(
            result.envelope.error.unwrap().commit_state,
            CommitState::Unknown
        );
        assert!(
            result.envelope.receipt.is_none()
                && result.envelope.read_generation.is_none()
                && result.envelope.readiness.is_none()
                && result.observation_receipts.is_empty()
        );
        assert_eq!(
            result.received_evidence.unwrap().receipts,
            vec![receipt.clone()]
        );
    }
    let crossed = peer(original, true, capabilities(true), true, id).await;
    assert_eq!(crossed.received_evidence.unwrap().receipts, vec![receipt]);
}

#[test]
fn raw_frame_preflight_preserves_scalar_structure_and_exact_limit() {
    let limit = response_limit();
    let raw = format!("{{\"x\":\"{}\"}}", "x".repeat(limit - 8));
    assert_eq!(raw.len(), limit);
    assert!(admitted_frame(raw.as_bytes()));
    assert!(!admitted_frame(format!("{raw} ").as_bytes()));
    for raw in ["{\"x\":1,\"x\":2}", "{\"x\":1e0}", "{\"x\":1e999}"] {
        assert!(!admitted_frame(raw.as_bytes()));
    }
    assert!(!admitted_frame(
        format!("{}0{}", "[".repeat(40), "]".repeat(40)).as_bytes()
    ));
}

/// Get-only transport fixture forwarding the real owner's typed consumer.
/// Hooks are per instance and never replace canonical ownership/publication.
pub(crate) struct GetOnly {
    pub inner: Arc<dyn SearchPipeline>,
    pub barrier: Option<fn(&crate::derived_completion::DerivedCompletion)>,
    pub inflate: bool,
    pub origins: std::sync::Mutex<Vec<u32>>,
}
impl GetOnly {
    pub fn new(inner: Arc<dyn SearchPipeline>) -> Self {
        Self {
            inner,
            barrier: None,
            inflate: false,
            origins: std::sync::Mutex::new(vec![]),
        }
    }
}
#[async_trait::async_trait]
impl SearchPipeline for GetOnly {
    fn supports_prepared_get(&self) -> bool {
        self.inner.supports_prepared_get()
    }
    async fn get_contract(&self, mut r: GetRequest, limit: usize) -> GetReply {
        self.origins
            .lock()
            .unwrap()
            .push(r.original_contract_version);
        r.before_barrier = self.barrier;
        let mut reply = self.inner.get_contract(r, limit).await;
        if self.inflate
            && let Some(data) = reply.envelope.data.as_mut()
        {
            data.memory.summary = Some("x".repeat(response_limit() + 1));
        }
        reply
    }
    async fn search(
        &self,
        _: crate::mcp::bridge::SearchInput,
    ) -> Result<crate::mcp::bridge::SearchResponse, crate::mcp::bridge::BridgeError> {
        panic!("get must not dispatch search")
    }
    async fn find_similar(
        &self,
        _: crate::model::MemoryId,
        _: usize,
        _: Option<f32>,
        _: bool,
        _: bool,
        _: Option<&str>,
    ) -> Result<Vec<crate::mcp::bridge::SearchHit>, crate::mcp::bridge::BridgeError> {
        panic!("get must not dispatch similar")
    }
    async fn scan_duplicates(
        &self,
        _: &str,
        _: f32,
        _: usize,
    ) -> Result<Vec<crate::mcp::bridge::DuplicateCluster>, crate::mcp::bridge::BridgeError> {
        panic!("get must not dispatch scan")
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_final_dispatch_overflow_retains_committed_receipt_in_result() {
    let (f, id) = fixture().await;
    let mut adapter = GetOnly::new(search(&f));
    adapter.inflate = true;
    let daemon = PreparedDaemon::start(Arc::new(adapter)).await;
    let before = f.record(id).unwrap().access_history.len();
    let reply = RemoteSearchAdapter::new(daemon.client.clone())
        .get_contract(input(id, true), MAX_MESSAGE_SIZE)
        .await;
    assert_eq!(reply.http_status, 503, "{reply:?}");
    assert_eq!(
        reply.envelope.error.as_ref().unwrap().commit_state,
        CommitState::Committed
    );
    assert_eq!(
        reply.envelope.error.as_ref().unwrap().code,
        "GET_RESPONSE_UNAVAILABLE"
    );
    assert!(reply.envelope.data.is_none() && reply.received_evidence.is_none());
    let receipt = reply.envelope.receipt.as_ref().unwrap();
    assert_eq!(reply.observation_receipts, vec![receipt.clone()]);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .lookup_operation(
                receipt.store_id,
                crate::storage::control::MutationScope::Namespace(crate::model::NamespaceId::new(
                    1
                )),
                receipt.operation_id
            )
            .unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(f.record(id).unwrap().access_history.len(), before + 1);
    daemon.finish().await;
}
