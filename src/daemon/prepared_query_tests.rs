//! Actual prepared adapter vs Remote boundary; no native fixture or activation.
use super::*;
use crate::daemon::bridge_adapters::RemoteSearchAdapter;
use crate::search::adapters::*;
use crate::test_support::{Fixture, MemorySpec};
use serde_json::json;

fn query_input(access: bool, namespace: &str) -> crate::serialization::query::QueryRequest {
    crate::serialization::query::QueryRequest::from_mcp(
        &json!({"contractVersion":2,"namespace":namespace,"queryMode":"metadata_only",
            "tags":["allow"],"recordAccess":access,"limit":2,"explain":true}),
        "wrong-default",
    ).unwrap()
}

async fn peer_query(
    input: crate::serialization::query::QueryRequest,
    result: serde_json::Value,
) -> crate::serialization::query_response::QueryReply {
    use crate::daemon::protocol::{read_framed_message, write_framed_message};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("peer.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE).await.unwrap();
        assert_eq!(ping.method, "ping");
        let caps = json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"preparedQueriesV1":true});
        write_framed_message(&mut stream, &DaemonResponse::success(ping.id,caps.clone()),MAX_MESSAGE_SIZE).await.unwrap();
        let query = read_framed_message(&mut stream, MAX_MESSAGE_SIZE).await.unwrap();
        assert_eq!(query.method, crate::daemon::prepared_query::QUERY);
        write_framed_message(&mut stream,&DaemonResponse::success(query.id,result),MAX_MESSAGE_SIZE).await.unwrap();
        let next = read_framed_message(&mut stream,MAX_MESSAGE_SIZE).await.unwrap();
        assert_eq!(next.method,"ping","no replay/fallback after malformed reply");
        write_framed_message(&mut stream,&DaemonResponse::success(next.id,caps),MAX_MESSAGE_SIZE).await.unwrap();
    });
    let client=Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
    let reply=RemoteSearchAdapter::new(client.clone()).query_contract(input).await;
    client.ping().await.unwrap();
    task.await.unwrap();
    reply
}

#[tokio::test]
async fn prepared_remote_decoder_binds_actual_success_scope_and_exposure_coverage() {
    let f=Fixture::new();
    for _ in 0..2 {
        f.insert_memory_with(MemorySpec {summary:"prepared evidence",tags:&["allow"],
            embedding:Some(&[1.,0.,0.,0.]),..MemorySpec::new(crate::model::NamespaceId::new(1))}).await;
    }
    f.completion().rebuild_all().await.unwrap();
    let search=search(&f);
    let mut failures=vec![];
    for access in [false,true] {
        let local=search.query_contract(query_input(access,"default")).await;
        assert_eq!(local.http_status,200,"actual owned producer control: {local:?}");
        let original:serde_json::Value=serde_json::from_slice(&serde_json::to_vec(&local).unwrap()).unwrap();
        assert_eq!(original["data"]["results"].as_array().unwrap().len(),2);
        if access { assert_eq!(local.observation_receipts.len(),1); }
        let normal=peer_query(query_input(access,"default"),json!({"wireVersion":1,"httpStatus":200,"reply":original})).await;
        assert_eq!(normal.http_status,200,"unmodified actual producer must decode: {normal:?}");
        let cases=if access {
            let mut missing=original.clone();
            for key in ["receipt","observationReceipts","readiness"] { missing.as_object_mut().unwrap().remove(key); }
            let mut partial=original.clone();
            partial["observationReceipts"][0]["outcome"]["accesses"].as_array_mut().unwrap().pop();
            partial["receipt"]=partial["observationReceipts"][0].clone();
            let mut readiness=original.clone();
            readiness["readiness"]["derived"][0]["name"]=json!("fabricated");
            vec![("missing exposures",missing),("partial exposures",partial),("invalid readiness owner",readiness)]
        } else {
            let mut scope=original.clone();
            scope["data"]["results"][0]["namespaceId"]=json!(2);
            vec![("foreign row scope",scope)]
        };
        for (case,value) in cases {
            let result=peer_query(query_input(access,"default"),json!({"wireVersion":1,"httpStatus":200,"reply":value})).await;
            eprintln!("R1_DECODE_NEGATIVE {case}: status={}",result.http_status);
            if result.http_status!=503 { failures.push(case); }
        }
        let canonical=peer_query(query_input(access,"DeFaUlT"),json!({"wireVersion":1,"httpStatus":200,"reply":original})).await;
        eprintln!("R1_CASE_CONTROL access={access}: status={}",canonical.http_status);
        if canonical.http_status!=200 { failures.push("canonical namespace casing"); }
    }
    assert!(failures.is_empty(),"actual producer/malformed peer cases: {failures:?}");
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

    let socket_dir = tempfile::tempdir().unwrap();
    let path = socket_dir.path().join("prepared.sock");
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
    client.ping().await.unwrap();
    let remote = RemoteSearchAdapter::new(client.clone());
    let reply = remote.query_contract(input()).await;
    // Exercise the same surviving connection even on the expected BEFORE error.
    client.ping().await.unwrap();
    tx.send(true).unwrap();
    drop(remote);
    drop(client);
    server.await.unwrap();
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
