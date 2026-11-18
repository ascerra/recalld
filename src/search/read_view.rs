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
    /// Present only when the owned engine copied every selected payload at G.
    pub payload: Option<CoherentPayload>,
}

impl CoherentSearchResponse {
    /// Final hydrated exposure selection only. This performs no recording and
    /// neither changes requested access nor chooses the numeric API origin.
    pub(crate) fn exposures(&self, returns_neighbors: bool) -> Result<Vec<(MemoryId, AccessKind)>> {
        let payload = self
            .payload
            .as_ref()
            .ok_or_else(|| unavailable("coherent payload missing for exposure selection"))?;
        if !returns_neighbors && !payload.neighbors.is_empty() {
            return Err(unavailable(
                "ranked-only response received unexpected supplemental neighbors",
            ));
        }
        if payload.rows.len() != self.response.response.results.len() {
            return Err(unavailable("coherent exposure page length mismatch"));
        }
        let mut accesses = Vec::<(MemoryId, AccessKind)>::new();
        let mut positions = HashMap::<MemoryId, usize>::new();
        let mut admit = |id, kind| {
            if let Some(&position) = positions.get(&id) {
                if kind == AccessKind::DirectRetrieval {
                    accesses[position].1 = kind;
                }
            } else {
                positions.insert(id, accesses.len());
                accesses.push((id, kind));
            }
        };
        for (selected, row) in self.response.response.results.iter().zip(&payload.rows) {
            let id = MemoryId::from_bytes(row.record.id);
            if id != selected.memory_id || row.record.namespace_id != payload.namespace.id.get() {
                return Err(unavailable(
                    "coherent exposure page identity/scope mismatch",
                ));
            }
            let kind = match self.response.diagnostics.semantics {
                RetrievalSemantics::V1 => {
                    // Exact named legacy classification, including injected
                    // terminals whose own activation field may be absent.
                    if selected.activation_score.is_some() {
                        AccessKind::AssociativeRetrieval
                    } else {
                        AccessKind::DirectRetrieval
                    }
                }
                RetrievalSemantics::V2(_) => {
                    let provenance = self
                        .response
                        .diagnostics
                        .provenance
                        .as_ref()
                        .and_then(|p| p.get(&id))
                        .ok_or_else(|| unavailable("missing V2 exposure provenance"))?;
                    let mut channels = provenance
                        .own_channels
                        .iter()
                        .chain(&provenance.inherited_channels)
                        .peekable();
                    if channels.peek().is_none() {
                        return Err(unavailable("empty V2 exposure provenance"));
                    }
                    if channels.any(|c| *c != RetrievalChannel::Graph) {
                        AccessKind::DirectRetrieval
                    } else {
                        AccessKind::AssociativeRetrieval
                    }
                }
            };
            admit(id, kind);
        }
        for neighbor in &payload.neighbors {
            if neighbor.payload.record.namespace_id != payload.namespace.id.get() {
                return Err(unavailable("coherent neighbor exposure scope mismatch"));
            }
            admit(
                MemoryId::from_bytes(neighbor.payload.record.id),
                AccessKind::AssociativeRetrieval,
            );
        }
        Ok(accesses)
    }
}

/// A direct fetch owns its canonical row and selected text, including markers.
pub struct CoherentGetResponse {
    pub read_generation: GenerationRef,
    pub evaluated_at: i64,
    pub requested_access: bool,
    pub payload: Option<(NamespaceConfig, crate::storage::engine::HydratedRecord)>,
    pub response: Option<MemoryResponse>,
}

pub struct CoherentSimilarResponse {
    /// Named legacy selector; coherence does not activate A2 version-2 ranking.
    pub semantics: RetrievalSemantics,
    pub read_generation: GenerationRef,
    pub evaluated_at: i64,
    pub requested_access: bool,
    pub results: Vec<SearchResult>,
    pub payload: CoherentPayload,
}

