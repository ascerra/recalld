//! Synthetic private-collector corruption after a real catalog commit. These
//! cases do not represent canonical history or a reachable fresh producer.
use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_api1_invalid_snapshot_stays_typed_and_mcp_error_is_bounded() {
    use crate::serialization::catalog_api1::{self, CatalogReplyAdmission};
    for mode in ["api1_invalid_multiple", "api1_invalid_conflict", "api1_invalid_unadmitted"] {
        let (f, path, provider) = fixture();
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let before = f.storage.read().unwrap().control_state().unwrap();
        let mut adapter = NativeCatalogAdapter::new(f.completion(), provider, Default::default(), Arc::new(FixedClock));
        adapter.failure = Some(mode);
        let error = adapter.create_api1_bounded(crate::storage::native_catalog::NativeNamespaceInput {
            name: "invalid_collector".into(), embedding_dim: None, embedding_profile_id: None,
            initial_stability: Some(3.7145), desired_retention: None, decay_rate_multiplier: None,
        }, CatalogReplyAdmission::http()).await.unwrap_err();
        let CatalogError::Api1WorkerUnknown(owned) = &error else { panic!("typed snapshot lost: {error:?}") };
        owned.assert_invalid_retained_for_test(mode);
        let rejected = catalog_api1::try_failure(&error).unwrap_err();
        assert!(std::ptr::eq(owned.as_ref(), rejected));
        let (status, wire) = catalog_api1::failure(&error);
        assert_eq!(status, 500);
        assert_eq!(wire["error"], "INTERNAL_ERROR");
        assert_eq!(wire["commitState"], "unknown");
        assert_eq!(wire["operationId"], Value::Null);
        for absent in ["receipt", "receipts", "ready", "retryable"] { assert!(wire.get(absent).is_none()); }
        assert!(serde_json::to_vec(&wire).unwrap().len() < 1024);
        // Projection did not consume or replace the typed cause.
        owned.assert_invalid_retained_for_test(mode);
        assert_eq!(tracker.status().pending, 2);
        assert_eq!(tracker.status().unresolved, 2);
        drop(error); drop(adapter);
        let dir = f.close();
        let reopened = NativeLifecycleSession::open(&path, NativePolicyInputs::default(), Arc::new(FixedClock)).unwrap();
        assert_eq!(reopened.engine.control_state().unwrap().committed_generation, before.committed_generation.next().unwrap());
        assert!(reopened.engine.get_namespace_by_name("invalid_collector").unwrap().is_some());
        drop(reopened); drop(dir);

        let (f, _, provider) = fixture();
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let out = api1_dispatch(api1_bridge(&f, provider, Some(mode)), "invalid_mcp", json!(19), crate::mcp::protocol::Era::Modern).await;
        assert_eq!(out["result"]["isError"], true);
        let content: Value = serde_json::from_str(out["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(content, out["result"]["structuredContent"]);
        assert_eq!(content["error"], "INTERNAL_ERROR");
        assert_eq!(content["commitState"], "unknown");
        assert_eq!(content["operationId"], Value::Null);
        for absent in ["receipt", "receipts", "ready", "retryable"] { assert!(content.get(absent).is_none()); }
        assert!(serde_json::to_vec(&out).unwrap().len() + 1 < 2048);
        assert_eq!(tracker.status().pending, 2);
        assert_eq!(tracker.status().unresolved, 2);
    }
}
