//! HTTP regressions over the same temporary storage used by the exported MCP probes.
use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;
use crate::api::adapters::*;
use crate::storage::StorageEngine as _;
use crate::test_support::Fixture;

fn fixture_clients(f: &Fixture) -> (Router, crate::mcp::bridge::McpBridge) {
    let (state, bridge) = fixture_state(f);
    (
        crate::api::routes::router(state, &Default::default()),
        bridge,
    )
}

fn fixture_state(f: &Fixture) -> (AppState, crate::mcp::bridge::McpBridge) {
    fixture_state_with_embedding(
        f,
        Arc::new(crate::embedding::PassthroughProvider::new(
            crate::test_support::DIM,
        )),
    )
}

fn fixture_clients_with_embedding(
    f: &Fixture,
    embedding: Arc<dyn crate::embedding::EmbeddingProvider>,
) -> (Router, crate::mcp::bridge::McpBridge) {
    let (state, bridge) = fixture_state_with_embedding(f, embedding);
    (
        crate::api::routes::router(state, &Default::default()),
        bridge,
    )
}

fn fixture_state_with_embedding(
    f: &Fixture,
    embedding: Arc<dyn crate::embedding::EmbeddingProvider>,
) -> (AppState, crate::mcp::bridge::McpBridge) {
    use crate::search::adapters::*;
    let engine = Arc::new(crate::search::QueryEngine::new(
        Arc::new(StorageNamespaceResolver::new(f.storage.clone())),
        Arc::new(SingleEmbeddingRegistry::new(embedding.clone())),
        Arc::new(SharedVectorIndexRegistry::new(f.vector_index.clone())),
        Arc::new(SharedFtsIndexRegistry::new(f.fts_index.clone())),
        Arc::new(SharedEntityIndexReader::new(f.entity_index.clone())),
        Arc::new(CacheManagerAdapter::new(f.cache.clone())),
        Arc::new(StorageMetadataAdapter::new(f.storage.clone())),
        Arc::new(SharedGraphReader::new(f.graph.clone())),
        Arc::new(RifProcessorAdapter::new(
            Arc::new(crate::rif::RifEngine::new(Default::default())),
            f.graph.clone(),
        )),
        Arc::new(StorageAccessRecorder::new(
            f.storage.clone(),
            f.cache.clone(),
        )),
    ));
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.search = Arc::new(crate::mcp::bridge_adapters::McpSearchAdapter::new(
        engine.clone(),
        embedding.clone(),
        f.storage.clone(),
        f.graph.clone(),
        chrono_tz::UTC,
    ));
    bridge.namespaces = Arc::new(crate::mcp::bridge_adapters::McpNamespaceAdapter::new(
        f.storage.clone(),
        embedding.clone(),
        f.cache.clone(),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
        chrono_tz::UTC,
    ));
    let config = Arc::new(crate::config::RecalldConfig::default());
    let state = AppState::new(
        Arc::new(SearchPipelineAdapter::new(
            engine,
            embedding.clone(),
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
        )),
        Arc::new(StorageEngineAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
            f.graph.clone(),
        )),
        Arc::new(RecordCacheAdapter::new(f.cache.clone())),
        Arc::new(RelationshipGraphAdapter::new(
            f.graph.clone(),
            f.vector_index.clone(),
            f.entity_index.clone(),
            f.storage.clone(),
            f.cache.clone(),
            config.clone(),
        )),
        Arc::new(FsrsEngineAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            f.graph.clone(),
            config,
            false,
        )),
        Arc::new(
            NamespaceRegistryAdapter::new(f.storage.clone(), embedding.clone())
                .with_completion(f.completion()),
        ),
        Arc::new(NoopMetricsCollector),
    );
    (state, bridge)
}

