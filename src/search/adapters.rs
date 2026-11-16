//! Adapter implementations connecting QueryEngine traits to real subsystems.
//!
//! Each adapter struct wraps a concrete subsystem type and implements
//! the corresponding DI trait from `pipeline.rs`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;

use crate::cache::CacheManager;
use crate::embedding::EmbeddingProvider;
use crate::graph::SharedGraph;
use crate::model::{AccessKind, CachedRecord, MemoryId, NamespaceConfig, NamespaceId};
use crate::rif::{NeighborInfo, RifEngine};
// Import the real StorageEngine trait so its methods are in scope
// for RedbStorageEngine via the RwLockReadGuard deref.
use crate::storage::RedbStorageEngine;
use crate::storage::StorageEngine as _;

use super::error::{Result, SearchError};
use super::pipeline::{
    AccessRecorder, EmbeddingProviderRegistry, EntityIndexReader, EntityRecallResult,
    FtsIndexRegistry, FtsResult, GraphReader, MetadataStore, NamespaceResolver, RecordCache,
    RifProcessor, RifSuppression, ScoredResult, VectorIndexRegistry,
};
use super::read_handle::{FtsHandle, ReadHandle};
use super::{EntityIndex, FtsIndex, SearchFilter, VectorIndexOwner};

// Explicit offline harness adapter. Ordinary storage open still rejects schema
// v2. Snapshot ownership pins canonical files; this adapter has no write API.
#[async_trait]
impl NamespaceResolver for crate::storage::offline_snapshot::OfflineSnapshot {
    async fn resolve_id(&self, id: NamespaceId) -> Result<NamespaceConfig> {
        self.namespaces()
            .iter()
            .find(|ns| ns.id == id)
            .cloned()
            .ok_or_else(|| SearchError::NamespaceNotFound(format!("id {}", id.get())))
    }
    async fn resolve(&self, name: &str) -> Result<NamespaceConfig> {
        self.namespaces()
            .iter()
            .find(|ns| ns.name == name)
            .cloned()
            .ok_or_else(|| SearchError::NamespaceNotFound(name.into()))
    }
}

#[async_trait]
impl MetadataStore for crate::storage::offline_snapshot::OfflineSnapshot {
    async fn get(&self, id: &MemoryId) -> Result<Option<CachedRecord>> {
        self.record(*id)
            .map(|item| item.map(|item| CachedRecord::from(&item.record)))
            .map_err(|e| SearchError::MetadataError(e.to_string()))
    }
    async fn get_batch(&self, ids: &[MemoryId]) -> Result<Vec<CachedRecord>> {
        let mut records = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(record) = self.get(id).await? {
                records.push(record);
            }
        }
        Ok(records)
    }
    async fn list_namespace_memories(&self, namespace: NamespaceId) -> Result<Vec<MemoryId>> {
        // ponytail: complete offline scan; a serving adapter must use A1's
        // certified namespace view. No claim of online enumeration performance.
        let mut ids = Vec::new();
        self.for_each_record(|item| {
            if item.record.namespace_id == namespace.get() {
                ids.push(MemoryId::from_bytes(item.record.id));
            }
            Ok(())
        })
        .map_err(|e| SearchError::MetadataError(e.to_string()))?;
        Ok(ids)
    }
}

// ── NamespaceResolver ──────────────────────────────────────────────

/// Adapts `RedbStorageEngine` to the `NamespaceResolver` trait.
///
/// Delegates `resolve(name)` to `StorageEngine::get_namespace_by_name`.
pub struct StorageNamespaceResolver {
    storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
}

impl StorageNamespaceResolver {
    pub fn new(storage: Arc<std::sync::RwLock<RedbStorageEngine>>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl NamespaceResolver for StorageNamespaceResolver {
    async fn resolve_id(&self, id: NamespaceId) -> Result<NamespaceConfig> {
        let storage = self.storage.clone();
        tokio::task::spawn_blocking(move || {
            storage
                .read()
                .map_err(|e| SearchError::Internal(format!("storage lock poisoned: {e}")))?
                .get_namespace(id)
                .map_err(|e| SearchError::Internal(e.to_string()))?
                .ok_or_else(|| SearchError::NamespaceNotFound(format!("id {}", id.get())))
        })
        .await
        .map_err(|e| SearchError::Internal(format!("blocking task join error: {e}")))?
    }

    async fn resolve(&self, name: &str) -> Result<NamespaceConfig> {
        let storage = self.storage.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            let storage_r = storage
                .read()
                .map_err(|e| SearchError::Internal(format!("storage lock poisoned: {e}")))?;
            storage_r
                .get_namespace_by_name(&name)
                .map_err(|e| SearchError::Internal(e.to_string()))?
                .ok_or_else(|| SearchError::NamespaceNotFound(name))
        })
        .await
        .map_err(|e| SearchError::Internal(format!("blocking task join error: {e}")))?
    }
}

