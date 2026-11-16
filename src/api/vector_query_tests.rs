//! Real HTTP/pipeline regressions with isolated storage and fixed local vectors.
use super::adapters::*;
use crate::model::{AccessKind, MemoryId, NamespaceId};
use crate::search::adapters::*;
use crate::search::{AccessRecorder, QueryEngine, SearchQuery, VectorIndex as _, VectorMetadata};
use crate::storage::StorageEngine as _;
use crate::test_support::{DIM, Fixture, MemorySpec};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Default)]
struct FixedEmbedding {
    calls: AtomicUsize,
    hold: AtomicBool,
    entered: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for FixedEmbedding {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.hold.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.resume.notified().await;
        }
        Ok(vec![0.0, 1.0, 0.0, 0.0])
    }
    fn dimensions(&self) -> usize {
        DIM
    }
    fn model_name(&self) -> &str {
        "test-fixed"
    }
}
struct ObservedAccess {
    inner: StorageAccessRecorder,
    completed: AtomicUsize,
    notify: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl AccessRecorder for ObservedAccess {
    async fn record_access_batch_at_with_work(
        &self,
        values: &[(MemoryId, AccessKind)],
        scope: Option<&crate::storage::control::ExposureScope>,
        sampled_at: i64,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        let receipts = self
            .inner
            .record_access_batch_at_with_work(values, scope, sampled_at, parent)
            .await?;
        self.completed.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
        Ok(receipts)
    }
    async fn record_access_batch_with_work(
        &self,
        values: &[(MemoryId, AccessKind)],
        scope: Option<&crate::storage::control::ExposureScope>,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        let receipts = self
            .inner
            .record_access_batch_with_work(values, scope, parent)
            .await?;
        self.completed.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
        Ok(receipts)
    }
    async fn record_access(
        &self,
        id: MemoryId,
        kind: AccessKind,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        let receipts = self.inner.record_access(id, kind).await?;
        self.completed.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
        Ok(receipts)
    }
    async fn record_access_batch(
        &self,
        values: &[(MemoryId, AccessKind)],
        scope: Option<&crate::storage::control::ExposureScope>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        let receipts = self.inner.record_access_batch(values, scope).await?;
        self.completed.fetch_add(1, Ordering::SeqCst);
        self.notify.notify_one();
        Ok(receipts)
    }
}
struct Harness {
    f: Fixture,
    engine: Arc<QueryEngine>,
    access: Arc<ObservedAccess>,
    ids: Vec<MemoryId>,
    embedding: Arc<FixedEmbedding>,
}
impl Harness {
    async fn new() -> Self {
        let f = Fixture::new();
        let access = Arc::new(ObservedAccess {
            inner: StorageAccessRecorder::new(f.storage.clone(), f.cache.clone()),
            completed: AtomicUsize::new(0),
            notify: tokio::sync::Notify::new(),
        });
        let embedding = Arc::new(FixedEmbedding::default());
        let engine = Arc::new(QueryEngine::new(
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
            access.clone(),
        ));
        let mut ids = Vec::new();
        for (summary, embedding) in [
            ("rarekeyword", [1.0, 0.0, 0.0, 0.0]),
            ("nearby", [0.8, 0.6, 0.0, 0.0]),
            ("other", [0.0, 1.0, 0.0, 0.0]),
        ] {
            let id = f
                .insert_memory_with(MemorySpec {
                    embedding: Some(&embedding),
                    summary,
                    tags: &["entity/fixture"],
                    ..MemorySpec::new(NamespaceId::new(1))
                })
                .await;
            f.vector_index
                .write()
                .await
                .add(
                    id,
                    &embedding,
                    VectorMetadata {
                        namespace_id: NamespaceId::new(1),
                        decay_phase: 1,
                        tags: vec![],
                    },
                )
                .unwrap();
            f.fts_index
                .lock()
                .await
                .add(NamespaceId::new(1), id, summary, None, &[])
                .unwrap();
            f.entity_index
                .write()
                .await
                .add(NamespaceId::new(1), id, &["fixture".into()]);
            ids.push(id);
        }
        Self {
            f,
            engine,
            access,
            ids,
            embedding,
        }
    }
    fn http(&self) -> axum::Router {
        let f = &self.f;
        let config = Arc::new(crate::config::RecalldConfig::default());
        crate::api::router(
            crate::api::AppState::new(
                Arc::new(SearchPipelineAdapter::new(
                    self.engine.clone(),
                    self.embedding.clone(),
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
            ),
            &crate::api::ApiConfig::default(),
        )
    }
    fn snapshot(&self) -> String {
        format!(
            "{:?}",
            self.ids
                .iter()
                .map(|id| self.f.record(*id))
                .collect::<Vec<_>>()
        )
    }
    async fn assert_quiet(&self, before: &str) {
        // Bounded detection of accidental queued access; not a whole-store barrier.
        assert!(
            tokio::time::timeout(Duration::from_millis(30), self.access.notify.notified())
                .await
                .is_err()
        );
        assert_eq!(self.access.completed.load(Ordering::SeqCst), 0);
        assert_eq!(self.snapshot(), before);
    }
    async fn observed(&self) {
        tokio::time::timeout(Duration::from_secs(5), self.access.notify.notified())
            .await
            .expect("legacy access did not finish");
        assert!(
            self.ids
                .iter()
                .any(|id| !self.f.record(*id).unwrap().access_history.is_empty())
        );
    }
}

fn query(value: Value) -> SearchQuery {
    serde_json::from_value(value).unwrap()
}
async fn request(h: &Harness, uri: &str, body: String) -> (axum::http::StatusCode, Value) {
    use tower::ServiceExt;
    let response = h
        .http()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vector_query_http_search_and_similarity_succeed_without_embedding_provider() {
    let h = Harness::new().await;
    for scale in [12.0, f32::MAX, f32::from_bits(1)] {
        let (status, body) = request(
            &h,
            "/search",
            json!({"embedding":[scale,0,0,0], "limit":1}).to_string(),
        )
        .await;
        assert!(status.is_success(), "{body}");
        assert_eq!(body["hits"][0]["memory"]["id"], h.ids[0].to_string());
        assert!((body["hits"][0]["score"].as_f64().unwrap() - 1.0).abs() < 1e-6);
        h.observed().await;
    }
    let (status, body) = request(
        &h,
        &format!("/similar/{}", h.ids[0]),
        json!({"limit":1}).to_string(),
    )
    .await;
    assert!(status.is_success(), "{body}");
    assert_eq!(body["hits"].as_array().unwrap().len(), 1);
    assert_eq!(body["hits"][0]["memory"]["id"], h.ids[1].to_string());
    h.observed().await;
    assert_eq!(h.embedding.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vector_query_precedence_preserves_fts_and_text_only_defaults() {
    let h = Harness::new().await;
    // Both text and vector: explicit vector wins over the opposite provider
    // vector, and the text still matches the FTS index.
    let response = h
        .engine
        .search(query(
            json!({"text":"rarekeyword", "queryEmbedding":[7,0,0,0]}),
        ))
        .await
        .unwrap();
    assert_eq!(response.results[0].memory_id, h.ids[0]);
    assert!(response.results[0].fts_score.is_some());
    h.observed().await;
    // Explicit keyword query retains its existing precedence over text.
    let response = h
        .engine
        .search(query(
            json!({"text":"notindexed", "ftsQuery":"rarekeyword", "queryEmbedding":[1,0,0,0]}),
        ))
        .await
        .unwrap();
    assert!(
        response
            .results
            .iter()
            .find(|r| r.memory_id == h.ids[0])
            .unwrap()
            .fts_score
            .is_some()
    );
    h.observed().await;
    let (status, body) = request(
        &h,
        "/search",
        json!({"query":"rarekeyword", "embedding":[1,0,0,0], "limit":1}).to_string(),
    )
    .await;
    assert!(status.is_success(), "{body}");
    assert_eq!(body["hits"][0]["memory"]["id"], h.ids[0].to_string());
    h.observed().await;
    assert_eq!(h.embedding.calls.load(Ordering::SeqCst), 0);
    let (status, body) = request(
        &h,
        "/search",
        json!({"query":"notindexed", "limit":1}).to_string(),
    )
    .await;
    assert!(status.is_success(), "{body}");
    assert_eq!(body["hits"][0]["memory"]["id"], h.ids[2].to_string());
    h.observed().await;
    assert_eq!(h.embedding.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vector_query_invalid_vectors_reject_before_access_work() {
    let h = Harness::new().await;
    let before = h.snapshot();
    for vector in [
        vec![],
        vec![1.0; 3],
        vec![1.0; 5],
        vec![0.0; 4],
        vec![-0.0; 4],
        vec![f32::NAN, 0.0, 0.0, 0.0],
        vec![f32::INFINITY, 0.0, 0.0, 0.0],
        vec![f32::NEG_INFINITY, 0.0, 0.0, 0.0],
    ] {
        let mut q = query(json!({"text":"rarekeyword"}));
        q.query_embedding = Some(vector);
        assert!(matches!(
            h.engine.search(q).await,
            Err(crate::search::SearchError::InvalidEmbedding(_))
        ));
    }
    for vector in [
        json!([]),
        json!([1, 0, 0]),
        json!([1, 0, 0, 0, 0]),
        json!([0, 0, 0, 0]),
    ] {
        let (status, body) = request(&h, "/search", json!({"embedding":vector}).to_string()).await;
        assert_eq!(
            status,
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "{body}"
        );
        assert_eq!(body["field"], "embedding");
    }
    // JSON cannot encode NaN/Infinity. Overflowing JSON numbers must still
    // fail, whether rejected by deserialization or by the vector validator.
    let (status, _) = request(&h, "/search", r#"{"embedding":[1e39,0,0,0]}"#.into()).await;
    assert!(status.is_client_error());
    h.f.with_namespace("five", 5);
    let (status, _) = request(
        &h,
        "/search",
        json!({"namespace":"five", "embedding":[1,0,0,0]}).to_string(),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    // Similarity uses an indexed vector; reject corrupt source vectors too.
    h.f.vector_index
        .write()
        .await
        .add(
            h.ids[0],
            &[0.0; DIM],
            VectorMetadata {
                namespace_id: NamespaceId::new(1),
                decay_phase: 1,
                tags: vec![],
            },
        )
        .unwrap();
    let (status, body) =
        request(&h, &format!("/similar/{}", h.ids[0]), json!({}).to_string()).await;
    assert_eq!(
        status,
        axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        "{body}"
    );
    assert_eq!(body["field"], "embedding");
    h.assert_quiet(&before).await;
    assert_eq!(h.embedding.calls.load(Ordering::SeqCst), 0);
}

// These are actual serialized HTTP/router and MCP tool consumers. The provider
// latch controls scheduling only; real storage, pipeline, hydration and exposure
// transactions run unchanged. No timing sleep is used to establish admission.
async fn accepted_query(h: &Harness, mcp: bool) {
    if mcp {
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.search = Arc::new(crate::mcp::bridge_adapters::McpSearchAdapter::new(
            h.engine.clone(),
            h.embedding.clone(),
            h.f.storage.clone(),
            h.f.graph.clone(),
            chrono_tz::UTC,
        ));
        let result = crate::mcp::tools::dispatch_tool(
            &bridge,
            "recall_memories",
            json!({
                "query":"rarekeyword", "namespace":"default", "limit":1,
                "depth":0, "recordAccess":true, "includeNeighbors":false,
            }),
        )
        .await;
        assert_ne!(result.is_error, Some(true), "{result:?}");
    } else {
        let (status, body) = request(
            h,
            "/search",
            json!({
                "query":"rarekeyword", "namespace":"default", "limit":1,
                "depth":0, "recordAccess":true,
            })
            .to_string(),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        assert_eq!(body["hits"].as_array().unwrap().len(), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_query_roots_allow_http_and_mcp_exposure_after_close() {
    for mcp in [false, true] {
        let h = Arc::new(Harness::new().await);
        h.embedding.hold.store(true, Ordering::SeqCst);
        let tracker =
            h.f.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .clone();
        let before =
            h.f.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .committed_generation;
        let running = {
            let h = h.clone();
            tokio::spawn(async move { accepted_query(&h, mcp).await })
        };
        tokio::time::timeout(Duration::from_secs(5), h.embedding.entered.notified())
            .await
            .unwrap();
        assert!(
            tracker.status().pending >= 2,
            "outer transport and pipeline roots must both be accepted"
        );
        let close = {
            let tracker = tracker.clone();
            tokio::task::spawn_blocking(move || {
                tracker.close_and_drain(std::time::Instant::now() + Duration::from_secs(5))
            })
        };
        let waiting = tracker.clone();
        tokio::task::spawn_blocking(move || waiting.wait_until_closed())
            .await
            .unwrap();
        assert!(!close.is_finished());
        assert!(tracker.begin().is_err());
        h.embedding.resume.notify_one();
        tokio::time::timeout(Duration::from_secs(5), running)
            .await
            .unwrap()
            .unwrap();
        let frozen = close.await.unwrap().unwrap();
        assert_eq!(tracker.status().pending, 0);
        assert_eq!(tracker.status().unresolved, 0);
        assert!(!tracker.status().admission_open);
        assert_eq!(h.access.completed.load(Ordering::SeqCst), 1);
        assert_eq!(
            h.ids
                .iter()
                .map(|id| h.f.record(*id).unwrap().access_history.len())
                .sum::<usize>(),
            1
        );
        assert_eq!(
            h.f.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .committed_generation
                .get(),
            before.get() + 1
        );
        drop(frozen);
        assert!(tracker.status().admission_open);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_query_cancel_keeps_http_and_mcp_roots_unresolved() {
    for mcp in [false, true] {
        let h = Arc::new(Harness::new().await);
        h.embedding.hold.store(true, Ordering::SeqCst);
        let tracker =
            h.f.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .clone();
        let before = h.snapshot();
        let running = {
            let h = h.clone();
            tokio::spawn(async move { accepted_query(&h, mcp).await })
        };
        tokio::time::timeout(Duration::from_secs(5), h.embedding.entered.notified())
            .await
            .unwrap();
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        assert_eq!(h.snapshot(), before);
        assert_eq!(h.access.completed.load(Ordering::SeqCst), 0);
        assert!(tracker.status().unresolved >= 2);
        assert!(matches!(tracker.close_and_drain(std::time::Instant::now()),
            Err(crate::storage::work::WorkError::Deadline{unresolved,..}) if unresolved>=2));
        assert!(tracker.status().admission_open);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_http_create_carries_provider_root_through_links_and_publication() {
    let h = Arc::new(Harness::new().await);
    h.embedding.hold.store(true, Ordering::SeqCst);
    let tracker =
        h.f.storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .clone();
    let running = {
        let h = h.clone();
        tokio::spawn(async move {
            request(
                &h,
                "/memories",
                json!({
                    "summary":"accepted after close", "namespace":"default",
                    "fullText":"retained synthetic text", "entities":["fixture"],
                    "checkDuplicates":false,
                })
                .to_string(),
            )
            .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), h.embedding.entered.notified())
        .await
        .unwrap();
    assert!(tracker.status().pending > 0);
    let close = {
        let tracker = tracker.clone();
        tokio::task::spawn_blocking(move || {
            tracker.close_and_drain(std::time::Instant::now() + Duration::from_secs(10))
        })
    };
    let waiting = tracker.clone();
    tokio::task::spawn_blocking(move || waiting.wait_until_closed())
        .await
        .unwrap();
    assert!(!close.is_finished());
    h.embedding.resume.notify_one();
    let (status, body) = tokio::time::timeout(Duration::from_secs(10), running)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED, "{body}");
    let id: MemoryId = serde_json::from_value(body["data"]["id"].clone()).unwrap();
    let frozen = close.await.unwrap().unwrap();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    assert_eq!(h.f.record(id).unwrap().summary, "accepted after close");
    assert!(
        h.f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    assert!(
        h.f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    // Linking can change the record after insertion. The bound cache is allowed
    // to reject the pre-link value; it must never serve stale graph metadata.
    if let Some(cached) = h.f.cache.get(id).await {
        assert_eq!(cached.edge_count, h.f.record(id).unwrap().edge_count);
    }
    drop(frozen);
    assert!(tracker.status().admission_open);
}

fn coherent_owner(h: &Harness) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: h.f.storage.clone(),
        cache: h.f.cache.clone(),
        graph: h.f.graph.clone(),
        vectors: h.f.vector_index.clone(),
        fts: h.f.fts_index.clone(),
        entities: h.f.entity_index.clone(),
    }
}
async fn coherent_harness() -> Harness {
    let mut h = Harness::new().await;
    let owner = coherent_owner(&h);
    let inserted = owner
        .insert(
            crate::storage::control::MutationContext::unkeyed(NamespaceId::new(1)),
            crate::storage::insertion::InsertMemory {
                id: None,
                summary: "coherent payload".into(),
                full_text: Some("owned résumé 日本語".into()),
                tags: vec![],
                initial_stability: None,
                created_at: None,
                embedding: Some(vec![0., 1., 0., 0.]),
                parent_id: None,
                supersedes: None,
            },
            Vec::new().into(),
            chrono::Utc::now().timestamp_millis(),
        )
        .await
        .unwrap();
    let id = crate::storage::insertion::inserted_id(&inserted.receipt).unwrap();
    h.ids.push(id);
    let mut disk = h.f.record(id).unwrap();
    disk.difficulty = 7.25;
    disk.strength = 0.42;
    disk.decay_strength = 0.67;
    h.f.storage
        .read()
        .unwrap()
        .meta_store()
        .batch_update_records(&[(id, disk)])
        .unwrap();
    owner.complete().await.unwrap();
    h.engine = Arc::new(Arc::try_unwrap(h.engine).ok().unwrap().with_coherent_owner(
        owner,
        Arc::new(crate::rif::RifEngine::new(Default::default())),
    ));
    h
}

async fn coherent_call(h: &Harness, mcp: bool, access: bool) -> (bool, String) {
    if mcp {
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.search = Arc::new(crate::mcp::bridge_adapters::McpSearchAdapter::new(
            h.engine.clone(),
            h.embedding.clone(),
            h.f.storage.clone(),
            h.f.graph.clone(),
            chrono_tz::UTC,
        ));
        let result = crate::mcp::tools::dispatch_tool(&bridge, "recall_memories",
            json!({"query":"payload","depth":0,"limit":10,"recordAccess":access,"includeNeighbors":true})).await;
        {
            let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
            (result.is_error != Some(true), text.clone())
        }
    } else {
        let (status, body) = request(
            h,
            "/search",
            json!({"query":"payload","depth":0,"limit":10,
            "recordAccess":access})
            .to_string(),
        )
        .await;
        (status.is_success(), body.to_string())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_payload_http_preserves_metadata_bytes_and_returned_only_exposure() {
    let h = coherent_harness().await;
    let before = h.snapshot();
    let input = json!({"embedding":[0.,1.,0.,0.],"limit":10,"depth":0,
        "includeEmbeddings":true,"recordAccess":false});
    let (status, body) = request(&h, "/search", input.to_string()).await;
    assert!(status.is_success(), "{body}");
    let rows = body["hits"].as_array().unwrap();
    let id = h.ids.last().unwrap().to_string();
    let row = rows.iter().find(|r| r["memory"]["id"] == id).unwrap();
    assert_eq!(row["memory"]["fullText"], "owned résumé 日本語");
    assert_eq!(row["memory"]["difficulty"], 7.25);
    assert!((row["memory"]["strength"].as_f64().unwrap() - 0.42).abs() < 1e-6);
    assert!((row["memory"]["decayStrength"].as_f64().unwrap() - 0.67).abs() < 1e-6);
    assert_eq!(row["memory"]["embedding"], json!([0., 1., 0., 0.]));
    h.assert_quiet(&before).await;
    let mut input = input;
    input["recordAccess"] = json!(true);
    input["limit"] = json!(1);
    let (status, body) = request(&h, "/search", input.to_string()).await;
    assert!(status.is_success(), "{body}");
    let returned = body["hits"][0]["memory"]["id"].as_str().unwrap();
    for id in &h.ids {
        assert_eq!(
            h.f.record(*id).unwrap().access_history.len(),
            usize::from(id.to_string() == returned)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_payload_http_never_exposes_unserialized_graph_neighbors() {
    let h = coherent_harness().await;
    let anchor = h.ids[0];
    for &id in &h.ids[1..] {
        h.f.storage
            .read()
            .unwrap()
            .add_edge(
                anchor,
                id,
                crate::model::EdgeType::Associative,
                1.0,
                false,
                100,
            )
            .unwrap();
    }
    coherent_owner(&h).complete().await.unwrap();
    let before = h.snapshot();
    let mut input = json!({"embedding":[1.,0.,0.,0.], "limit":1,
        "depth":0, "recordAccess":false});
    let (status, body) = request(&h, "/search", input.to_string()).await;
    assert!(status.is_success(), "{body}");
    assert_eq!(body["hits"].as_array().unwrap().len(), 1);
    assert_eq!(body["hits"][0]["memory"]["id"], anchor.to_string());
    assert!(body.get("neighbors").is_none());
    h.assert_quiet(&before).await;
    input["recordAccess"] = json!(true);
    let (status, body) = request(&h, "/search", input.to_string()).await;
    assert!(status.is_success(), "{body}");
    assert_eq!(body["hits"].as_array().unwrap().len(), 1);
    assert_eq!(body["hits"][0]["memory"]["id"], anchor.to_string());
    for &id in &h.ids {
        assert_eq!(
            h.f.record(id).unwrap().access_history.len(),
            usize::from(id == anchor)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_payload_transports_reject_concurrent_forget_and_compaction_before_exposure() {
    for mcp in [false, true] {
        for compact in [false, true] {
            for access in [false, true] {
                let h = Arc::new(coherent_harness().await);
                let owner = coherent_owner(&h);
                let at =
                    h.f.storage
                        .read()
                        .unwrap()
                        .meta_store()
                        .publication_state()
                        .unwrap()
                        .canonical;
                h.embedding.hold.store(true, Ordering::SeqCst);
                let child = h.clone();
                let query = tokio::spawn(async move { coherent_call(&child, mcp, access).await });
                tokio::time::timeout(Duration::from_secs(10), h.embedding.entered.notified())
                    .await
                    .unwrap();
                let id = *h.ids.last().unwrap();
                let writer = tokio::spawn(async move {
                    if compact {
                        let storage = owner.storage.clone();
                        tokio::task::spawn_blocking(move || {
                            storage.write().unwrap().compact_text_log()
                        })
                        .await
                        .unwrap()
                        .unwrap();
                    } else {
                        assert!(owner.forget(id, Some("default")).await.unwrap());
                    }
                });
                tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        if h.f
                            .storage
                            .read()
                            .unwrap()
                            .meta_store()
                            .publication_state()
                            .unwrap()
                            .canonical
                            != at
                        {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                h.embedding.resume.notify_one();
                let (status, body) = tokio::time::timeout(Duration::from_secs(10), query)
                    .await
                    .unwrap()
                    .unwrap();
                assert!(
                    !status,
                    "mcp={mcp} compact={compact} access={access}: {body}"
                );
                assert!(!body.to_string().contains("owned résumé"));
                assert_eq!(h.access.completed.load(Ordering::SeqCst), 0);
                tokio::time::timeout(Duration::from_secs(10), writer)
                    .await
                    .unwrap()
                    .unwrap();
                let state =
                    h.f.storage
                        .read()
                        .unwrap()
                        .meta_store()
                        .work_tracker()
                        .status();
                assert_eq!(state.pending, 0);
                assert_eq!(state.unresolved, 0);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_payload_postcommit_faults_retain_receipts() {
    for mcp in [false, true] {
        for fault in [1, 2] {
            let h = coherent_harness().await;
            h.f.storage
                .read()
                .unwrap()
                .meta_store()
                .observation_fault
                .store(fault, Ordering::SeqCst);
            let before = h.snapshot();
            let (ok, body) = coherent_call(&h, mcp, false).await;
            assert!(ok, "{mcp}/{fault}: {body}");
            assert!(body.contains("owned résumé"));
            h.assert_quiet(&before).await;
            let (ok, body) = coherent_call(&h, mcp, true).await;
            assert!(!ok, "{mcp}/{fault}: {body}");
            assert!(!body.contains("owned résumé"));
            let error: Value = if mcp {
                let start = body.find("{\"commitState\"").expect(&body);
                serde_json::Deserializer::from_str(&body[start..])
                    .into_iter()
                    .next()
                    .unwrap()
                    .unwrap()
            } else {
                serde_json::from_str::<Value>(&body).unwrap()["observation"].clone()
            };
            assert_eq!(error["commitState"], "committed", "{body}");
            let receipts: Vec<crate::storage::control::WriteReceipt> =
                serde_json::from_value(error["receipts"].clone()).unwrap();
            assert_eq!(receipts.len(), 1);
            let crate::storage::control::MutationOutcome::ExposuresRecorded { accesses } =
                &receipts[0].outcome
            else {
                panic!("{body}");
            };
            assert!(!accesses.is_empty());
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coherent_payload_query_and_exposure_share_one_clock_sample_after_provider_wait() {
    for mcp in [false, true] {
        let mut h = coherent_harness().await;
        let sampled = 2_000_000_000_000;
        let clock = Arc::new(crate::time::ManualClock::new(sampled));
        h.engine = Arc::new(
            Arc::try_unwrap(h.engine)
                .ok()
                .unwrap()
                .with_lifecycle_inputs(clock.clone(), Default::default(), 0.0),
        );
        h.embedding.hold.store(true, Ordering::SeqCst);
        let h = Arc::new(h);
        let child = h.clone();
        let query = tokio::spawn(async move { coherent_call(&child, mcp, true).await });
        tokio::time::timeout(Duration::from_secs(10), h.embedding.entered.notified())
            .await
            .unwrap();
        clock.advance_to(sampled + 86_400_000).unwrap();
        h.embedding.resume.notify_one();
        let (ok, body) = tokio::time::timeout(Duration::from_secs(10), query)
            .await
            .unwrap()
            .unwrap();
        assert!(ok, "{body}");
        let mut exposed = 0;
        for id in &h.ids {
            let record = h.f.record(*id).unwrap();
            for event in &record.access_history {
                assert_eq!(event.timestamp, sampled);
                exposed += 1;
            }
        }
        assert!(exposed > 0);
        assert!(
            h.f.storage
                .read()
                .unwrap()
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
        // A later false query observes the published state without needing a write-side repair.
        h.embedding.hold.store(false, Ordering::SeqCst);
        let before = h.snapshot();
        let (ok, body) = coherent_call(&h, mcp, false).await;
        assert!(ok, "{body}");
        assert_eq!(h.snapshot(), before);
    }
}

#[path = "coherent_by_id_tests.rs"]
mod coherent_by_id_tests;
