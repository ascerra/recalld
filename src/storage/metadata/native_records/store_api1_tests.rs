//! Public API1 routes over real migrated/native storage and the accepted owner.
use super::*;
use readers::bytes;

fn retained_insert(f: &Fixture, id: MemoryId) -> crate::storage::control::StoredReceipt {
    use redb::ReadableTable;
    let store = f.storage.read().unwrap();
    let tx = store.meta_store().db.begin_read().unwrap();
    for row in tx
        .open_table(crate::storage::control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
    {
        let (_, value) = row.unwrap();
        let stored = crate::storage::control::validate_receipt(value.value()).unwrap();
        if matches!(&stored.receipt.outcome,MutationOutcome::NativeMemoryInsertedWithLinks{transition} if transition.memory_id==id)
        {
            return stored;
        }
    }
    panic!("creation receipt absent")
}

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
    assert!(
        store
            .get_outgoing_edges(d.parent)
            .unwrap()
            .iter()
            .any(|(target, kind)| *target == id && *kind == crate::model::EdgeType::ParentChild)
    );
    drop(store);
    let stored = retained_insert(&f, id);
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &stored.receipt.outcome
    else {
        panic!()
    };
    let mut original=crate::mcp::args::parse_store_input_fields(&json!({"namespace":"native","summary":"API1 native HTTP","fullText":"native full payload","entities":["John Smith","José","Alice's"],"parentId":d.parent,"checkDuplicates":false}),"default").unwrap();
    original.embedding = Some(vec![2., -0., 0., 0.]);
    let request = crate::serialization::store_api1::Request {
        input: original,
        profile_id: Some(d.profile.profile_id().unwrap()),
        created_at: None,
        require_native: true,
    };
    let input = request.prepare_native().unwrap().input;
    let primary = &transition.primary().unwrap().after;
    let hash = |version| {
        let mut ctx = MutationContext::unkeyed(d.ns);
        ctx.contract_version = version;
        input
            .request_hash_with_profile_id(
                &ctx,
                primary.lifecycle.stability.get(),
                primary.lifecycle.difficulty.get(),
                transition.embedding_dim,
                primary.content.profile_id.as_deref().unwrap(),
                &transition.settings,
                request.profile_id.as_deref(),
            )
            .unwrap()
    };
    assert_eq!(stored.request_hash, hash(1));
    assert_ne!(stored.request_hash, hash(2));
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
            400,
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
    let (d, f, a, provider, clock) = setup().await;
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
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
        panic!()
    };
    let id = transition.memory_id;
    assert_eq!(retained_insert(&f, id).receipt, receipt);
    drop(api);
    let root = f.close();
    let reopened = open(&d, clock);
    let f = Fixture::from_open_engine(root, reopened.engine);
    assert_eq!(retained_insert(&f, id).receipt, receipt);
}

struct InputSpy {
    inner: Arc<RecoveringProvider>,
    texts: std::sync::Mutex<Vec<String>>,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for InputSpy {
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "input-spy"
    }
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        crate::embedding::EmbeddingProvider::profile(self.inner.as_ref())
    }
    async fn embed(&self, text: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.texts.lock().unwrap().push(text.to_owned());
        crate::embedding::EmbeddingProvider::embed(self.inner.as_ref(), text).await
    }
}

