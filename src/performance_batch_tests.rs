//! Actual legacy durable batch instrumentation; no native or performance claim.
use super::*;
use crate::storage::batch::{BatchOutcome, BatchReservationState, BatchStatus};

fn batch_request() -> String {
    serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "_meta":{META_PROTOCOL_VERSION:PROTOCOL_VERSION, META_CLIENT_CAPABILITIES:{}},
        "name":"store_memories","arguments":{"batchResultVersion":1,
            "idempotencyKey":"private-batch-key-canary","memories":[
                {"summary":"private-batch-input-canary","fullText":"private-batch-body-canary","checkDuplicates":false},
                {"summary":""}
            ]}
    }}).to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_batch_actual_partial_stdio_requires_parent_and_child_facts() {
    let fx = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let server = server(&fx, calls.clone());
    let before = fx.storage.read().unwrap().control_state().unwrap();
    let output = Output::default();
    let captured = output.clone();
    let capture = Capture::default();
    let input = format!("{}\n", batch_request());
    crate::mcp::transport::serve_io(server, input.as_bytes(), output)
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    assert_eq!(captured.flushes.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let response: serde_json::Value =
        serde_json::from_slice(&captured.bytes.lock().unwrap()).unwrap();
    assert!(response.get("error").is_none(), "{response}");
    let value: serde_json::Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let outcome: BatchOutcome = serde_json::from_value(value["batch"].clone()).unwrap();
    assert_eq!(outcome.reservation_state, BatchReservationState::Committed);
    assert_eq!(
        (outcome.succeeded, outcome.failed, outcome.unresolved),
        (1, 1, 0)
    );
    assert_eq!(outcome.items[0].status, BatchStatus::Ok);
    assert_eq!(outcome.items[1].status, BatchStatus::Error);
    assert!(outcome.items[0].receipt.is_some());
    assert!(outcome.items[1].receipt.is_none());
    assert!(outcome.all_ready());
    let storage = fx.storage.read().unwrap();
    assert_eq!(storage.scan_all().unwrap().len(), 1);
    assert_eq!(
        storage.control_state().unwrap().committed_generation.get(),
        before.committed_generation.get() + 2
    );
    let progress = storage
        .meta_store()
        .lookup_batch(outcome.store_id, outcome.batch_operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        Some(progress.reservation_receipt),
        outcome.reservation_receipt
    );
    assert!(
        storage
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    drop(storage);
    let trace = events(&capture);
    // All existing mutation/delivery assertions above precede the intended RED.
    assert!(
        trace.iter().any(|e| e["kind"] == "batch_reservation"
            && e["state"] == "committed"
            && e["replayed"] == false),
        "missing actual batch reservation diagnostics"
    );
    assert!(trace.iter().any(|e| e["kind"] == "commit_call"
        && e["resource"] == "batch_reservation"
        && e["phase"] == "return"
        && e["success"] == true));
    assert!(trace.iter().any(|e| e["kind"] == "batch_outcome"
        && e["succeeded"] == 1
        && e["failed"] == 1
        && e["unresolved"] == 0));
}
