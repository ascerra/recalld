//! First internal coherent consumer. No public transport-v2 or reset activation.
use super::*;
use crate::derived_completion::DerivedCompletion;
use crate::search::adapters::{
    RifProcessorAdapter, SharedEntityIndexReader, SharedFtsIndexRegistry, SharedGraphReader,
    SharedVectorIndexRegistry,
};
use crate::storage::control::GenerationRef;
use crate::storage::metadata::CanonicalReadView;

/// A result computed against one actual canonical transaction and matching
/// derived leases. This is not a durable snapshot or a deterministic reset.
pub struct CoherentSearchResponse {
    pub read_generation: GenerationRef,
    pub evaluated_at: i64,
    pub response: SemanticsResponse,
}

fn unavailable(error: impl std::fmt::Display) -> SearchError {
    SearchError::Unavailable(error.to_string())
}

#[async_trait::async_trait]
impl NamespaceResolver for CanonicalReadView {
    async fn resolve(&self, name: &str) -> Result<NamespaceConfig> {
        let configs = self.namespaces().map_err(unavailable)?;
        configs
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .or_else(|| {
                configs
                    .into_iter()
                    .find(|c| c.name.eq_ignore_ascii_case(name))
            })
            .ok_or_else(|| SearchError::NamespaceNotFound(name.into()))
    }
    async fn resolve_id(&self, id: NamespaceId) -> Result<NamespaceConfig> {
        self.namespaces()
            .map_err(unavailable)?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| SearchError::NamespaceNotFound(id.get().to_string()))
    }
}

#[async_trait::async_trait]
impl MetadataStore for CanonicalReadView {
    async fn get(&self, id: &MemoryId) -> Result<Option<CachedRecord>> {
        CanonicalReadView::get(self, *id).map_err(unavailable)
    }
    async fn get_batch(&self, ids: &[MemoryId]) -> Result<Vec<CachedRecord>> {
        let mut records = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(record) = CanonicalReadView::get(self, *id).map_err(unavailable)? {
                records.push(record);
            }
        }
        Ok(records)
    }
    async fn list_namespace_memories(&self, id: NamespaceId) -> Result<Vec<MemoryId>> {
        Ok(self
            .records(id)
            .map_err(unavailable)?
            .into_iter()
            .map(|r| r.id)
            .collect())
    }
    async fn namespace_records(&self, id: NamespaceId) -> Result<Vec<CachedRecord>> {
        self.records(id).map_err(unavailable)
    }
}

struct BypassCache;
impl RecordCache for BypassCache {
    fn get(&self, _: &MemoryId) -> Option<CachedRecord> {
        None
    }
}
struct RejectExposure;
#[async_trait::async_trait]
impl AccessRecorder for RejectExposure {
    async fn record_access(
        &self,
        _: MemoryId,
        _: AccessKind,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
        Err(unavailable(
            "coherent read-only view cannot record exposure",
        ))
    }
    async fn record_access_batch(
        &self,
        _: &[(MemoryId, AccessKind)],
        _: Option<&crate::storage::control::ExposureScope>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
        Err(unavailable(
            "coherent read-only view cannot record exposure",
        ))
    }
}