async fn post(app: &Router, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn namespace_tree(
    path: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        path: &std::path::Path,
        files: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let bytes = if path.is_dir() {
                walk(root, &path, files);
                vec![]
            } else {
                std::fs::read(&path).unwrap()
            };
            files.insert(path.strip_prefix(root).unwrap().into(), bytes);
        }
    }
    let mut files = Default::default();
    walk(path, path, &mut files);
    files
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn namespace_profile_http_mcp_use_active_width_before_catalog_allocation() {
    let f = Fixture::new();
    // Existing default is 4; actual provider is 8. Omission must not inherit 4.
    use crate::embedding::EmbeddingProvider as _;
    use crate::mcp::bridge::StorageEngine as _;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountedProvider {
        inner: crate::embedding::OllamaProvider,
        calls: AtomicUsize,
    }
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for CountedProvider {
        fn dimensions(&self) -> usize {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.dimensions()
        }
        fn model_name(&self) -> &str {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.model_name()
        }
        fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.profile()
        }
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            panic!("namespace creation and metadata pages must not embed")
        }
    }
    struct NoPageClock;
    impl crate::time::Clock for NoPageClock {
        fn now_millis(&self) -> i64 {
            panic!("metadata pages must not sample a clock")
        }
    }
    let provider = Arc::new(CountedProvider {
        inner: crate::embedding::OllamaProvider::new("fixture-model".into(), 8),
        calls: AtomicUsize::new(0),
    });
    let configured_profile = provider.inner.profile().unwrap().profile_id().unwrap();
    let (app, bridge) = fixture_clients_with_embedding(&f, provider.clone());
    let before = namespace_tree(f.path());
    let generation = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    for width in [1, 4, 8192] {
        let input = json!({"name":"rejected", "embeddingDim":width});
        let (status, body) = post(&app, "/namespaces", input.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["field"], "embeddingDim");
        let result = crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", input).await;
        assert_eq!(result.is_error, Some(true));
        assert!(
            serde_json::to_string(&result)
                .unwrap()
                .contains("active provider dimension 8")
        );
        assert_eq!(namespace_tree(f.path()), before);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .committed_generation,
            generation
        );
    }
    for (index, input) in [
        json!({"name":"http-omitted"}),
        json!({"name":"mcp-omitted"}),
        json!({"name":"http-explicit", "embeddingDim":8}),
        json!({"name":"mcp-explicit", "embeddingDim":8}),
    ]
    .into_iter()
    .enumerate()
    {
        if index % 2 == 0 {
            let (status, body) = post(&app, "/namespaces", input.clone()).await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
        } else {
            let result =
                crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", input.clone()).await;
            assert_ne!(result.is_error, Some(true), "{result:?}");
        }
        let storage = f.storage.read().unwrap();
        let ns = storage
            .get_namespace_by_name(input["name"].as_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(ns.embedding_dim, 8);
        assert_eq!(
            ns.id.get(),
            index as u32 + 2,
            "rejections must not reserve namespace IDs"
        );
        assert!(
            ns.schema.is_none(),
            "known config must not auto-attest legacy namespace"
        );
        assert_eq!(
            storage.control_state().unwrap().committed_generation.get(),
            generation.get() + index as u64 + 1
        );
    }
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .get_namespace_by_name("default")
            .unwrap()
            .unwrap()
            .embedding_dim,
        4
    );

    // Read the actual public-created namespaces over the same owner's private
    // daemon connection. Configuration identity must not attest legacy rows.
    let owned = Arc::new(
        crate::mcp::bridge_adapters::McpStorageAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            provider.clone(),
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
            f.graph.clone(),
            Arc::new(Default::default()),
            chrono_tz::UTC,
        )
        .with_catalog_profile_context(provider.as_ref())
        .with_clock(Arc::new(NoPageClock)),
    );
    let calls = provider.calls.load(Ordering::SeqCst);
    let before_pages = namespace_tree(f.path());
    let control = f.storage.read().unwrap().control_state().unwrap();
    let at = crate::storage::control::GenerationRef {
        store_id: control.store_id,
        generation: control.committed_generation,
    };
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        owned.clone(),
        &socket.path().join("profile-pages.sock"),
    )
    .await;
    remote.test_client().ping().await.unwrap();
    let request = crate::serialization::catalog_page::CatalogPageRequest::from_mcp(
        &json!({"contractVersion":2,"limit":10}),
        crate::serialization::catalog_page::CatalogKind::Namespaces,
    )
    .unwrap();
    let local = serde_json::to_value(
        owned
            .list_catalog_page(request.clone())
            .await
            .unwrap()
            .into_envelope(),
    )
    .unwrap();
    let catalog = serde_json::to_value(
        remote
            .list_catalog_page(request)
            .await
            .unwrap()
            .into_envelope(),
    )
    .unwrap();
    assert_eq!(catalog, local);
    assert_eq!(catalog["readGeneration"], serde_json::to_value(at).unwrap());
    assert_eq!(catalog["data"]["total"], "5");
    assert_eq!(catalog["data"]["complete"], true);
    assert_eq!(
        catalog["data"]["configuredEmbedding"],
        json!({"identity":"configuration","dimension":8,"profileId":configured_profile})
    );
    let rows = catalog["data"]["namespaces"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    for row in rows {
        let name = row["name"].as_str().unwrap();
        assert_eq!(row["embeddingDim"], if name == "default" { 4 } else { 8 });
        assert!(row["embeddingProfileId"].is_null());
        assert!(row["bindingProvenance"].is_null());
        assert_eq!(row["profileCompatibility"], "unknownNamespaceProfile");
        let request = crate::serialization::list::ListPageRequest::from_mcp(
            &json!({"contractVersion":2,"namespace":name,"limit":1}),
        )
        .unwrap();
        let page = serde_json::to_value(
            remote
                .list_canonical_page(request)
                .await
                .unwrap()
                .into_envelope(),
        )
        .unwrap();
        assert_eq!(page["readGeneration"], serde_json::to_value(at).unwrap());
        assert_eq!(page["namespace"], name);
        assert_eq!(page["namespaceId"], row["id"]);
        assert_eq!(page["data"]["total"], "0");
        assert_eq!(page["data"]["memories"], json!([]));
    }
    drop(remote);
    task.await.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
    assert_eq!(namespace_tree(f.path()), before_pages);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn namespace_profile_invalid_runtime_is_unavailable_without_catalog_mutation() {
    struct Provider {
        dim: usize,
        profile: Option<crate::model::schema::EmbeddingProfile>,
    }
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for Provider {
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            panic!("namespace creation must not embed")
        }
        fn dimensions(&self) -> usize {
            self.dim
        }
        fn model_name(&self) -> &str {
            "not provenance"
        }
        fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
            self.profile.clone()
        }
    }
    let f = Fixture::new();
    let before = namespace_tree(f.path());
    let mut bad_profile = crate::embedding::runtime::tests::profile();
    bad_profile.identity_version = 99;
    for provider in [
        Provider {
            dim: 0,
            profile: None,
        },
        Provider {
            dim: 8193,
            profile: None,
        },
        Provider {
            dim: 4,
            profile: Some(bad_profile),
        },
        Provider {
            dim: 8,
            profile: Some(crate::embedding::runtime::tests::profile()),
        },
    ] {
        let (app, bridge) = fixture_clients_with_embedding(&f, Arc::new(provider));
        let input = json!({"name":"bad-runtime"});
        let (status, body) = post(&app, "/namespaces", input.clone()).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        let result = crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", input).await;
        assert_eq!(result.is_error, Some(true));
        assert!(
            serde_json::to_string(&result)
                .unwrap()
                .contains("unavailable")
        );
        assert_eq!(namespace_tree(f.path()), before);
    }
    // No descriptor remains allowed only as explicitly unverified v1 compatibility.
    let (app, _) = fixture_clients_with_embedding(
        &f,
        Arc::new(Provider {
            dim: 4,
            profile: None,
        }),
    );
    assert_eq!(
        post(&app, "/namespaces", json!({"name":"custom-legacy"}))
            .await
            .0,
        StatusCode::CREATED
    );
    assert!(
        f.storage
            .read()
            .unwrap()
            .get_namespace_by_name("custom-legacy")
            .unwrap()
            .unwrap()
            .schema
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_quality_rejects_exported_probe_values_without_mutation() {
    let f = Fixture::new();
    let (app, _) = fixture_clients(&f);
    let id = f.insert_memory().await;
    let path = format!("/memories/{id}/reinforce");
    let before = format!("{:?}", f.record(id).unwrap());
    for value in [
        json!(-1),
        json!(1.5),
        json!(3.0),
        json!("3"),
        json!(true),
        Value::Null,
        json!([]),
        json!({}),
        json!(0),
        json!(5),
        json!(256),
        json!(259),
        json!(u64::MAX),
    ] {
        let (status, body) = post(&app, &path, json!({"quality": value})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{value}: {body}");
        assert_eq!(body["field"], "quality", "{body}");
        assert_eq!(before, format!("{:?}", f.record(id).unwrap()));
    }
    for value in [
        json!({}),
        json!({"quality": 1}),
        json!({"quality": 2}),
        json!({"quality": 3}),
        json!({"quality": 4}),
    ] {
        let before = format!("{:?}", f.record(id).unwrap());
        let (status, body) = post(&app, &path, value).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_ne!(before, format!("{:?}", f.record(id).unwrap()));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_namespace_rejects_exported_probe_values_without_creation() {
    let f = Fixture::new();
    let (app, _) = fixture_clients(&f);
    for value in [
        json!(-1),
        json!(1.5),
        json!(4.0),
        json!("4"),
        json!(true),
        Value::Null,
        json!([]),
        json!({}),
        json!(0),
        json!(8193),
        json!(65535),
        json!(65536),
        json!(65537),
        json!(u64::MAX),
    ] {
        let (status, body) = post(
            &app,
            "/namespaces",
            json!({"name": "invalid", "embeddingDim": value}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{value}: {body}");
        assert_eq!(body["field"], "embeddingDim", "{body}");
        assert!(
            f.storage
                .read()
                .unwrap()
                .get_namespace_by_name("invalid")
                .unwrap()
                .is_none()
        );
    }
    for (name, dim) in [("one", 1), ("maximum", 8192)] {
        let (app, _) = fixture_clients_with_embedding(
            &f,
            Arc::new(crate::embedding::PassthroughProvider::new(dim as usize)),
        );
        let (status, body) = post(
            &app,
            "/namespaces",
            json!({"name": name, "embeddingDim": dim}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .get_namespace_by_name(name)
                .unwrap()
                .unwrap()
                .embedding_dim,
            dim
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn namespace_creation_typed_validation_and_unavailable_errors() {
    use crate::storage::StorageError;
    use axum::response::IntoResponse;
    let operation_id = uuid::Uuid::new_v4();
    for (error, status) in [
        (
            StorageError::InvalidMutation("invalid policy".into()),
            StatusCode::BAD_REQUEST,
        ),
        (
            StorageError::RecoveryRequired("reopen".into()),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            StorageError::CommittedEffectsPending {
                operation_id,
                source: Box::new(StorageError::RecoveryRequired("publish".into())),
            },
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            StorageError::CommitOutcomeUnknown {
                operation_id,
                source: redb::CommitError::Storage(redb::StorageError::Io(std::io::Error::other(
                    "lost commit acknowledgement",
                ))),
            },
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let carries_operation = matches!(
            &error,
            StorageError::CommittedEffectsPending { .. }
                | StorageError::CommitOutcomeUnknown { .. }
        );
        let response = namespace_create_error(Box::new(error)).into_response();
        assert_eq!(response.status(), status);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(!body.contains("already exists"));
        if carries_operation {
            assert!(body.contains(&operation_id.to_string()));
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn namespace_creation_storage_failure_is_not_a_duplicate() {
    let f = Fixture::new();
    let (app, _) = fixture_clients(&f);
    let (status, _) = post(&app, "/namespaces", json!({"name":"duplicate"})).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, body) = post(&app, "/namespaces", json!({"name":"duplicate"})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let before = f.storage.read().unwrap().control_state().unwrap();

    // Force a real catalog filesystem failure while keeping its database handles
    // open. Restore the synthetic root before checking the response or dropping it.
    let parked = tempfile::tempdir().unwrap();
    let store = parked.path().join("store");
    std::fs::rename(f.path(), &store).unwrap();
    std::fs::write(f.path(), b"not a directory").unwrap();
    let (status, body) = post(&app, "/namespaces", json!({"name":"io-failure"})).await;
    std::fs::remove_file(f.path()).unwrap();
    std::fs::rename(&store, f.path()).unwrap();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(!body.to_string().contains("already exists"), "{body}");
    let storage = f.storage.read().unwrap();
    assert_eq!(storage.control_state().unwrap(), before);
    assert!(
        storage
            .get_namespace_by_name("io-failure")
            .unwrap()
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn namespace_option_parity_and_no_creation_on_invalid_input() {
    let f = Fixture::new();
    let (app, _) = fixture_clients(&f);
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.namespaces = Arc::new(f.namespace_adapter());
    for field in [
        "initialStability",
        "desiredRetention",
        "decayRateMultiplier",
    ] {
        let mut bad_values = vec![
            json!(-1),
            json!("1"),
            json!(true),
            json!([]),
            json!({}),
            json!(1e100),
        ];
        if field == "initialStability" {
            bad_values.extend([json!(0), json!(1e-100)]);
        }
        if field == "desiredRetention" {
            bad_values.push(json!(1.01));
        }
        for value in bad_values {
            let mut args = json!({"name": "invalid"});
            args[field] = value;
            let (status, body) = post(&app, "/namespaces", args.clone()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{args}: {body}");
            assert_eq!(body["field"], field, "{body}");
            let result =
                crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", args.clone()).await;
            assert_eq!(result.is_error, Some(true), "{args}");
            assert!(serde_json::to_string(&result).unwrap().contains(field));
            assert!(
                f.storage
                    .read()
                    .unwrap()
                    .get_namespace_by_name("invalid")
                    .unwrap()
                    .is_none()
            );
        }
    }
    // Null floats retain v1 omission semantics; dimension omission selects active provider width 4.
    for (index, options) in [
        json!({"initialStability": null, "desiredRetention": null, "decayRateMultiplier": null}),
        json!({"initialStability": 1, "desiredRetention": 0, "decayRateMultiplier": 0}),
        json!({"initialStability": 3e38, "desiredRetention": 1, "decayRateMultiplier": 3e38}),
    ]
    .into_iter()
    .enumerate()
    {
        for transport in ["http", "mcp"] {
            let name = format!("{transport}-{index}");
            let mut args = options.clone();
            args["name"] = json!(name);
            if transport == "http" {
                let (status, body) = post(&app, "/namespaces", args).await;
                assert_eq!(status, StatusCode::CREATED, "{body}");
            } else {
                let result =
                    crate::mcp::tools::dispatch_tool(&bridge, "create_namespace", args).await;
                assert_ne!(result.is_error, Some(true), "{result:?}");
            }
            assert_eq!(
                f.storage
                    .read()
                    .unwrap()
                    .get_namespace_by_name(&name)
                    .unwrap()
                    .unwrap()
                    .embedding_dim,
                crate::test_support::DIM as u32
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_duplicate_coverage_reports_caps_missing_vectors_and_empty_samples() {
    let f = Fixture::new();
    let (app, bridge) = fixture_clients(&f);
    // Even an empty cluster response must not claim an exhaustive scan.
    let result =
        crate::mcp::tools::dispatch_tool(&bridge, "find_similar_memories", json!({"mode": "scan"}))
            .await;
    assert_ne!(result.is_error, Some(true));
    let response: serde_json::Value = serde_json::from_str(
        serde_json::to_value(&result).unwrap()["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let coverage = &response["coverage"];
    assert_eq!(coverage["complete"], false);
    assert_eq!(coverage["sampleCap"], 200);
    assert_eq!(coverage["neighborCap"], 11);
    assert_eq!(coverage["countsAvailable"], true);
    for key in ["sampledCount", "eligibleCount", "skippedVectorCount"] {
        assert_eq!(coverage[key], 0, "{key}");
    }
    assert_eq!(coverage["truncated"], false);
    for count in 0..=2 {
        if count > 0 {
            f.insert_memory().await;
        }
        let (status, body) = post(
            &app,
            "/namespaces/default/duplicates",
            json!({"maxMemories": 1}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let coverage = &body["data"]["coverage"];
        assert_eq!(coverage["eligibleCount"], count);
        assert_eq!(coverage["sampledCount"], count.min(1));
        assert_eq!(coverage["skippedVectorCount"], count.min(1));
        assert_eq!(coverage["truncated"], count > 1);
        assert_eq!(coverage["complete"], false);
        assert_eq!(coverage["neighborCap"], Value::Null);
    }
    for args in [
        json!({"threshold": -0.1}),
        json!({"threshold": 1.1}),
        json!({"maxMemories": 0}),
        json!({"maxMemories": 10001}),
    ] {
        assert_eq!(
            post(&app, "/namespaces/default/duplicates", args).await.0,
            StatusCode::BAD_REQUEST
        );
    }
}

// Run the actual HTTP router and MCP dispatch against the same canonical fixture.
async fn scope_request(app: &Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned())),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scope_assertions_reject_wrong_namespace_before_access_search_or_mutation() {
    let f = Fixture::new();
    let other = f.create_namespace("other");
    let id = f.insert_memory().await;
    // Deliberately spoof cached scope. The guard must use disk, not cache.
    let mut cached = crate::model::CachedRecord::from(&f.record(id).unwrap());
    cached.id = id;
    cached.namespace_id = other;
    f.cache.insert(id, cached).await;
    let before = format!("{:?}", f.record(id));
    let (app, mut bridge) = fixture_clients(&f);
    bridge.storage = Arc::new(f.storage_adapter());
    bridge.default_namespace = "other".into();
    for namespace in ["other", "missing"] {
        for (method, suffix) in [
            ("GET", ""),
            ("DELETE", ""),
            ("POST", "/reinforce"),
            ("POST", "/similar"),
        ] {
            let path = if suffix == "/similar" {
                format!("/similar/{id}")
            } else if method == "POST" {
                format!("/memories/{id}{suffix}")
            } else {
                format!("/memories/{id}?namespace={namespace}")
            };
            let (status, body) =
                scope_request(&app, method, &path, json!({"namespace":namespace})).await;
            assert_eq!(status, StatusCode::CONFLICT, "{method} {path}: {body}");
            assert_eq!(body["error"], "NAMESPACE_MISMATCH");
            assert_eq!(before, format!("{:?}", f.record(id)));
        }
        for tool in [
            "get_memory",
            "reinforce_memory",
            "forget_memory",
            "find_similar_memories",
        ] {
            let result = crate::mcp::tools::dispatch_tool(
                &bridge,
                tool,
                json!({"id":id.to_string(), "namespace":namespace}),
            )
            .await;
            assert_eq!(result.is_error, Some(true), "{tool}");
            assert!(
                serde_json::to_string(&result)
                    .unwrap()
                    .contains("NAMESPACE_MISMATCH")
            );
            assert_eq!(before, format!("{:?}", f.record(id)));
        }
    }
    // Assertion input cannot silently become omission, even if a valid record exists.
    for value in [
        Value::Null,
        json!(42),
        json!(false),
        json!([]),
        json!(""),
        json!("bad space"),
    ] {
        for tool in [
            "get_memory",
            "reinforce_memory",
            "forget_memory",
            "find_similar_memories",
        ] {
            let result = crate::mcp::tools::dispatch_tool(
                &bridge,
                tool,
                json!({"id":id.to_string(), "namespace":value}),
            )
            .await;
            assert_eq!(result.is_error, Some(true), "{tool}: {value}");
        }
        for suffix in ["reinforce", "similar"] {
            let (status, _) = scope_request(
                &app,
                "POST",
                &if suffix == "similar" {
                    format!("/similar/{id}")
                } else {
                    format!("/memories/{id}/{suffix}")
                },
                json!({"namespace":value}),
            )
            .await;
            assert!(status.is_client_error(), "{suffix}: {value}");
        }
    }
    assert_eq!(before, format!("{:?}", f.record(id)));
    // Unknown IDs stay not found, rather than revealing/inferring a namespace.
    let missing = crate::model::MemoryId::new();
    let (status, _) = scope_request(
        &app,
        "GET",
        &format!("/memories/{missing}?namespace=other"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Retained tombstones still have canonical scope to assert.
    f.tombstone(id);
    let (status, _) = scope_request(
        &app,
        "DELETE",
        &format!("/memories/{id}?namespace=other"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scope_assertions_preserve_record_scope_despite_wrong_session_default() {
    for asserted in [false, true] {
        let f = Fixture::new();
        f.create_namespace("other");
        let records = f.create_namespace("records");
        let id = f.insert_memory_in(records, &[]).await;
        let (app, mut bridge) = fixture_clients(&f);
        bridge.storage = Arc::new(f.storage_adapter());
        bridge.default_namespace = "other".into();
        let mut arguments = json!({"id":id.to_string()});
        if asserted {
            arguments["namespace"] = json!("RECORDS");
        }
        for tool in ["get_memory", "reinforce_memory", "forget_memory"] {
            let result = crate::mcp::tools::dispatch_tool(&bridge, tool, arguments.clone()).await;
            assert_ne!(result.is_error, Some(true), "{tool}: {result:?}");
        }
        assert_eq!(
            f.record(id).unwrap().phase,
            crate::model::DecayPhase::Tombstone
        );
        let id = f.insert_memory_in(records, &[]).await;
        let suffix = if asserted { "?namespace=RECORDS" } else { "" };
        let (status, body) =
            scope_request(&app, "GET", &format!("/memories/{id}{suffix}"), json!({})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["namespace"], "records");
        assert!(!f.record(id).unwrap().access_history.is_empty());
        let body = if asserted {
            json!({"namespace":"RECORDS"})
        } else {
            json!({})
        };
        let (status, body) =
            scope_request(&app, "POST", &format!("/memories/{id}/reinforce"), body).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, body) = scope_request(
            &app,
            "DELETE",
            &format!("/memories/{id}{suffix}"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            f.record(id).unwrap().phase,
            crate::model::DecayPhase::Tombstone
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scope_assertions_similarity_uses_canonical_source_scope_with_stale_cache() {
    use crate::search::VectorIndex;
    let f = Fixture::new();
    let local = crate::model::NamespaceId::new(1);
    let other = f.create_namespace("other");
    let mut ids = vec![];
    for namespace in [local, local, other] {
        let id = f
            .insert_memory_with(crate::test_support::MemorySpec {
                embedding: Some(&[1.0, 0.0, 0.0, 0.0]),
                ..crate::test_support::MemorySpec::new(namespace)
            })
            .await;
        f.vector_index
            .write()
            .await
            .add(
                id,
                &[1.0, 0.0, 0.0, 0.0],
                crate::search::VectorMetadata {
                    namespace_id: namespace,
                    decay_phase: crate::model::DecayPhase::Full.as_u8(),
                    tags: vec![],
                },
            )
            .unwrap();
        ids.push(id);
    }
    let mut cached = crate::model::CachedRecord::from(&f.record(ids[0]).unwrap());
    cached.id = ids[0];
    cached.namespace_id = other;
    f.cache.insert(ids[0], cached).await;
    let (_, mut bridge) = fixture_clients(&f);
    bridge.default_namespace = "other".into();
    for namespace in [None, Some("default")] {
        let mut args = json!({"id":ids[0].to_string()});
        if let Some(namespace) = namespace {
            args["namespace"] = json!(namespace);
        }
        let result = crate::mcp::tools::dispatch_tool(&bridge, "find_similar_memories", args).await;
        assert_ne!(result.is_error, Some(true), "{result:?}");
        let wire = serde_json::to_value(result).unwrap();
        let data: Value =
            serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(data["count"], 1);
        assert_eq!(data["memories"][0]["id"], ids[1].to_string());
    }
}

#[path = "sweep_clock_tests.rs"]
mod sweep_clock;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn performance_sampling_actual_http_and_mcp_share_root_decision_and_keep_bytes() {
    use crate::search::VectorIndex;
    struct Fixed;
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for Fixed {
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            Ok(vec![1.0, 0.0, 0.0, 0.0])
        }
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> &str {
            "test-fixed"
        }
    }
    let f = Fixture::new();
    let namespace = crate::model::NamespaceId::new(1);
    let id = f
        .insert_memory_with(crate::test_support::MemorySpec {
            embedding: Some(&[1.0, 0.0, 0.0, 0.0]),
            tags: &["entity/fixture"],
            summary: "private-needle",
            ..crate::test_support::MemorySpec::new(namespace)
        })
        .await;
    f.vector_index
        .write()
        .await
        .add(
            id,
            &[1.0, 0.0, 0.0, 0.0],
            crate::search::VectorMetadata {
                namespace_id: namespace,
                decay_phase: 1,
                tags: vec![],
            },
        )
        .unwrap();
    f.fts_index
        .lock()
        .await
        .add(namespace, id, "private-needle", None, &[])
        .unwrap();
    let (app, bridge) = fixture_clients_with_embedding(&f, Arc::new(Fixed));
    f.storage.write().unwrap().sync().unwrap();
    let files = [
        f.path().join("meta.db"),
        f.path().join("fulltext.dat"),
        f.namespace_dir("default").join("vectors.dat"),
    ];
    let before: Vec<_> = files.iter().map(|p| std::fs::read(p).unwrap()).collect();
    let mut sampled = 0;
    let mut skipped = 0;
    for ordinal in 1..=8 {
        let ((status, body), http_events) = crate::performance::tests::capture_sampled(
            ordinal,
            post(
                &app,
                "/search",
                json!({"query":"private-needle","tags":["entity/fixture"],"recordAccess":false}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body.to_string().contains(&id.to_string()), "{body}");
        let (response, mcp_events) = crate::performance::tests::capture_sampled(
            ordinal,
            crate::mcp::tools::dispatch_tool(
                &bridge,
                "recall_memories",
                json!({"query":"private-needle", "tags":["entity/fixture"],"recordAccess":false}),
            ),
        )
        .await;
        assert_ne!(response.is_error, Some(true), "{response:?}");
        assert!(
            serde_json::to_string(&response)
                .unwrap()
                .contains(&id.to_string())
        );
        assert_eq!(http_events.is_empty(), mcp_events.is_empty());
        if http_events.is_empty() {
            skipped += 1;
        } else {
            sampled += 1;
        }
        for (operation, events) in [("rest.search", http_events), ("mcp.search", mcp_events)] {
            if events.is_empty() {
                continue;
            }
            assert!(events.iter().any(|e| e["fields"]["stage"] == operation));
            assert!(
                events
                    .iter()
                    .any(|e| e["fields"]["stage"] == "engine.search")
            );
            assert!(
                events
                    .iter()
                    .any(|e| e["fields"]["kind"] == "canonical_read")
            );
            for event in &events {
                for span in event["spans"].as_array().unwrap() {
                    assert_eq!(span["root_ordinal"], ordinal);
                    assert_eq!(span["sample_every"], 2);
                }
            }
            assert!(
                !serde_json::to_string(&events)
                    .unwrap()
                    .contains(&id.to_string())
            );
            assert!(
                !serde_json::to_string(&events)
                    .unwrap()
                    .contains("entity/fixture")
            );
        }
        tokio::task::yield_now().await;
        assert_eq!(
            before,
            files
                .iter()
                .map(|p| std::fs::read(p).unwrap())
                .collect::<Vec<_>>()
        );
    }
    assert!(sampled > 0 && skipped > 0);
}
