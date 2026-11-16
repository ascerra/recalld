//! Actual prepared adapter vs Remote boundary; no native fixture or activation.
use super::*;
use crate::daemon::bridge_adapters::RemoteSearchAdapter;
use crate::search::adapters::*;
use crate::test_support::{Fixture, MemorySpec};
use serde_json::json;

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
        Arc::new(StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())),
    )
    .with_lifecycle_inputs(
        Arc::new(crate::time::ManualClock::new(2000)),
        Default::default(),
        1.0,
    )
    .with_coherent_owner(f.completion(), rif);
    Arc::new(McpSearchAdapter::new(
        Arc::new(engine), provider, f.storage.clone(), f.graph.clone(), chrono_tz::UTC,
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_remote_actual_query_matches_owned_false_read() {
    let f = Fixture::new();
    let id = f.insert_memory_with(MemorySpec {
        summary: "prepared Remote retained record",
        tags: &["allow"],
        embedding: Some(&[1., 0., 0., 0.]),
        ..MemorySpec::new(crate::model::NamespaceId::new(1))
    }).await;
    f.completion().rebuild_all().await.unwrap();
    let search = search(&f);
    let arguments = json!({"contractVersion":2,"namespace":"default", "queryMode":"metadata_only",
        "tags":["allow"],"recordAccess":false,"limit":1,"explain":true});
    let input = || crate::serialization::query::QueryRequest::from_mcp(&arguments, "wrong-default").unwrap();
    let local = search.query_contract(input()).await;
    assert_eq!(local.http_status, 200, "actual local control: {local:?}");
    let local_value = serde_json::from_slice::<serde_json::Value>(&serde_json::to_vec(&local).unwrap()).unwrap();
    assert_eq!(local_value["data"]["results"][0]["memoryId"], id.to_string());
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
        handle_connection(search, unused.clone(), unused.clone(), unused, stream,
            Arc::new(AtomicU32::new(1)), Arc::new(AtomicI64::new(0)), rx, server_tx).await;
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
    eprintln!("R1_REMOTE_BOUNDARY local={} remote={} reply={reply:?}", local.http_status, reply.http_status);
    assert_eq!(reply.http_status, 200, "Remote prepared method must reach the real consumer");
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&serde_json::to_vec(&reply).unwrap()).unwrap(), local_value);
}