// ── EmbeddingProviderRegistry ──────────────────────────────────────

/// Adapts a single `EmbeddingProvider` to `EmbeddingProviderRegistry`.
///
/// In the current design all namespaces share one provider. If
/// per-namespace providers are needed later, this adapter can hold
/// a `HashMap<String, Arc<dyn EmbeddingProvider>>`.
pub struct SingleEmbeddingRegistry {
    provider: Arc<dyn EmbeddingProvider>,
}

impl SingleEmbeddingRegistry {
    pub fn new(provider: Arc<dyn EmbeddingProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl EmbeddingProviderRegistry for SingleEmbeddingRegistry {
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        self.provider.profile()
    }
    fn dimensions(&self) -> Option<usize> {
        Some(self.provider.dimensions())
    }
    async fn embed(&self, _namespace: &str, text: &str) -> Result<Vec<f32>> {
        self.provider
            .embed(text)
            .await
            .map_err(|e| SearchError::EmbeddingFailed(e.to_string()))
    }

    async fn embed_query(&self, _namespace: &str, text: &str) -> Result<Vec<f32>> {
        self.provider
            .embed_query(text)
            .await
            .map_err(|e| SearchError::EmbeddingFailed(e.to_string()))
    }
}

// ── VectorIndexRegistry ────────────────────────────────────────────

/// Adapts a shared `VectorIndexOwner` to `VectorIndexRegistry`.
///
/// All namespaces share one index; the namespace_id is passed through
/// the search filter to the underlying `VectorIndex::search`.
pub struct SharedVectorIndexRegistry {
    index: ReadHandle<VectorIndexOwner>,
}

impl SharedVectorIndexRegistry {
    pub fn new(index: Arc<tokio::sync::RwLock<VectorIndexOwner>>) -> Self {
        Self {
            index: ReadHandle::Live(index),
        }
    }
    pub(crate) fn pinned(value: Arc<tokio::sync::OwnedRwLockReadGuard<VectorIndexOwner>>) -> Self {
        Self {
            index: ReadHandle::Pinned(value),
        }
    }
}

impl VectorIndexRegistry for SharedVectorIndexRegistry {
    fn search(
        &self,
        namespace_id: NamespaceId,
        query_vec: &[f32],
        k: usize,
        eligible_ids: Option<&HashSet<MemoryId>>,
    ) -> Result<Vec<ScoredResult>> {
        let wait = crate::performance::Timer::new("vector.lock_wait");
        let index = self.index.read();
        wait.complete();
        let index = crate::performance::Hold::new("vector.lock_hold", index);
        let filter = SearchFilter {
            namespace_id: Some(namespace_id),
            ..Default::default()
        };
        let results = index
            .search_scoped(namespace_id, query_vec, k, &filter, eligible_ids)
            .map_err(|e| SearchError::VectorIndexError(e.to_string()))?;
        Ok(results
            .into_iter()
            .map(|r| ScoredResult {
                memory_id: r.id,
                score: r.score,
            })
            .collect())
    }

