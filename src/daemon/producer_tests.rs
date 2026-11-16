use super::*;
use crate::storage::StorageEngine as _;
use crate::test_support::{Fixture, StubBridge};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trace_provenance_actual_daemon_after_unsubscribed_first_callsite() {
    const CHILD: &str = "RECALLD_DAEMON_COLD_TRACE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // Cold process-global callsites, even when the parent runs a full suite.
        // The marker belongs only to this subprocess, never the parent env.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "daemon::server::producer_tests::trace_provenance_actual_daemon_after_unsubscribed_first_callsite",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "cold daemon child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    use tracing::instrument::WithSubscriber;
    let capture = crate::performance::tests::Capture::default();
    let dispatch = tracing::Dispatch::new(crate::performance::tests::subscriber(capture.clone()));
    assert!(
        !std::thread::spawn(crate::performance::enabled)
            .join()
            .unwrap()
    );
    let enabled_under_capture =
        tracing::dispatcher::with_default(&dispatch, crate::performance::enabled);
    let fx = Fixture::new();
    let shared = Arc::new(StubBridge::default());
    let (mut client, stream) = tokio::net::UnixStream::pair().unwrap();
    let (shutdown, rx) = watch::channel(false);
    let connections = Arc::new(AtomicU32::new(1));
    let task = tokio::spawn(
        handle_connection(
            shared.clone(),
            Arc::new(fx.storage_adapter()),
            shared.clone(),
            shared,
            stream,
            connections.clone(),
            Arc::new(AtomicI64::new(0)),
            rx,
            shutdown,
        )
        .with_subscriber(dispatch),
    );
    let request = DaemonRequest {
        jsonrpc: "2.0".into(),
        id: 1,
        method: "store_memory".into(),
        params: serde_json::json!({"summary":"cold-daemon-fixture", "tags":[],
            "namespace":"default", "embedding":[1.0,0.0,0.0,0.0], "checkDuplicates":false}),
    };
    let bytes = serde_json::to_vec(&request).unwrap().len();
    write_framed_request(&mut client, &request, MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    let payload = read_frame(&mut client, MAX_MESSAGE_SIZE).await.unwrap();
    let response: DaemonResponse = serde_json::from_slice(&payload).unwrap();
    assert!(response.error.is_none(), "{response:?}");
    drop(client);
    tokio::time::timeout(Duration::from_secs(5), task)
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
    let raw = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    let rows: Vec<serde_json::Value> = raw
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    eprintln!(
        "ACTUAL_DAEMON enabled_under_capture={enabled_under_capture} captured_rows={} rows={rows:?}",
        rows.len()
    );
    assert!(rows.iter().any(|e| e["fields"]["kind"] == "transport_input"
        && e["fields"]["source_bytes"] == bytes
        && e["fields"]["framing_bytes"] == 4));
    assert!(
        rows.iter()
            .any(|e| e["fields"]["kind"] == "transport_output"
                && e["fields"]["phase"] == "complete"
                && e["fields"]["source_bytes"] == payload.len())
    );
}

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
        jsonrpc: "2.0".into(),
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
    let response_payload = read_frame(&mut client, MAX_MESSAGE_SIZE).await.unwrap();
    let response: DaemonResponse = serde_json::from_slice(&response_payload).unwrap();
    assert!(response.error.is_none(), "{response:?}");
    // Measure the actual wire bytes, not a second serialization of parsed JSON.
    let response_bytes = response_payload.len();
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_batch_actual_daemon_socket_frames_include_complete_delivery() {
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
        jsonrpc: "2.0".into(),
        id: 1,
        method: "mutate_batch".into(),
        params: serde_json::to_value(crate::storage::batch::BatchInput {
            request_context: None,
            expected_generation: None,
            mutation_version: 1,
            result_version: 1, kind: crate::storage::batch::BatchKind::Insert,
            default_namespace: "default".into(),
            store_id: Some(fx.storage.read().unwrap().control_state().unwrap().store_id),
            idempotency_key: Some("private-batch-key-canary".into()), batch_operation_id: None,
            items: vec![serde_json::json!({"summary":"private-daemon-canary", "embedding":[1.,0.,0.,0.], "checkDuplicates":false}), serde_json::json!({"summary":""})],
        }).unwrap(),
    };
    let bytes = serde_json::to_vec(&request).unwrap().len();
    write_framed_request(&mut client, &request, MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    let response_payload = read_frame(&mut client, MAX_MESSAGE_SIZE).await.unwrap();
    let response: DaemonResponse = serde_json::from_slice(&response_payload).unwrap();
    assert!(response.error.is_none(), "{response:?}");
    let outcome: crate::storage::batch::BatchOutcome =
        serde_json::from_value(response.result.clone().unwrap()).unwrap();
    assert_eq!(
        (outcome.succeeded, outcome.failed, outcome.unresolved),
        (1, 1, 0)
    );
    assert!(outcome.items[0].receipt.is_some());
    // Measure the actual wire bytes, not a second serialization of parsed JSON.
    let response_bytes = response_payload.len();
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
        .find(|e| e["fields"]["stage"] == "batch.reservation.queue")
        .unwrap();
    assert_eq!(worker["spans"][0]["operation"], "daemon.rpc");
    assert_eq!(worker["spans"][0]["intent"], "unknown");
    assert!(worker["spans"][0].get("record_access").is_none());
    assert!(rows.iter().any(|e| e["fields"]["kind"] == "batch_outcome"
        && e["fields"]["succeeded"] == 1
        && e["fields"]["failed"] == 1));
    assert!(rows.iter().any(|e| e["fields"]["kind"] == "commit_call"
        && e["fields"]["resource"] == "batch_reservation"
        && e["fields"]["success"] == true));
    let raw = serde_json::to_string(&rows).unwrap();
    assert!(!raw.contains("private-daemon-canary"));
    assert!(!raw.contains("private-body-canary"));
}