/// Canonical owned rows in exactly the selected result order. No handler reload.
pub struct CoherentPayload {
    pub namespace: NamespaceConfig,
    pub rows: Vec<crate::storage::engine::HydratedRecord>,
    pub related: HashMap<MemoryId, Vec<CoherentRelation>>,
    pub neighbors: Vec<CoherentNeighbor>,
    /// None is named legacy selection, not complete V2 supplemental coverage.
    pub supplemental: Option<super::payload_selection::SupplementalDiagnostics>,
}
pub struct CoherentRelation {
    pub id: MemoryId,
    pub edge_type: String,
    pub weight: f32,
}
pub struct CoherentNeighbor {
    pub payload: crate::storage::engine::HydratedRecord,
    pub edge_type: String,
    pub weight: f32,
    pub connected_to: MemoryId,
}

fn unavailable(error: impl std::fmt::Display) -> SearchError {
    SearchError::Unavailable(error.to_string())
}

fn canonical_read_error(error: crate::storage::StorageError) -> SearchError {
    match error {
        crate::storage::StorageError::NamespaceMismatch { .. }
        | crate::storage::StorageError::InvalidNamespaceName { .. }
        | crate::storage::StorageError::NotFound(_) => SearchError::CanonicalRead(Box::new(error)),
        _ => unavailable(error),
    }
}

#[async_trait::async_trait]
impl NamespaceResolver for CanonicalReadView {
    fn is_native_owned(&self) -> Result<bool> {
        Ok(CanonicalReadView::is_native_owned(self))
    }
    fn check_native_vector_namespace(&self, namespace: &NamespaceConfig) -> Result<()> {
        CanonicalReadView::check_native_vector_namespace(self, namespace).map_err(unavailable)
    }
    fn check_native_query_profile(
        &self,
        namespace: &NamespaceConfig,
        profile: Option<&crate::model::schema::EmbeddingProfile>,
    ) -> Result<()> {
        CanonicalReadView::check_native_query_profile(self, namespace, profile).map_err(unavailable)
    }
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
// Deferred selection retains the original observer capability, but this
// pinned adapter still rejects EVERY attempted write.
struct RejectExposure {
    native_writer_available: bool,
}
#[async_trait::async_trait]
impl AccessRecorder for RejectExposure {
    fn supports_native_exposure(&self) -> bool {
        self.native_writer_available
    }
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
        self.coherent_selection(owner, rif, query, semantics, deadline, None, None)
            .await
    }

    pub(crate) fn with_coherent_owner(
        mut self,
        owner: DerivedCompletion,
        rif: Arc<crate::rif::RifEngine>,
    ) -> Self {
        self.coherent_owner = Some((owner, rif));
        self
    }

