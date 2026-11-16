//! Actual prepared adapter vs Remote boundary; no native fixture or activation.
use super::*;
use crate::daemon::bridge_adapters::RemoteSearchAdapter;
use crate::search::adapters::*;
use crate::test_support::{Fixture, MemorySpec};
use serde_json::json;

pub(crate) struct PreparedDaemon {
    _root: tempfile::TempDir,
    pub(crate) client: Arc<crate::daemon::DaemonClient>,
    shutdown: watch::Sender<bool>,
    server: tokio::task::JoinHandle<()>,
}
impl PreparedDaemon {
    pub(crate) async fn start(search: Arc<dyn crate::mcp::bridge::SearchPipeline>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prepared.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let (tx, rx) = watch::channel(false);
        let server_tx = tx.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let unused = Arc::new(crate::test_support::StubBridge::default());
            handle_connection(
                search,
                unused.clone(),
                unused.clone(),
                unused,
                stream,
                Arc::new(AtomicU32::new(1)),
                Arc::new(AtomicI64::new(0)),
                rx,
                server_tx,
            )
            .await;
        });
        let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
        Self {
            _root: root,
            client,
            shutdown: tx,
            server,
        }
    }
    pub(crate) async fn finish(self) {
        self.client.ping().await.unwrap();
        self.shutdown.send(true).unwrap();
        drop(self.client);
        self.server.await.unwrap();
    }
}

fn query_input(access: bool, namespace: &str) -> crate::serialization::query::QueryRequest {
    crate::serialization::query::QueryRequest::from_mcp(
        &json!({"contractVersion":2,"namespace":namespace,"queryMode":"metadata_only",
            "tags":["allow"],"recordAccess":access,"limit":2,"explain":true}),
        "wrong-default",
    )
    .unwrap()
}

pub(crate) async fn peer_query(
    input: crate::serialization::query::QueryRequest,
    result: serde_json::Value,
) -> crate::serialization::query_response::QueryReply {
    use crate::daemon::protocol::{read_framed_message, write_framed_message};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("peer.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(ping.method, "ping");
        let caps =
            json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"preparedQueriesV1":true});
        write_framed_message(
            &mut stream,
            &DaemonResponse::success(ping.id, caps.clone()),
            MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
        let query = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(query.method, crate::daemon::prepared_query::QUERY);
        write_framed_message(
            &mut stream,
            &DaemonResponse::success(query.id, result),
            MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
        let next = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(
            next.method, "ping",
            "no replay/fallback after malformed reply"
        );
        write_framed_message(
            &mut stream,
            &DaemonResponse::success(next.id, caps),
            MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
    });
    let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
    let reply = RemoteSearchAdapter::new(client.clone())
        .query_contract(input)
        .await;
    client.ping().await.unwrap();
    task.await.unwrap();
    reply
}

#[tokio::test]
async fn prepared_remote_decoder_binds_actual_success_scope_and_exposure_coverage() {
    let f = Fixture::new();
    for _ in 0..2 {
        f.insert_memory_with(MemorySpec {
            summary: "prepared evidence",
            tags: &["allow"],
            embedding: Some(&[1., 0., 0., 0.]),
            ..MemorySpec::new(crate::model::NamespaceId::new(1))
        })
        .await;
    }
    f.completion().rebuild_all().await.unwrap();
    let search = search(&f);
    let mut failures = vec![];
    for access in [false, true] {
        let local = search.query_contract(query_input(access, "default")).await;
        assert_eq!(
            local.http_status, 200,
            "actual owned producer control: {local:?}"
        );
        let original: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&local).unwrap()).unwrap();
        assert_eq!(original["data"]["results"].as_array().unwrap().len(), 2);
        if access {
            assert_eq!(local.observation_receipts.len(), 1);
        }
        let normal = peer_query(
            query_input(access, "default"),
            json!({"wireVersion":1,"httpStatus":200,"reply":original}),
        )
        .await;
        assert_eq!(
            normal.http_status, 200,
            "unmodified actual producer must decode: {normal:?}"
        );
        let cases = if access {
            let mut missing = original.clone();
            for key in ["receipt", "observationReceipts", "readiness"] {
                missing.as_object_mut().unwrap().remove(key);
            }
            let mut partial = original.clone();
            partial["observationReceipts"][0]["outcome"]["accesses"]
                .as_array_mut()
                .unwrap()
                .pop();
            partial["receipt"] = partial["observationReceipts"][0].clone();
            let mut readiness = original.clone();
            readiness["readiness"]["derived"][0]["name"] = json!("fabricated");
            vec![
                ("missing exposures", missing),
                ("partial exposures", partial),
                ("invalid readiness owner", readiness),
            ]
        } else {
            let mut scope = original.clone();
            scope["data"]["results"][0]["namespaceId"] = json!(2);
            vec![("foreign row scope", scope)]
        };
        for (case, value) in cases {
            let result = peer_query(
                query_input(access, "default"),
                json!({"wireVersion":1,"httpStatus":200,"reply":value}),
            )
            .await;
            eprintln!("R1_DECODE_NEGATIVE {case}: status={}", result.http_status);
            if result.http_status != 503 {
                failures.push(case);
            }
        }
        let canonical = peer_query(
            query_input(access, "DeFaUlT"),
            json!({"wireVersion":1,"httpStatus":200,"reply":original}),
        )
        .await;
        eprintln!(
            "R1_CASE_CONTROL access={access}: status={}",
            canonical.http_status
        );
        if canonical.http_status != 200 {
            failures.push("canonical namespace casing");
        }
    }
    assert!(
        failures.is_empty(),
        "actual producer/malformed peer cases: {failures:?}"
    );
}

