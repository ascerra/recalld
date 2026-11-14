//! Canonical, deterministic candidates for the future same-insert native plan.
//! No derived cache/index is trusted for eligibility or payload. These values
//! are ephemeral; only the core's new additive proof may persist final outcomes.
use crate::config::GraphConfig;
use crate::embedding::runtime::GeneratedEmbedding;
use crate::model::{DecayPhase, EdgeType, MemoryId, NamespaceId};
use crate::search::{canonicalize_entity, dot_product_simd};
use crate::storage::control::{Generation, GenerationRef};
use crate::storage::native_records::NativeInsertIntent;
use crate::storage::{RedbStorageEngine, StorageEngine, StorageError};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub(crate) struct NativeCandidate {
    pub id: MemoryId,
    pub record_generation: Generation,
    pub created_at: i64,
    pub similarity: f32,
    pub shared_tags: bool,
    pub shared_entities: usize,
    pub entity_union: usize,
}
#[derive(Debug, Clone)]
pub(crate) struct NativeCandidates {
    pub generation: GenerationRef,
    pub namespace_id: NamespaceId,
    pub profile_id: String,
    pub candidates: Vec<NativeCandidate>,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SelectedNativeLink {
    pub target: MemoryId,
    pub edge_type: EdgeType,
    pub weight: f32,
}

fn keys(values: &[String]) -> BTreeSet<String> {
    values.iter().flat_map(|v| canonicalize_entity(v)).collect()
}
fn conflict(message: &str) -> StorageError {
    StorageError::LifecycleConflict(message.into())
}

/// Caller holds engine ownership while copying canonical vectors/records. The
/// generation must also be checked by the final same-insert transaction, which
/// catches newly inserted candidates as well as changed existing endpoints.
/// ponytail: exact O(namespace * dimension) scan; optimize only with a verified
/// same-generation canonical/derived view, never an unversioned cache shortcut.
pub(crate) fn capture(
    engine: &RedbStorageEngine,
    intent: &NativeInsertIntent,
    generated: &GeneratedEmbedding,
) -> Result<NativeCandidates, StorageError> {
    engine.ensure_available()?;
    let generation = engine.control_state()?.generation_ref();
    if generation != intent.capture_generation() {
        return Err(conflict("native candidates changed after insert capture"));
    }
    let input = &intent.original;
    let profile = input
        .embedding_profile
        .as_ref()
        .or(generated.profile.as_ref())
        .ok_or_else(|| StorageError::InvalidMutation("native candidate profile missing".into()))?;
    let profile_id = crate::embedding::profile::require_compatible(&intent.namespace, profile)
        .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
    let mut query = input
        .memory
        .embedding
        .as_deref()
        .unwrap_or(&generated.values)
        .to_vec();
    crate::storage::insertion::normalize_embedding(
        &mut query,
        intent.namespace.embedding_dim,
        input.memory.embedding.is_some(),
    )?;
    let namespace_id = intent.scope.namespace_id;
    let new_entities = keys(&input.entities);
    let mut candidates = Vec::new();
    for record in engine.meta_store().records_in_namespace(namespace_id)? {
        if !matches!(record.phase, DecayPhase::Full | DecayPhase::Summary) {
            continue;
        }
        let id = MemoryId::from_bytes(record.id);
        if input.memory.id == Some(id) {
            continue;
        }
        let schema = record.schema.as_ref().ok_or_else(|| {
            StorageError::InvalidControl("native candidate schema missing".into())
        })?;
        let reference = schema.vector.as_ref().ok_or_else(|| {
            StorageError::OwnershipMismatch("native candidate vector missing".into())
        })?;
        if reference.slot != record.vector_slot {
            return Err(StorageError::OwnershipMismatch(
                "native candidate vector slot mismatch".into(),
            ));
        }
        let vector = engine
            .get_vector(namespace_id, record.vector_slot)?
            .ok_or_else(|| {
                StorageError::OwnershipMismatch("native candidate vector unavailable".into())
            })?;
        if vector.len() != query.len() {
            return Err(StorageError::OwnershipMismatch(
                "native candidate vector dimension mismatch".into(),
            ));
        }
        crate::storage::canonical_schema::validate_bound_vector(&vector)?;
        let mut hash = Sha256::new();
        for value in &vector {
            hash.update(value.to_le_bytes());
        }
        if <[u8; 32]>::from(hash.finalize()) != reference.sha256 {
            return Err(StorageError::OwnershipMismatch(
                "native candidate vector hash mismatch".into(),
            ));
        }
        let candidate_entities = keys(&schema.labels.entities);
        candidates.push(NativeCandidate {
            id,
            record_generation: schema.lifecycle.record_generation,
            created_at: record.created_at,
            // Bound vectors can round a dot product just outside the cosine range.
            similarity: dot_product_simd(&query, &vector).clamp(-1., 1.),
            shared_tags: input
                .memory
                .tags
                .iter()
                .any(|tag| record.tags.contains(tag)),
            shared_entities: new_entities.intersection(&candidate_entities).count(),
            entity_union: new_entities.union(&candidate_entities).count(),
        });
    }
    engine.ensure_available()?;
    if engine.control_state()?.generation_ref() != generation {
        return Err(conflict("native candidates changed during capture"));
    }
    candidates.sort_by_key(|c| c.id);
    Ok(NativeCandidates {
        generation,
        namespace_id,
        profile_id,
        candidates,
    })
}

impl NativeCandidates {
    /// Selected semantic/entity links; temporal selection waits for the core's
    /// one final sampled clock. No UUID or clock allocation occurs here.
    pub(crate) fn static_links(
        &self,
        settings: &GraphConfig,
    ) -> Result<Vec<SelectedNativeLink>, StorageError> {
        if !settings.auto_link_threshold.is_finite()
            || !(0.0..=1.0).contains(&settings.auto_link_threshold)
        {
            return Err(StorageError::InvalidMutation(
                "invalid native autolink threshold".into(),
            ));
        }
        if !settings.autolink_enabled {
            return Ok(vec![]);
        }
        let floor = super::autolink::THRESHOLD_HARD_FLOOR;
        let threshold = (settings.auto_link_threshold as f32).max(floor);
        let mut semantic: Vec<_> = self
            .candidates
            .iter()
            .filter(|c| {
                c.similarity
                    >= if c.shared_tags {
                        (threshold - super::autolink::TAG_THRESHOLD_ADJUSTMENT).max(floor)
                    } else {
                        threshold
                    }
            })
            .collect();
        semantic.sort_by(|a, b| b.similarity.total_cmp(&a.similarity).then(a.id.cmp(&b.id)));
        let mut links: Vec<_> = semantic
            .into_iter()
            .take(settings.max_auto_links)
            .map(|c| SelectedNativeLink {
                target: c.id,
                edge_type: EdgeType::Associative,
                weight: c.similarity,
            })
            .collect();
        let mut entity: Vec<_> = self
            .candidates
            .iter()
            .filter(|c| c.shared_entities > 0)
            .collect();
        entity.sort_by(|a, b| {
            b.shared_entities
                .cmp(&a.shared_entities)
                .then(a.id.cmp(&b.id))
        });
        links.extend(entity.into_iter().take(settings.max_entity_links).map(|c| {
            SelectedNativeLink {
                target: c.id,
                edge_type: EdgeType::Entity,
                weight: c.shared_entities as f32 / c.entity_union as f32,
            }
        }));
        Ok(links)
    }

