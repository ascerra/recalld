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
    crate::mcp::bridge_adapters::native_purge_api1::structured_failure(&message)
        .expect("typed full evidence")
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