    fn get_vector(&self, id: MemoryId) -> Option<Vec<f32>> {
        let wait = crate::performance::Timer::new("vector.lock_wait");
        let index = self.index.read();
        wait.complete();
        let index = crate::performance::Hold::new("vector.lock_hold", index);
        index.get_vector(id)
    }
}

// ── FtsIndexRegistry ──────────────────────────────────────────────

/// Adapts a shared `FtsIndex` to `FtsIndexRegistry`.
///
/// Wraps a single `FtsIndex` behind a `tokio::sync::Mutex`.
/// Namespace filtering is handled inside `FtsIndex::search` via SQL.
pub struct SharedFtsIndexRegistry {
    index: FtsHandle,
}

impl SharedFtsIndexRegistry {
    pub fn new(index: Arc<tokio::sync::Mutex<FtsIndex>>) -> Self {
        Self {
            index: FtsHandle::Live(index),
        }
    }
    pub(crate) fn pinned(value: tokio::sync::OwnedMutexGuard<FtsIndex>) -> Self {
        Self {
            index: FtsHandle::Pinned(std::sync::Mutex::new(value)),
        }
    }
}

impl FtsIndexRegistry for SharedFtsIndexRegistry {
    fn search(&self, namespace_id: NamespaceId, query: &str, k: usize) -> Result<Vec<FtsResult>> {
        let wait = crate::performance::Timer::new("fts.lock_wait");
        let index = self.index.lock();
        wait.complete();
        let index = crate::performance::Hold::new("fts.lock_hold", index);
        index
            .search(namespace_id, query, k)
            .map(|results| {
                results
                    .into_iter()
                    .map(|(id, score)| FtsResult {
                        memory_id: id,
                        score,
                    })
                    .collect()
            })
            .map_err(|e| SearchError::Internal(format!("FTS search failed: {e}")))
    }

    fn search_eligible(
        &self,
        namespace_id: NamespaceId,
        query: &str,
        k: usize,
        eligible_ids: &HashSet<MemoryId>,
    ) -> Result<super::EligibleFtsResults> {
        let wait = crate::performance::Timer::new("fts.lock_wait");
        let index = self.index.lock();
        wait.complete();
        let index = crate::performance::Hold::new("fts.lock_hold", index);
        index
            .search_eligible(namespace_id, query, k, eligible_ids)
            .map_err(|e| SearchError::Internal(format!("eligible FTS search failed: {e}")))
    }
}

// ── RecordCache ────────────────────────────────────────────────────

/// Adapts `CacheManager` to `RecordCache`.
///
/// `CacheManager::get()` is async and returns `Option<Arc<CachedRecord>>`;
/// the `RecordCache` trait has a sync `get`. We use
/// `tokio::runtime::Handle::current().block_on()` to bridge the gap,
/// which is safe because the underlying moka cache operations are
/// non-blocking in practice.
pub struct CacheManagerAdapter {
    cache: Arc<CacheManager>,
}

impl CacheManagerAdapter {
    pub fn new(cache: Arc<CacheManager>) -> Self {
        Self { cache }
    }
}

impl RecordCache for CacheManagerAdapter {
    fn get(&self, id: &MemoryId) -> Option<CachedRecord> {
        // Fast path: sync check if the key exists at all.
        if !self.cache.contains(id) {
            return None;
        }
        // Bridge async get to sync context.
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.cache.get(*id))
        })
        .map(|arc| (*arc).clone())
    }
}

// ── MetadataStore ──────────────────────────────────────────────────

/// Adapts `RedbStorageEngine` to `MetadataStore`.
///
/// `StorageEngine::get_record` returns `DiskRecord`; the adapter
/// converts it to `CachedRecord` via `From<&DiskRecord>`.
pub struct StorageMetadataAdapter {
    storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
}