    /// Use final newrecord.created_at: preserve a supplied caller time, otherwise
    /// use the core's single sampled clock. Edge timestamps still use that clock.
    /// abs_diff avoids signed overflow at i64 extremes; ties are UUID-stable.
    pub(crate) fn temporal_links(
        &self,
        created_at: i64,
        settings: &GraphConfig,
    ) -> Vec<SelectedNativeLink> {
        if !settings.autolink_enabled || settings.temporal_window_ms == 0 {
            return vec![];
        }
        let mut candidates: Vec<_> = self
            .candidates
            .iter()
            .filter_map(|c| {
                let delta = created_at.abs_diff(c.created_at);
                (delta <= settings.temporal_window_ms).then_some((c.id, delta))
            })
            .collect();
        candidates.sort_by_key(|&(id, delta)| (delta, id));
        candidates
            .into_iter()
            .take(settings.max_temporal_links)
            .map(|(target, delta)| SelectedNativeLink {
                target,
                edge_type: EdgeType::Temporal,
                weight: (1. - delta as f64 / settings.temporal_window_ms as f64).max(0.05) as f32,
            })
            .collect()
    }

    /// Numeric/ID-only selection; hydration of current visible summaries belongs
    /// to the response path. Disabled and checked-empty remain distinguishable.
    pub(crate) fn advisory(&self, requested: bool) -> Option<Vec<(MemoryId, f32)>> {
        if !requested {
            return None;
        }
        let mut matches: Vec<_> = self
            .candidates
            .iter()
            .filter(|c| {
                c.similarity >= crate::model::constants::DUPLICATE_SIMILARITY_THRESHOLD as f32
            })
            .map(|c| (c.id, c.similarity))
            .collect();
        matches.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        matches.truncate(crate::model::constants::MAX_NEAR_DUPLICATE_MATCHES);
        Some(matches)
    }
}