#[tokio::test]
async fn native_api1_store_actual_generated_document_and_wrong_binding() {
    let (d, f, a, provider, clock) = setup().await;
    drop(a);
    let spy = Arc::new(InputSpy {
        inner: provider.clone(),
        texts: Default::default(),
    });
    let a = Arc::new(adapter(&f, spy.clone(), clock, Default::default()));
    let api = router(&f, a, spy.clone());
    let mut wrong = d.profile.clone();
    wrong.model_id.push_str("-different");
    let before = f.storage.read().unwrap().control_state().unwrap();
    let (status,out)=public_http(&api,json!({"namespace":"native","summary":"wrong binding","embedding":[1.,0.,0.,0.],"embeddingProfileId":wrong.profile_id().unwrap()})).await;
    assert_eq!(status.as_u16(), 409, "{out}");
    assert!(out.get("receipt").is_none());
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let (status,out)=public_http(&api,json!({"namespace":"native","summary":"header","fullText":"body","tags":["raw"],"entities":["José","John Smith"],"topics":["Rust"],"emotions":["Calm"],"checkDuplicates":false})).await;
    assert_eq!(status.as_u16(), 201, "{out}");
    assert_eq!(
        *spy.texts.lock().unwrap(),
        ["header\n\nbody raw entity/josé entity/john smith topic/rust emotion/calm"]
    );
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
    assert_eq!(out["data"]["entities"], json!(["Alice"]));
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
    let mut old_input = crate::mcp::args::parse_store_input_fields(
        &json!({"summary":"old raw legacy RPC","checkDuplicates":false}),
        "default",
    )
    .unwrap();
    old_input.embedding = Some(vec![1., 0., 0., 0.]);
    let old = crate::daemon::server::native_method_test_rpc_with_limit(
        a.clone(),
        serde_json::to_value(old_input).unwrap(),
        "store_memory",
        None,
    )
    .await;
    assert!(old.error.is_none(), "{old:?}");
    assert_eq!(old.result.unwrap()["namespace"], "default");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_api1_store_remote_negotiation_small_peer_and_native_reopen() {
    let (d, f, a, provider, clock) = setup().await;
    let request=crate::serialization::store_api1::Request{
        input:crate::mcp::args::parse_store_input_fields(&json!({"summary":"RPC Unicode","entities":["José","John Smith"],"checkDuplicates":false}),"native").unwrap(),
        profile_id:None,created_at:None,require_native:false,
    };
    let before = bytes(&d.path);
    let ticks = clock.0.load(Ordering::SeqCst);
    let old = crate::daemon::server::native_method_test_rpc_with_limit(
        a.clone(),
        serde_json::to_value(&request.input).unwrap(),
        "store_memory",
        None,
    )
    .await;
    assert!(
        old.error
            .as_ref()
            .is_some_and(|e| e.message.contains("api1StoreV1")),
        "{old:?}"
    );
    assert_eq!(bytes(&d.path), before);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(provider.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
    for limit in [None, Some(crate::daemon::protocol::LEGACY_MAX_MESSAGE_SIZE)] {
        let rejected = crate::daemon::server::native_method_test_rpc_with_limit(
            a.clone(),
            serde_json::to_value(&request).unwrap(),
            "store_api1_v1",
            limit,
        )
        .await;
        assert!(rejected.error.is_some());
        assert!(rejected.result.is_none());
        assert_eq!(bytes(&d.path), before);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        assert_eq!(provider.profiles.load(Ordering::SeqCst), 0);
        assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
    }
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        a.clone(),
        &socket.path().join("s.sock"),
    )
    .await;
    let mut b = StubBridge::default().into_bridge();
    b.default_namespace = "native".into();
    b.storage = Arc::new(remote);
    let result = crate::mcp::tools::dispatch_tool(
        &b,
        "store_memory",
        json!({"summary":"RPC Unicode","entities":["José","John Smith"],"checkDuplicates":false}),
    )
    .await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
    let value: Value = serde_json::from_str(text).unwrap();
    assert!(value.get("contractVersion").is_none());
    let id: MemoryId = serde_json::from_value(value["id"].clone()).unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    drop(b);
    task.abort();
    let _ = task.await;
    drop(a);
    let root = f.close();
    let reopened = open(&d, clock);
    let record = reopened.engine.get_record(id).unwrap().unwrap();
    assert_eq!(
        record.schema.unwrap().labels.entities,
        ["José", "John Smith"]
    );
    drop((reopened, root));
}

#[path = "batch_api1_tests.rs"]
mod batch_api1;
