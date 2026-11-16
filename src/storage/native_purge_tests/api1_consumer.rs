//! Actual API1 namespace adapter against an explicitly owned native fixture.
use super::*;
use crate::mcp::bridge::{BridgeError, DeleteNamespaceInput, NamespaceRegistry};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_retains_known_receipt_on_publication_failure() {
    let root = tempfile::tempdir().unwrap();
    let (path, namespace, _) = fixture(root.path());
    let session = open(&path);
    let fx = crate::test_support::Fixture::from_open_engine(root, session.engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    publisher.rebuild_all().await.unwrap();
    *fx.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
    let result = fx
        .namespace_adapter()
        .delete_namespace(DeleteNamespaceInput {
            name: "doomed".into(),
            force: true,
        })
        .await;
    eprintln!("ACTUAL_NATIVE_API1_PURGE_RESULT {result:?}");
    let progress = fx
        .storage
        .read()
        .unwrap()
        .meta_store()
        .purge_progress()
        .unwrap()
        .into_iter()
        .find(|p| p.namespace_id == namespace)
        .unwrap();
    eprintln!("ACTUAL_NATIVE_API1_PURGE_STAGE {:?}", progress.stage);
    assert_eq!(progress.stage, PurgeStage::Complete);
    let Err(BridgeError::PartiallyApplied(message)) = result else {
        panic!("known commit must retain its structured receipt: {result:?}");
    };
    let payload: serde_json::Value = serde_json::from_str(&message)
        .expect("known native purge failure must preserve a typed full receipt");
    let receipt: control::WriteReceipt =
        serde_json::from_value(payload["receipt"].clone()).unwrap();
    let canonical = fx
        .storage
        .read()
        .unwrap()
        .meta_store()
        .lookup_operation(
            receipt.store_id,
            MutationScope::Namespace(namespace),
            receipt.operation_id,
        )
        .unwrap()
        .unwrap();
    assert_eq!(receipt, canonical);
    let pending = fx
        .storage
        .read()
        .unwrap()
        .meta_store()
        .pending_effects()
        .unwrap();
    assert_eq!(receipt, pending.first().unwrap().receipt);
    let control::MutationOutcome::NativeNamespacePurgeAdvanced { transition } = &receipt.outcome
    else {
        panic!("expected matched purge child");
    };
    assert_eq!(
        transition.parent_operation_id,
        progress.start_receipt.operation_id
    );
    assert_eq!(transition.namespace_id, namespace);
    assert_eq!(transition.stage, PurgeStage::Deleting);
    assert!(receipt.committed_generation < progress.last_receipt.committed_generation);
    assert_eq!(payload["commitEvidenceScope"], "purgeStep");
    assert_eq!(payload["error"], "SERVICE_UNAVAILABLE");
    assert_eq!(payload["commitState"], "committed");
    assert!(crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&message).is_some());
    for bad_value in [
        serde_json::json!(1.5),
        serde_json::json!(true),
        serde_json::json!(null),
    ] {
        let mut bad = payload.clone();
        bad["receipt"]["outcome"]["transition"]["chunk"]["deletions"][0]["before"]["lifecycle"]["createdAt"] =
            bad_value;
        assert!(
            crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&bad.to_string())
                .is_none()
        );
    }
    for field in ["ready", "readiness", "applied", "pendingWork", "data"] {
        assert!(payload.get(field).is_none(), "invented {field}");
    }
}

