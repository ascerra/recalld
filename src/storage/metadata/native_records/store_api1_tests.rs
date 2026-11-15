//! Public API1 routes over real migrated/native storage and the accepted owner.
use super::*;
use readers::bytes;

async fn setup() -> (
    Data,
    Fixture,
    Arc<McpStorageAdapter>,
    Arc<RecoveringProvider>,
    Arc<FixedClock>,
) {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, session.engine);
    readers::completion(&f).complete().await.unwrap();
    let provider = Arc::new(RecoveringProvider {
        profile: d.profile.clone(),
        unavailable: false.into(),
        calls: AtomicUsize::new(0),
        profiles: AtomicUsize::new(0),
        corrupt: Default::default(),
    });
    let a = Arc::new(adapter(
        &f,
        provider.clone(),
        clock.clone(),
        Default::default(),
    ));
    (d, f, a, provider, clock)
}

fn router(
    f: &Fixture,
    a: Arc<McpStorageAdapter>,
    provider: Arc<dyn crate::embedding::EmbeddingProvider>,
) -> axum::Router {
    use crate::api::adapters::*;
    use crate::search::adapters::*;
    let rif = Arc::new(crate::rif::RifEngine::new(Default::default()));
    let engine = Arc::new(
        crate::search::QueryEngine::new(
            Arc::new(StorageNamespaceResolver::new(f.storage.clone())),
            Arc::new(SingleEmbeddingRegistry::new(provider.clone())),
            Arc::new(SharedVectorIndexRegistry::new(f.vector_index.clone())),
            Arc::new(SharedFtsIndexRegistry::new(f.fts_index.clone())),
            Arc::new(SharedEntityIndexReader::new(f.entity_index.clone())),
            Arc::new(CacheManagerAdapter::new(f.cache.clone())),
            Arc::new(StorageMetadataAdapter::new(f.storage.clone())),
            Arc::new(SharedGraphReader::new(f.graph.clone())),
            Arc::new(RifProcessorAdapter::new(rif.clone(), f.graph.clone())),
            Arc::new(StorageAccessRecorder::new(
                f.storage.clone(),
                f.cache.clone(),
            )),
        )
        .with_coherent_owner(readers::completion(f), rif),
    );
    let config = Arc::new(crate::config::RecalldConfig::default());
    crate::api::router(
        crate::api::AppState::new(
            Arc::new(SearchPipelineAdapter::new(
                engine,
                provider,
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
            Arc::new(NamespaceRegistryAdapter::new(f.storage.clone())),
            Arc::new(NoopMetricsCollector),
        )
        .with_native_batches(a, "native".into()),
        &crate::api::ApiConfig::default(),
    )
}

async fn public_http(api: &axum::Router, body: Value) -> (axum::http::StatusCode, Value) {
    use tower::ServiceExt;
    let r = api
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/memories")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = r.status();
    let bytes = axum::body::to_bytes(r.into_body(), crate::daemon::protocol::MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn native_api1_store_actual_mcp_unicode_and_ignored_single_vector() {
    let (d, f, a, provider, _clock) = setup().await;
    let b = bridge(a);
    let r = crate::mcp::tools::dispatch_tool(
        &b,
        "store_memory",
        json!({
            "summary":"API1 native Unicode", "entities":["John Smith","José","Alice's"],
            "embedding":[0.], "embeddingProfileId":"historically ignored", "checkDuplicates":false
        }),
    )
    .await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let crate::mcp::protocol::ContentBlock::Text { text } = &r.content[0];
    let value: Value = serde_json::from_str(text).unwrap();
    assert!(value.get("contractVersion").is_none());
    let id: MemoryId = serde_json::from_value(value["id"].clone()).unwrap();
    let record = f.storage.read().unwrap().get_record(id).unwrap().unwrap();
    assert_eq!(record.namespace_id, d.ns.get());
    assert_eq!(
        record.schema.unwrap().labels.entities,
        ["John Smith", "José", "Alice's"]
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn native_api1_store_actual_http_supplied_profile_and_atomic_links() {
    let (d, f, a, provider, _clock) = setup().await;
    let api = router(&f, a, provider.clone());
    let before = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    let (status,value)=public_http(&api,json!({
        "namespace":"native","summary":"API1 native HTTP", "fullText":"native full payload",
        "entities":["John Smith","José","Alice's"],"embedding":[2.,-0.,0.,0.],
        "embeddingProfileId":d.profile.profile_id().unwrap(),"parentId":d.parent,"checkDuplicates":false
    })).await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "{value}");
    assert!(value.get("contractVersion").is_none());
    let id: MemoryId = serde_json::from_value(value["data"]["id"].clone()).unwrap();
    let store = f.storage.read().unwrap();
    assert_eq!(
        store.control_state().unwrap().committed_generation,
        before.next().unwrap()
    );
    let record = store.get_record(id).unwrap().unwrap();
    assert_eq!(
        value["data"]["labels"]["entities"],
        json!(["John Smith", "José", "Alice's"])
    );
    assert_eq!(
        record.schema.unwrap().labels.entities,
        ["John Smith", "José", "Alice's"]
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn native_api1_store_fresh_invalid_profile_and_vector_no_work() {
    let (d, f, a, provider, clock) = setup().await;
    let api = router(&f, a, provider.clone());
    let profile = d.profile.profile_id().unwrap();
    let before = bytes(&d.path);
    let tick = clock.0.load(Ordering::SeqCst);
    for (fields, status, code) in [
        (
            json!({"embedding":[1.,0.,0.,0.]}),
            422,
            "EMBEDDING_PROFILE_REQUIRED",
        ),
        (json!({"embeddingProfileId":null}), 400, "INVALID_INPUT"),
        (
            json!({"embeddingProfileId":"not a profile"}),
            400,
            "INVALID_INPUT",
        ),
        (
            json!({"embeddingProfileId":profile,"embedding":[0.,0.,0.,0.]}),
            422,
            "OUT_OF_RANGE",
        ),
        (
            json!({"embeddingProfileId":profile,"embedding":[1.,0.,0.,0.],"initialStability":0}),
            422,
            "OUT_OF_RANGE",
        ),
    ] {
        let mut body =
            json!({"namespace":"native","summary":"invalid fresh request","checkDuplicates":false});
        body.as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        let (actual, out) = public_http(&api, body).await;
        assert_eq!(actual.as_u16(), status, "{out}");
        assert_eq!(out["error"], code, "{out}");
        assert_eq!(out["commitState"], "notCommitted", "{out}");
        assert!(out.get("receipt").is_none());
        assert_eq!(bytes(&d.path), before);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        assert_eq!(provider.profiles.load(Ordering::SeqCst), 0);
        assert_eq!(clock.0.load(Ordering::SeqCst), tick);
    }
}

#[tokio::test]
async fn native_api1_store_publication_failure_keeps_full_receipt_and_projection_evidence() {
    let (_d, f, a, provider, _clock) = setup().await;
    *provider.corrupt.lock().unwrap() = Some(f.vector_index.clone());
    let api = router(&f, a, provider.clone());
    let (status,out)=public_http(&api,json!({"namespace":"native","summary":"committed before completion failure","checkDuplicates":false})).await;
    assert_eq!(status.as_u16(), 503, "{out}");
    assert_eq!(out["commitState"], "committed");
    assert_eq!(out["ready"], false);
    let receipt: WriteReceipt = serde_json::from_value(out["receipt"].clone()).unwrap();
    assert_eq!(out["receipt"], canonical(&receipt));
    assert_eq!(out["readiness"]["ready"], false);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    // Malformed/incomplete postcommit identity must not lose the full in-hand
    // receipt, observational readiness or warnings through API1 projection.
    let mut reply =
        crate::serialization::native_single::Reply::failure(StorageError::CommittedMutation {
            receipt: Box::new(receipt.clone()),
            source: Box::new(StorageError::RecoveryRequired("fixture".into())),
        });
    reply.readiness = serde_json::from_value(out["readiness"].clone()).unwrap();
    reply.warnings = vec!["retained warning".into()];
    reply.error = None;
    let (status, failure) = crate::serialization::store_api1::failure(reply);
    let failure = canonical(&failure);
    assert_ne!(status, 200);
    assert_eq!(failure["receipt"], canonical(&receipt));
    assert_eq!(failure["readiness"], out["readiness"]);
    assert_eq!(failure["warnings"], json!(["retained warning"]));
    assert_eq!(failure["commitState"], "committed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_store_legacy_shapes_and_explicit_profile_constraint() {
    let (_d, _native, _a, provider, clock) = setup().await;
    let f = Fixture::new();
    readers::completion(&f).complete().await.unwrap();
    let a = Arc::new(adapter(&f, provider.clone(), clock, Default::default()));
    let api = router(&f, a.clone(), provider.clone());
    let (status,out)=public_http(&api,json!({"summary":"legacy supplied","embedding":[2.,0.,0.,0.],"entities":["Alice"],"checkDuplicates":false})).await;
    assert_eq!(status.as_u16(), 201, "{out}");
    assert!(out.get("contractVersion").is_none());
    assert!(out["data"].get("labels").is_none());
    assert_eq!(out["data"]["entities"], json!(["alice"]));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let generation = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    let (status,out)=public_http(&api,json!({"summary":"explicit identity cannot be ignored","embedding":[1.,0.,0.,0.],"embeddingProfileId":provider.profile.profile_id().unwrap()})).await;
    assert_eq!(status.as_u16(), 400, "{out}");
    assert_eq!(out["error"], "UNSUPPORTED_CONSTRAINT");
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        generation
    );
    let mut b = bridge(a);
    b.default_namespace = "default".into();
    let r=crate::mcp::tools::dispatch_tool(&b,"store_memory",json!({"summary":"legacy generated","embedding":[0.],"embeddingProfileId":"still ignored","checkDuplicates":false})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let crate::mcp::protocol::ContentBlock::Text { text } = &r.content[0];
    let value: Value = serde_json::from_str(text).unwrap();
    assert!(value.get("contractVersion").is_none());
    assert_eq!(value["namespace"], "default");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}
