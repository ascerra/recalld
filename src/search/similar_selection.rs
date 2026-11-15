//! Explicit stored-vector similarity; no public V2 activation or provider call.
use super::semantics::Resolution;
use super::*;
use crate::model::schema::{EmbeddingBinding, EmbeddingProfile};
use crate::storage::control::GenerationRef;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SimilarityScope {
    SameNamespace,
    AllCompatible,
}

pub(crate) struct SimilaritySelection {
    pub semantics: RetrievalSemantics,
    pub scope: SimilarityScope,
    pub filter: super::super::query::SearchFilter,
    pub limit: usize,
    pub min_score: Option<f32>,
    pub record_access: bool,
    pub include_embeddings: bool,
    pub original_contract_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SimilarityExclusion {
    UnknownProfile,
    DifferentProfile,
    DifferentDimension,
}

#[derive(Debug, Default)]
pub(crate) struct SimilarityCoverage {
    pub searched: Vec<NamespaceId>,
    pub excluded: BTreeMap<NamespaceId, SimilarityExclusion>,
    pub examined_records: usize,
    pub eligible_records: usize,
    pub scored_records: usize,
    pub score_admitted: usize,
    pub resolved_candidates: usize,
    pub result_limit_applied: bool,
    pub strengths: BTreeMap<NamespaceId, StrengthDiagnostics>,
}

pub(crate) struct SelectedSimilarRow {
    pub result: SearchResult,
    pub namespace: NamespaceConfig,
    pub payload: crate::storage::engine::HydratedRecord,
}

pub(crate) struct SelectedSimilarity {
    pub source_namespace: NamespaceConfig,
    pub read_generation: GenerationRef,
    pub evaluated_at: i64,
    pub rows: Vec<SelectedSimilarRow>,
    pub coverage: SimilarityCoverage,
    pub diagnostics: SemanticsDiagnostics,
    requested_access: bool,
    original_contract_version: u32,
    exposure: crate::decay::similarity_scope::SimilarityExposure,
}

fn unavailable(error: impl std::fmt::Display) -> SearchError {
    SearchError::Unavailable(error.to_string())
}

fn bound_profile(ns: &NamespaceConfig) -> Result<Option<&EmbeddingProfile>> {
    let Some(schema) = &ns.schema else {
        return Ok(None);
    };
    schema.validate(ns).map_err(unavailable)?;
    Ok(match &schema.embedding_binding {
        EmbeddingBinding::Bound { descriptor, .. } => Some(descriptor),
        EmbeddingBinding::UnknownLegacy => None,
    })
}

impl QueryEngine {
    /// Actual consumer shared by explicit V2 and the legacy explicit cross-scope
    /// operation. Existing V1 same-namespace convenience calls remain unchanged.
    pub(crate) async fn similar_selected_with_work(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        options: SimilaritySelection,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Observed<SelectedSimilarity>> {
        let work = self.accept_work(parent)?;
        let result = async {
            let selected = self
                .prepare_similar_selection(id, namespace, options, work.as_ref())
                .await?;
            let receipts = self
                .record_selected_similarity(&selected, work.as_ref())
                .await?;
            Ok(Observed {
                value: selected,
                receipts,
            })
        }
        .await;
        self.finish_work(work);
        result
    }

    pub(crate) async fn record_selected_similarity(
        &self,
        selected: &SelectedSimilarity,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
        if !selected.requested_access || selected.rows.is_empty() {
            return Ok(Vec::new());
        }
        let work = self.accept_work(parent)?;
        let result = async {
            let accesses: Vec<_> = selected
                .rows
                .iter()
                .map(|r| (r.result.memory_id, AccessKind::AssociativeRetrieval))
                .collect();
            let policy = crate::storage::native_lifecycle::NativePolicyInputs {
                global: self.decay_policy.clone(),
                multiplier: self.decay_multiplier,
            };
            let observation = self
                .access_recorder
                .record_similarity_for_publication(
                    &accesses,
                    &selected.exposure,
                    selected.evaluated_at,
                    work.as_ref(),
                    &policy,
                    selected.original_contract_version,
                )
                .await?;
            let observed = Observed {
                value: (),
                receipts: observation.receipts,
            }
            .publish_derived(self.coherent_owner.as_ref().map(|(o, _)| o), work.as_ref())
            .await?;
            Ok(observed.receipts)
        }
        .await;
        self.finish_work(work);
        result
    }

    pub(crate) async fn prepare_similar_selection(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
        options: SimilaritySelection,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<SelectedSimilarity> {
        if options.limit == 0
            || options.limit > SearchQuery::MAX_USER_LIMIT
            || options
                .min_score
                .is_some_and(|v| !v.is_finite() || !(-1.0..=1.0).contains(&v))
            || options.filter.min_strength.is_some_and(|v| !v.is_finite())
        {
            return Err(SearchError::InvalidFilter(
                "invalid similarity limit or threshold".into(),
            ));
        }
        let (owner, rif) = self
            .coherent_owner
            .as_ref()
            .ok_or_else(|| unavailable("coherent similarity owner unavailable"))?;
        let work = owner.accept_work(parent).map_err(unavailable)?;
        let result = async {
            let (engine, canonical, _graph, _publication, evaluated_at) =
                self.pin_read_engine(owner, rif.clone(), &work).await?;
            let source = canonical
                .get(id)
                .map_err(unavailable)?
                .ok_or(SearchError::MemoryNotFound(id))?;
            if matches!(source.phase, DecayPhase::Ghost | DecayPhase::Tombstone) {
                return Err(SearchError::MemoryNotFound(id));
            }
            let source_ns = engine
                .namespace_resolver
                .resolve_id(source.namespace_id)
                .await?;
            if let Some(name) = namespace {
                crate::model::validation::validate_namespace_name(name).map_err(|error| {
                    SearchError::CanonicalRead(Box::new(
                        crate::storage::StorageError::InvalidNamespaceName {
                            name: name.into(),
                            reason: error.to_string(),
                        },
                    ))
                })?;
                if !source_ns.name.eq_ignore_ascii_case(name) {
                    return Err(SearchError::CanonicalRead(Box::new(
                        crate::storage::StorageError::NamespaceMismatch {
                            id: id.into_inner(),
                            namespace: name.into(),
                        },
                    )));
                }
            }
            let profile = bound_profile(&source_ns)?
                .cloned()
                .ok_or_else(|| unavailable("source embedding provenance is unknown"))?;
            let mut vector = engine
                .vector_indexes
                .get_vector(id)
                .ok_or_else(|| unavailable("source vector unavailable"))?;
            super::super::query::normalize_query_embedding(
                &mut vector,
                source_ns.embedding_dim as usize,
            )?;
            let mut coverage = SimilarityCoverage::default();
            let mut diagnostics = SemanticsDiagnostics::new(options.semantics);
            // Similarity does not perform associative expansion or RIF.
            diagnostics.rif_off = true;
            let mut candidates = BTreeMap::<MemoryId, (Candidate, NamespaceConfig)>::new();
            let mut namespaces = canonical.namespaces().map_err(unavailable)?;
            namespaces.sort_by_key(|ns| ns.id);
            for ns in namespaces {
                if options.scope == SimilarityScope::SameNamespace && ns.id != source_ns.id {
                    continue;
                }
                let binding = bound_profile(&ns)?;
                let exclusion = if ns.embedding_dim != profile.dimension {
                    Some(SimilarityExclusion::DifferentDimension)
                } else if binding.is_none() {
                    Some(SimilarityExclusion::UnknownProfile)
                } else if binding != Some(&profile) {
                    Some(SimilarityExclusion::DifferentProfile)
                } else {
                    None
                };
                if let Some(reason) = exclusion {
                    coverage.excluded.insert(ns.id, reason);
                    continue;
                }
                coverage.searched.push(ns.id);
                let mut filter = options.filter.clone();
                filter
                    .labels
                    .validate(ns.schema.is_some())
                    .map_err(SearchError::InvalidFilter)?;
                filter.compiled_labels = Some(filter.labels.compile());
                let query = SearchQuery {
                    record_access: options.record_access,
                    query_embedding: None,
                    text: None,
                    fts_query: None,
                    namespace: ns.name.clone(),
                    filter,
                    limit: options.limit,
                    min_score: 0.,
                    include_ghosts: false,
                    query_mode: QueryMode::EmbeddingOnly,
                    graph_depth: 0,
                    time_range_start: None,
                    time_range_end: None,
                    entities: vec![],
                };
                let mut context = StrengthContext::pin(&engine, &ns, options.record_access)?;
                coverage.strengths.insert(ns.id, context.diagnostics);
                let mut records = canonical.records(ns.id).map_err(unavailable)?;
                context.project_namespace(&mut records, engine.graph.as_ref())?;
                records.sort_by_key(|r| r.id);
                coverage.examined_records += records.len();
                for record in records {
                    if record.id == id || !Self::passes_hard_filters(&record, &query, ns.id) {
                        continue;
                    }
                    semantics::validate_record(&record)?;
                    coverage.eligible_records += 1;
                    let stored = engine
                        .vector_indexes
                        .get_vector(record.id)
                        .ok_or_else(|| unavailable("eligible similarity vector unavailable"))?;
                    if stored.len() != vector.len() || stored.iter().any(|v| !v.is_finite()) {
                        return Err(unavailable("invalid eligible similarity vector"));
                    }
                    let score = super::super::simd::dot_product_simd(&vector, &stored);
                    if !score.is_finite() {
                        return Err(unavailable("nonfinite similarity score"));
                    }
                    let score = if score == 0. { 0. } else { score };
                    coverage.scored_records += 1;
                    if options.min_score.is_some_and(|min| score < min) {
                        continue;
                    }
                    coverage.score_admitted += 1;
                    let (resolved, status) = match options.semantics {
                        RetrievalSemantics::V1 => (record.clone(), None),
                        RetrievalSemantics::V2(mode) => {
                            let resolution = engine
                                .resolve_path(
                                    record.clone(),
                                    ns.id,
                                    &mut diagnostics,
                                    Some(&context),
                                )
                                .await?;
                            match (mode, resolution) {
                                (_, Resolution::Current(r)) => (r, Some(VersionStatus::Current)),
                                (VersionMode::History, Resolution::Resolved { .. }) => {
                                    (record.clone(), Some(VersionStatus::Historical))
                                }
                                (VersionMode::History, Resolution::Unresolved(reason)) => {
                                    *diagnostics.unresolved.entry(reason).or_default() += 1;
                                    (record.clone(), Some(VersionStatus::Unresolved(reason)))
                                }
                                (VersionMode::Current, Resolution::Resolved { terminal, .. }) => {
                                    (terminal, Some(VersionStatus::Current))
                                }
                                (VersionMode::Current, Resolution::Unresolved(reason)) => {
                                    *diagnostics.unresolved.entry(reason).or_default() += 1;
                                    continue;
                                }
                            }
                        }
                    };
                    if resolved.id == id || !Self::passes_hard_filters(&resolved, &query, ns.id) {
                        *diagnostics
                            .unresolved
                            .entry(UnresolvedReason::Ineligible)
                            .or_default() += 1;
                        continue;
                    }
                    semantics::validate_record(&resolved)?;
                    let target = resolved.id;
                    if let Some(status) = status {
                        diagnostics.versions.insert(target, status);
                    }
                    let (candidate, _) = candidates.entry(target).or_insert_with(|| {
                        let mut c = Candidate::new(resolved);
                        c.composite_score = f32::NEG_INFINITY;
                        c.effective_r = Self::compute_effective_r(&c.record);
                        (c, ns.clone())
                    });
                    candidate.composite_score = candidate.composite_score.max(score);
                    if target == record.id {
                        candidate.raw_vector_score = Some(score);
                        candidate
                            .provenance
                            .own_channels
                            .insert(RetrievalChannel::Vector);
                        candidate.provenance.own_vector_score = Some(score);
                        candidate.provenance.own_score = Some(score);
                    } else {
                        candidate
                            .provenance
                            .inherited_channels
                            .insert(RetrievalChannel::Vector);
                        candidate.provenance.inherited_floor = Some(
                            candidate
                                .provenance
                                .inherited_floor
                                .map_or(score, |s| s.max(score)),
                        );
                    }
                    candidate.provenance.final_score = candidate.composite_score;
                }
            }
            let mut candidates: Vec<_> = candidates.into_values().collect();
            candidates.sort_by(|(a, _), (b, _)| {
                b.composite_score
                    .total_cmp(&a.composite_score)
                    .then(a.record.id.cmp(&b.record.id))
            });
            coverage.resolved_candidates = candidates.len();
            coverage.result_limit_applied = candidates.len() > options.limit;
            candidates.truncate(options.limit);
            let ids: HashSet<_> = candidates.iter().map(|(c, _)| c.record.id).collect();
            diagnostics.versions.retain(|id, _| ids.contains(id));
            if let Some(provenance) = &mut diagnostics.provenance {
                *provenance = candidates
                    .iter()
                    .map(|(c, _)| (c.record.id, c.provenance.clone()))
                    .collect();
            }
            diagnostics.result_limit_applied = coverage.result_limit_applied;
            if let Some(channels) = &mut diagnostics.channels {
                let vector = channels
                    .get_mut(&RetrievalChannel::Vector)
                    .expect("vector channel");
                vector.participation = ChannelParticipation::Completed;
                vector.examined = Some(coverage.scored_records);
                vector.eligible = Some(coverage.eligible_records);
                vector.offered = Some(coverage.score_admitted);
                vector.admitted = Some(coverage.score_admitted);
                vector.exhausted = Some(true);
                vector.candidate_limited = Some(false);
                vector.budget_exhausted = Some(false);
                vector.returned_own = candidates
                    .iter()
                    .filter(|(c, _)| {
                        c.provenance
                            .own_channels
                            .contains(&RetrievalChannel::Vector)
                    })
                    .count();
                vector.returned_inherited = candidates
                    .iter()
                    .filter(|(c, _)| {
                        c.provenance
                            .inherited_channels
                            .contains(&RetrievalChannel::Vector)
                    })
                    .count();
            }
            let exposure = crate::decay::similarity_scope::SimilarityExposure::selected(
                canonical.at,
                id,
                &source_ns,
                profile,
                candidates.iter().map(|(c, ns)| (c.record.id, ns.clone())),
            );
            let at = canonical.at;
            let storage = owner.storage.clone();
            let include_embeddings = options.include_embeddings;
            let child = work.child().map_err(unavailable)?;
            let rows = tokio::task::spawn_blocking(move || {
                let storage = storage.read().map_err(unavailable)?;
                let result = (|| {
                    let mut rows = Vec::new();
                    for (candidate, namespace) in candidates {
                        let mut payload = storage
                            .hydrate_at(
                                at,
                                namespace.id,
                                &[candidate.record.id],
                                include_embeddings,
                            )
                            .map_err(unavailable)?;
                        if payload.len() != 1 {
                            return Err(unavailable("similarity hydration missing"));
                        }
                        let mut result =
                            Self::build_results(&[candidate], &namespace.name).remove(0);
                        let payload = payload.remove(0);
                        result.full_text = payload.full_text.clone();
                        rows.push(SelectedSimilarRow {
                            result,
                            namespace,
                            payload,
                        });
                    }
                    Ok::<_, SearchError>(rows)
                })();
                storage.meta_store().finish_work(child);
                result
            })
            .await
            .map_err(unavailable)??;
            Ok(SelectedSimilarity {
                source_namespace: source_ns.clone(),
                read_generation: at,
                evaluated_at,
                rows,
                coverage,
                diagnostics,
                requested_access: options.record_access,
                original_contract_version: options.original_contract_version,
                exposure,
            })
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(30), result).await;
        owner.finish_known_work(work);
        result.map_err(|_| unavailable("similarity selection deadline expired"))?
    }
}
