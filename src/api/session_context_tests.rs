use super::*;
use crate::model::schema::EmbeddingProfile;
use serde_json::{Value, json};
use tower::ServiceExt;

struct IdentityProvider {
    profile: Option<EmbeddingProfile>,
}
#[async_trait::async_trait]
impl EmbeddingProvider for IdentityProvider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, EmbeddingError> {
        panic!("discovery embedded text")
    }
    fn dimensions(&self) -> usize {
        DIM
    }
    fn model_name(&self) -> &str {
        panic!("display label is not profile evidence")
    }
    fn profile(&self) -> Option<EmbeddingProfile> {
        self.profile.clone()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_context_real_http_mcp_identity_defaults_and_immutable_source() {
    for known in [false, true] {
        let f = Fixture::new();
        let project = f.create_namespace("Project");
        let profile = known.then(crate::embedding::runtime::tests::profile);
        let provider = Arc::new(IdentityProvider {
            profile: profile.clone(),
        });
        let app = crate::api::router(state(&f, provider.clone()), &Default::default());
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.default_namespace = "project".into();
        bridge.namespaces = Arc::new(f.namespace_adapter());
        bridge.storage = Arc::new(crate::mcp::bridge_adapters::McpStorageAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            provider,
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
            f.graph.clone(),
            Arc::new(Default::default()),
            chrono_tz::UTC,
        ));
        let before = files(f.path());
        let store = f.storage.read().unwrap().control_state().unwrap();
        let response = app
            .oneshot(
                axum::http::Request::get("/capabilities")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "private, no-store");
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let http: Value = serde_json::from_slice(&body).unwrap();
        let resource = crate::mcp::resources::dispatch_resource(&bridge, "recalld://session")
            .await
            .unwrap();
        let mcp: Value = serde_json::from_str(resource.contents[0].text.as_ref().unwrap()).unwrap();
        assert_eq!(http["defaultNamespace"]["requestedName"], "default");
        assert_eq!(http["defaultNamespace"]["source"], "legacyHttpDefault");
        assert_eq!(mcp["defaultNamespace"]["requestedName"], "project");
        assert_eq!(mcp["defaultNamespace"]["source"], "bridge");
        assert_eq!(mcp["defaultNamespace"]["resolved"]["id"], project.get());
        assert_eq!(http["backendContext"], mcp["backendContext"]);
        assert_eq!(http["limits"], mcp["limits"]);
        assert_eq!(http["duplicateScan"], mcp["duplicateScan"]);
        for context in [http, mcp] {
            assert_eq!(context["supportedContractVersions"], json!([1]));
            assert_eq!(
                context["duplicateScan"]["algorithm"],
                "sampledNearestNeighbors"
            );
            assert_eq!(context["duplicateScan"]["complete"], false);
            assert_eq!(context["backendContext"]["storeId"], json!(store.store_id));
            let active = &context["backendContext"]["activeEmbedding"];
            assert_eq!(active["dimension"], DIM);
            if let Some(profile) = &profile {
                assert_eq!(active["identity"], "configuration");
                assert_eq!(active["profileId"], profile.profile_id().unwrap());
            } else {
                assert_eq!(active["identity"], "unknown");
                assert!(active.get("profileId").is_none());
            }
            for unsupported in ["readGeneration", "ready", "receipt", "modelRevision"] {
                assert!(!context.to_string().contains(unsupported));
            }
            assert!(!context.to_string().contains(f.path().to_str().unwrap()));
        }
        assert_eq!(files(f.path()), before);
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), store);
    }
}

#[tokio::test]
async fn session_context_invalid_actual_profile_fails_without_blessing_unknown() {
    let f = Fixture::new();
    let mut profile = crate::embedding::runtime::tests::profile();
    profile.dimension += 1;
    let provider = Arc::new(IdentityProvider {
        profile: Some(profile),
    });
    let app = crate::api::router(state(&f, provider), &Default::default());
    let before = files(f.path());
    let response = app
        .oneshot(
            axum::http::Request::get("/capabilities")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert_eq!(files(f.path()), before);
}
