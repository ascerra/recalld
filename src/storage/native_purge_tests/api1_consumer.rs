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
