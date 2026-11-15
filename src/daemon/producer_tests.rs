use super::*;
use crate::storage::StorageEngine as _;
use crate::test_support::{Fixture, StubBridge};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_producer_actual_daemon_socket_frames_include_complete_delivery() {
    let fx = Fixture::new();
    let shared = Arc::new(StubBridge::default());
    let storage = Arc::new(fx.storage_adapter()); // Explicit-vector, no provider fallback.
    let (mut client, stream) = tokio::net::UnixStream::pair().unwrap();
    let (shutdown, rx) = watch::channel(false);
    let connections = Arc::new(AtomicU32::new(1));
    let count = connections.clone();
    let task = tokio::spawn(crate::performance::tests::capture_full(handle_connection(
        shared.clone(),
        storage,
        shared.clone(),
        shared,
        stream,
        count,
        Arc::new(AtomicI64::new(0)),
        rx,
        shutdown,
    )));
    let request = DaemonRequest {
        id: 1,
        method: "store_memory".into(),
        params: serde_json::json!({
            "summary":"private-daemon-canary", "fullText":"private-body-canary", "tags":[],
            "namespace":"default", "embedding":[1.0,0.0,0.0,0.0], "checkDuplicates":false
        }),
    };
    let bytes = serde_json::to_vec(&request).unwrap().len();
    write_framed_request(&mut client, &request, MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    let response = read_framed_response(&mut client, MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    assert!(response.error.is_none(), "{response:?}");
    let response_bytes = serde_json::to_vec(&response).unwrap().len();
    drop(client);
    let (_, rows) = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connections.load(Ordering::Relaxed), 0);
    assert_eq!(fx.storage.read().unwrap().scan_all().unwrap().len(), 1);
    assert!(
        fx.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    assert!(rows.iter().any(|e| e["fields"]["kind"] == "transport_input"
        && e["fields"]["source_bytes"] == bytes
        && e["fields"]["framing_bytes"] == 4));
    assert!(
        rows.iter()
            .any(|e| e["fields"]["kind"] == "transport_output"
                && e["fields"]["phase"] == "complete"
                && e["fields"]["source_bytes"] == response_bytes)
    );
    assert!(!rows.iter().any(|e| e["fields"]["kind"] == "provider_call"));
    let worker = rows
        .iter()
        .find(|e| e["fields"]["stage"] == "insert.worker_queue")
        .unwrap();
    assert_eq!(worker["spans"][0]["operation"], "daemon.rpc");
    assert_eq!(worker["spans"][0]["intent"], "unknown");
    assert!(worker["spans"][0].get("record_access").is_none());
    let raw = serde_json::to_string(&rows).unwrap();
    assert!(!raw.contains("private-daemon-canary"));
    assert!(!raw.contains("private-body-canary"));
}
