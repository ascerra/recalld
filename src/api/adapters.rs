//! Adapter implementations connecting API-layer traits to real subsystems.
//!
//! Each adapter wraps the concrete subsystem type and implements the
//! corresponding API trait from `state.rs`. These adapters bridge the
//! gap between the API server's DI traits and the concrete subsystem
//! types held by the `Recalld` struct.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::cache::CacheManager;
use crate::config::RecalldConfig;
use crate::embedding::EmbeddingProvider;
use crate::graph::SharedGraph;
use crate::model::{
    AccessKind, CachedRecord, DecayPhase, DiskRecord, MemoryId, NamespaceConfig, NamespaceId, Tag,
    structured_metadata,
};
use crate::search::{EntityIndex, FtsIndex, VectorIndexOwner};
use crate::storage::RedbStorageEngine;
// Import the StorageEngine trait so its methods are in scope.
use super::state;
use crate::storage::StorageEngine as StorageEngineTrait;

// ═══════════════════════════════════════════════════════════════════════
// LinkError -> AppError
// ═══════════════════════════════════════════════════════════════════════

/// A rejected `supersedes`/`parentId` target becomes a 422 naming the
/// offending field — 422 is exactly the "syntactically fine, semantically
/// impossible" case. The two internal-inconsistency variants are ours, not
/// the caller's, so they become 500s instead.
impl From<crate::graph::LinkError> for super::errors::AppError {
    fn from(e: crate::graph::LinkError) -> Self {
        use crate::graph::LinkError;
        let field = e.field().to_string();
        match e {
            LinkError::TargetNotFound { .. }
            | LinkError::NamespaceMismatch { .. }
            | LinkError::SelfReference { .. } => super::errors::AppError::UnprocessableEntity {
                message: e.to_string(),
                field: Some(field),
            },
            LinkError::TargetMissingFromGraph { .. } | LinkError::Lookup { .. } => {
                super::errors::AppError::Internal {
                    source: Box::new(e),
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SearchPipelineAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Adapts the real search subsystems to the API `SearchPipeline` trait.
pub struct SearchPipelineAdapter {
    query_engine: Arc<crate::search::QueryEngine>,
    embedding: Arc<dyn EmbeddingProvider>,
    vector_index: Arc<tokio::sync::RwLock<VectorIndexOwner>>,
    fts_index: Arc<tokio::sync::Mutex<FtsIndex>>,
    entity_index: Arc<tokio::sync::RwLock<EntityIndex>>,
}

impl SearchPipelineAdapter {
    /// Creates a new adapter from the real search subsystem components.
    pub fn new(
        query_engine: Arc<crate::search::QueryEngine>,
        embedding: Arc<dyn EmbeddingProvider>,
        vector_index: Arc<tokio::sync::RwLock<VectorIndexOwner>>,
        fts_index: Arc<tokio::sync::Mutex<FtsIndex>>,
        entity_index: Arc<tokio::sync::RwLock<EntityIndex>>,
    ) -> Self {
        Self {
            query_engine,
            embedding,
            vector_index,
            fts_index,
            entity_index,
        }
    }
}

#[async_trait]
impl state::SearchPipeline for SearchPipelineAdapter {
    fn active_embedding(
        &self,
    ) -> Result<crate::serialization::session::ActiveEmbedding, crate::embedding::EmbeddingError>
    {
        crate::serialization::session::ActiveEmbedding::from_provider(self.embedding.as_ref())
    }
    fn accept_work(
        &self,
    ) -> Result<Option<crate::storage::work::WorkTicket>, crate::search::SearchError> {
        self.query_engine.accept_work(None)
    }
    fn finish_work(&self, work: Option<crate::storage::work::WorkTicket>) {
        self.query_engine.finish_work(work);
    }
    async fn prepare_coherent_search_with_work(
        &self,
        query: state::SearchQuery,
        include_embeddings: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Option<crate::search::CoherentSearchResponse>, crate::search::SearchError> {
        self.query_engine
            .prepare_coherent_payload(
                Self::pipeline_query(query)?,
                include_embeddings,
                false,
                parent,
            )
            .await
    }

    async fn prepare_coherent_get_with_work(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        requested_access: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Option<crate::search::CoherentGetResponse>, crate::search::SearchError> {
        self.query_engine
            .prepare_coherent_get(id, namespace, requested_access, parent)
            .await
    }
    async fn prepare_coherent_similar_with_work(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        limit: usize,
        min_score: Option<f32>,
        requested_access: bool,
        include_embeddings: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Option<crate::search::CoherentSimilarResponse>, crate::search::SearchError> {
        self.query_engine
            .prepare_coherent_similar(
                id,
                namespace,
                limit,
                min_score,
                requested_access,
                include_embeddings,
                parent,
            )
            .await
    }

    async fn prepare_search_with_work(
        &self,
        query: state::SearchQuery,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<state::ResolvedSearchResult>, crate::search::SearchError> {
        self.search_inner(query, true, parent).await
    }
    async fn prepare_similar_with_work(
        &self,
        id: MemoryId,
        limit: usize,
        min_score: Option<f32>,
        requested_access: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::search::PipelineSearchResult>, crate::search::SearchError> {
        self.query_engine
            .prepare_similar_filtered_with_work(id, limit, min_score, requested_access, parent)
            .await
    }
    async fn record_similar_with_work(
        &self,
        ids: &[MemoryId],
        scope: crate::storage::control::ExposureScope,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        self.query_engine
            .record_similar_accesses_with_work(ids, scope, parent)
            .await
    }
    async fn record_coherent_exposures_with_work(
        &self,
        accesses: &[(MemoryId, AccessKind)],
        scope: crate::storage::control::ExposureScope,
        sampled_at: i64,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        self.query_engine
            .record_exposures_at_with_work(accesses, scope, sampled_at, parent)
            .await
    }

    async fn record_exposures_with_work(
        &self,
        accesses: &[(MemoryId, AccessKind)],
        scope: crate::storage::control::ExposureScope,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        self.query_engine
            .record_exposures_with_work(accesses, scope, parent)
            .await
    }

    async fn embed_text(
        &self,
        text: &str,
        namespace: &NamespaceConfig,
    ) -> Result<crate::embedding::runtime::GeneratedEmbedding, crate::embedding::EmbeddingError>
    {
        crate::embedding::runtime::generate(self.embedding.as_ref(), namespace, text).await
    }

    async fn record_exposures(
        &self,
        accesses: &[(MemoryId, AccessKind)],
        scope: crate::storage::control::ExposureScope,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        self.query_engine.record_exposures(accesses, scope).await
    }

    async fn search(
        &self,
        query: state::SearchQuery,
    ) -> Result<Vec<state::ResolvedSearchResult>, crate::search::SearchError> {
        self.search_inner(query, false, None).await
    }
    async fn prepare_search(
        &self,
        query: state::SearchQuery,
    ) -> Result<Vec<state::ResolvedSearchResult>, crate::search::SearchError> {
        self.search_inner(query, true, None).await
    }

    async fn find_similar(
        &self,
        id: MemoryId,
        limit: usize,
        min_score: Option<f32>,
        record_access: bool,
    ) -> Result<Vec<crate::search::PipelineSearchResult>, crate::search::SearchError> {
        self.query_engine
            .similar_filtered_with_access(id, limit, min_score, record_access)
            .await
    }

    async fn prepare_similar_filtered(
        &self,
        id: MemoryId,
        limit: usize,
        min_score: Option<f32>,
        requested_access: bool,
    ) -> Result<Vec<crate::search::PipelineSearchResult>, crate::search::SearchError> {
        self.query_engine
            .prepare_similar_filtered(id, limit, min_score, requested_access)
            .await
    }

    async fn record_similar_accesses(
        &self,
        ids: &[MemoryId],
        scope: crate::storage::control::ExposureScope,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>, crate::search::SearchError> {
        self.query_engine.record_similar_accesses(ids, scope).await
    }

    async fn near_duplicates(
        &self,
        embedding: &[f32],
        namespace_id: NamespaceId,
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>, super::errors::AppError> {
        let stamp = self.query_engine.effects_stamp().await?;
        let index = self.vector_index.read().await;
        let result = crate::search::find_near_duplicates(
            index
                .uniform_index()
                .map_err(|e| crate::storage::StorageError::RecoveryRequired(e.to_string()))?,
            embedding,
            namespace_id,
            threshold,
            limit,
        );
        drop(index);
        if self.query_engine.effects_stamp().await? != stamp {
            return Err(crate::storage::StorageError::RecoveryRequired(
                "effects changed during advisory".into(),
            )
            .into());
        }
        Ok(result)
    }

    async fn index_memory(&self, id: MemoryId, embedding: &[f32], namespace_id: NamespaceId) {
        use crate::search::VectorMetadata;
        let mut index = self.vector_index.write().await;
        let metadata = VectorMetadata {
            namespace_id,
            decay_phase: DecayPhase::Full.as_u8(),
            tags: Vec::new(),
        };
        let _ = index.add(id, embedding, metadata);
    }

    async fn remove_from_index(&self, id: MemoryId) {
        let mut index = self.vector_index.write().await;
        let _ = index.remove(id);
    }

    fn get_embedding(&self, id: MemoryId) -> Option<Vec<f32>> {
        let index = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.vector_index.read())
        });
        index.get_vector(id)
    }

    fn get_embeddings_batch(
        &self,
        ids: &[MemoryId],
    ) -> std::collections::HashMap<MemoryId, Vec<f32>> {
        // Acquire the lock once for the entire batch instead of
        // once per ID, amortizing the block_in_place overhead.
        let index = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.vector_index.read())
        });
        let mut result = std::collections::HashMap::with_capacity(ids.len());
        for &id in ids {
            if let Some(vec) = index.get_vector(id) {
                result.insert(id, vec);
            }
        }
        result
    }

    fn indexed_count(&self) -> usize {
        let index = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.vector_index.read())
        });
        index.len()
    }

    async fn embedding_provider_healthy(&self) -> bool {
        // A simple health check: try to embed a trivial string.
        self.embedding.embed("health check").await.is_ok()
    }

    async fn fts_add(
        &self,
        namespace_id: NamespaceId,
        memory_id: MemoryId,
        summary: &str,
        full_text: Option<&str>,
        tags: &[String],
    ) {
        let fts = self.fts_index.lock().await;
        if let Err(e) = fts.add(namespace_id, memory_id, summary, full_text, tags) {
            tracing::warn!(
                memory_id = %memory_id,
                %e,
                "FTS5 indexing failed (non-fatal)"
            );
        }
    }

    async fn fts_remove(&self, memory_id: MemoryId) {
        let fts = self.fts_index.lock().await;
        if let Err(e) = fts.remove(memory_id) {
            tracing::warn!(
                memory_id = %memory_id,
                %e,
                "FTS5 removal failed (non-fatal)"
            );
        }
    }

    async fn entity_index_add(
        &self,
        namespace_id: NamespaceId,
        memory_id: MemoryId,
        entities: &[String],
    ) {
        self.entity_index
            .write()
            .await
            .add(namespace_id, memory_id, entities);
    }

    async fn entity_index_remove(&self, namespace_id: NamespaceId, memory_id: MemoryId) {
        self.entity_index
            .write()
            .await
            .remove(namespace_id, memory_id);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// StorageEngineAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Adapts `RedbStorageEngine` to the API `StorageEngine` trait.
pub struct StorageEngineAdapter {
    native_policy: Option<crate::storage::native_lifecycle::NativePolicyInputs>,
    clock: Arc<dyn crate::time::Clock>,
    storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    /// Held so `delete_memory` can clean the indexes storage cannot
    /// reach. Without them this adapter could tombstone a record and
    /// leave it indexed everywhere else.
    vector_index: Arc<tokio::sync::RwLock<VectorIndexOwner>>,
    /// See [`vector_index`](Self::vector_index).
    fts_index: Arc<tokio::sync::Mutex<FtsIndex>>,
    /// See [`vector_index`](Self::vector_index).
    entity_index: Arc<tokio::sync::RwLock<EntityIndex>>,
    /// See [`vector_index`](Self::vector_index).
    graph: crate::graph::SharedGraph,
}

impl StorageEngineAdapter {
    pub(crate) fn with_native_lifecycle_policy(
        mut self,
        policy: crate::storage::native_lifecycle::NativePolicyInputs,
    ) -> Self {
        self.native_policy = Some(policy);
        self
    }

    /// Creates a new adapter wrapping the storage engine, the cache, and
    /// the four indexes a delete has to clean.
    pub fn new(
        storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
        cache: Arc<CacheManager>,
        vector_index: Arc<tokio::sync::RwLock<VectorIndexOwner>>,
        fts_index: Arc<tokio::sync::Mutex<FtsIndex>>,
        entity_index: Arc<tokio::sync::RwLock<EntityIndex>>,
        graph: crate::graph::SharedGraph,
    ) -> Self {
        cache.bind_storage(&storage);
        Self {
            native_policy: None,
            clock: Arc::new(crate::time::SystemClock),
            storage,
            cache,
            vector_index,
            fts_index,
            entity_index,
            graph,
        }
    }
    /// Use the system instance's clock (or an isolated manual clock).
    pub fn with_clock(mut self, clock: Arc<dyn crate::time::Clock>) -> Self {
        self.clock = clock;
        self
    }
}

#[async_trait]
impl state::StorageEngine for StorageEngineAdapter {
    async fn forget_api1(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
    ) -> crate::serialization::lifecycle_api1::Reply {
        let owner = crate::derived_completion::DerivedCompletion {
            storage: self.storage.clone(),
            cache: self.cache.clone(),
            graph: self.graph.clone(),
            vectors: self.vector_index.clone(),
            fts: self.fts_index.clone(),
            entities: self.entity_index.clone(),
        };
        crate::serialization::lifecycle_api1::execute(
            &owner,
            crate::serialization::lifecycle_api1::request(id, 3, namespace),
            crate::serialization::native_lifecycle::Operation::Forget,
            self.native_policy.as_ref(),
            Default::default(),
            self.clock.clone(),
        )
        .await
    }

    async fn forget_native(
        &self,
        request: crate::serialization::native_lifecycle::Request,
    ) -> crate::serialization::native_lifecycle::Reply {
        let Some(policy) = self.native_policy.as_ref() else {
            return crate::serialization::native_lifecycle::Reply::unavailable(
                "native lifecycle policy not configured",
            );
        };
        let owner = crate::derived_completion::DerivedCompletion {
            storage: self.storage.clone(),
            cache: self.cache.clone(),
            graph: self.graph.clone(),
            vectors: self.vector_index.clone(),
            fts: self.fts_index.clone(),
            entities: self.entity_index.clone(),
        };
        crate::mcp::native_lifecycle_adapter::execute(
            &owner,
            request,
            crate::serialization::native_lifecycle::Operation::Forget,
            policy,
            self.clock.clone(),
        )
        .await
    }

    async fn store_identity(&self) -> Result<uuid::Uuid, crate::storage::StorageError> {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            let engine = storage.read().map_err(|_| {
                crate::storage::StorageError::RecoveryRequired("store identity unavailable".into())
            })?;
            Ok(engine.control_state()?.store_id)
        })
        .await
        .map_err(|_| {
            crate::storage::StorageError::RecoveryRequired("store identity unavailable".into())
        })?
    }
    async fn scan_duplicate_pairs(
        &self,
        request: crate::serialization::scan::ExactScanRequest,
    ) -> Result<crate::search::exact_scan::ExactScanPage, crate::search::exact_scan::ExactScanError>
    {
        crate::search::exact_scan::scan_exact(self.storage.clone(), request).await
    }

    async fn publication_lease(
        &self,
    ) -> Result<Option<tokio::sync::OwnedRwLockReadGuard<()>>, crate::storage::StorageError> {
        Ok(Some(
            crate::derived_completion::publication_gate(&self.storage)
                .await?
                .read_owned()
                .await,
        ))
    }

    async fn effects_stamp(&self) -> Result<u64, crate::storage::StorageError> {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            storage
                .read()
                .map_err(|e| crate::storage::StorageError::RecoveryRequired(e.to_string()))?
                .meta_store()
                .effects_stamp()
        })
        .await
        .map_err(|e| crate::storage::StorageError::RecoveryRequired(e.to_string()))?
    }

    async fn native_store_format(&self) -> Result<bool, crate::storage::StorageError> {
        self.storage
            .read()
            .map_err(|e| crate::storage::StorageError::RecoveryRequired(e.to_string()))?
            .control_state()
            .map(|c| c.control_schema_version == 2)
    }

    async fn insert_memory(
        &self,
        namespace_id: NamespaceId,
        input: crate::storage::insertion::InsertMemory,
        generated_embedding: crate::embedding::runtime::GeneratedEmbedding,
    ) -> Result<crate::storage::insertion::InsertResult, crate::storage::StorageError> {
        self.insert_memory_with_work(namespace_id, input, generated_embedding, None)
            .await
    }

    async fn insert_memory_with_work(
        &self,
        namespace_id: NamespaceId,
        input: crate::storage::insertion::InsertMemory,
        generated_embedding: crate::embedding::runtime::GeneratedEmbedding,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<crate::storage::insertion::InsertResult, crate::storage::StorageError> {
        crate::derived_completion::DerivedCompletion {
            storage: self.storage.clone(),
            cache: self.cache.clone(),
            graph: self.graph.clone(),
            vectors: self.vector_index.clone(),
            fts: self.fts_index.clone(),
            entities: self.entity_index.clone(),
        }
        .insert_reserved_with_work(
            crate::storage::control::MutationContext::unkeyed(namespace_id),
            input,
            generated_embedding,
            self.clock.now_millis(),
            None,
            parent,
        )
        .await
    }

    async fn complete_insertion(
        &self,
        receipt: &crate::storage::control::WriteReceipt,
    ) -> Result<(), crate::storage::StorageError> {
        self.complete_insertion_with_work(receipt, None).await
    }

    async fn complete_insertion_with_work(
        &self,
        receipt: &crate::storage::control::WriteReceipt,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<(), crate::storage::StorageError> {
        crate::derived_completion::DerivedCompletion {
            storage: self.storage.clone(),
            cache: self.cache.clone(),
            graph: self.graph.clone(),
            vectors: self.vector_index.clone(),
            fts: self.fts_index.clone(),
            entities: self.entity_index.clone(),
        }
        .complete_with_work(parent)
        .await
        .map_err(|source| crate::storage::StorageError::CommittedMutation {
            receipt: Box::new(receipt.clone()),
            source: Box::new(source),
        })
    }

    async fn create_memory(
        &self,
        namespace_id: NamespaceId,
        summary: &str,
        full_text: Option<&str>,
        tags: &[String],
        embedding: &[f32],
        initial_stability: Option<f32>,
        created_at: Option<i64>,
    ) -> Result<CachedRecord, crate::storage::StorageError> {
        let result = self
            .insert_memory(
                namespace_id,
                crate::storage::insertion::InsertMemory {
                    id: None,
                    summary: summary.into(),
                    full_text: full_text.map(str::to_owned),
                    tags: crate::model::parse_tags_lossy("create_memory.tags", tags),
                    embedding: Some(embedding.to_vec()),
                    initial_stability,
                    created_at,
                    parent_id: None,
                    supersedes: None,
                },
                vec![].into(),
            )
            .await?;
        result.record.ok_or_else(|| {
            crate::storage::StorageError::NotFound(
                crate::storage::insertion::inserted_id(&result.receipt)
                    .expect("insertion receipt")
                    .into_inner(),
            )
        })
    }

    async fn get_record(&self, id: MemoryId) -> Option<DiskRecord> {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            let storage_r = storage.read().ok()?;
            storage_r.get_record(id).ok().flatten()
        })
        .await
        .ok()
        .flatten()
    }

    async fn assert_memory_namespace(
        &self,
        id: MemoryId,
        namespace: &str,
    ) -> Result<(), crate::storage::StorageError> {
        let storage = self.storage.clone();
        let namespace = namespace.to_owned();
        tokio::task::spawn_blocking(move || {
            let storage = storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::other(e.to_string()))
            })?;
            storage.assert_memory_namespace(id, &namespace)
        })
        .await
        .map_err(|e| crate::storage::StorageError::Io(std::io::Error::other(e.to_string())))?
    }

    async fn delete_memory(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
    ) -> Result<bool, crate::storage::StorageError> {
        self.delete_memory_with_work(id, namespace, None).await
    }

    async fn delete_memory_with_work(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<bool, crate::storage::StorageError> {
        crate::derived_completion::DerivedCompletion {
            storage: self.storage.clone(),
            cache: self.cache.clone(),
            graph: self.graph.clone(),
            vectors: self.vector_index.clone(),
            fts: self.fts_index.clone(),
            entities: self.entity_index.clone(),
        }
        .forget_with_work(id, namespace, parent)
        .await
    }

    async fn namespace_stats(
        &self,
        namespace_id: NamespaceId,
    ) -> Result<state::NamespaceStats, crate::storage::StorageError> {
        let storage = self.storage.clone();

        tokio::task::spawn_blocking(move || {
            let storage_r = storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("storage lock poisoned: {e}"),
                ))
            })?;

            let all_records = storage_r.scan_all()?;

            let mut memory_count: u64 = 0;
            let mut tombstone_count: u64 = 0;
            let mut phase_1_count: u64 = 0;
            let mut phase_2_count: u64 = 0;
            let mut phase_3_count: u64 = 0;
            let mut permastore_count: u64 = 0;
            let mut strength_sum: f64 = 0.0;
            let mut edge_count: u64 = 0;

            for (_id, record) in &all_records {
                if NamespaceId::new(record.namespace_id) != namespace_id {
                    continue;
                }

                // Mirrors the MCP path exactly: a tombstoned record is not
                // a memory, and counting it dragged avg_strength toward
                // zero while still contributing its (unzeroed) permastore
                // flag and edge count.
                if record.phase == DecayPhase::Tombstone {
                    tombstone_count += 1;
                    continue;
                }
                memory_count += 1;

                match record.phase {
                    DecayPhase::Full => phase_1_count += 1,
                    DecayPhase::Summary => phase_2_count += 1,
                    DecayPhase::Ghost => phase_3_count += 1,
                    // Unreachable: skipped above. An arm, not an
                    // `unreachable!()` -- this is a read path.
                    DecayPhase::Tombstone => {}
                }

                if record.is_permastore != 0 {
                    permastore_count += 1;
                }

                strength_sum += record.decay_strength as f64;
                edge_count += record.edge_count as u64;
            }

            let avg_strength = if memory_count > 0 {
                (strength_sum / memory_count as f64) as f32
            } else {
                0.0
            };

            Ok(state::NamespaceStats {
                memory_count,
                tombstone_count,
                phase_1_count,
                phase_2_count,
                phase_3_count,
                permastore_count,
                avg_strength,
                edge_count,
            })
        })
        .await
        .map_err(|e| {
            crate::storage::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("blocking task join error: {e}"),
            ))
        })?
    }

    async fn list_memories(
        &self,
        filter: &crate::api::models::ListFilter,
    ) -> Result<Vec<CachedRecord>, crate::storage::StorageError> {
        let storage = self.storage.clone();
        let filter = filter.clone();

        tokio::task::spawn_blocking(move || {
            let storage_r = storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("storage lock poisoned: {e}"),
                ))
            })?;

            let all_records = storage_r.scan_all()?;

            let filtered: Vec<CachedRecord> = all_records
                .into_iter()
                .filter(|(_id, record)| {
                    if record.phase == DecayPhase::Tombstone {
                        return false;
                    }
                    if let Some(ns_id) = filter.namespace_id {
                        if NamespaceId::new(record.namespace_id) != ns_id {
                            return false;
                        }
                    }
                    if let Some(phase) = filter.phase {
                        if record.phase != phase {
                            return false;
                        }
                    }
                    if !filter.tags.is_empty()
                        && !filter
                            .tags
                            .iter()
                            .all(|tag| record.tags.iter().any(|t| t.as_str() == tag))
                    {
                        return false;
                    }
                    if let Some(start) = filter.time_range_start {
                        if record.created_at < start {
                            return false;
                        }
                    }
                    if let Some(end) = filter.time_range_end {
                        if record.created_at > end {
                            return false;
                        }
                    }
                    true
                })
                .map(|(_id, disk_record)| CachedRecord::from(&disk_record))
                .collect();

            Ok(filtered)
        })
        .await
        .map_err(|e| {
            crate::storage::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("blocking task join error: {e}"),
            ))
        })?
    }

    async fn get_full_text(
        &self,
        id: MemoryId,
    ) -> Result<Option<String>, crate::storage::StorageError> {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            let storage_r = storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("storage lock poisoned: {e}"),
                ))
            })?;
            let record = match storage_r.get_record(id)? {
                Some(r) => r,
                None => return Ok(None),
            };
            if record.text_length == 0 {
                return Ok(None);
            }
            let text_ref = crate::storage::TextRef {
                file_offset: record.text_offset,
                length: record.text_length,
            };
            storage_r.get_text(text_ref)
        })
        .await
        .map_err(|e| {
            crate::storage::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("blocking task join error: {e}"),
            ))
        })?
    }

    async fn list_memories_filtered(
        &self,
        namespace_id: NamespaceId,
        require_tags: &[Tag],
        time_range_start: Option<i64>,
        time_range_end: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<(MemoryId, DiskRecord)>, u64), crate::storage::StorageError> {
        let storage = self.storage.clone();
        let tags_owned: Vec<Tag> = require_tags.to_vec();

        tokio::task::spawn_blocking(move || {
            let storage_r = storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("storage lock poisoned: {e}"),
                ))
            })?;
            storage_r.meta_store().list_memories_filtered(
                namespace_id,
                &tags_owned,
                time_range_start,
                time_range_end,
                offset,
                limit,
            )
        })
        .await
        .map_err(|e| {
            crate::storage::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("blocking task join error: {e}"),
            ))
        })?
    }

    async fn ping(&self) -> bool {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || storage.read().is_ok())
            .await
            .unwrap_or(false)
    }

    async fn scan_all(&self) -> Result<Vec<(MemoryId, DiskRecord)>, crate::storage::StorageError> {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            let storage_r = storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("storage lock poisoned: {e}"),
                ))
            })?;
            storage_r.scan_all()
        })
        .await
        .map_err(|e| {
            crate::storage::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("blocking task join error: {e}"),
            ))
        })?
    }

    async fn scan_phase_records(
        &self,
        phase: DecayPhase,
    ) -> Result<Vec<(MemoryId, DiskRecord)>, crate::storage::StorageError> {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            let storage_r = storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("storage lock poisoned: {e}"),
                ))
            })?;
            storage_r.scan_phase_records(phase)
        })
        .await
        .map_err(|e| {
            crate::storage::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("blocking task join error: {e}"),
            ))
        })?
    }

    async fn list_tags(&self) -> Result<Vec<(String, u64)>, crate::storage::StorageError> {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            let storage_r = storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("storage lock poisoned: {e}"),
                ))
            })?;
            storage_r.list_tags()
        })
        .await
        .map_err(|e| {
            crate::storage::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("blocking task join error: {e}"),
            ))
        })?
    }

    fn storage_path(&self) -> Result<PathBuf, crate::storage::StorageError> {
        tokio::task::block_in_place(|| {
            let storage_r = self.storage.read().map_err(|e| {
                crate::storage::StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("storage lock poisoned: {e}"),
                ))
            })?;
            Ok(storage_r.db_path().to_path_buf())
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RecordCacheAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Adapts `CacheManager` to the API `RecordCache` trait.
pub struct RecordCacheAdapter {
    cache: Arc<CacheManager>,
}

impl RecordCacheAdapter {
    /// Creates a new adapter wrapping the cache manager.
    pub fn new(cache: Arc<CacheManager>) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl state::RecordCache for RecordCacheAdapter {
    async fn get_or_load(
        &self,
        id: MemoryId,
        storage: &dyn state::StorageEngine,
    ) -> Option<Arc<CachedRecord>> {
        // Use the injected storage trait for loading on cache miss.
        // First check the cache directly for a fast-path hit.
        if let Some(arc) = self.cache.get(id).await {
            return Some(arc);
        }

        // Cache miss: load from the injected storage engine.
        match storage.get_record(id).await {
            Some(disk) => {
                let record = CachedRecord::from(&disk);
                self.cache.insert(id, record.clone()).await;
                // Re-fetch to get the Arc-wrapped version from moka.
                self.cache.get(id).await
            }
            None => None,
        }
    }

    async fn insert(&self, record: &CachedRecord) {
        self.cache.insert(record.id, record.clone()).await;
    }

    async fn remove(&self, id: MemoryId) {
        self.cache.invalidate(id).await;
    }

    fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }

    fn hit_rate(&self) -> f64 {
        // moka does not expose hit rate directly; return 0.0.
        0.0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RelationshipGraphAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Adapts `SharedGraph` to the API `RelationshipGraph` trait.
pub struct RelationshipGraphAdapter {
    graph: SharedGraph,
    vector_index: Arc<tokio::sync::RwLock<VectorIndexOwner>>,
    entity_index: Arc<tokio::sync::RwLock<EntityIndex>>,
    storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    config: Arc<RecalldConfig>,
}

impl RelationshipGraphAdapter {
    /// Creates a new adapter wrapping the shared relationship graph and
    /// subsystems needed for post-creation linking.
    pub fn new(
        graph: SharedGraph,
        vector_index: Arc<tokio::sync::RwLock<VectorIndexOwner>>,
        entity_index: Arc<tokio::sync::RwLock<EntityIndex>>,
        storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
        cache: Arc<CacheManager>,
        config: Arc<RecalldConfig>,
    ) -> Self {
        Self {
            graph,
            vector_index,
            entity_index,
            storage,
            cache,
            config,
        }
    }
}

#[async_trait]
impl state::RelationshipGraph for RelationshipGraphAdapter {
    async fn check_link_target(
        &self,
        field: &'static str,
        new_id: Option<MemoryId>,
        target: MemoryId,
        expected_ns: NamespaceId,
    ) -> Result<(), crate::graph::LinkError> {
        crate::graph::check_link_target(
            field,
            new_id,
            target,
            expected_ns,
            &self.graph,
            &self.storage,
            &self.cache,
        )
        .await
    }

    async fn apply_supersedes_edge(
        &self,
        new_id: MemoryId,
        target: MemoryId,
    ) -> crate::model::SupersedesOutcome {
        crate::graph::apply_supersedes_edge(new_id, target, &self.graph, &self.storage, &self.cache)
            .await
    }

    async fn apply_parent_edge(&self, parent: MemoryId, child: MemoryId) {
        crate::graph::apply_parent_edge(parent, child, &self.graph, &self.storage, &self.cache)
            .await;
    }

    async fn tombstone_node(&self, id: MemoryId) -> Result<(), crate::graph::GraphError> {
        self.tombstone_node_with_work(id, None).await
    }

    async fn tombstone_node_with_work(
        &self,
        id: MemoryId,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<(), crate::graph::GraphError> {
        let work = crate::storage::work::accept_work(&self.storage, parent)
            .map_err(|e| crate::graph::GraphError::Unavailable(e.to_string()))?;
        let result = async {
            let mut graph = self.graph.write().await;
            graph.update_node_state(id, DecayPhase::Tombstone, 0.0)?;
            Ok(())
        }
        .await;
        crate::storage::work::finish_work(&self.storage, work);
        result
    }

    async fn add_node(
        &self,
        id: MemoryId,
        namespace_id: NamespaceId,
        phase: DecayPhase,
        strength: f32,
        vector_slot: u32,
    ) -> Result<(), crate::graph::GraphError> {
        let work = crate::storage::work::accept_work(&self.storage, None)
            .map_err(|e| crate::graph::GraphError::Unavailable(e.to_string()))?;
        let result = async {
            let mut graph = self.graph.write().await;
            let _ = graph.add_node(id, namespace_id, phase, strength, vector_slot);
            Ok(())
        }
        .await;
        crate::storage::work::finish_work(&self.storage, work);
        result
    }

    async fn perform_post_creation_links(
        &self,
        memory_id: MemoryId,
        namespace_id: NamespaceId,
        embedding: &[f32],
        tags: &[String],
        entities: &[String],
        created_at: i64,
    ) {
        if let Err(error) = self
            .post_creation_links_with_work(
                memory_id,
                namespace_id,
                embedding,
                tags,
                entities,
                created_at,
                None,
            )
            .await
        {
            tracing::warn!(%error, "post-creation graph admission rejected");
        }
    }

    async fn post_creation_links_with_work(
        &self,
        memory_id: MemoryId,
        namespace_id: NamespaceId,
        embedding: &[f32],
        tags: &[String],
        entities: &[String],
        created_at: i64,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<(), crate::graph::GraphError> {
        if !self.config.graph.autolink_enabled {
            return Ok(());
        }

        let work = crate::storage::work::accept_work(&self.storage, parent)
            .map_err(|e| crate::graph::GraphError::Unavailable(e.to_string()))?;
        let threshold = self.config.graph.auto_link_threshold as f32;
        let max_links = self.config.graph.max_auto_links;

        // Autolink: discover and create edges to similar existing memories.
        if let Err(e) = crate::graph::perform_autolink_with_work(
            memory_id,
            namespace_id,
            embedding,
            tags,
            threshold,
            max_links,
            &self.vector_index,
            &self.graph,
            &self.storage,
            &self.cache,
            Some(&work),
        )
        .await
        {
            tracing::warn!(
                memory_id = %memory_id,
                %e,
                "autolink failed (non-fatal)"
            );
        }

        // Entity linking: connect memories sharing named entities.
        if !entities.is_empty() {
            let max_entity_links = self.config.graph.max_entity_links;
            let _ = crate::graph::perform_entity_link_with_work(
                memory_id,
                namespace_id,
                entities,
                max_entity_links,
                &self.entity_index,
                &self.graph,
                &self.storage,
                &self.cache,
                Some(&work),
            )
            .await;
        }

        // Temporal linking: connect to recently-stored memories.
        let temporal_window_ms = self.config.graph.temporal_window_ms;
        let max_temporal_links = self.config.graph.max_temporal_links;
        if temporal_window_ms > 0 {
            let recent_memories: Vec<(MemoryId, i64)> = self
                .cache
                .iter()
                .filter_map(|(mid, record)| {
                    if mid == memory_id {
                        return None;
                    }
                    if record.namespace_id != namespace_id {
                        return None;
                    }
                    let delta = (created_at - record.created_at).unsigned_abs();
                    if delta <= temporal_window_ms {
                        Some((mid, record.created_at))
                    } else {
                        None
                    }
                })
                .collect();

            if !recent_memories.is_empty() {
                if let Err(e) = crate::graph::perform_temporal_link_with_work(
                    memory_id,
                    namespace_id,
                    created_at,
                    temporal_window_ms,
                    max_temporal_links,
                    &recent_memories,
                    &self.graph,
                    &self.storage,
                    &self.cache,
                    Some(&work),
                )
                .await
                {
                    tracing::warn!(
                        memory_id = %memory_id,
                        %e,
                        "temporal link failed (non-fatal)"
                    );
                }
            }
        }
        crate::storage::work::finish_work(&self.storage, work);
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FsrsEngineAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Adapts the decay subsystem to the API `FsrsEngine` trait.
pub struct FsrsEngineAdapter {
    native_policy: Option<crate::storage::native_lifecycle::NativePolicyInputs>,
    clock: Arc<dyn crate::time::Clock>,
    completion: Option<crate::derived_completion::DerivedCompletion>,
    storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    graph: crate::graph::SharedGraph,
    config: Arc<RecalldConfig>,
    sweep_alive: bool,
}

impl FsrsEngineAdapter {
    pub(crate) async fn trigger_sweep_with_origin(
        &self,
        as_of_millis: Option<i64>,
        original_contract_version: u32,
    ) -> Result<crate::decay::SweepResult, Box<dyn std::error::Error + Send + Sync>> {
        use crate::decay::sweep::{DecaySweepRunner, SweepConfig};
        use crate::graph::ActivationConfig;

        if !matches!(original_contract_version, 1 | 2) {
            return Err(Box::new(
                crate::storage::StorageError::InvalidMutationContext(
                    "unsupported original application contract".into(),
                ),
            ));
        }
        let native = self
            .storage
            .read()
            .map_err(|e| e.to_string())?
            .meta_store()
            .uses_native_lifecycle();
        let legacy = crate::storage::native_lifecycle::NativePolicyInputs {
            global: crate::decay::DecayConfig::default(),
            multiplier: self.config.decay.decay_rate_multiplier as f64,
        };
        let policy = if native {
            self.native_policy.as_ref().ok_or_else(|| {
                crate::storage::StorageError::InvalidMutationContext(
                    "native manual sweep requires configured lifecycle policy".into(),
                )
            })?
        } else {
            &legacy
        };
        let sweep_config = SweepConfig::default();
        let activation_config = ActivationConfig::default();

        let result = DecaySweepRunner::execute_sweep_with_origin_work(
            &sweep_config,
            &policy.global,
            &activation_config,
            &self.storage,
            &self.graph,
            &self.cache,
            policy.multiplier,
            Some(as_of_millis.unwrap_or_else(|| self.clock.now_millis())),
            None,
            self.completion.as_ref(),
            original_contract_version,
        )
        .await;

        // Do not let a subsequent unavailable-handle error hide the original
        // canonical receipt or uncertain operation identity.
        if let Some((receipt, message)) = &result.failed_commit {
            return Err(Box::new(crate::storage::StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(crate::storage::StorageError::RecoveryRequired(
                    message.clone(),
                )),
            }));
        }
        if let Some(operation_id) = result.uncertain_operation {
            return Err(Box::new(crate::storage::StorageError::RecoveryRequired(
                format!(
                    "commitState=unknown operationId={operation_id}; reopen and reconcile before retry"
                ),
            )));
        }
        if let Some(error) = &result.publication_error {
            return Err(error.clone().into());
        }
        Ok(result)
    }

    pub(crate) fn with_native_lifecycle_policy(
        mut self,
        policy: crate::storage::native_lifecycle::NativePolicyInputs,
    ) -> Self {
        self.native_policy = Some(policy);
        self
    }

    /// Creates a new adapter with the storage engine, cache, and sweep-thread liveness flag.
    pub fn new(
        storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
        cache: Arc<CacheManager>,
        graph: crate::graph::SharedGraph,
        config: Arc<RecalldConfig>,
        has_sweep: bool,
    ) -> Self {
        cache.bind_storage(&storage);
        Self {
            native_policy: None,
            clock: Arc::new(crate::time::SystemClock),
            storage,
            cache,
            graph,
            config,
            sweep_alive: has_sweep,
            completion: None,
        }
    }
    /// Use the system instance's clock (or an isolated manual clock).
    pub fn with_clock(mut self, clock: Arc<dyn crate::time::Clock>) -> Self {
        self.clock = clock;
        self
    }
    /// Attach the live derived views so manual sweeps complete committed effects.
    pub fn with_indexes(
        mut self,
        vectors: Arc<tokio::sync::RwLock<VectorIndexOwner>>,
        fts: Arc<tokio::sync::Mutex<FtsIndex>>,
        entities: Arc<tokio::sync::RwLock<EntityIndex>>,
    ) -> Self {
        self.completion = Some(crate::derived_completion::DerivedCompletion {
            storage: self.storage.clone(),
            cache: self.cache.clone(),
            graph: self.graph.clone(),
            vectors,
            fts,
            entities,
        });
        self
    }
}

#[async_trait]
impl state::FsrsEngine for FsrsEngineAdapter {
    async fn feedback_api1(
        &self,
        id: MemoryId,
        quality: u8,
        namespace: Option<&str>,
    ) -> crate::serialization::lifecycle_api1::Reply {
        let Some(owner) = self.completion.as_ref() else {
            // Existing Rust adapters without attached derived views retain the
            // legacy entry. Native mode requires the actual canonical owner.
            if self
                .storage
                .read()
                .map(|s| s.meta_store().uses_native_lifecycle())
                .unwrap_or(true)
            {
                return crate::serialization::lifecycle_api1::Reply::failure(
                    crate::serialization::native_lifecycle::Reply::unavailable(
                        "lifecycle completion unavailable",
                    ),
                );
            }
            return crate::serialization::lifecycle_api1::Reply::legacy_feedback(
                self.reinforce(id, quality, namespace).await,
                id,
            );
        };
        let legacy = crate::storage::native_lifecycle::NativePolicyInputs {
            global: Default::default(),
            multiplier: self.config.decay.decay_rate_multiplier,
        };
        crate::serialization::lifecycle_api1::execute(
            owner,
            crate::serialization::lifecycle_api1::request(id, quality, namespace),
            crate::serialization::native_lifecycle::Operation::Feedback,
            self.native_policy.as_ref(),
            legacy,
            self.clock.clone(),
        )
        .await
    }

    async fn feedback_native(
        &self,
        request: crate::serialization::native_lifecycle::Request,
    ) -> crate::serialization::native_lifecycle::Reply {
        let Some(policy) = self.native_policy.as_ref() else {
            return crate::serialization::native_lifecycle::Reply::unavailable(
                "native lifecycle policy not configured",
            );
        };
        let owner = match &self.completion {
            Some(owner) => owner.clone(),
            None => {
                return crate::serialization::native_lifecycle::Reply::unavailable(
                    "native completion unavailable",
                );
            }
        };
        crate::mcp::native_lifecycle_adapter::execute(
            &owner,
            request,
            crate::serialization::native_lifecycle::Operation::Feedback,
            policy,
            self.clock.clone(),
        )
        .await
    }

    async fn record_access_batch(
        &self,
        accesses: &[(MemoryId, AccessKind)],
    ) -> Result<
        Vec<crate::storage::control::WriteReceipt>,
        crate::decay::observation::ObservationError,
    > {
        crate::decay::observation::record_exposures(
            self.storage.clone(),
            self.cache.clone(),
            accesses.to_vec(),
            self.clock.now_millis(),
        )
        .await
        .map_err(Into::into)
    }

    async fn reinforce(
        &self,
        id: MemoryId,
        quality: u8,
        namespace: Option<&str>,
    ) -> Result<state::ReinforceResult, Box<dyn std::error::Error + Send + Sync>> {
        self.reinforce_with_work(id, quality, namespace, None).await
    }

    async fn reinforce_with_work(
        &self,
        id: MemoryId,
        quality: u8,
        namespace: Option<&str>,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<state::ReinforceResult, Box<dyn std::error::Error + Send + Sync>> {
        let outcome = crate::decay::storage_adapter::reinforce_with_completion(
            self.storage.clone(),
            self.cache.clone(),
            id,
            quality,
            self.config.decay.decay_rate_multiplier,
            self.clock.now_millis(),
            namespace.map(str::to_owned),
            parent,
            self.completion.as_ref(),
        )
        .await;
        let plan = outcome?;
        Ok(state::ReinforceResult {
            strength: plan.strength,
            stability: plan.stability,
            phase: plan.phase,
            is_permastore: plan.is_permastore,
        })
    }

    fn sweep_thread_alive(&self) -> bool {
        self.sweep_alive
    }

    fn last_sweep_time(&self) -> Option<std::time::Instant> {
        None
    }

    async fn trigger_sweep(
        &self,
        as_of_millis: Option<i64>,
    ) -> Result<crate::decay::SweepResult, Box<dyn std::error::Error + Send + Sync>> {
        self.trigger_sweep_with_origin(as_of_millis, 1).await
    }
}

// ═══════════════════════════════════════════════════════════════════════
// NamespaceRegistryAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Adapts `RedbStorageEngine` to the API `NamespaceRegistry` trait.
pub struct NamespaceRegistryAdapter {
    completion: Option<crate::derived_completion::DerivedCompletion>,
    storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
    clock: Arc<dyn crate::time::Clock>,
}

impl NamespaceRegistryAdapter {
    /// Storage-only reads. Creation requires `from_system` to bind actual publishers.
    pub fn new(storage: Arc<std::sync::RwLock<RedbStorageEngine>>) -> Self {
        Self {
            completion: None,
            storage,
            clock: Arc::new(crate::time::SystemClock),
        }
    }
    /// Bind creates to the same system's accepted work and derived publishers.
    pub fn from_system(system: &crate::Recalld) -> Self {
        Self::new(system.storage().clone())
            .with_completion(crate::derived_completion::DerivedCompletion {
                storage: system.storage().clone(),
                cache: system.cache().clone(),
                graph: system.graph().clone(),
                vectors: system.vector_index().clone(),
                fts: system.fts_index().clone(),
                entities: system.entity_index().clone(),
            })
            .with_clock(system.clock().clone())
    }
    pub(crate) fn with_completion(
        mut self,
        completion: crate::derived_completion::DerivedCompletion,
    ) -> Self {
        assert!(
            Arc::ptr_eq(&self.storage, &completion.storage),
            "namespace completion belongs to another owner"
        );
        self.completion = Some(completion);
        self
    }
    pub fn with_clock(mut self, clock: Arc<dyn crate::time::Clock>) -> Self {
        self.clock = clock;
        self
    }
}

#[async_trait]
impl state::NamespaceRegistry for NamespaceRegistryAdapter {
    fn resolve(&self, name: &str) -> Option<NamespaceConfig> {
        tokio::task::block_in_place(|| {
            let storage_r = self.storage.read().ok()?;
            storage_r.get_namespace_by_name(name).ok().flatten()
        })
    }

    fn get_by_id(&self, id: u32) -> Option<NamespaceConfig> {
        tokio::task::block_in_place(|| {
            let storage_r = self.storage.read().ok()?;
            storage_r.get_namespace(NamespaceId::new(id)).ok().flatten()
        })
    }

    fn name_for(&self, id: NamespaceId) -> Option<String> {
        tokio::task::block_in_place(|| {
            let storage_r = self.storage.read().ok()?;
            storage_r
                .get_namespace(id)
                .ok()
                .flatten()
                .map(|ns| ns.name.clone())
        })
    }

    async fn list_all(&self) -> Vec<state::NamespaceListInfo> {
        let storage = self.storage.clone();

        tokio::task::spawn_blocking(move || {
            let storage_r = match storage.read() {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };

            let namespaces = match storage_r.list_namespaces() {
                Ok(ns) => ns,
                Err(_) => return Vec::new(),
            };

            // Live memories only. Counting tombstones here made every
            // namespace listing disagree with what a caller could actually
            // fetch from it.
            let mut counts = std::collections::HashMap::<u32, u64>::new();
            if let Ok(all_records) = storage_r.scan_all() {
                for (_id, record) in &all_records {
                    if record.phase == DecayPhase::Tombstone {
                        continue;
                    }
                    *counts.entry(record.namespace_id).or_default() += 1;
                }
            }

            namespaces
                .into_iter()
                .map(|ns| {
                    let memory_count = counts.get(&ns.id.get()).copied().unwrap_or(0);
                    state::NamespaceListInfo {
                        id: ns.id.get(),
                        name: ns.name.clone(),
                        embedding_dim: ns.embedding_dim,
                        memory_count,
                        created_at: ns.created_at,
                    }
                })
                .collect()
        })
        .await
        .unwrap_or_default()
    }

    async fn create(
        &self,
        name: &str,
        embedding_dim: Option<u32>,
        initial_stability: f32,
        desired_retention: f32,
        decay_rate_multiplier: Option<f32>,
    ) -> Result<NamespaceConfig, Box<dyn std::error::Error + Send + Sync>> {
        let completion = self.completion.as_ref().ok_or_else(|| {
            crate::storage::StorageError::InvalidMutation("namespace creation requires an owner-bound registry; use NamespaceRegistryAdapter::from_system".into())
        })?;
        let config = NamespaceConfig {
            schema: None,
            id: NamespaceId::UNSET,
            name: name.into(),
            embedding_dim: 1536,
            initial_stability,
            default_difficulty: 5.0,
            phase_thresholds: crate::model::namespace::PhaseThresholds::default(),
            permastore_threshold: 1500.0,
            created_at: 0,
            desired_retention,
            decay_rate_multiplier,
        };
        // Preserve the existing concrete LifecycleError for standalone callers,
        // before accepting work or sampling the create clock.
        crate::decay::lifecycle::resolve_policy(
            &config,
            &crate::decay::DecayConfig::default(),
            1.0,
        )?;
        Ok(completion
            .create_legacy_namespace(config, embedding_dim, self.clock.clone())
            .await?)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// NoopMetricsCollector
// ═══════════════════════════════════════════════════════════════════════

/// No-op metrics collector until a real metrics module is implemented.
pub struct NoopMetricsCollector;

#[async_trait]
impl state::MetricsCollector for NoopMetricsCollector {
    async fn render_prometheus(&self) -> String {
        String::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::state::StorageEngine as _;
    use crate::search::VectorMetadata;
    use crate::test_support::{DIM, Fixture, MemorySpec};

    #[tokio::test]
    async fn insertion_http_boundary_distinguishes_provider_unavailable_from_caller_invalid() {
        use axum::response::IntoResponse;
        let fx = Fixture::new();
        for supplied in [false, true] {
            let input = crate::storage::insertion::InsertMemory {
                id: None,
                summary: "vector boundary".into(),
                full_text: None,
                tags: vec![],
                initial_stability: None,
                created_at: None,
                embedding: supplied.then(|| vec![0.; DIM]),
                parent_id: None,
                supersedes: None,
            };
            let before = fx
                .storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .committed_generation;
            let error = match adapter(&fx)
                .insert_memory(NamespaceId::new(1), input, vec![0.; DIM].into())
                .await
            {
                Ok(_) => panic!("zero vector accepted"),
                Err(error) => error,
            };
            let response = super::super::errors::AppError::from(error).into_response();
            assert_eq!(
                response.status(),
                if supplied {
                    axum::http::StatusCode::UNPROCESSABLE_ENTITY
                } else {
                    axum::http::StatusCode::SERVICE_UNAVAILABLE
                }
            );
            assert_eq!(
                fx.storage
                    .read()
                    .unwrap()
                    .control_state()
                    .unwrap()
                    .committed_generation,
                before
            );
            assert!(fx.storage.read().unwrap().scan_all().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn actual_http_insertion_adapter_reports_durable_receipt_if_publication_fails() {
        use axum::response::IntoResponse;
        let fx = Fixture::new();
        *fx.vector_index.write().await = VectorIndexOwner::uniform(DIM + 1);
        let error = match adapter(&fx)
            .insert_memory(
                NamespaceId::new(1),
                crate::storage::insertion::InsertMemory {
                    id: None,
                    summary: "http committed insertion".into(),
                    full_text: None,
                    tags: vec![],
                    initial_stability: None,
                    created_at: None,
                    embedding: Some(unit_vector()),
                    parent_id: None,
                    supersedes: None,
                },
                vec![].into(),
            )
            .await
        {
            Ok(_) => panic!("publication failure reported success"),
            Err(error) => error,
        };
        let response = super::super::errors::AppError::from(error).into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["commitState"], "committed");
        assert_eq!(body["ready"], false);
        let receipt: crate::storage::control::WriteReceipt =
            serde_json::from_value(body["receipt"].clone()).unwrap();
        let id = crate::storage::insertion::inserted_id(&receipt).unwrap();
        assert!(fx.record(id).is_some());
        assert!(
            fx.storage
                .read()
                .unwrap()
                .meta_store()
                .effects_stamp()
                .is_err()
        );
        *fx.vector_index.write().await = VectorIndexOwner::uniform(DIM);
        adapter(&fx).complete_insertion(&receipt).await.unwrap();
        assert!(fx.vector_index.read().await.get_vector(id).is_some());
        assert!(
            fx.storage
                .read()
                .unwrap()
                .meta_store()
                .effects_stamp()
                .is_ok()
        );
    }

    /// A vector of the fixture's width, so it matches the index.
    fn unit_vector() -> Vec<f32> {
        let mut v = vec![0.0; DIM];
        v[0] = 1.0;
        v
    }

    fn adapter(fx: &Fixture) -> StorageEngineAdapter {
        StorageEngineAdapter::new(
            fx.storage.clone(),
            fx.cache.clone(),
            fx.vector_index.clone(),
            fx.fts_index.clone(),
            fx.entity_index.clone(),
            fx.graph.clone(),
        )
    }

    /// Put a memory into every index, which is the state the running
    /// system leaves after a successful store.
    async fn indexed_memory(fx: &Fixture) -> MemoryId {
        let ns = NamespaceId::new(1);
        let id = fx
            .insert_memory_with(MemorySpec {
                tags: &["entity/sarah"],
                ..MemorySpec::new(ns)
            })
            .await;

        fx.fts_index
            .lock()
            .await
            .add(ns, id, "a fact about Sarah", None, &[])
            .expect("fts add");
        fx.vector_index
            .write()
            .await
            .add(
                id,
                &unit_vector(),
                VectorMetadata {
                    namespace_id: ns,
                    decay_phase: 1,
                    tags: Vec::new(),
                },
            )
            .expect("vector add");
        fx.entity_index
            .write()
            .await
            .add(ns, id, &["Sarah".to_string()]);

        id
    }

    /// THE HTTP-delete regression. This path used to stop after
    /// `cache.invalidate`, leaving the memory in all four indexes: a
    /// vector entry burning a top-k slot in every search and reading as
    /// a live autolink target, an FTS row that survives restart forever,
    /// an entity entry, and a graph node still reporting Full phase at
    /// strength 1.0 and so still propping up its neighbours' decay
    /// resistance.
    #[tokio::test]
    async fn w35_http_delete_clears_every_index_and_marks_the_graph_node() {
        let fx = Fixture::new();
        let id = indexed_memory(&fx).await;

        assert!(adapter(&fx).delete_memory(id, None).await.expect("delete"));

        assert!(
            fx.fts_index.lock().await.is_empty().expect("fts"),
            "FTS row survived an HTTP delete"
        );
        assert_eq!(
            fx.vector_index.read().await.len(),
            0,
            "vector index entry survived an HTTP delete"
        );
        assert_eq!(
            fx.entity_index.read().await.len(),
            0,
            "entity index entry survived an HTTP delete"
        );
        assert_eq!(
            fx.graph
                .read()
                .await
                .get_node(&id)
                .expect("the node is kept, only re-phased")
                .decay_phase,
            DecayPhase::Tombstone
        );
    }

    /// A second delete is a no-op, not a second round of index churn.
    #[tokio::test]
    async fn w36_http_delete_of_an_already_tombstoned_memory_reports_false() {
        let fx = Fixture::new();
        let id = indexed_memory(&fx).await;
        let adapter = adapter(&fx);

        assert!(adapter.delete_memory(id, None).await.expect("first delete"));

        // Re-index it, so a second pass through the cleanup would be
        // visible rather than idempotent by accident.
        fx.fts_index
            .lock()
            .await
            .add(NamespaceId::new(1), id, "re-indexed", None, &[])
            .expect("fts add");

        assert!(
            !adapter
                .delete_memory(id, None)
                .await
                .expect("second delete"),
            "an already-tombstoned memory must report false"
        );
        assert!(
            !fx.fts_index.lock().await.is_empty().expect("fts"),
            "the second delete ran the index cleanup anyway"
        );
    }
    #[tokio::test]
    async fn creation_adapters_reject_unusable_lifecycle_policy_before_persistence() {
        use crate::api::state::NamespaceRegistry as _;
        use crate::mcp::bridge::{BridgeError, CreateNamespaceInput, NamespaceRegistry as _};
        let f = Fixture::new();
        let http = NamespaceRegistryAdapter::new(f.storage.clone()).with_completion(f.completion());
        let mcp = f.namespace_adapter();
        for (stability, multiplier) in [
            (0.0, None),
            (-1.0, None),
            (f32::NAN, None),
            (f32::INFINITY, None),
            (1.0, Some(-1.0)),
            (1.0, Some(f32::NAN)),
            (1.0, Some(f32::INFINITY)),
        ] {
            let error = http
                .create("invalid-http", Some(DIM as u32), stability, 0.9, multiplier)
                .await
                .unwrap_err();
            assert!(matches!(
                error.downcast_ref::<crate::decay::lifecycle::LifecycleError>(),
                Some(crate::decay::lifecycle::LifecycleError::InvalidInput(_))
            ));
            let error = mcp
                .create_namespace(CreateNamespaceInput {
                    name: "invalid-mcp".into(),
                    embedding_dim: Some(DIM as u16),
                    initial_stability: Some(stability),
                    desired_retention: None,
                    decay_rate_multiplier: multiplier,
                })
                .await
                .unwrap_err();
            assert!(matches!(error, BridgeError::InvalidInput(_)));
        }
        assert!(
            f.storage
                .read()
                .unwrap()
                .get_namespace_by_name("invalid-http")
                .unwrap()
                .is_none()
        );
        assert!(
            f.storage
                .read()
                .unwrap()
                .get_namespace_by_name("invalid-mcp")
                .unwrap()
                .is_none()
        );
        // Zero decay is valid, zero stability is not; inherited settings stay valid.
        for multiplier in [None, Some(0.0)] {
            let name = if multiplier.is_none() {
                "inherited"
            } else {
                "disabled"
            };
            let ns = http
                .create(name, Some(DIM as u32), 1.0, 0.9, multiplier)
                .await
                .unwrap();
            crate::decay::lifecycle::resolve_policy(
                &ns,
                &crate::decay::DecayConfig::default(),
                1.0,
            )
            .unwrap();
        }
    }
}

impl SearchPipelineAdapter {
    fn pipeline_query(
        query: state::SearchQuery,
    ) -> Result<crate::search::SearchQuery, crate::search::SearchError> {
        // Convert API-layer SearchQuery to the pipeline's SearchQuery.
        let (text, query_embedding) = match query.query {
            state::QueryInput::Text(t) => (Some(t), None),
            state::QueryInput::Vector(vector) => (None, Some(vector)),
            state::QueryInput::TextAndVector { text, embedding } => (Some(text), Some(embedding)),
        };

        // Raw tags remain strict. The pipeline validates structured filters
        // against the resolved namespace before provider/channel work.
        let require_tags: Vec<Tag> =
            crate::model::parse_filter_tags("includeTags", "", &query.include_tags)
                .map_err(|e| crate::search::SearchError::InvalidFilter(e.to_string()))?;
        // `excludeTags` is the safe direction — dropping one can only
        // ever return MORE than asked for in the same sense a missing
        // exclusion does, and it never masquerades as a match — so it
        // keeps the lossy parse.
        let exclude_tags: Vec<Tag> =
            crate::model::parse_tags_lossy("search.excludeTags", &query.exclude_tags);
        let phases: Vec<DecayPhase> = query
            .decay_phases
            .as_ref()
            .map(|ps| {
                ps.iter()
                    .filter_map(|&p| match p {
                        0 => Some(DecayPhase::Full),
                        1 => Some(DecayPhase::Summary),
                        2 => Some(DecayPhase::Ghost),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(crate::search::SearchQuery {
            record_access: query.record_access,
            query_embedding,
            text,
            fts_query: None,
            namespace: query.namespace_name.clone(), // Issue 1: pass actual namespace name
            filter: crate::search::PipelineSearchFilter {
                labels: crate::model::tag::StructuredLabelFilter {
                    entities: query.entities.clone(),
                    ..Default::default()
                },
                require_tags,
                exclude_tags,
                phases,
                min_strength: query.min_score, // Issue 2: pass min_strength
                ..Default::default()
            },
            limit: query.k,
            min_score: 0.0,
            include_ghosts: false,
            query_mode: crate::search::QueryMode::default(),
            graph_depth: (query.graph_depth as u8).min(3),
            time_range_start: query.time_range_start, // Issue 3: pass time range
            time_range_end: query.time_range_end,     // Issue 3: pass time range
            entities: query.entities.clone(),         // Issue 4: pass entities
        })
    }

    async fn search_inner(
        &self,
        query: state::SearchQuery,
        defer_recording: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<state::ResolvedSearchResult>, crate::search::SearchError> {
        let pipeline_query = Self::pipeline_query(query.clone())?;
        let response = if defer_recording {
            self.query_engine
                .prepare_search_with_work(pipeline_query, parent)
                .await?
        } else {
            self.query_engine
                .search_with_work(pipeline_query, parent)
                .await?
        };
        Ok(response
            .results
            .into_iter()
            .map(|r| {
                // Preserve canonical metadata; only legacy records parse retained tags.
                let metadata = structured_metadata(&r.tags, r.labels.as_ref());
                state::ResolvedSearchResult {
                    access_kind: if r.activation_score.is_some() {
                        AccessKind::AssociativeRetrieval
                    } else {
                        AccessKind::DirectRetrieval
                    },
                    memory: CachedRecord {
                        labels: r.labels,
                        lifecycle: None,
                        id: r.memory_id,
                        namespace_id: query.namespace_id, // Issue 6: use actual namespace_id
                        created_at: r.created_at,
                        last_accessed_at: r.last_accessed_at,
                        phase: r.phase,
                        strength: r.retrievability,
                        decay_strength: r.effective_r,
                        stability: r.stability,
                        difficulty: 5.0,
                        is_permastore: r.is_permastore,
                        summary: r.summary.unwrap_or_default(),
                        tags: r.tags,
                        edge_count: r.edge_count,
                        vector_slot: 0,
                        entities: metadata.entities,
                    },
                    score: r.score.unwrap_or(0.0),
                }
            })
            .collect())
    }
}
