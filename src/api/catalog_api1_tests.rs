//! Full HTTP create responses using the same native owner as the MCP probes.
use super::*;
use crate::native_catalog_adapter::tests::{FixedClock, fixture};
use crate::storage::{
    control::MutationScope,
    native_lifecycle::{NativeLifecycleSession, NativePolicyInputs},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_api1_http_invalid_snapshot_is_bounded_unknown_error() {
    for mode in [
        "api1_invalid_multiple",
        "api1_invalid_conflict",
        "api1_invalid_unadmitted",
    ] {
        let (f, _, provider) = fixture();
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let (mut state, bridge) = fixture_state(&f);
        drop(bridge);
        state.namespaces = Arc::new(
            NamespaceRegistryAdapter::new(f.storage.clone(), provider.clone())
                .with_native_catalog(
                    provider,
                    NativePolicyInputs::default(),
                    Arc::new(FixedClock),
                    f.cache.clone(),
                    f.graph.clone(),
                    f.vector_index.clone(),
                    f.fts_index.clone(),
                    f.entity_index.clone(),
                )
                .with_catalog_fault(mode),
        );
        let app = crate::api::router(state, &Default::default());
        let response = app
            .oneshot(
                Request::post("/namespaces")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"name":"invalid_http"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"], "INTERNAL_ERROR");
        assert_eq!(value["commitState"], "unknown");
        assert_eq!(value["operationId"], Value::Null);
        for absent in ["receipt", "receipts", "ready", "retryable"] {
            assert!(value.get(absent).is_none());
        }
        assert_eq!(tracker.status().pending, 2);
        assert_eq!(tracker.status().unresolved, 2);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalog_api1_http_success_known_unknown_full_bodies() {
    for point in [None, Some("before_handle_publish"), Some("commit_unknown")] {
        let (f, path, provider) = fixture();
        let initial = f.storage.read().unwrap().control_state().unwrap();
        let (mut state, bridge) = fixture_state(&f);
        drop(bridge);
        let mut adapter = NamespaceRegistryAdapter::new(f.storage.clone(), provider.clone())
            .with_native_catalog(
                provider,
                NativePolicyInputs::default(),
                Arc::new(FixedClock),
                f.cache.clone(),
                f.graph.clone(),
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
            );
        if let Some(point) = point {
            adapter = adapter.with_catalog_fault(point);
        }
        state.namespaces = Arc::new(adapter);
        let app = crate::api::router(state, &Default::default());
        let response = app
            .clone()
            .oneshot(
                Request::post("/namespaces")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name":"created","initialStability":3.7145}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 8_388_608).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        if point.is_none() {
            assert_eq!(status, StatusCode::CREATED, "{value}");
            assert_eq!(value["data"]["name"], "created");
            assert_eq!(value["data"]["embeddingDim"], 4);
            assert!(value.get("receipt").is_none());
            assert!(value.get("ready").is_none());
        } else {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{value}");
            assert_eq!(value["error"], "SERVICE_UNAVAILABLE");
            assert!(value.get("ready").is_none());
            assert_eq!(
                value["commitState"],
                if point == Some("commit_unknown") {
                    "unknown"
                } else {
                    "committed"
                }
            );
        }
        drop(app);
        let dir = f.close();
        let reopened = NativeLifecycleSession::open(
            &path,
            NativePolicyInputs::default(),
            Arc::new(FixedClock),
        )
        .unwrap();
        assert_eq!(
            reopened
                .engine
                .control_state()
                .unwrap()
                .committed_generation,
            initial.committed_generation.next().unwrap()
        );
        if point.is_some() {
            let operation = uuid::Uuid::parse_str(value["operationId"].as_str().unwrap()).unwrap();
            let receipt = reopened
                .engine
                .meta_store()
                .lookup_operation(initial.store_id, MutationScope::Catalog, operation)
                .unwrap()
                .unwrap();
            if point == Some("commit_unknown") {
                assert!(value.get("receipt").is_none());
            } else {
                let decoded: crate::storage::control::WriteReceipt =
                    serde_json::from_value(value["receipt"].clone()).unwrap();
                assert_eq!(
                    crate::storage::control::encode(&decoded).unwrap(),
                    crate::storage::control::encode(&receipt).unwrap()
                );
                let mut original = value.clone();
                original["receipt"] = serde_json::to_value(&receipt).unwrap();
                assert_eq!(
                    body.as_ref(),
                    serde_json::to_vec(&original).unwrap().as_slice(),
                    "actual HTTP error Value encoding"
                );
            }
        }
        drop(reopened);
        drop(dir);
    }
}
