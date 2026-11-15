//! Actual legacy durable batch instrumentation; no native or performance claim.
use super::*;
use crate::storage::batch::{BatchOutcome, BatchReservationState, BatchStatus};

fn batch_request(store_id: uuid::Uuid) -> String {
    serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "_meta":{META_PROTOCOL_VERSION:PROTOCOL_VERSION, META_CLIENT_CAPABILITIES:{}},
        "name":"store_memories","arguments":{"batchResultVersion":1,"storeId":store_id,
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
    let input = format!("{}\n", batch_request(before.store_id));
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
    // all_ready additionally requires zero rejected items; this is partial.
    assert!(!outcome.all_ready());
    assert!(outcome.items.iter().all(|item| item.ready));
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

use crate::mcp::bridge::StorageEngine as _;
use crate::storage::batch::{BatchInput, BatchItemState, BatchKind};
fn typed_request(fx: &Fixture, items: Vec<serde_json::Value>) -> BatchInput {
    BatchInput {
        result_version: 1,
        kind: BatchKind::Insert,
        items,
        default_namespace: "default".into(),
        store_id: Some(fx.storage.read().unwrap().control_state().unwrap().store_id),
        idempotency_key: Some("private-key-canary".into()),
        batch_operation_id: None,
    }
}
fn explicit_item() -> serde_json::Value {
    serde_json::json!({"summary":"private-batch-input-canary", "embedding":[1.,0.,0.,0.], "checkDuplicates":false})
}
fn generation(fx: &Fixture) -> u64 {
    fx.storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation
        .get()
}
fn count(trace: &[serde_json::Value], kind: &str, phase: &str) -> usize {
    trace
        .iter()
        .filter(|e| e["kind"] == kind && e["phase"] == phase)
        .count()
}
fn assert_no_fresh_work(trace: &[serde_json::Value]) {
    assert!(
        !trace.iter().any(|e| matches!(
            e["kind"].as_str(),
            Some("provider_call" | "payload_append" | "sync_call" | "commit_call" | "batch_child")
        )),
        "{trace:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_batch_flush_failure_replays_inventory_without_fresh_work() {
    let fx = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let server = server(&fx, calls.clone());
    let input = format!(
        "{}\n",
        batch_request(fx.storage.read().unwrap().control_state().unwrap().store_id)
    );
    let mut prior = None;
    let mut committed_generation = 0;
    for failed_delivery in [true, false] {
        let capture = Capture::default();
        let output = Output {
            fail_flush: failed_delivery,
            ..Default::default()
        };
        let bytes = output.bytes.clone();
        let result = crate::mcp::transport::serve_io(server.clone(), input.as_bytes(), output)
            .with_subscriber(subscriber(capture.clone()))
            .await;
        assert_eq!(result.is_err(), failed_delivery);
        let reply: serde_json::Value = serde_json::from_slice(&bytes.lock().unwrap()).unwrap();
        let payload: serde_json::Value =
            serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        let outcome: BatchOutcome = serde_json::from_value(payload["batch"].clone()).unwrap();
        assert_eq!(
            (outcome.succeeded, outcome.failed, outcome.unresolved),
            (1, 1, 0)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let identity = (
            outcome.batch_operation_id,
            outcome.items[0].receipt.as_ref().unwrap().clone(),
        );
        let trace = events(&capture);
        if failed_delivery {
            committed_generation = generation(&fx);
            prior = Some(identity);
            assert_eq!(count(&trace, "commit_call", "attempt"), 2);
        } else {
            assert_eq!(Some(identity), prior);
            assert_eq!(generation(&fx), committed_generation);
            assert_no_fresh_work(&trace);
            assert!(
                trace
                    .iter()
                    .any(|e| e["kind"] == "batch_reservation" && e["replayed"] == true)
            );
            assert!(
                trace
                    .iter()
                    .any(|e| e["kind"] == "batch_outcome" && e["retained_child_receipts"] == 1)
            );
        }
        assert!(
            trace
                .iter()
                .any(|e| e["kind"] == "transport_delivery" && e["success"] == !failed_delivery)
        );
        let raw = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
        for secret in [
            "private-batch-key-canary",
            "private-batch-input-canary",
            "private-batch-body-canary",
        ] {
            assert!(!raw.contains(secret));
        }
        for row in raw_events(&capture).iter().filter(|e| {
            e["fields"]["kind"] == "batch_reservation"
                || e["fields"]["stage"] == "batch.progress.queue"
        }) {
            assert_eq!(row["spans"][0]["intent"], "unknown");
            assert_eq!(row["spans"][0]["operation"], "mcp.stdio");
        }
    }
}

#[tokio::test]
async fn legacy_batch_all_invalid_is_parent_commit_not_child_work() {
    let fx = Fixture::new();
    let before = generation(&fx);
    let adapter = fx.storage_adapter();
    let input = typed_request(&fx, vec![serde_json::json!({"summary":""})]);
    for replay in [false, true] {
        let capture = Capture::default();
        let result = adapter
            .mutate_batch(input.clone())
            .with_subscriber(subscriber(capture.clone()))
            .await
            .unwrap();
        assert_eq!(
            (result.succeeded, result.failed, result.unresolved),
            (0, 1, 0)
        );
        assert_eq!(generation(&fx), before + 1);
        assert!(fx.storage.read().unwrap().scan_all().unwrap().is_empty());
        let trace = events(&capture);
        assert_eq!(count(&trace, "batch_child", "attempt"), 0);
        assert_eq!(
            count(&trace, "commit_call", "attempt"),
            usize::from(!replay)
        );
        assert!(
            trace
                .iter()
                .any(|e| e["kind"] == "batch_reservation" && e["replayed"] == replay)
        );
    }
}

#[tokio::test]
async fn legacy_batch_raw_commit_success_and_caller_unknown_reconcile_only_after_reopen() {
    for stage in [1, 2, 3] {
        let fx = Fixture::new();
        let adapter = fx.storage_adapter();
        fx.storage
            .read()
            .unwrap()
            .meta_store()
            .fail_batch_commit
            .store(stage, Ordering::SeqCst);
        let mut item = explicit_item();
        if stage == 2 {
            item["embedding"] = serde_json::json!([0., 0., 0., 0.]);
        }
        let input = typed_request(&fx, vec![item, explicit_item()]);
        let capture = Capture::default();
        let outcome = adapter
            .mutate_batch(input)
            .with_subscriber(subscriber(capture.clone()))
            .await
            .unwrap();
        assert_eq!(outcome.items[0].status, BatchStatus::Unknown);
        assert!(!outcome.items[0].ready);
        let trace = events(&capture);
        let resource = match stage {
            1 => "batch_reservation",
            2 => "batch_rejection",
            _ => "metadata_effects",
        };
        assert!(trace.iter().any(|e| e["kind"] == "commit_call"
            && e["resource"] == resource
            && e["phase"] == "return"
            && e["success"] == true));
        let kind = match stage {
            1 => "batch_reservation",
            2 => "batch_bookkeeping",
            _ => "batch_child",
        };
        assert!(
            trace
                .iter()
                .any(|e| e["kind"] == kind && e["state"] == "unknown")
        );
        assert_eq!(
            count(&trace, "batch_child", "attempt"),
            usize::from(stage != 1)
        );
        assert!(
            fx.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .status()
                .unresolved
                > 0
        );
        drop(adapter);
        let dir = fx.close();
        let engine = crate::storage::RedbStorageEngine::open(dir.path()).unwrap();
        let progress = engine
            .meta_store()
            .lookup_batch(outcome.store_id, outcome.batch_operation_id)
            .unwrap()
            .unwrap();
        match stage {
            1 => assert!(
                progress
                    .items
                    .iter()
                    .all(|i| matches!(i.state, BatchItemState::NotAttempted {}))
            ),
            2 => {
                assert!(matches!(
                    progress.items[0].state,
                    BatchItemState::Error { .. }
                ));
                assert_eq!(
                    progress.bookkeeping_receipts[0].operation_id,
                    outcome.unknown_bookkeeping_operations[0]
                );
            }
            _ => assert!(
                matches!(&progress.items[0].state,BatchItemState::Ok{receipt} if Some(receipt.operation_id)==outcome.items[0].operation_id)
            ),
        }
    }
}

#[tokio::test]
async fn legacy_batch_rejection_bookkeeping_and_resume_count_actual_attempts() {
    struct FailSecond(Arc<AtomicUsize>);
    #[async_trait::async_trait]
    impl EmbeddingProvider for FailSecond {
        async fn embed(&self, _: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(if self.0.fetch_add(1, Ordering::SeqCst) == 1 {
                vec![0.; 4]
            } else {
                vec![1., 0., 0., 0.]
            })
        }
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> &str {
            "offline-fail-second"
        }
    }
    let fx = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut adapter = fx.storage_adapter();
    adapter.embedding = Arc::new(FailSecond(calls.clone()));
    let mut invalid = explicit_item();
    invalid["embedding"] = serde_json::json!([0., 0., 0., 0.]);
    let generated =
        serde_json::json!({"summary":"private-generated-canary","checkDuplicates":false});
    let input = typed_request(
        &fx,
        vec![generated.clone(), invalid, generated, explicit_item()],
    );
    let before = generation(&fx);
    let capture = Capture::default();
    let first = adapter
        .mutate_batch(input.clone())
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    assert_eq!((first.succeeded, first.failed, first.unresolved), (1, 1, 2));
    assert_eq!(first.items[2].status, BatchStatus::NotAttempted);
    assert_eq!(first.bookkeeping_receipts.len(), 1);
    assert_eq!(generation(&fx), before + 3);
    let trace = events(&capture);
    assert_eq!(count(&trace, "batch_child", "attempt"), 3);
    assert_eq!(count(&trace, "commit_call", "attempt"), 3);
    assert!(
        trace
            .iter()
            .any(|e| e["kind"] == "batch_child" && e["state"] == "provider_unavailable")
    );
    assert!(
        trace
            .iter()
            .any(|e| e["kind"] == "batch_bookkeeping" && e["state"] == "returned_progress")
    );
    let capture = Capture::default();
    let resumed = adapter
        .mutate_batch(input.clone())
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    assert_eq!(
        (resumed.succeeded, resumed.failed, resumed.unresolved),
        (3, 1, 0)
    );
    assert_eq!(resumed.items[0].receipt, first.items[0].receipt);
    assert_eq!(resumed.bookkeeping_receipts, first.bookkeeping_receipts);
    assert_eq!(generation(&fx), before + 5);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    let trace = events(&capture);
    assert_eq!(count(&trace, "batch_child", "attempt"), 2);
    assert_eq!(count(&trace, "commit_call", "attempt"), 2);
    assert!(
        trace
            .iter()
            .any(|e| e["kind"] == "batch_outcome" && e["retained_bookkeeping_receipts"] == 1)
    );
    let capture = Capture::default();
    adapter
        .mutate_batch(input)
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    assert_no_fresh_work(&events(&capture));
}

#[tokio::test]
async fn legacy_batch_committed_unready_keeps_receipt_and_repairs_on_replay() {
    let fx = Fixture::new();
    let adapter = fx.storage_adapter();
    let input = typed_request(&fx, vec![explicit_item()]);
    *fx.vector_index.write().await = FlatVectorIndex::new(5);
    let capture = Capture::default();
    let first = adapter
        .mutate_batch(input.clone())
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    assert_eq!(first.succeeded, 1);
    assert!(!first.items[0].ready);
    assert!(first.items[0].receipt.is_some());
    assert!(
        events(&capture)
            .iter()
            .any(|e| e["kind"] == "batch_child" && e["state"] == "committed_unready")
    );
    let committed = generation(&fx);
    *fx.vector_index.write().await = FlatVectorIndex::new(4);
    let capture = Capture::default();
    let replay = adapter
        .mutate_batch(input)
        .with_subscriber(subscriber(capture.clone()))
        .await
        .unwrap();
    assert!(replay.all_ready());
    assert_eq!(replay.items[0].receipt, first.items[0].receipt);
    assert_eq!(generation(&fx), committed);
    assert_no_fresh_work(&events(&capture));
}

#[tokio::test]
async fn legacy_batch_disabled_unsampled_and_selected_inherit_single_decision() {
    for selected in [false, true] {
        let fx = Fixture::new();
        let adapter = fx.storage_adapter();
        let input = typed_request(&fx, vec![explicit_item()]);
        let capture = Capture::default();
        let sampler = Sampler::new(std::num::NonZeroU64::new(100));
        let decision = Decision {
            selected,
            every: 100,
            root_ordinal: 27,
            sampled_roots_seen: 1,
        };
        let result = REQUEST
            .scope(
                decision,
                measure_with_sampler(
                    "test.batch_root",
                    true,
                    adapter.mutate_batch(input),
                    Some(&sampler),
                ),
            )
            .with_subscriber(subscriber(capture.clone()))
            .await
            .unwrap();
        assert!(result.all_ready());
        let rows = raw_events(&capture);
        if !selected {
            assert!(rows.is_empty());
        } else {
            assert!(
                rows.iter()
                    .any(|e| e["fields"]["kind"] == "batch_reservation")
            );
            for row in rows.iter().filter(|e| {
                e["fields"]["kind"] == "batch_reservation" || e["fields"]["kind"] == "commit_call"
            }) {
                assert_eq!(row["spans"][0]["operation"], "test.batch_root");
                assert_eq!(row["spans"][0]["root_ordinal"], 27);
            }
        }
        assert_eq!(sampler.roots.load(Ordering::Relaxed), 0);
    }
    // Disabled diagnostic-only primitives must not evaluate fields/clock or allocate.
    let decision = Decision {
        selected: false,
        every: 100,
        root_ordinal: 1,
        sampled_roots_seen: 1,
    };
    REQUEST.sync_scope(decision, || {
        assert_eq!(
            allocation_probe::count(|| {
                let timer = Timer::new("batch.disabled");
                assert!(timer.start.is_none());
                let timeline = Timeline::new();
                assert!(timeline.start.is_none());
                Worker::new("batch.disabled.queue").run(|| {
                    event!(
                        kind = "disabled",
                        value = {
                            panic!("evaluated disabled field");
                            #[allow(unreachable_code)]
                            0
                        }
                    )
                });
            }),
            0
        )
    });
}

#[test]
fn legacy_batch_aborted_root_does_not_drain_queued_reservation() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let fx = Fixture::new();
        let adapter = fx.storage_adapter();
        let input = typed_request(&fx, vec![explicit_item()]);
        let before = generation(&fx);
        let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            entered_tx.send(()).unwrap();
            let _ = release_rx.recv();
        });
        entered_rx.await.unwrap();
        let capture = Capture::default();
        let task = tokio::spawn(
            async move { adapter.mutate_batch(input).await }
                .with_subscriber(subscriber(capture.clone())),
        );
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while tracker.status().pending < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(tracker.status().pending > 0);
        assert!(tracker.status().unresolved > 0);
        let trace = events(&capture);
        assert!(!trace.iter().any(|e| e["kind"] == "batch_reservation"));
        assert!(
            trace
                .iter()
                .any(|e| e["stage"] == "mcp.batch" && e["outcome"] == "cancelled_error_or_unwound")
        );
        release_tx.send(()).unwrap();
        blocker.await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !events(&capture)
                .iter()
                .any(|e| e["stage"] == "batch.reservation.lock.hold")
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(generation(&fx), before + 1);
        assert!(fx.storage.read().unwrap().scan_all().unwrap().is_empty());
        assert!(
            tracker.status().unresolved > 0,
            "worker success does not settle dropped root"
        );
        let trace = events(&capture);
        assert!(
            trace
                .iter()
                .any(|e| e["kind"] == "batch_reservation" && e["state"] == "committed")
        );
        assert_eq!(count(&trace, "batch_child", "attempt"), 0);
        let rows = raw_events(&capture);
        let end = rows
            .iter()
            .find(|e| e["fields"]["stage"] == "mcp.batch")
            .unwrap();
        let returned = rows
            .iter()
            .find(|e| e["fields"]["kind"] == "batch_reservation")
            .unwrap();
        assert_eq!(
            end["spans"][0]["trace_id"],
            returned["spans"][0]["trace_id"]
        );
    });
}