fn setup() -> (
    crate::test_support::Fixture,
    NamespaceId,
    crate::derived_completion::DerivedCompletion,
    PathBuf,
) {
    let root = tempfile::tempdir().unwrap();
    let (path, namespace, _) = fixture(root.path());
    let engine = RedbStorageEngine::open_with_ids(
        &path,
        true,
        Box::new(crate::storage::ids::SeededIds::new([71; 32])),
    )
    .unwrap();
    let fx = crate::test_support::Fixture::from_open_engine(root, engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    (fx, namespace, publisher, path)
}
fn pause(
    fx: &crate::test_support::Fixture,
) -> (
    tokio::sync::oneshot::Receiver<PurgeProgress>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (started, receive) = tokio::sync::oneshot::channel();
    let (resume, wait) = tokio::sync::oneshot::channel();
    *fx.storage
        .read()
        .unwrap()
        .meta_store()
        .native_purge_pause
        .lock()
        .unwrap() = Some((started, wait));
    (receive, resume)
}
fn payload(error: BridgeError) -> serde_json::Value {
    let BridgeError::PartiallyApplied(message) = error else {
        panic!("wrong error {error:?}");
    };
    let parsed = crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&message);
    if parsed.is_none() {
        let value: serde_json::Value = serde_json::from_str(&message).unwrap();
        let receipt = serde_json::from_value::<control::WriteReceipt>(value["receipt"].clone());
        eprintln!(
            "CLASSIFIER_REJECT code={} state={} scope={} receipt_decode_error={:?} roundtrip_equal={:?}",
            value["error"],
            value["commitState"],
            value["commitEvidenceScope"],
            receipt.as_ref().err(),
            receipt
                .as_ref()
                .ok()
                .map(|r| serde_json::to_value(r).unwrap() == value["receipt"])
        );
        eprintln!("CLASSIFIER_REJECT_MESSAGE {message}");
    }
    parsed.expect("typed full evidence")
}
fn no_readiness(payload: &serde_json::Value) {
    for key in [
        "ready",
        "readiness",
        "applied",
        "pendingWork",
        "data",
        "deleted",
        "ftsRowsRemoved",
        "vectorFileRemoved",
    ] {
        assert!(payload.get(key).is_none(), "invented {key}");
    }
}
fn counters(
    fx: &crate::test_support::Fixture,
) -> (control::GenerationRef, crate::storage::ids::IdState) {
    let engine = fx.storage.read().unwrap();
    (
        engine.control_state().unwrap().generation_ref(),
        engine
            .meta_store()
            .id_source()
            .checkpoint()
            .unwrap()
            .unwrap(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_success_counts_actual_orphan_fts_rows_and_directory() {
    let (fx, namespace, publisher, path) = setup();
    publisher.rebuild_all().await.unwrap();
    fx.fts_index
        .lock()
        .await
        .add(namespace, MemoryId::new(), "orphan", None, &[])
        .unwrap();
    let result = fx
        .namespace_adapter()
        .with_clock(Arc::new(NoClock))
        .delete_namespace(DeleteNamespaceInput {
            name: "doomed".into(),
            force: true,
        })
        .await
        .unwrap();
    assert_eq!(
        (
            result.memories_deleted,
            result.tombstones_purged,
            result.edges_removed
        ),
        (3, 1, 2)
    );
    assert_eq!(result.fts_rows_removed, 4); // three nonterminal rows plus the actual orphan
    assert!(result.vector_file_removed);
    assert!(!path.join("doomed").exists());
    let progress = fx
        .storage
        .read()
        .unwrap()
        .meta_store()
        .purge_progress()
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(progress.stage, PurgeStage::Complete);
    let control::MutationOutcome::NativeNamespacePurgeStarted { transition } =
        progress.start_receipt.outcome
    else {
        panic!("wrong start");
    };
    assert_eq!(
        crate::storage::request_context::RetainedRequestContext::native_contract(Some(
            &transition.request.unwrap().context
        ))
        .unwrap(),
        1
    );
    let target = counters(&fx).0;
    let ready = publisher
        .readiness_barrier(
            target,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(ready.ready && ready.derived.len() == 4);
    let before = counters(&fx);
    let result = fx
        .namespace_adapter()
        .delete_namespace(DeleteNamespaceInput {
            name: "doomed".into(),
            force: true,
        })
        .await;
    assert!(matches!(result, Err(BridgeError::NotFound(_))));
    assert_eq!(counters(&fx), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_other_completion_wins_without_inventing_physical_metrics() {
    let (fx, namespace, publisher, path) = setup();
    publisher.rebuild_all().await.unwrap();
    let (started, resume) = pause(&fx);
    let adapter = fx.namespace_adapter().with_clock(Arc::new(NoClock));
    let task = tokio::spawn(async move {
        adapter
            .delete_namespace(DeleteNamespaceInput {
                name: "doomed".into(),
                force: true,
            })
            .await
    });
    let start = tokio::time::timeout(std::time::Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    publisher.complete().await.unwrap(); // actual competing owner; no new purge operation
    let completed = fx
        .storage
        .read()
        .unwrap()
        .lookup_namespace_purge(start.store_id, namespace, start.start_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(completed.stage, PurgeStage::Complete);
    let before = counters(&fx);
    let bytes = std::fs::read(path.join("meta.db")).unwrap();
    let edges = std::fs::read(path.join("edges.db")).unwrap();
    resume.send(()).unwrap();
    let result = payload(task.await.unwrap().unwrap_err());
    assert_eq!(result["error"], "PURGE_METRICS_UNAVAILABLE");
    assert_eq!(result["commitState"], "committed");
    assert_eq!(result["commitEvidenceScope"], "purgeComplete");
    assert_eq!(
        serde_json::from_value::<control::WriteReceipt>(result["receipt"].clone()).unwrap(),
        completed.last_receipt
    );
    no_readiness(&result);
    assert_eq!(counters(&fx), before);
    assert_eq!(std::fs::read(path.join("meta.db")).unwrap(), bytes);
    assert_eq!(std::fs::read(path.join("edges.db")).unwrap(), edges);
    let ready = publisher
        .readiness_barrier(
            before.0,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(ready.ready && ready.derived.len() == 4);
}

struct At2000;
impl crate::time::Clock for At2000 {
    fn now_millis(&self) -> i64 {
        2000
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_foreign_pending_first_preserves_known_start_without_readback() {
    let (fx, namespace, publisher, _) = setup();
    publisher.rebuild_all().await.unwrap();
    let foreign = {
        let mut engine = fx.storage.write().unwrap();
        let kept = engine.get_namespace_by_name("kept").unwrap().unwrap();
        let id = engine
            .scan_all()
            .unwrap()
            .into_iter()
            .find(|(_, r)| r.namespace_id == kept.id.get())
            .unwrap()
            .0;
        let policy = NativePolicyInputs::default();
        let intent = engine
            .meta_store()
            .capture_native_tombstone(id, Some(kept.id), &policy)
            .unwrap();
        engine
            .tombstone_native_record(
                &MutationContext::unkeyed(kept.id),
                &intent,
                &policy,
                &At2000,
                None,
            )
            .unwrap()
    };
    fx.storage
        .read()
        .unwrap()
        .meta_store()
        .catalog_test_fault
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let failure = publisher
        .purge_native_api1("doomed".into(), true)
        .await
        .err()
        .unwrap();
    let crate::derived_completion::PurgeApi1Failure::Storage {
        source: StorageError::CommittedMutation { receipt, .. },
        known_receipt,
    } = &failure
    else {
        panic!("wrong typed source {failure:?}");
    };
    assert_eq!(**receipt, foreign);
    let known = known_receipt.as_ref().unwrap().as_ref().clone();
    assert!(matches!(
        known.outcome,
        control::MutationOutcome::NativeNamespacePurgeStarted { .. }
    ));
    // One intentional pre-resume target lookup, no error-path upgrade readback.
    assert_eq!(
        fx.storage
            .read()
            .unwrap()
            .meta_store()
            .purge_lookup_count
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    let before = counters(&fx);
    let progress = fx
        .storage
        .read()
        .unwrap()
        .meta_store()
        .purge_progress()
        .unwrap()
        .into_iter()
        .find(|p| p.namespace_id == namespace)
        .unwrap();
    assert_eq!(progress.stage, PurgeStage::Complete);
    assert_eq!(known, progress.start_receipt);
    let result = payload(crate::mcp::bridge_adapters::native_purge_api1::project(
        failure,
    ));
    assert_eq!(result["error"], "SERVICE_UNAVAILABLE");
    assert_eq!(result["commitEvidenceScope"], "purgeStart");
    assert_eq!(result["commitState"], "committed");
    assert_eq!(
        serde_json::from_value::<control::WriteReceipt>(result["receipt"].clone()).unwrap(),
        known
    );
    assert_ne!(
        result["receipt"]["operationId"],
        foreign.operation_id.to_string()
    );
    no_readiness(&result);
    assert_eq!(counters(&fx), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_unknown_child_keeps_actual_reserved_id_and_known_start() {
    let (fx, namespace, publisher, _) = setup();
    publisher.rebuild_all().await.unwrap();
    fx.storage
        .read()
        .unwrap()
        .meta_store()
        .fail_purge_child_commit
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (started, resume) = pause(&fx);
    let adapter = fx.namespace_adapter();
    let task = tokio::spawn(async move {
        adapter
            .delete_namespace(DeleteNamespaceInput {
                name: "doomed".into(),
                force: true,
            })
            .await
    });
    let start = tokio::time::timeout(std::time::Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    resume.send(()).unwrap();
    let result = payload(task.await.unwrap().unwrap_err());
    assert_eq!(result["error"], "COMMIT_OUTCOME_UNKNOWN");
    assert_eq!(result["commitState"], "unknown");
    assert_eq!(
        result["operationId"],
        start.next_operation_id.unwrap().to_string()
    );
    assert_eq!(result["commitEvidenceScope"], "purgeStart");
    assert_eq!(
        serde_json::from_value::<control::WriteReceipt>(result["receipt"].clone()).unwrap(),
        start.last_receipt
    );
    no_readiness(&result);
    let engine = fx.storage.read().unwrap();
    assert!(engine.meta_store().work_tracker().status().unresolved > 0);
    let p = engine
        .lookup_namespace_purge(start.store_id, namespace, start.start_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        p.last_receipt.operation_id,
        start.next_operation_id.unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_lost_begin_has_no_invented_identity_or_receipt() {
    let (fx, _, publisher, _) = setup();
    publisher.rebuild_all().await.unwrap();
    let before = counters(&fx);
    fx.storage
        .read()
        .unwrap()
        .meta_store()
        .native_purge_worker_fault
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let result = payload(
        fx.namespace_adapter()
            .delete_namespace(DeleteNamespaceInput {
                name: "doomed".into(),
                force: true,
            })
            .await
            .unwrap_err(),
    );
    assert_eq!(result["error"], "COMMIT_OUTCOME_UNKNOWN");
    assert_eq!(result["commitState"], "unknown");
    assert!(result.get("operationId").unwrap().is_null());
    assert!(result.get("receipt").is_none() && result.get("commitEvidenceScope").is_none());
    no_readiness(&result);
    assert_eq!(counters(&fx), before);
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
}

async fn mcp_delete(
    namespaces: Arc<dyn NamespaceRegistry>,
    modern: bool,
) -> crate::mcp::protocol::JsonRpcResponse {
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.namespaces = namespaces;
    let mut server = crate::mcp::server::McpServer::new(Arc::new(bridge));
    let args =
        serde_json::json!({"name":"delete_namespace","arguments":{"name":"doomed","force":true}});
    let request = if modern {
        crate::mcp::server::tests::modern_request(2, "tools/call", args)
    } else {
        let init = serde_json::from_value(serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"purge-probe","version":"1"}}})).unwrap();
        assert!(server.handle_message(init).await.unwrap().error.is_none());
        serde_json::from_value(
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":args}),
        )
        .unwrap()
    };
    server.handle_message(request).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_race_actual_mcp_eras_remote_and_legacy_frames() {
    for surface in 0..4 {
        let (fx, namespace, publisher, _) = setup();
        publisher.rebuild_all().await.unwrap();
        let (started, resume) = pause(&fx);
        let direct: Arc<dyn NamespaceRegistry> =
            Arc::new(fx.namespace_adapter().with_clock(Arc::new(NoClock)));
        let socket_dir = tempfile::tempdir().unwrap();
        let (namespaces, server): (Arc<dyn NamespaceRegistry>, _) = if surface == 2 {
            let (remote, server) = crate::daemon::server::catalog_test_remote(
                direct,
                &socket_dir.path().join("purge.sock"),
            )
            .await;
            (remote, Some(server))
        } else {
            (direct, None)
        };
        let task = tokio::spawn(async move {
            if surface == 3 {
                let response =
                    crate::daemon::server::native_purge_test_legacy_rpc(namespaces).await;
                assert!(response.result.is_none());
                let bytes = serde_json::to_vec(&response).unwrap().len();
                let error = response.error.unwrap();
                assert_eq!(error.code, -32006);
                (
                    serde_json::from_str::<serde_json::Value>(&error.message).unwrap(),
                    bytes,
                )
            } else {
                let response = mcp_delete(namespaces, surface != 1).await;
                let bytes = serde_json::to_vec(&response).unwrap().len();
                assert!(response.error.is_none(), "{response:?}");
                let result = response.result.unwrap();
                assert_eq!(result["isError"], true);
                let text: serde_json::Value =
                    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
                if surface == 1 {
                    assert!(result.get("structuredContent").is_none());
                } else {
                    assert_eq!(result["structuredContent"], text);
                }
                (text, bytes)
            }
        });
        let start = tokio::time::timeout(std::time::Duration::from_secs(5), started)
            .await
            .unwrap()
            .unwrap();
        publisher.complete().await.unwrap();
        let completed = fx
            .storage
            .read()
            .unwrap()
            .lookup_namespace_purge(start.store_id, namespace, start.start_receipt.operation_id)
            .unwrap()
            .unwrap();
        let before = counters(&fx);
        resume.send(()).unwrap();
        let (result, bytes) = task.await.unwrap();
        assert_eq!(result["error"], "PURGE_METRICS_UNAVAILABLE");
        assert_eq!(result["commitEvidenceScope"], "purgeComplete");
        assert_eq!(
            serde_json::from_value::<control::WriteReceipt>(result["receipt"].clone()).unwrap(),
            completed.last_receipt
        );
        no_readiness(&result);
        assert!(bytes < crate::daemon::protocol::LEGACY_MAX_MESSAGE_SIZE);
        eprintln!("ACTUAL_PURGE_COMPLETE_FRAME surface={surface} bytes={bytes}");
        assert_eq!(counters(&fx), before);
        if let Some(server) = server {
            tokio::time::timeout(std::time::Duration::from_secs(5), server)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_full_start_and_child_errors_survive_actual_frames() {
    for child in [false, true] {
        let (fx, namespace, publisher, _) = setup();
        publisher.rebuild_all().await.unwrap();
        if child {
            *fx.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
        } else {
            fx.storage
                .read()
                .unwrap()
                .meta_store()
                .fail_purge_child_commit
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        let response =
            crate::daemon::server::native_purge_test_legacy_rpc(Arc::new(fx.namespace_adapter()))
                .await;
        let bytes = serde_json::to_vec(&response).unwrap().len();
        let error = response.error.unwrap();
        assert_eq!(error.code, -32006);
        let result =
            crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&error.message)
                .unwrap();
        let p = fx
            .storage
            .read()
            .unwrap()
            .meta_store()
            .purge_progress()
            .unwrap()
            .into_iter()
            .find(|p| p.namespace_id == namespace)
            .unwrap();
        let receipt: control::WriteReceipt =
            serde_json::from_value(result["receipt"].clone()).unwrap();
        if child {
            assert_eq!(result["commitEvidenceScope"], "purgeStep");
            assert_eq!(
                receipt,
                fx.storage
                    .read()
                    .unwrap()
                    .meta_store()
                    .pending_effects()
                    .unwrap()
                    .first()
                    .unwrap()
                    .receipt
            );
        } else {
            assert_eq!(result["commitEvidenceScope"], "purgeStart");
            assert_eq!(receipt, p.start_receipt);
            assert_eq!(result["commitState"], "unknown");
        }
        no_readiness(&result);
        eprintln!("ACTUAL_PURGE_EVIDENCE_FRAME child={child} bytes={bytes}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_structured_remote_validation_keeps_f32_bits_and_rejects_extras() {
    let (fx, _, publisher, _) = setup();
    publisher.rebuild_all().await.unwrap();
    fx.storage
        .read()
        .unwrap()
        .meta_store()
        .fail_purge_child_commit
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let good = payload(
        fx.namespace_adapter()
            .delete_namespace(DeleteNamespaceInput {
                name: "doomed".into(),
                force: true,
            })
            .await
            .unwrap_err(),
    );
    assert!(
        crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&good.to_string())
            .is_some()
    );
    let retained: control::WriteReceipt = serde_json::from_value(good["receipt"].clone()).unwrap();
    let control::MutationOutcome::NativeNamespacePurgeStarted { transition } = retained.outcome
    else {
        panic!("expected start");
    };
    assert_eq!(
        transition
            .before_namespace
            .namespace()
            .unwrap()
            .initial_stability
            .to_bits(),
        0x406dba5e
    );
    for change in 0..14 {
        let mut bad = good.clone();
        match change {
            0 => bad["ready"] = serde_json::json!(false),
            1 => bad["commitEvidenceScope"] = serde_json::json!("purgeComplete"),
            2 => bad["receipt"]["invented"] = serde_json::json!(true),
            3 => {
                bad["receipt"]["outcome"]["transition"]["beforeNamespace"]["invented"] =
                    serde_json::json!(true)
            }
            4 => bad["receipt"]["storeId"] = serde_json::json!(uuid::Uuid::nil()),
            5 => bad["message"] = serde_json::json!("界".repeat(172)),
            6 => {
                bad["receipt"]["outcome"]["transition"]["previousGeneration"] =
                    serde_json::json!("0")
            }
            7 => {
                bad["receipt"]["outcome"]["transition"]["beforeNamespace"]["embeddingDim"] =
                    serde_json::json!(9000)
            }
            8 => {
                bad["receipt"]["outcome"]["transition"]["beforeNamespace"]["policy"]["initialStability"] =
                    serde_json::json!(-1)
            }
            9 => {
                bad["receipt"]["outcome"]["transition"]["beforeNamespace"]["namespaceId"] =
                    serde_json::json!(1.5)
            }
            10 => {
                bad["receipt"]["outcome"]["transition"]["beforeNamespace"]["schema"]["extensions"] =
                    serde_json::json!({"x":1})
            }
            11 => bad["receipt"]["committedGeneration"] = serde_json::json!("18446744073709551615"),
            12 => {
                bad["receipt"]["outcome"]["transition"]["beforeNamespace"]["policy"]["initialStability"] =
                    serde_json::json!(1e39)
            }
            _ => {
                bad["receipt"]["outcome"]["transition"]["beforeNamespace"]["namespaceId"] =
                    serde_json::json!(4294967296u64)
            }
        }
        assert!(
            crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&bad.to_string())
                .is_none(),
            "case{change}"
        );
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.namespaces = Arc::new(crate::test_support::StubBridge {
            delete_namespace_error: Some(BridgeError::PartiallyApplied(bad.to_string())),
            ..Default::default()
        });
        let result = crate::mcp::tools::dispatch_tool(
            &bridge,
            "delete_namespace",
            serde_json::json!({"name":"doomed","force":true}),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        assert!(result.structured_content.is_none());
    }
    for token in ["1e400", "NaN", "Infinity", "-Infinity"] {
        let mut bad = good.clone();
        bad["receipt"]["outcome"]["transition"]["beforeNamespace"]["policy"]["initialStability"] =
            serde_json::json!("RAW_INVALID_NUMBER");
        let raw = bad.to_string().replace("\"RAW_INVALID_NUMBER\"", token);
        assert!(
            serde_json::from_str::<serde_json::Value>(&raw).is_err(),
            "{token}"
        );
        assert!(crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&raw).is_none());
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.namespaces = Arc::new(crate::test_support::StubBridge {
            delete_namespace_error: Some(BridgeError::PartiallyApplied(raw)),
            ..Default::default()
        });
        let result = crate::mcp::tools::dispatch_tool(
            &bridge,
            "delete_namespace",
            serde_json::json!({"name":"doomed","force":true}),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        assert!(result.structured_content.is_none());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_cancelled_completion_retains_worker_publication_lease() {
    use std::time::Duration;
    let (fx, namespace, publisher, _) = setup();
    publisher.rebuild_all().await.unwrap();
    let (started, resume) = pause(&fx);
    let gate = crate::derived_completion::publication_gate(&fx.storage)
        .await
        .unwrap();
    let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
    let adapter = fx.namespace_adapter();
    let task = tokio::spawn(async move {
        adapter
            .delete_namespace(DeleteNamespaceInput {
                name: "doomed".into(),
                force: true,
            })
            .await
    });
    let start = tokio::time::timeout(Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    let graph = fx.graph.clone().write_owned().await;
    resume.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while gate.try_read().is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let storage = fx.storage.clone();
    let (held, wait) = tokio::sync::oneshot::channel();
    let (release, block) = std::sync::mpsc::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        let _storage = storage.write().unwrap();
        held.send(()).unwrap();
        block.recv().unwrap();
    });
    wait.await.unwrap();
    drop(graph);
    tokio::time::timeout(Duration::from_secs(5), async {
        while tracker.status().pending < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let publication_held = gate.try_read().is_err();
    let graph_held = fx.graph.try_write().is_err();
    release.send(()).unwrap();
    blocker.await.unwrap();
    let guard = tokio::time::timeout(Duration::from_secs(5), gate.write())
        .await
        .unwrap();
    drop(guard);
    assert!(
        publication_held && graph_held,
        "cancelled completion lost worker guards"
    );
    assert!(tracker.status().unresolved >= 2);
    let p = fx
        .storage
        .read()
        .unwrap()
        .lookup_namespace_purge(start.store_id, namespace, start.start_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(p.stage, PurgeStage::Complete);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_lost_completion_preserves_start_without_invented_final() {
    let (fx, namespace, publisher, path) = setup();
    publisher.rebuild_all().await.unwrap();
    let (started, resume) = pause(&fx);
    let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
    fx.storage
        .read()
        .unwrap()
        .meta_store()
        .native_purge_worker_fault
        .store(2, std::sync::atomic::Ordering::SeqCst);
    let adapter = fx.namespace_adapter();
    let task = tokio::spawn(async move {
        adapter
            .delete_namespace(DeleteNamespaceInput {
                name: "doomed".into(),
                force: true,
            })
            .await
    });
    let start = tokio::time::timeout(std::time::Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    resume.send(()).unwrap();
    let result = payload(task.await.unwrap().unwrap_err());
    assert_eq!(result["commitState"], "unknown");
    assert!(result.get("operationId").unwrap().is_null());
    assert_eq!(result["commitEvidenceScope"], "purgeStart");
    assert_eq!(
        serde_json::from_value::<control::WriteReceipt>(result["receipt"].clone()).unwrap(),
        start.last_receipt
    );
    assert!(tracker.status().unresolved >= 2);
    no_readiness(&result);
    drop(publisher);
    let _root = fx.close();
    let reopened = open(&path);
    let p = reopened
        .engine
        .lookup_namespace_purge(start.store_id, namespace, start.start_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(p.stage, PurgeStage::Complete);
    assert!(p.last_receipt.committed_generation > start.last_receipt.committed_generation);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_purge_actual_max_chunk_receipt_survives_legacy_frame() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("native");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let mut config = NamespaceConfig::default_namespace(100);
    config.name = "doomed".into();
    config.embedding_dim = 4;
    let namespace = engine.create_namespace(&config).unwrap();
    for n in 0..crate::storage::metadata::purge::CHUNK_LIMIT {
        engine
            .insert_memory_with_receipt(
                &MutationContext::unkeyed(namespace),
                &insertion::InsertMemory {
                    id: None,
                    summary: format!("retained row {n}"),
                    full_text: None,
                    tags: vec![],
                    initial_stability: None,
                    created_at: Some(1000),
                    embedding: Some(vec![1., 0., 0., 0.]),
                    parent_id: None,
                    supersedes: None,
                },
                &[],
                1000,
            )
            .unwrap();
    }
    config.name = "kept".into();
    let kept = engine.create_namespace(&config).unwrap();
    engine
        .insert_memory_with_receipt(
            &MutationContext::unkeyed(kept),
            &insertion::InsertMemory {
                id: None,
                summary: "surviving row".into(),
                full_text: None,
                tags: vec![],
                initial_stability: None,
                created_at: Some(1000),
                embedding: Some(vec![1., 0., 0., 0.]),
                parent_id: None,
                supersedes: None,
            },
            &[],
            1000,
        )
        .unwrap();
    engine.sync().unwrap();
    drop(engine);
    schema_migration::migrate_schema(
        &source,
        &target,
        &schema_migration::MigrationOptions {
            migration_now: 2000,
            expected_source: schema_migration::inspect_schema(&source)
                .unwrap()
                .control
                .generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let session = open(&target);
    let fx = crate::test_support::Fixture::from_open_engine(root, session.engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    publisher.rebuild_all().await.unwrap();
    *fx.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
    let response =
        crate::daemon::server::native_purge_test_legacy_rpc(Arc::new(fx.namespace_adapter())).await;
    let p = fx
        .storage
        .read()
        .unwrap()
        .meta_store()
        .purge_progress()
        .unwrap()
        .into_iter()
        .find(|p| p.namespace_id == namespace)
        .unwrap();
    let first = fx
        .storage
        .read()
        .unwrap()
        .meta_store()
        .pending_effects()
        .unwrap()
        .first()
        .unwrap()
        .receipt
        .clone();
    let receipt_bytes = serde_json::to_vec(&first).unwrap().len();
    eprintln!(
        "ACTUAL_MAX_CHUNK_FRAME rows={} canonical={:?} full_receipt_bytes={} response={response:?}",
        p.removed, p.stage, receipt_bytes
    );
    assert_eq!(p.stage, PurgeStage::Complete);
    assert_eq!(
        p.removed,
        crate::storage::metadata::purge::CHUNK_LIMIT as u64
    );
    let error = response.error.unwrap();
    assert_eq!(
        error.code, -32006,
        "full actual receipt lost at legacy frame boundary: {error:?}"
    );
    let result =
        crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&error.message).unwrap();
    assert_eq!(
        serde_json::from_value::<control::WriteReceipt>(result["receipt"].clone()).unwrap(),
        first
    );
}