    /// Explicitly absent on immutable/offline adapters. Actual owned production
    /// construction installs the same coordinator, never a second registry.
    pub(crate) async fn prepare_coherent_payload(
        &self,
        query: SearchQuery,
        include_embeddings: bool,
        include_neighbors: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Option<CoherentSearchResponse>> {
        self.prepare_coherent_payload_with_semantics(
            query,
            RetrievalSemantics::V1,
            include_embeddings,
            include_neighbors,
            parent,
        )
        .await
    }

    /// Internal explicit selector. API contract origin and exposure intent are
    /// independent inputs; selecting V2 neither changes them nor activates wire V2.
    pub(crate) async fn prepare_coherent_payload_with_semantics(
        &self,
        query: SearchQuery,
        semantics: RetrievalSemantics,
        include_embeddings: bool,
        include_neighbors: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Option<CoherentSearchResponse>> {
        let Some((owner, rif)) = &self.coherent_owner else {
            return Ok(None);
        };
        self.coherent_selection(
            owner,
            rif.clone(),
            query,
            semantics,
            std::time::Instant::now() + std::time::Duration::from_secs(30),
            Some((include_embeddings, include_neighbors)),
            parent,
        )
        .await
        .map(Some)
    }

    pub(crate) async fn prepare_coherent_get(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        requested_access: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Option<CoherentGetResponse>> {
        let Some((owner, rif)) = &self.coherent_owner else {
            return Ok(None);
        };
        let work = owner.accept_work(parent).map_err(unavailable)?;
        let result = async {
            let (engine, canonical, _graph, _publication, evaluated_at) =
                self.pin_read_engine(owner, rif.clone(), &work).await?;
            let at = canonical.at;
            let storage = owner.storage.clone();
            let namespace = namespace.map(str::to_owned);
            let child = work.child().map_err(unavailable)?;
            let payload = tokio::task::spawn_blocking(move || {
                let storage = storage.read().map_err(unavailable)?;
                let result = storage
                    .hydrate_get_at(at, id, namespace.as_deref())
                    .map_err(canonical_read_error);
                storage.meta_store().finish_work(child);
                result
            })
            .await
            .map_err(unavailable)??;
            let mut response = engine
                .get_memory_with_access_admitted(id, requested_access, true, None, 1)
                .await?
                .finish::<SearchError>(Ok(()))?;
            if let (Some(response), Some((_, payload))) = (&mut response, &payload) {
                response.full_text = payload.full_text.clone();
                response.access_count = payload.record.access_history.len();
            }
            Ok(CoherentGetResponse {
                read_generation: at,
                evaluated_at,
                requested_access,
                payload,
                response,
            })
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(30), result).await;
        owner.finish_known_work(work);
        result
            .map_err(|_| unavailable("coherent get deadline expired"))?
            .map(Some)
    }

    pub(crate) async fn prepare_coherent_similar(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        k: usize,
        min_score: Option<f32>,
        requested_access: bool,
        include_embeddings: bool,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Option<CoherentSimilarResponse>> {
        let Some((owner, rif)) = &self.coherent_owner else {
            return Ok(None);
        };
        let work = owner.accept_work(parent).map_err(unavailable)?;
        let result = async {
            let (engine, canonical, _graph, _publication, evaluated_at) =
                self.pin_read_engine(owner, rif.clone(), &work).await?;
            let at = canonical.at;
            let storage = owner.storage.clone();
            let asserted = namespace.map(str::to_owned);
            let child = work.child().map_err(unavailable)?;
            let source = tokio::task::spawn_blocking(move || {
                let storage = storage.read().map_err(unavailable)?;
                let result = storage
                    .hydrate_get_at(at, id, asserted.as_deref())
                    .map_err(canonical_read_error);
                storage.meta_store().finish_work(child);
                result
            })
            .await
            .map_err(unavailable)??
            .ok_or(SearchError::MemoryNotFound(id))?;
            let namespace = source.0;
            let results = engine
                .similar_with_access_admitted(id, k, min_score, requested_access, true, None, 1)
                .await?
                .finish::<SearchError>(Ok(()))?;
            let ids: Vec<_> = results.iter().map(|r| r.memory_id).collect();
            let storage = owner.storage.clone();
            let namespace_id = namespace.id;
            let child = work.child().map_err(unavailable)?;
            let rows = tokio::task::spawn_blocking(move || {
                let storage = storage.read().map_err(unavailable)?;
                let result = storage
                    .hydrate_at(at, namespace_id, &ids, include_embeddings)
                    .map_err(unavailable);
                storage.meta_store().finish_work(child);
                result
            })
            .await
            .map_err(unavailable)??;
            Ok(CoherentSimilarResponse {
                semantics: RetrievalSemantics::V1,
                read_generation: at,
                evaluated_at,
                requested_access,
                results,
                payload: CoherentPayload {
                    namespace,
                    rows,
                    related: HashMap::new(),
                    neighbors: Vec::new(),
                    supplemental: None,
                },
            })
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(30), result).await;
        owner.finish_known_work(work);
        result
            .map_err(|_| unavailable("coherent similar deadline expired"))?
            .map(Some)
    }

    pub(super) async fn pin_read_engine(
        &self,
        owner: &DerivedCompletion,
        rif: Arc<crate::rif::RifEngine>,
        work: &crate::storage::work::WorkTicket,
    ) -> Result<(
        Self,
        Arc<CanonicalReadView>,
        Arc<tokio::sync::OwnedRwLockReadGuard<crate::graph::RelationshipGraph>>,
        tokio::sync::OwnedRwLockReadGuard<()>,
        i64,
    )> {
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
        let canonical = Arc::new(canonical);
        let evaluated_at = self.clock.now_millis();
        let engine = Self {
            rif_mode: self.rif_mode,
            native_query_admission: self.native_query_admission,
            coherent_owner: None,
            clock: Arc::new(crate::time::ManualClock::new(evaluated_at)),
            decay_policy: self.decay_policy.clone(),
            decay_multiplier: self.decay_multiplier,
            namespace_resolver: canonical.clone(),
            embedding_providers: self.embedding_providers.clone(),
            vector_indexes: Arc::new(SharedVectorIndexRegistry::pinned(vectors)),
            fts_index: Arc::new(SharedFtsIndexRegistry::pinned(fts)),
            entity_index: Arc::new(SharedEntityIndexReader::pinned(entities)),
            cache: Arc::new(BypassCache),
            meta_store: canonical.clone(),
            graph: Arc::new(SharedGraphReader::pinned(graph.clone())),
            rif_engine: Arc::new(RifProcessorAdapter::pinned(rif, graph.clone())),
            access_recorder: Arc::new(RejectExposure {
                native_writer_available: self.access_recorder.supports_native_exposure(),
            }),
        };
        Ok((engine, canonical, graph, publication, evaluated_at))
    }

    async fn coherent_selection(
        &self,
        owner: &DerivedCompletion,
        rif: Arc<crate::rif::RifEngine>,
        query: SearchQuery,
        semantics: RetrievalSemantics,
        deadline: std::time::Instant,
        payload_embeddings: Option<(bool, bool)>,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<CoherentSearchResponse> {
        let work = owner.accept_work(parent).map_err(unavailable)?;
        let result = async {
            let (engine, canonical, graph, publication, evaluated_at) =
                self.pin_read_engine(owner, rif, &work).await?;
            let at = canonical.at;
            // Preserve requested mode for policy resolution, but never expose
            // until all selected content is copied and transport selection is final.
            let (mut response, selection) = engine
                // The outer owned root covers this immutable pinned adapter;
                // CanonicalReadView cannot accept another owned work ticket.
                .search_checked_with_selection(query, semantics, true, None, 1)
                .await?;
            let namespace = selection.namespace.clone();
            let payload = if let Some((include_embeddings, include_neighbors)) = payload_embeddings
            {
                let mut ids: Vec<_> = response
                    .response
                    .results
                    .iter()
                    .map(|r| r.memory_id)
                    .collect();
                let selected_count = ids.len();
                let (related, neighbors, supplemental) = if let RetrievalSemantics::V2(mode) =
                    semantics
                {
                    let selected = engine
                        .select_payload_v2(
                            &graph,
                            &selection,
                            &ids,
                            include_neighbors,
                            mode,
                            &mut response.diagnostics,
                        )
                        .await?;
                    let mut neighbors = Vec::new();
                    for (position, (id, link)) in selected.neighbors.into_iter().enumerate() {
                        ids.push(id);
                        neighbors.push((link, position < 5));
                    }
                    (selected.related, neighbors, Some(selected.diagnostics))
                } else {
                    let selected: HashSet<_> = ids.iter().copied().collect();
                    let mut related = HashMap::new();
                    let mut candidates: HashMap<MemoryId, (f32, String, MemoryId)> = HashMap::new();
                    for &id in &ids {
                        let Some(key) = graph.resolve(&id) else {
                            continue;
                        };
                        let mut edges = Vec::new();
                        for edge in graph.edges_for(&id) {
                            let other = if edge.source == key {
                                edge.target
                            } else {
                                edge.source
                            };
                            let Some(node) = graph.get_node_by_key(other) else {
                                continue;
                            };
                            let kind = format!("{:?}", edge.edge_type).to_lowercase();
                            if selected.contains(&node.memory_id) {
                                edges.push(CoherentRelation {
                                    id: node.memory_id,
                                    edge_type: kind,
                                    weight: edge.weight,
                                });
                            } else if include_neighbors {
                                let entry = candidates.entry(node.memory_id).or_insert((
                                    0.,
                                    kind.clone(),
                                    id,
                                ));
                                if edge.weight > entry.0 {
                                    *entry = (edge.weight, kind, id);
                                }
                            }
                        }
                        related.insert(id, edges);
                    }
                    // Preserve named v1's cap-before-filter and five-text policy.
                    // Equal-weight HashMap order (including per-neighbor link ties)
                    // is intentionally still legacy behavior, NOT deterministic replay.
                    // Before v2/reset promotion: filter before cap and test stable
                    // UUID/edge/connected-to tie ordering. Eligibility is at this G.
                    let mut candidates: Vec<_> = candidates.into_iter().collect();
                    candidates.sort_by(|a, b| {
                        b.1.0
                            .partial_cmp(&a.1.0)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    candidates.truncate(10);
                    let mut neighbors = Vec::new();
                    for (position, (id, link)) in candidates.into_iter().enumerate() {
                        let Some(record) = canonical.get(id).map_err(unavailable)? else {
                            continue;
                        };
                        if record.namespace_id != namespace.id
                            || matches!(record.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
                            || record.summary.is_empty()
                        {
                            continue;
                        }
                        ids.push(id);
                        neighbors.push((link, position < 5));
                    }
                    (related, neighbors, None)
                };
                let storage = owner.storage.clone();
                let namespace_id = namespace.id;
                let child = work.child().map_err(unavailable)?;
                let mut rows = tokio::task::spawn_blocking(move || {
                    let storage = storage.read().map_err(unavailable)?;
                    let result = storage
                        .hydrate_at(at, namespace_id, &ids, include_embeddings)
                        .map_err(unavailable);
                    storage.meta_store().finish_work(child);
                    result
                })
                .await
                .map_err(unavailable)??;
                let neighbor_rows = rows.split_off(selected_count);
                let neighbors = neighbor_rows
                    .into_iter()
                    .zip(neighbors)
                    .map(
                        |(mut payload, ((weight, edge_type, connected_to), full_text))| {
                            if !full_text {
                                payload.full_text = None;
                            }
                            CoherentNeighbor {
                                payload,
                                weight,
                                edge_type,
                                connected_to,
                            }
                        },
                    )
                    .collect();
                Some(CoherentPayload {
                    namespace,
                    rows,
                    related,
                    neighbors,
                    supplemental,
                })
            } else {
                None
            };
            // Explicit release before root completion, including the canonical tx.
            drop(engine);
            drop(canonical);
            drop(graph);
            drop(publication);
            Ok(CoherentSearchResponse {
                read_generation: at,
                evaluated_at,
                response,
                payload,
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

    include!("payload_tests.rs");
    include!("query_wire_tests.rs");
    include!("get_wire_tests.rs");
    include!("similar_selection_tests.rs");

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
            Arc::new(RejectExposure {
                native_writer_available: false,
            }),
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
        // Inspect this owned fixture's actual catalog/selection. Offline full
        // validation opens database copies and is not a live-handle byte reader.
        // This is a test invariant, never permission to capture a snapshot.
        let storage = f.storage.read().unwrap();
        let mut paths: Vec<_> = ["meta.db", "edges.db", "fulltext.dat"]
            .into_iter()
            .map(|name| f.path().join(name))
            .collect();
        for namespace in storage.list_namespaces().unwrap() {
            paths.push(f.path().join(namespace.name).join("vectors.dat"));
        }
        if let Some(selected) = storage.meta_store().text_selection().unwrap() {
            paths.push(f.path().join(selected.filename));
        }
        paths.sort();
        paths.dedup();
        paths
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