#[tokio::test]
async fn prepared_remote_malformed_wrapper_keeps_genuine_parent_only_as_unverified_evidence() {
    use crate::storage::{
        batch::{BatchInput, BatchKind},
        control::MutationOutcome,
    };
    let f = Fixture::new();
    let receipt = {
        let engine = f.storage.read().unwrap();
        let before = engine.control_state().unwrap();
        let input = BatchInput {
            request_context: None,
            expected_generation: None,
            mutation_version: 1,
            result_version: 1,
            kind: BatchKind::Insert,
            items: vec![json!({"summary":false})],
            default_namespace: "default".into(),
            store_id: Some(before.store_id),
            idempotency_key: Some("unrelated-query-parent".into()),
            batch_operation_id: None,
        };
        let plans = crate::mcp::batches::prepare(&input)
            .unwrap()
            .into_iter()
            .map(|p| p.plan)
            .collect::<Vec<_>>();
        let progress = engine
            .meta_store()
            .reserve_batch(&input.context(), BatchKind::Insert, &plans)
            .unwrap();
        assert_eq!(progress.unresolved(), 0);
        assert!(matches!(
            progress.reservation_receipt.outcome,
            MutationOutcome::BatchReserved { .. }
        ));
        progress.reservation_receipt
    };
    let root = f.close();
    let engine = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
    let before = engine.control_state().unwrap();
    let progress = engine
        .meta_store()
        .lookup_batch(receipt.store_id, receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(progress.reservation_receipt, receipt);
    let expected: serde_json::Value =
        serde_json::from_slice(&serde_json::to_vec(&receipt).unwrap()).unwrap();
    for access in [false, true] {
        let reply = peer_query(
            query_input(access, "default"),
            json!({"wireVersion":1,"httpStatus":200,
            "reply":{"receipt":expected,"observationReceipts":[expected],"malformed":true}}),
        )
        .await;
        assert_eq!(reply.http_status, 503);
        assert!(
            reply.envelope.receipt.is_none()
                && reply.envelope.readiness.is_none()
                && reply.envelope.data.is_none()
        );
        let error = reply.envelope.error.as_ref().unwrap();
        assert_eq!(
            error.commit_state,
            if access {
                crate::serialization::contract::CommitState::Unknown
            } else {
                crate::serialization::contract::CommitState::NotCommitted
            }
        );
        assert!(error.operation_id.is_none());
        assert_eq!(
            reply.received_evidence.as_ref().unwrap().receipts,
            vec![expected.clone()]
        );
        let wire = serde_json::from_slice(&serde_json::to_vec(&reply).unwrap()).unwrap();
        crate::serialization::query_schema::record_fixture(
            if access {
                "remote-parent-true"
            } else {
                "remote-parent-false"
            },
            false,
            &wire,
        );
        assert_eq!(
            reply.mcp().structured_content.unwrap()["receivedEvidence"]["receipts"][0],
            expected
        );
    }
    let mut malformed = vec![];
    for (field, value) in [
        ("receiptVersion", json!(2)),
        ("committedGeneration", json!(42)),
        ("operationId", json!(42)),
        ("storeId", json!(uuid::Uuid::nil())),
        ("unexpected", json!(true)),
    ] {
        let mut v = expected.clone();
        v[field] = value;
        malformed.push(v);
    }
    for field in [
        "storeId",
        "operationId",
        "committedGeneration",
        "receiptVersion",
        "outcome",
        "replayed",
    ] {
        let mut v = expected.clone();
        v.as_object_mut().unwrap().remove(field);
        malformed.push(v);
    }
    assert!(expected["outcome"].get("item_count").is_some());
    for (field, value) in [
        ("kind", json!("futureUnknownKind")),
        ("item_count", json!(1.5)),
        ("item_count", json!(u64::MAX)),
        ("extraNested", json!(true)),
    ] {
        let mut v = expected.clone();
        v["outcome"][field] = value;
        malformed.push(v);
    }
    for value in malformed {
        let reply = peer_query(
            query_input(true, "default"),
            json!({"wireVersion":1,"httpStatus":200,
            "reply":{"receipt":value,"malformed":true}}),
        )
        .await;
        assert_eq!(reply.http_status, 503);
        assert!(
            reply.received_evidence.is_none(),
            "invalid evidence was acquired: {value}"
        );
        assert_eq!(
            reply.envelope.error.unwrap().commit_state,
            crate::serialization::contract::CommitState::Unknown
        );
    }
    // Authored malicious peer payload: valid complete Rust codec, NOT a
    // canonical stored rejection or a claim about ordinary producer limits.
    // Escaped quotes approach the second-encoding bound. The genuine retained
    // BatchReserved identity/bytes above remain unchanged in storage.
    let mut large = receipt.clone();
    large.outcome = MutationOutcome::BatchItemRejected {
        batch_operation_id: receipt.operation_id,
        index: 0,
        error: crate::storage::batch::BatchError {
            code: String::new(),
            field: None,
        },
    };
    let mut large =
        serde_json::from_slice::<serde_json::Value>(&serde_json::to_vec(&large).unwrap()).unwrap();
    let mut result =
        json!({"wireVersion":1,"httpStatus":200,"reply":{"receipt":large,"invalidPadding":""}});
    let fixed = serde_json::to_vec(&DaemonResponse::success(2, result.clone()))
        .unwrap()
        .len();
    let limit = crate::daemon::prepared_query::response_limit();
    large["outcome"]["error"]["code"] = json!("\"".repeat((limit - fixed) / 2));
    result["reply"]["receipt"] = large.clone();
    let used = serde_json::to_vec(&DaemonResponse::success(2, result.clone()))
        .unwrap()
        .len();
    result["reply"]["invalidPadding"] = json!("x".repeat(limit - used));
    assert_eq!(
        serde_json::to_vec(&DaemonResponse::success(2, result.clone()))
            .unwrap()
            .len(),
        limit
    );
    let at_limit = peer_query(query_input(true, "default"), result.clone()).await;
    assert_eq!(
        at_limit.received_evidence.as_ref().unwrap().receipts,
        vec![large]
    );
    let mcp = at_limit.mcp();
    let mcp_bytes = serde_json::to_vec(&mcp).unwrap();
    let framed = serde_json::to_vec(&DaemonResponse::success(
        u64::MAX,
        serde_json::to_value(mcp).unwrap(),
    ))
    .unwrap();
    let rest = serde_json::to_vec(&at_limit).unwrap();
    eprintln!(
        "R1_UNVERIFIED_AT_LIMIT input={limit} MCP-inner={} framed={} REST={}",
        mcp_bytes.len(),
        framed.len(),
        rest.len()
    );
    assert!(framed.len() <= MAX_MESSAGE_SIZE && rest.len() <= MAX_MESSAGE_SIZE);
    let pad = result["reply"]["invalidPadding"]
        .as_str()
        .unwrap()
        .to_owned()
        + "x";
    result["reply"]["invalidPadding"] = json!(pad);
    assert_eq!(
        serde_json::to_vec(&DaemonResponse::success(2, result.clone()))
            .unwrap()
            .len(),
        limit + 1
    );
    let over = peer_query(query_input(true, "default"), result).await;
    assert!(
        over.received_evidence.is_none(),
        "over-limit bytes rejected before typed acquisition"
    );
    assert_eq!(
        over.envelope.error.unwrap().commit_state,
        crate::serialization::contract::CommitState::Unknown
    );
    assert_eq!(engine.control_state().unwrap(), before);
    assert_eq!(
        engine
            .meta_store()
            .lookup_batch(receipt.store_id, receipt.operation_id)
            .unwrap()
            .unwrap(),
        progress
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_remote_whole_budget_and_unnegotiated_alias_precede_exposure() {
    let f = Fixture::new();
    let completion = f.completion();
    let mut ids = vec![];
    for _ in 0..2 {
        let receipt = completion
            .insert(
                crate::storage::control::MutationContext::unkeyed(crate::model::NamespaceId::new(
                    1,
                )),
                crate::storage::insertion::InsertMemory {
                    id: None,
                    summary: "large prepared payload".into(),
                    full_text: Some("x".repeat(crate::model::constants::FULL_TEXT_MAX_BYTES)),
                    tags: vec![crate::model::Tag::new("allow").unwrap()],
                    initial_stability: None,
                    created_at: Some(100),
                    embedding: Some(vec![1., 0., 0., 0.]),
                    parent_id: None,
                    supersedes: None,
                },
                Vec::new().into(),
                100,
            )
            .await
            .unwrap();
        ids.push(crate::storage::insertion::inserted_id(&receipt.receipt).unwrap());
    }
    let snapshot = || {
        (
            f.storage.read().unwrap().control_state().unwrap(),
            ids.iter()
                .map(|id| f.record(*id).unwrap().encode().unwrap())
                .collect::<Vec<_>>(),
            ["meta.db", "edges.db", "fulltext.dat", "default/vectors.dat"]
                .map(|name| std::fs::read(f.path().join(name)).unwrap()),
        )
    };
    let before = snapshot();
    let daemon = PreparedDaemon::start(search(&f)).await;
    let input = query_input(true, "default");
    let denied=daemon.client.call(crate::daemon::prepared_query::QUERY,
        json!({"arguments":input.rpc_arguments().unwrap(),"responseLimit":crate::daemon::prepared_query::response_limit()})).await;
    assert!(
        denied.is_err(),
        "unnegotiated alias must reject before owned work"
    );
    assert_eq!(snapshot(), before);
    let remote = RemoteSearchAdapter::new(daemon.client.clone());
    for access in [false, true] {
        let reply = remote.query_contract(query_input(access, "default")).await;
        assert_eq!(reply.http_status, 400, "{reply:?}");
        assert_eq!(
            reply.envelope.error.as_ref().unwrap().code,
            "RESPONSE_TOO_LARGE"
        );
        assert_eq!(
            reply.envelope.error.as_ref().unwrap().commit_state,
            crate::serialization::contract::CommitState::NotCommitted
        );
        assert!(reply.envelope.receipt.is_none() && reply.observation_receipts.is_empty());
        assert_eq!(snapshot(), before);
    }
    let mut input = query_input(true, "default");
    input.query.limit = 1;
    let reply = remote.query_contract(input).await;
    assert_eq!(reply.http_status, 200, "bounded real positive: {reply:?}");
    assert!(reply.envelope.readiness.as_ref().unwrap().ready);
    assert_eq!(reply.observation_receipts.len(), 1);
    let wrapped =
        crate::daemon::prepared_query::compact_value(&crate::daemon::prepared_query::Reply {
            wire_version: 1,
            http_status: 200,
            reply: &reply,
        })
        .unwrap();
    let daemon_bytes = serde_json::to_vec(&DaemonResponse::success(u64::MAX, wrapped)).unwrap();
    let mcp = serde_json::to_vec(&reply.mcp()).unwrap();
    let rest = serde_json::to_vec(&reply).unwrap();
    eprintln!(
        "R1_REAL_BUDGET F={} daemon={} MCP-inner={} REST={}",
        crate::daemon::prepared_query::response_limit(),
        daemon_bytes.len(),
        mcp.len(),
        rest.len()
    );
    assert!(daemon_bytes.len() <= crate::daemon::prepared_query::response_limit());
    assert!(mcp.len() <= MAX_MESSAGE_SIZE && rest.len() <= MAX_MESSAGE_SIZE);
    drop(remote);
    daemon.finish().await;
}

#[tokio::test]
async fn prepared_remote_capability_refusal_and_lost_reply_never_fallback_or_replay() {
    use crate::daemon::protocol::{
        LEGACY_MAX_MESSAGE_SIZE, read_framed_message, write_framed_message,
    };
    use crate::serialization::contract::CommitState;
    for (capable, limit) in [(false, MAX_MESSAGE_SIZE), (true, LEGACY_MAX_MESSAGE_SIZE)] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("old.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            for _ in 0..2 {
                let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
                assert_eq!(
                    ping.method, "ping",
                    "unsupported peer must receive no query or legacy fallback"
                );
                write_framed_message(&mut stream,&DaemonResponse::success(ping.id,
                    json!({"protocolVersion":2,"maxMessageSize":limit,"preparedQueriesV1":capable})),MAX_MESSAGE_SIZE).await.unwrap();
            }
        });
        let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
        let remote = RemoteSearchAdapter::new(client.clone());
        let reply = remote.query_contract(query_input(true, "default")).await;
        assert_eq!(reply.http_status, 503);
        assert_eq!(
            reply.envelope.error.unwrap().commit_state,
            CommitState::NotCommitted
        );
        assert!(reply.received_evidence.is_none() && reply.observation_receipts.is_empty());
        client.ping().await.unwrap();
        task.await.unwrap();
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("lost.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(ping.method, "ping");
        write_framed_message(&mut stream,&DaemonResponse::success(ping.id,
            json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"preparedQueriesV1":true})),MAX_MESSAGE_SIZE).await.unwrap();
        let sent = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(sent.method, crate::daemon::prepared_query::QUERY);
        drop(stream);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "no reconnect/replay after query send"
        );
    });
    let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
    let reply = RemoteSearchAdapter::new(client.clone())
        .query_contract(query_input(true, "default"))
        .await;
    assert_eq!(reply.http_status, 503);
    assert_eq!(
        reply.envelope.error.unwrap().commit_state,
        CommitState::Unknown
    );
    assert!(
        reply.envelope.receipt.is_none()
            && reply.received_evidence.is_none()
            && reply.observation_receipts.is_empty()
    );
    assert!(!client.is_connected());
    task.await.unwrap();
}