impl QueryEngine {
    pub(crate) async fn search_coherent_read_only(
        &self,
        owner: &DerivedCompletion,
        rif: Arc<crate::rif::RifEngine>,
        query: SearchQuery,
        semantics: RetrievalSemantics,
        deadline: std::time::Instant,
    ) -> Result<CoherentSearchResponse> {
        if query.record_access {
            return Err(unavailable(
                "coherent read-only entry point requires recordAccess=false",
            ));
        }
        let work = owner.accept_work(None).map_err(unavailable)?;
        let result = async {
            let storage = owner.storage.clone();
            let child = work.child().map_err(unavailable)?;
            let gate = tokio::task::spawn_blocking(move || {
                let storage = storage.read().map_err(unavailable)?;
                let gate = storage.meta_store().derived_publication.clone();
                storage.meta_store().finish_work(child);
                Ok::<_, SearchError>(gate)
            })
            .await
            .map_err(unavailable)??;
            // The same lock order as completion. Adapters below read these leases
            // directly, avoiding recursive reads behind a queued publisher.
            let publication = gate.read_owned().await;
            let graph = Arc::new(owner.graph.clone().read_owned().await);
            let vectors = Arc::new(owner.vectors.clone().read_owned().await);
            let fts = owner.fts.clone().lock_owned().await;
            let entities = Arc::new(owner.entities.clone().read_owned().await);
            let storage = owner.storage.clone();
            let child = work.child().map_err(unavailable)?;
            let canonical = tokio::task::spawn_blocking(move || {
                let storage = storage.read().map_err(unavailable)?;
                let result = storage
                    .meta_store()
                    .capture_read_view()
                    .map_err(unavailable);
                storage.meta_store().finish_work(child);
                result
            })
            .await
            .map_err(unavailable)??;
            let at = canonical.at;
            let canonical = Arc::new(canonical);
            let evaluated_at = self.clock.now_millis();
            let engine = Self {
                clock: Arc::new(crate::time::ManualClock::new(evaluated_at)),
                decay_policy: self.decay_policy.clone(),
                decay_multiplier: self.decay_multiplier,
                namespace_resolver: canonical.clone(),
                embedding_providers: self.embedding_providers.clone(),
                vector_indexes: Arc::new(SharedVectorIndexRegistry::pinned(vectors)),
                fts_index: Arc::new(SharedFtsIndexRegistry::pinned(fts)),
                entity_index: Arc::new(SharedEntityIndexReader::pinned(entities)),
                cache: Arc::new(BypassCache),
                meta_store: canonical,
                graph: Arc::new(SharedGraphReader::pinned(graph.clone())),
                rif_engine: Arc::new(RifProcessorAdapter::pinned(rif, graph)),
                access_recorder: Arc::new(RejectExposure),
            };
            let response = engine.search_with_semantics(query, semantics).await?;
            // Explicit release before root completion, including the canonical tx.
            drop(engine);
            drop(publication);
            Ok(CoherentSearchResponse {
                read_generation: at,
                evaluated_at,
                response,
            })
        };
        let result =
            tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), result).await;
        match result {
            Ok(result) => {
                owner.finish_known_work(work);
                result
            }
            Err(_) => {
                // Every spawned worker has its own registered child. A known
                // timeout ends this read, while an unfinished/unknown child
                // independently continues to block the drain. External async
                // cancellation still drops the root unresolved.
                owner.finish_known_work(work);
                Err(unavailable("coherent query deadline expired"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::adapters::{StorageMetadataAdapter, StorageNamespaceResolver};
    use crate::storage::{StorageEngine as _, control::MutationContext, insertion::InsertMemory};
    use crate::test_support::Fixture;
    use std::time::{Duration, Instant};
    use tokio::sync::Notify;

    struct Embeddings {
        entered: Notify,
        resume: Notify,
    }
    #[async_trait::async_trait]
    impl EmbeddingProviderRegistry for Embeddings {
        fn dimensions(&self) -> Option<usize> {
            Some(4)
        }
        async fn embed(&self, _: &str, _: &str) -> Result<Vec<f32>> {
            self.entered.notify_one();
            self.resume.notified().await;
            Ok(vec![1.0, 0.0, 0.0, 0.0])
        }
    }
    fn owner(f: &Fixture) -> DerivedCompletion {
        DerivedCompletion {
            storage: f.storage.clone(),
            cache: f.cache.clone(),
            graph: f.graph.clone(),
            vectors: f.vector_index.clone(),
            fts: f.fts_index.clone(),
            entities: f.entity_index.clone(),
        }
    }
    fn engine(
        f: &Fixture,
        embeddings: Arc<Embeddings>,
        rif: Arc<crate::rif::RifEngine>,
    ) -> QueryEngine {
        QueryEngine::new(
            Arc::new(StorageNamespaceResolver::new(f.storage.clone())),
            embeddings,
            Arc::new(SharedVectorIndexRegistry::new(f.vector_index.clone())),
            Arc::new(SharedFtsIndexRegistry::new(f.fts_index.clone())),
            Arc::new(SharedEntityIndexReader::new(f.entity_index.clone())),
            Arc::new(BypassCache),
            Arc::new(StorageMetadataAdapter::new(f.storage.clone())),
            Arc::new(SharedGraphReader::new(f.graph.clone())),
            Arc::new(RifProcessorAdapter::new(rif, f.graph.clone())),
            Arc::new(RejectExposure),
        )
        .with_lifecycle_inputs(
            Arc::new(crate::time::ManualClock::new(123)),
            Default::default(),
            1.0,
        )
    }
    async fn seed(owner: &DerivedCompletion) -> MemoryId {
        let inserted = owner
            .insert(
                MutationContext::unkeyed(NamespaceId::new(1)),
                InsertMemory {
                    id: None,
                    summary: "fixture".into(),
                    full_text: Some("fixture full text".into()),
                    tags: vec![],
                    initial_stability: None,
                    created_at: Some(100),
                    embedding: Some(vec![1.0, 0.0, 0.0, 0.0]),
                    parent_id: None,
                    supersedes: None,
                },
                Vec::new().into(),
                100,
            )
            .await
            .unwrap();
        crate::storage::insertion::inserted_id(&inserted.receipt).unwrap()
    }
    fn query(precomputed: bool) -> SearchQuery {
        let mut value = serde_json::json!({"text":"fixture", "recordAccess":false,
            "graphDepth":1, "limit":10, "minScore":0.0});
        if precomputed {
            value["queryEmbedding"] = serde_json::json!([1.0, 0.0, 0.0, 0.0]);
        }
        serde_json::from_value(value).unwrap()
    }
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }
    fn bytes(f: &Fixture) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        crate::storage::validation::canonical_files(f.path())
            .unwrap()
            .into_iter()
            .map(|p| {
                let bytes = std::fs::read(&p).unwrap();
                (p, bytes)
            })
            .collect()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn coherent_read_view_actual_pipeline_preserves_canonical_bytes_and_requested_mode() {
        let f = Fixture::new();
        let owner = owner(&f);
        let id = seed(&owner).await;
        let rif = Arc::new(crate::rif::RifEngine::new(Default::default()));
        let embedding = Arc::new(Embeddings {
            entered: Notify::new(),
            resume: Notify::new(),
        });
        let engine = engine(&f, embedding, rif.clone());
        f.storage.write().unwrap().sync().unwrap();
        let before = bytes(&f);
        let at = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .canonical;
        let result = engine
            .search_coherent_read_only(
                &owner,
                rif.clone(),
                query(true),
                RetrievalSemantics::V1,
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(result.read_generation, at);
        assert_eq!(result.evaluated_at, 123);
        assert_eq!(result.response.response.results.len(), 1);
        assert_eq!(result.response.response.results[0].memory_id, id);
        assert_eq!(bytes(&f), before);
        let mut requested_true = query(true);
        requested_true.record_access = true;
        assert!(
            engine
                .search_coherent_read_only(
                    &owner,
                    rif,
                    requested_true,
                    RetrievalSemantics::V1,
                    deadline()
                )
                .await
                .is_err()
        );
        assert_eq!(bytes(&f), before);
        let status = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .status();
        assert_eq!(status.pending, 0);
        assert_eq!(status.unresolved, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn coherent_read_view_pins_canonical_record_and_does_not_relock_behind_publisher() {
        let f = Fixture::new();
        let owner = owner(&f);
        let id = seed(&owner).await;
        let rif = Arc::new(crate::rif::RifEngine::new(Default::default()));
        let embedding = Arc::new(Embeddings {
            entered: Notify::new(),
            resume: Notify::new(),
        });
        let engine = Arc::new(engine(&f, embedding.clone(), rif.clone()));
        let before = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .canonical;
        let search = {
            let engine = engine.clone();
            let owner = owner.clone();
            let rif = rif.clone();
            tokio::spawn(async move {
                engine
                    .search_coherent_read_only(
                        &owner,
                        rif,
                        query(false),
                        RetrievalSemantics::V1,
                        deadline(),
                    )
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(5), embedding.entered.notified())
            .await
            .unwrap();
        // The canonical transaction is already pinned. A real observation may
        // publish newer canonical state, but cannot replace this query's view.
        let storage = f.storage.clone();
        let receipt = tokio::task::spawn_blocking(move || {
            storage
                .write()
                .unwrap()
                .update_access_with_receipt(
                    &MutationContext::unkeyed(NamespaceId::new(1)),
                    id,
                    999,
                    AccessKind::DirectRetrieval,
                )
                .unwrap()
        })
        .await
        .unwrap();
        let (queued, waiting) = tokio::sync::oneshot::channel();
        let graph = f.graph.clone();
        let publisher = tokio::spawn(async move {
            queued.send(()).unwrap();
            let _guard = graph.write().await;
        });
        waiting.await.unwrap();
        assert!(!publisher.is_finished());
        embedding.resume.notify_one();
        let response = tokio::time::timeout(Duration::from_secs(5), search)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(response.read_generation, before);
        assert_eq!(response.response.response.results[0].last_accessed_at, 100);
        publisher.await.unwrap();
        let current = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap();
        assert_eq!(current.canonical.generation, receipt.committed_generation);
        assert!(!current.matches_canonical());
        assert_eq!(f.record(id).unwrap().last_accessed_at, 999);
        // A new read cannot reuse old stamps after the observation commit.
        assert!(
            engine
                .search_coherent_read_only(
                    &owner,
                    rif,
                    query(true),
                    RetrievalSemantics::V1,
                    deadline()
                )
                .await
                .is_err()
        );
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .status()
                .pending,
            0
        );
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn coherent_barrier_checks_actual_target_and_releases_admission_on_timeout() {
        let f = Fixture::new();
        let owner = owner(&f);
        seed(&owner).await;
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let at = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .canonical;
        let report = owner.readiness_barrier(at, deadline()).await.unwrap();
        assert!(report.ready);
        assert_eq!(report.pending_work, 0);
        assert_eq!(report.derived.len(), 4);
        assert_eq!(report.committed_generation, at.generation);
        assert!(serde_json::to_value(&report).unwrap()["committedGeneration"].is_string());
        assert!(tracker.status().admission_open);
        let foreign = GenerationRef {
            store_id: uuid::Uuid::new_v4(),
            ..at
        };
        assert!(owner.readiness_barrier(foreign, deadline()).await.is_err());
        let future = GenerationRef {
            generation: at.generation.next().unwrap(),
            ..at
        };
        assert!(owner.readiness_barrier(future, deadline()).await.is_err());
        assert!(tracker.status().admission_open);
        let held = f.graph.write().await;
        assert!(
            owner
                .readiness_barrier(at, Instant::now() + Duration::from_millis(100))
                .await
                .is_err()
        );
        assert!(tracker.status().admission_open);
        drop(held);
        assert!(owner.readiness_barrier(at, deadline()).await.unwrap().ready);
        // An effect-free canonical catalog change still invalidates old stamps.
        f.create_namespace("new-scope");
        assert!(owner.readiness_barrier(at, deadline()).await.is_err());
        assert!(tracker.status().admission_open);
        owner.complete().await.unwrap();
        let new = owner.readiness_barrier(at, deadline()).await.unwrap();
        assert!(new.committed_generation > at.generation);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn coherent_barrier_waits_for_actual_query_root_and_preserves_false_bytes() {
        let f = Fixture::new();
        let owner = owner(&f);
        seed(&owner).await;
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let at = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .canonical;
        let rif = Arc::new(crate::rif::RifEngine::new(Default::default()));
        let embedding = Arc::new(Embeddings {
            entered: Notify::new(),
            resume: Notify::new(),
        });
        let engine = engine(&f, embedding.clone(), rif.clone());
        f.storage.write().unwrap().sync().unwrap();
        let before = bytes(&f);
        let searching = {
            let owner = owner.clone();
            tokio::spawn(async move {
                engine
                    .search_coherent_read_only(
                        &owner,
                        rif,
                        query(false),
                        RetrievalSemantics::V1,
                        deadline(),
                    )
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(5), embedding.entered.notified())
            .await
            .unwrap();
        let barrier = {
            let owner = owner.clone();
            tokio::spawn(async move { owner.readiness_barrier(at, deadline()).await })
        };
        let waiting = tracker.clone();
        tokio::task::spawn_blocking(move || waiting.wait_until_closed())
            .await
            .unwrap();
        assert!(!barrier.is_finished());
        assert!(tracker.status().pending > 0);
        embedding.resume.notify_one();
        assert_eq!(searching.await.unwrap().unwrap().read_generation, at);
        let report = barrier.await.unwrap().unwrap();
        assert!(report.ready);
        assert_eq!(report.pending_work, 0);
        assert!(tracker.status().admission_open);
        assert_eq!(bytes(&f), before);
    }
}