impl StorageMetadataAdapter {
    pub fn new(storage: Arc<std::sync::RwLock<RedbStorageEngine>>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl MetadataStore for StorageMetadataAdapter {
    fn accept_work(
        &self,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Option<crate::storage::work::WorkTicket>> {
        crate::storage::work::accept_work(&self.storage, parent)
            .map(Some)
            .map_err(|e| SearchError::Unavailable(e.to_string()))
    }
    fn finish_work(&self, work: crate::storage::work::WorkTicket) {
        crate::storage::work::finish_work(&self.storage, work);
    }

    async fn effects_stamp(&self) -> Result<u64> {
        let storage = self.storage.clone();
        let worker = crate::performance::Worker::new("storage.read.queue");
        tokio::task::spawn_blocking(move || {
            worker.run(|| {
                storage
                    .read()
                    .map_err(|e| SearchError::Unavailable(e.to_string()))?
                    .meta_store()
                    .effects_stamp()
                    .map_err(|e| SearchError::Unavailable(e.to_string()))
            })
        })
        .await
        .map_err(|e| SearchError::Internal(e.to_string()))?
    }

    async fn get(&self, id: &MemoryId) -> Result<Option<CachedRecord>> {
        let storage = self.storage.clone();
        let id = *id;
        let worker = crate::performance::Worker::new("storage.read.queue");
        tokio::task::spawn_blocking(move || {
            worker.run(|| {
                let wait = crate::performance::Timer::new("storage.read.wait");
                let storage_r = storage.read().map_err(|e| {
                    SearchError::MetadataError(format!("storage lock poisoned: {e}"))
                })?;
                wait.complete();
                let storage_r = crate::performance::Hold::new("storage.read.hold", storage_r);
                storage_r
                    .get_record(id)
                    .map(|opt| opt.map(|disk| CachedRecord::from(&disk)))
                    .map_err(|e| SearchError::MetadataError(e.to_string()))
            })
        })
        .await
        .map_err(|e| SearchError::Internal(format!("blocking task join error: {e}")))?
    }

    async fn get_batch(&self, ids: &[MemoryId]) -> Result<Vec<CachedRecord>> {
        let storage = self.storage.clone();
        let ids = ids.to_vec();
        let worker = crate::performance::Worker::new("storage.read.queue");
        tokio::task::spawn_blocking(move || {
            worker.run(|| {
                let wait = crate::performance::Timer::new("storage.read.wait");
                let storage_r = storage.read().map_err(|e| {
                    SearchError::MetadataError(format!("storage lock poisoned: {e}"))
                })?;
                wait.complete();
                let storage_r = crate::performance::Hold::new("storage.read.hold", storage_r);
                let mut results = Vec::with_capacity(ids.len());
                for id in &ids {
                    match storage_r.get_record(*id) {
                        Ok(Some(disk)) => results.push(CachedRecord::from(&disk)),
                        Ok(None) => {}
                        Err(e) => return Err(SearchError::MetadataError(e.to_string())),
                    }
                }
                Ok(results)
            })
        })
        .await
        .map_err(|e| SearchError::Internal(format!("blocking task join error: {e}")))?
    }

    async fn namespace_records(&self, namespace_id: NamespaceId) -> Result<Vec<CachedRecord>> {
        let storage = self.storage.clone();
        let worker = crate::performance::Worker::new("storage.read.queue");
        tokio::task::spawn_blocking(move || {
            worker.run(|| {
                let wait = crate::performance::Timer::new("storage.read.wait");
                let storage = storage.read().map_err(|e| {
                    SearchError::MetadataError(format!("storage lock poisoned: {e}"))
                })?;
                wait.complete();
                let storage = crate::performance::Hold::new("storage.read.hold", storage);
                storage
                    .meta_store()
                    .records_in_namespace(namespace_id)
                    .map(|records| records.iter().map(CachedRecord::from).collect())
                    .map_err(|e| SearchError::MetadataError(e.to_string()))
            })
        })
        .await
        .map_err(|e| SearchError::Internal(format!("blocking task join error: {e}")))?
    }

    async fn list_namespace_memories(&self, namespace_id: NamespaceId) -> Result<Vec<MemoryId>> {
        let storage = self.storage.clone();
        let worker = crate::performance::Worker::new("storage.read.queue");
        tokio::task::spawn_blocking(move || {
            worker.run(|| {
                let wait = crate::performance::Timer::new("storage.read.wait");
                let storage_r = storage.read().map_err(|e| {
                    SearchError::MetadataError(format!("storage lock poisoned: {e}"))
                })?;
                wait.complete();
                let storage_r = crate::performance::Hold::new("storage.read.hold", storage_r);
                storage_r
                    .meta_store()
                    .memories_in_namespace(namespace_id)
                    .map_err(|e| SearchError::MetadataError(e.to_string()))
            })
        })
        .await
        .map_err(|e| SearchError::Internal(format!("blocking task join error: {e}")))?
    }
}

// ── RifProcessor ───────────────────────────────────────────────────

/// Adapts `RifEngine` to `RifProcessor`.
///
/// The real `RifEngine::compute_effects` takes `NeighborInfo` structs;
/// the `RifProcessor` trait takes raw `MemoryId` slices. This adapter
/// constructs minimal `NeighborInfo` entries from the graph.
pub struct RifProcessorAdapter {
    engine: Arc<RifEngine>,
    graph: ReadHandle<crate::graph::RelationshipGraph>,
}

impl RifProcessorAdapter {
    pub fn new(engine: Arc<RifEngine>, graph: SharedGraph) -> Self {
        Self {
            engine,
            graph: ReadHandle::Live(graph),
        }
    }
    pub(crate) fn pinned(
        engine: Arc<RifEngine>,
        graph: Arc<tokio::sync::OwnedRwLockReadGuard<crate::graph::RelationshipGraph>>,
    ) -> Self {
        Self {
            engine,
            graph: ReadHandle::Pinned(graph),
        }
    }
}

impl RifProcessor for RifProcessorAdapter {
    fn compute_suppressions(
        &self,
        retrieved: &[MemoryId],
        neighbor_ids: &[MemoryId],
    ) -> Vec<RifSuppression> {
        let graph = self.graph.read();
        let mut suppressions = Vec::new();
        for &retrieved_id in retrieved {
            // Build NeighborInfo from graph data for each neighbor.
            let neighbors: Vec<NeighborInfo> = neighbor_ids
                .iter()
                .filter_map(|&nid| {
                    let node = graph.get_node(&nid)?;
                    // Find the edge between retrieved_id and nid.
                    let edges = graph.edges_for(&retrieved_id);
                    let edge = edges.iter().find(|e| {
                        let source_node = graph.nodes.get(e.source);
                        let target_node = graph.nodes.get(e.target);
                        matches!(
                            (source_node, target_node),
                            (Some(s), Some(t))
                            if (s.memory_id == retrieved_id && t.memory_id == nid)
                               || (s.memory_id == nid && t.memory_id == retrieved_id)
                        )
                    });
                    let (edge_weight, edge_type, distance) = match edge {
                        Some(e) => (e.weight, e.edge_type, 1),
                        None => return None, // not actually a neighbor
                    };
                    Some(NeighborInfo {
                        memory_id: nid,
                        edge_weight,
                        edge_type,
                        graph_distance: distance,
                        retrievability: node.strength,
                        stability: 1.0, // not available from graph node
                        similarity: None,
                    })
                })
                .collect();
            let effects = self.engine.compute_effects(retrieved_id, &neighbors);
            for update in effects {
                // Convert StabilityUpdate to RifSuppression.
                if update.multiplier < 1.0 {
                    suppressions.push(RifSuppression {
                        target: update.memory_id,
                        suppression_factor: update.multiplier,
                    });
                }
            }
        }
        suppressions
    }
}

// ── AccessRecorder ─────────────────────────────────────────────────

/// Adapts `RedbStorageEngine` to `AccessRecorder`.
pub struct StorageAccessRecorder {
    storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    clock: Arc<dyn crate::time::Clock>,
    completion: Option<crate::derived_completion::DerivedCompletion>,
}

impl StorageAccessRecorder {
    pub fn new(
        storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
        cache: Arc<CacheManager>,
    ) -> Self {
        Self {
            storage,
            cache,
            clock: Arc::new(crate::time::SystemClock),
            completion: None,
        }
    }
    pub fn with_clock(mut self, clock: Arc<dyn crate::time::Clock>) -> Self {
        self.clock = clock;
        self
    }
    pub(crate) fn with_completion(
        mut self,
        owner: crate::derived_completion::DerivedCompletion,
    ) -> Self {
        assert!(Arc::ptr_eq(&self.storage, &owner.storage));
        self.completion = Some(owner);
        self
    }
}

#[async_trait]
impl AccessRecorder for StorageAccessRecorder {
    fn supports_native_exposure(&self) -> bool {
        self.completion.is_some()
            && self
                .storage
                .read()
                .is_ok_and(|s| s.meta_store().uses_native_lifecycle())
    }

    async fn record_access_batch_for_publication(
        &self,
        accesses: &[(MemoryId, AccessKind)],
        scope: Option<&crate::storage::control::ExposureScope>,
        sampled_at: Option<i64>,
        parent: Option<&crate::storage::work::WorkTicket>,
        policy: Option<&crate::storage::native_lifecycle::NativePolicyInputs>,
        original_contract_version: u32,
    ) -> Result<crate::decay::observation::AccessObservation> {
        if accesses.is_empty() {
            return Ok(crate::decay::observation::AccessObservation::from_receipts(
                Vec::new(),
            ));
        }
        if !matches!(original_contract_version, 1 | 2) {
            return Err(SearchError::Unavailable(
                "unsupported original application contract".into(),
            ));
        }
        let native = self
            .storage
            .read()
            .map_err(|e| SearchError::Unavailable(e.to_string()))?
            .meta_store()
            .uses_native_lifecycle();
        let receipts = if native {
            let completion = self.completion.as_ref().ok_or_else(|| {
                SearchError::Unavailable(
                    "native exposure requires the configured publication owner".into(),
                )
            })?;
            let policy = policy.ok_or_else(|| {
                SearchError::Unavailable(
                    "native exposure requires the query's configured lifecycle policy".into(),
                )
            })?;
            completion
                .record_native_exposures_with_work(
                    accesses.to_vec(),
                    scope.cloned(),
                    sampled_at.unwrap_or_else(|| self.clock.now_millis()),
                    policy.clone(),
                    parent,
                    original_contract_version,
                )
                .await?
        } else {
            crate::decay::observation::record_exposures_scoped_with_origin_work(
                self.storage.clone(),
                self.cache.clone(),
                accesses.to_vec(),
                sampled_at.unwrap_or_else(|| self.clock.now_millis()),
                scope.cloned(),
                parent,
                original_contract_version,
            )
            .await?
        };
        // Native exposure initially uses the existing full publication fallback.
        Ok(crate::decay::observation::AccessObservation::from_receipts(
            receipts,
        ))
    }

    async fn record_access(
        &self,
        id: MemoryId,
        kind: AccessKind,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
        self.record_access_batch(&[(id, kind)], None).await
    }
    async fn record_access_batch(
        &self,
        accesses: &[(MemoryId, AccessKind)],
        scope: Option<&crate::storage::control::ExposureScope>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
        self.record_access_batch_with_work(accesses, scope, None)
            .await
    }

    async fn record_access_batch_at_with_work(
        &self,
        accesses: &[(MemoryId, AccessKind)],
        scope: Option<&crate::storage::control::ExposureScope>,
        sampled_at: i64,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
        crate::decay::observation::record_exposures_scoped_with_work(
            self.storage.clone(),
            self.cache.clone(),
            accesses.to_vec(),
            sampled_at,
            scope.cloned(),
            parent,
        )
        .await
        .map_err(Into::into)
    }
    async fn record_access_batch_with_work(
        &self,
        accesses: &[(MemoryId, AccessKind)],
        scope: Option<&crate::storage::control::ExposureScope>,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
        crate::decay::observation::record_exposures_scoped_with_work(
            self.storage.clone(),
            self.cache.clone(),
            accesses.to_vec(),
            self.clock.now_millis(),
            scope.cloned(),
            parent,
        )
        .await
        .map_err(Into::into)
    }
}

// ── GraphReader ────────────────────────────────────────────────────

/// Adapts `SharedGraph` (`Arc<RwLock<RelationshipGraph>>`) to `GraphReader`.
pub struct SharedGraphReader {
    graph: ReadHandle<crate::graph::RelationshipGraph>,
}

impl SharedGraphReader {
    pub fn new(graph: SharedGraph) -> Self {
        Self {
            graph: ReadHandle::Live(graph),
        }
    }
    pub(crate) fn pinned(
        value: Arc<tokio::sync::OwnedRwLockReadGuard<crate::graph::RelationshipGraph>>,
    ) -> Self {
        Self {
            graph: ReadHandle::Pinned(value),
        }
    }
}

impl GraphReader for SharedGraphReader {
    fn connection_bonuses(
        &self,
        strengths: &HashMap<MemoryId, f32>,
    ) -> Result<HashMap<MemoryId, f32>> {
        let wait = crate::performance::Timer::new("graph.lock_wait");
        let graph = self.graph.read();
        wait.complete();
        let graph = crate::performance::Hold::new("graph.lock_hold", graph);
        let config = crate::graph::activation::ActivationConfig::default();
        Ok(strengths
            .keys()
            .map(|&id| {
                (
                    id,
                    crate::graph::activation::connection_bonus_with_strengths(
                        id, &graph, &config, strengths,
                    ),
                )
            })
            .collect())
    }

    fn spreading_activation_canonical(
        &self,
        seeds: &[(MemoryId, f32)],
        namespace: NamespaceId,
        depth: u8,
        records: &HashMap<MemoryId, CachedRecord>,
    ) -> Result<Vec<(MemoryId, f32)>> {
        let wait = crate::performance::Timer::new("graph.lock_wait");
        let graph = self.graph.read();
        wait.complete();
        let graph = crate::performance::Hold::new("graph.lock_hold", graph);
        Ok(crate::graph::activation::spreading_activation_with_records(
            &graph,
            seeds,
            namespace,
            &spreading_config(depth),
            records,
        ))
    }

    fn supersession_links(
        &self,
        id: MemoryId,
        allowance: usize,
    ) -> Result<crate::graph::query::SupersessionLinks> {
        let graph = self.graph.read();
        Ok(crate::graph::query::supersession_links(
            &graph, id, allowance,
        ))
    }

    fn expand_eligible(
        &self,
        seeds: &[(MemoryId, f32)],
        namespace: NamespaceId,
        depth: u8,
        eligible: &std::collections::HashMap<MemoryId, CachedRecord>,
    ) -> Result<crate::graph::query::GraphExpansion> {
        let wait = crate::performance::Timer::new("graph.lock_wait");
        let graph = self.graph.read();
        wait.complete();
        let graph = crate::performance::Hold::new("graph.lock_hold", graph);
        crate::graph::query::expand(&graph, seeds, namespace, depth, eligible)
            .map_err(|e| SearchError::Internal(e.into()))
    }

    fn neighbors(&self, id: &MemoryId) -> Vec<MemoryId> {
        let graph = self.graph.read();
        graph
            .outgoing_edges(id)
            .iter()
            .filter_map(|edge| graph.nodes.get(edge.target).map(|node| node.memory_id))
            .chain(
                graph
                    .incoming_edges(id)
                    .iter()
                    .filter_map(|edge| graph.nodes.get(edge.source).map(|node| node.memory_id)),
            )
            .collect()
    }

    fn spreading_activation(
        &self,
        seeds: &[(MemoryId, f32)],
        namespace_id: NamespaceId,
        graph_depth: u8,
    ) -> Vec<(MemoryId, f32)> {
        let wait = crate::performance::Timer::new("graph.lock_wait");
        let graph = self.graph.read();
        wait.complete();
        let graph = crate::performance::Hold::new("graph.lock_hold", graph);
        let config = spreading_config(graph_depth);
        crate::graph::activation::spreading_activation(&graph, seeds, namespace_id, &config)
    }

    fn superseded_by(&self, id: &MemoryId) -> Option<MemoryId> {
        let graph = self.graph.read();
        // Chain-walking (and its cycle guard) lives on the graph itself,
        // where it can be unit-tested against real nodes and edges.
        graph.supersedes_terminal(id)
    }
}

// ── EntityIndexReader ─────────────────────────────────────────────

/// Adapts a shared `EntityIndex` to `EntityIndexReader`.
///
/// Namespace postings and canonical eligible IDs are selected before the
/// adapter's candidate cap, so foreign or filtered records cannot crowd out hits.
pub struct SharedEntityIndexReader {
    entity_index: ReadHandle<EntityIndex>,
}

impl SharedEntityIndexReader {
    pub fn new(entity_index: Arc<tokio::sync::RwLock<EntityIndex>>) -> Self {
        Self {
            entity_index: ReadHandle::Live(entity_index),
        }
    }
    pub(crate) fn pinned(value: Arc<tokio::sync::OwnedRwLockReadGuard<EntityIndex>>) -> Self {
        Self {
            entity_index: ReadHandle::Pinned(value),
        }
    }
}

impl EntityIndexReader for SharedEntityIndexReader {
    fn find_by_entities(
        &self,
        namespace_id: NamespaceId,
        entities: &[String],
        exclude_id: MemoryId,
        k: usize,
        eligible_ids: &HashSet<MemoryId>,
    ) -> Result<Vec<EntityRecallResult>> {
        if entities.is_empty() {
            return Ok(Vec::new());
        }
        let index = self.entity_index.read();
        let results = index.find_by_entities_filtered(namespace_id, entities, exclude_id, |id| {
            eligible_ids.contains(&id)
        });
        Ok(results
            .into_iter()
            .take(k)
            .map(|(memory_id, shared_count)| EntityRecallResult {
                memory_id,
                shared_count,
            })
            .collect())
    }
}

fn spreading_config(graph_depth: u8) -> crate::graph::activation::SpreadingActivationConfig {
    let mut config = crate::graph::activation::SpreadingActivationConfig::default();
    match graph_depth {
        2 => {
            config.max_budget = 200;
            config.firing_threshold = 0.02;
            config.output_threshold = 0.015;
        }
        3 => {
            config.max_budget = 400;
            config.hop_decay = 0.85;
            config.firing_threshold = 0.015;
            config.output_threshold = 0.01;
        }
        _ => {}
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DecayPhase;
    use crate::test_support::{Fixture, MemorySpec};

    #[tokio::test]
    async fn namespace_enumeration_scopes_ids_and_batch_reads_current_canonical_records() {
        let fixture = Fixture::new();
        let namespace = NamespaceId::new(1);
        let foreign_namespace = fixture.create_namespace("foreign");
        let live = fixture
            .insert_memory_in(namespace, &["project/recalld"])
            .await;
        let ghost = fixture
            .insert_memory_with(MemorySpec {
                phase: DecayPhase::Ghost,
                decay_strength: 0.2,
                tags: &["type/ghost"],
                ..MemorySpec::new(namespace)
            })
            .await;
        let removed = fixture.insert_memory_in(namespace, &[]).await;
        let foreign = fixture
            .insert_memory_in(foreign_namespace, &["project/recalld"])
            .await;
        let adapter = StorageMetadataAdapter::new(fixture.storage.clone());
        let snapshot = adapter.namespace_records(namespace).await.unwrap();
        assert_eq!(
            snapshot.iter().map(|r| r.id).collect::<HashSet<_>>(),
            HashSet::from([live, ghost, removed])
        );
        assert!(snapshot.iter().all(|r| r.namespace_id == namespace));
        assert!(
            adapter
                .namespace_records(NamespaceId::new(999))
                .await
                .unwrap()
                .is_empty()
        );
        let mut ids = adapter.list_namespace_memories(namespace).await.unwrap();
        assert_eq!(
            ids.iter().copied().collect::<HashSet<_>>(),
            HashSet::from([live, ghost, removed])
        );
        assert_eq!(
            adapter
                .list_namespace_memories(foreign_namespace)
                .await
                .unwrap(),
            vec![foreign]
        );
        assert!(
            adapter
                .list_namespace_memories(NamespaceId::new(999))
                .await
                .unwrap()
                .is_empty()
        );

        // The batch read must observe canonical state, not treat an enumerated ID as eligible.
        fixture.tombstone(removed);
        let after = adapter.namespace_records(namespace).await.unwrap();
        assert_eq!(
            after.iter().map(|r| r.id).collect::<HashSet<_>>(),
            HashSet::from([live, ghost])
        );
        assert!(
            after
                .iter()
                .any(|r| r.id == ghost && r.phase == DecayPhase::Ghost)
        );

        ids.push(MemoryId::new()); // A missing canonical row is skipped.
        let records = adapter.get_batch(&ids).await.unwrap();
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|r| r.namespace_id == namespace));
        let removed = records.iter().find(|r| r.id == removed).unwrap();
        assert_eq!(removed.phase, DecayPhase::Tombstone);
        let ghost = records.iter().find(|r| r.id == ghost).unwrap();
        assert_eq!(ghost.phase, DecayPhase::Ghost);
        assert_eq!(ghost.strength, 0.2);
        assert_eq!(ghost.tags[0].as_str(), "type/ghost");
        assert_eq!(
            adapter
                .list_namespace_memories(namespace)
                .await
                .unwrap()
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from([live, ghost.id])
        );
    }
}

#[cfg(test)]
mod entity_namespace_tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn entity_reader_selects_namespace_before_candidate_cap() {
        let ns = NamespaceId::new(1);
        let other = NamespaceId::new(2);
        let mut index = EntityIndex::new();
        let entities = vec!["John Smith".into(), "José".into()];
        let mut eligible = HashSet::new();
        for _ in 0..100 {
            let foreign = MemoryId::new();
            index.add(other, foreign, &entities);
            eligible.insert(foreign);
        }
        // Higher-count local postings must also be filtered before the cap.
        index.add(ns, MemoryId::new(), &entities);
        let local = MemoryId::new();
        index.add(ns, local, &["José".into()]);
        eligible.insert(local);
        let reader = SharedEntityIndexReader::new(Arc::new(tokio::sync::RwLock::new(index)));
        let hits = reader
            .find_by_entities(ns, &entities, MemoryId::new(), 1, &eligible)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory_id, local);
        assert_eq!(hits[0].shared_count, 1);
        assert!(
            reader
                .find_by_entities(ns, &entities, local, 1, &eligible)
                .unwrap()
                .is_empty()
        );
    }
}