fn search(f: &Fixture) -> Arc<McpSearchAdapter> {
    let provider = Arc::new(crate::embedding::PassthroughProvider::new(4));
    let rif = Arc::new(crate::rif::RifEngine::new(crate::rif::RifConfig {
        enabled: false,
        ..Default::default()
    }));
    let engine = crate::search::QueryEngine::new(
        Arc::new(StorageNamespaceResolver::new(f.storage.clone())),
        Arc::new(SingleEmbeddingRegistry::new(provider.clone())),
        Arc::new(SharedVectorIndexRegistry::new(f.vector_index.clone())),
        Arc::new(SharedFtsIndexRegistry::new(f.fts_index.clone())),
        Arc::new(SharedEntityIndexReader::new(f.entity_index.clone())),
        Arc::new(CacheManagerAdapter::new(f.cache.clone())),
        Arc::new(StorageMetadataAdapter::new(f.storage.clone())),
        Arc::new(SharedGraphReader::new(f.graph.clone())),
        Arc::new(RifProcessorAdapter::new(rif.clone(), f.graph.clone())),
        Arc::new(StorageAccessRecorder::new(
            f.storage.clone(),
            f.cache.clone(),
        )),
    )
    .with_lifecycle_inputs(
        Arc::new(crate::time::ManualClock::new(2000)),
        Default::default(),
        1.0,
    )
    .with_coherent_owner(f.completion(), rif);
    Arc::new(McpSearchAdapter::new(
        Arc::new(engine),
        provider,
        f.storage.clone(),
        f.graph.clone(),
        chrono_tz::UTC,
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_remote_actual_query_matches_owned_false_read() {
    let f = Fixture::new();
    let id = f
        .insert_memory_with(MemorySpec {
            summary: "prepared Remote retained record",
            tags: &["allow"],
            embedding: Some(&[1., 0., 0., 0.]),
            ..MemorySpec::new(crate::model::NamespaceId::new(1))
        })
        .await;
    f.completion().rebuild_all().await.unwrap();
    let search = search(&f);
    let arguments = json!({"contractVersion":2,"namespace":"default", "queryMode":"metadata_only",
        "tags":["allow"],"recordAccess":false,"limit":1,"explain":true});
    let input = || {
        crate::serialization::query::QueryRequest::from_mcp(&arguments, "wrong-default").unwrap()
    };
    let local = search.query_contract(input()).await;
    assert_eq!(local.http_status, 200, "actual local control: {local:?}");
    let local_value =
        serde_json::from_slice::<serde_json::Value>(&serde_json::to_vec(&local).unwrap()).unwrap();
    assert_eq!(
        local_value["data"]["results"][0]["memoryId"],
        id.to_string()
    );
    let before = f.storage.read().unwrap().control_state().unwrap();
    let record = f.record(id).unwrap().encode().unwrap();

    let daemon = PreparedDaemon::start(search).await;
    let remote = RemoteSearchAdapter::new(daemon.client.clone());
    let reply = remote.query_contract(input()).await;
    drop(remote);
    daemon.finish().await;
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert_eq!(f.record(id).unwrap().encode().unwrap(), record);
    eprintln!(
        "R1_REMOTE_BOUNDARY local={} remote={} reply={reply:?}",
        local.http_status, reply.http_status
    );
    assert_eq!(
        reply.http_status, 200,
        "Remote prepared method must reach the real consumer"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&serde_json::to_vec(&reply).unwrap()).unwrap(),
        local_value
    );
}
