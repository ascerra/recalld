//! Same-insertion native linking. This is an internal Rust intent, not a serving
//! activation or an extension of the historical two-edge insertion proof.
use serde::{Deserialize, Serialize};

use super::control::GenerationRef;
use super::native_records::{NativeInsert, NativeInsertIntent};
use crate::model::{EdgeType, MemoryId};

#[derive(Debug, Clone)]
pub struct NativeStoreInput {
    pub insert: NativeInsert,
    /// Caller/default expansion is part of the new operation's identity.
    pub check_duplicates: bool,
}

/// Only settings consumed by link selection. Pin before provider work; matching
/// retries use the original receipt's expansion, not mutable current defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeStoreSettings {
    pub autolink_enabled: bool,
    /// Original f64 bits, before the producer's f32 comparison conversion.
    pub auto_link_threshold_bits: u64,
    pub max_auto_links: u32,
    pub max_entity_links: u32,
    pub temporal_window_ms: u64,
    pub max_temporal_links: u32,
}

#[derive(Debug, Clone)]
pub struct NativeStoreIntent {
    pub(crate) insert: NativeInsertIntent,
    pub(crate) check_duplicates: bool,
    pub(crate) settings: NativeStoreSettings,
}
impl NativeStoreIntent {
    pub fn insert_intent(&self) -> &NativeInsertIntent {
        &self.insert
    }
    pub fn settings(&self) -> &NativeStoreSettings {
        &self.settings
    }
}

/// Producer-computed, ephemeral canonical selection. Core checks the capture G,
/// namespace/profile, live peers and temporal creation times before allocation.
/// Completed receipts contain final IDs/numeric outcomes, never source payloads.
#[derive(Debug, Clone)]
pub struct FrozenInsertLinks {
    pub generation: GenerationRef,
    pub profile_id: String,
    /// New record -> existing target; explicit parent/supersedes stay in input.
    pub static_links: Vec<(MemoryId, EdgeType, f32)>,
    /// All eligible temporal candidates, before one core-clock selection.
    pub temporal: Vec<(MemoryId, i64)>,
    /// None means disabled; Some(empty) means requested and checked.
    pub advisory: Option<Vec<(MemoryId, f32)>>,
}

#[derive(Debug, Clone)]
pub struct PreparedNativeStore {
    pub(crate) intent: NativeStoreIntent,
    pub(crate) links: FrozenInsertLinks,
}

use super::StorageError;
use super::control::{Generation, MutationContext, WriteReceipt};
use super::native_lifecycle::invalid;
use super::native_records::{NativeEdgeProof, NativeRecordChange, ProofRecord};
use crate::model::{DecayPhase, NamespaceConfig, NamespaceId, schema::LifecycleVersion};
use std::collections::{BTreeMap, BTreeSet};

impl NativeStoreSettings {
    pub fn validate(&self) -> Result<(), StorageError> {
        let threshold = f64::from_bits(self.auto_link_threshold_bits);
        if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
            return Err(StorageError::InvalidMutation(
                "invalid native link threshold".into(),
            ));
        }
        Ok(())
    }
}
impl NativeStoreInput {
    pub(crate) fn request_hash(
        &self,
        ctx: &MutationContext,
        stability: f32,
        difficulty: f32,
        dimensions: u32,
        profile: &str,
        settings: &NativeStoreSettings,
    ) -> Result<[u8; 32], StorageError> {
        let original = self
            .insert
            .request_hash(ctx, stability, difficulty, dimensions, profile)?;
        let expansion = super::control::encode(&(self.check_duplicates, settings))?;
        Ok(super::control::request_hash(
            ctx,
            "memory_insert_with_links_v2",
            &[&original, &expansion],
        ))
    }
}

/// Shared deterministic selector. Caller time wins; otherwise the core's one
/// sample is both the creation anchor and the edge publication timestamp.
pub fn select_temporal_links(
    candidates: &[(MemoryId, i64)],
    created_at: i64,
    settings: &NativeStoreSettings,
) -> Vec<(MemoryId, EdgeType, f32)> {
    if !settings.autolink_enabled || settings.temporal_window_ms == 0 {
        return vec![];
    }
    let mut selected: Vec<_> = candidates
        .iter()
        .filter_map(|&(id, at)| {
            let delta = created_at.abs_diff(at);
            (delta <= settings.temporal_window_ms).then_some((id, delta))
        })
        .collect();
    selected.sort_by_key(|&(id, delta)| (delta, id));
    selected
        .into_iter()
        .take(settings.max_temporal_links as usize)
        .map(|(id, delta)| {
            (
                id,
                EdgeType::Temporal,
                (1. - delta as f64 / settings.temporal_window_ms as f64).max(0.05) as f32,
            )
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeAdvisoryMatch {
    pub memory_id: MemoryId,
    pub score_bits: u32,
}

/// A distinct proof version selected by the additive outcome. Final states are
/// real atomic states: primary token 1, affected peers incremented exactly once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeStoreWrite {
    pub namespace_id: NamespaceId,
    pub memory_id: MemoryId,
    pub lifecycle_version: LifecycleVersion,
    pub policy_revision: Generation,
    pub previous_generation: Generation,
    pub capture_generation: GenerationRef,
    pub sampled_at: i64,
    pub embedding_dim: u32,
    pub records: Vec<NativeRecordChange>,
    pub edges: Vec<NativeEdgeProof>,
    pub parent_id: Option<MemoryId>,
    pub supersedes: Option<MemoryId>,
    pub settings: NativeStoreSettings,
    pub check_duplicates: bool,
    pub advisory: Option<Vec<NativeAdvisoryMatch>>,
}
impl NativeStoreWrite {
    pub(crate) fn primary(&self) -> Result<&NativeRecordChange, StorageError> {
        self.records
            .iter()
            .find(|r| r.memory_id == self.memory_id)
            .ok_or_else(|| invalid("linked creation primary missing"))
    }
    pub fn advisory_matches(&self) -> Option<Vec<(MemoryId, f32)>> {
        self.advisory.as_ref().map(|a| {
            a.iter()
                .map(|r| (r.memory_id, f32::from_bits(r.score_bits)))
                .collect()
        })
    }
    pub(crate) fn validate_namespace_profiles(
        &self,
        namespace: &NamespaceConfig,
    ) -> Result<(), StorageError> {
        if namespace.id != self.namespace_id {
            return Err(invalid("linked creation namespace mismatch"));
        }
        for r in &self.records {
            if let Some(before) = &r.before {
                before.content.validate_namespace_profile(namespace)?;
            }
            r.after.content.validate_namespace_profile(namespace)?;
        }
        Ok(())
    }
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        self.settings
            .validate()
            .map_err(|_| invalid("invalid linked creation settings"))?;
        if self.namespace_id.is_unset()
            || self.memory_id == MemoryId::nil()
            || self.capture_generation.store_id != receipt.store_id
            || self.capture_generation.generation != self.previous_generation
            || self.previous_generation.next()? != receipt.committed_generation
            || !(1..=8192).contains(&self.embedding_dim)
            || chrono::DateTime::from_timestamp_millis(self.sampled_at).is_none()
            || self.edges.len() > super::native_graph::MAX_NATIVE_GRAPH_EDGES
            || self.check_duplicates != self.advisory.is_some()
        {
            return Err(invalid("invalid linked creation boundary"));
        }
        let primary = self.primary()?;
        super::native_records::validate_creation_primary(
            primary,
            self.sampled_at,
            receipt.committed_generation,
        )?;
        let mut required = BTreeSet::from([self.memory_id]);
        let mut keys = BTreeSet::new();
        let mut counts = BTreeMap::<MemoryId, u16>::new();
        let mut kinds = BTreeMap::<u8, usize>::new();
        for edge in &self.edges {
            let weight = f32::from_bits(edge.weight_bits);
            if !keys.insert((edge.source, edge.edge_type.as_u8(), edge.target))
                || edge.source == edge.target
                || !weight.is_finite()
                || !(0.0..=1.0).contains(&weight)
                || edge.created_at != self.sampled_at.max(0) as u64
            {
                return Err(invalid("invalid linked creation edge"));
            }
            match edge.edge_type {
                EdgeType::ParentChild
                    if self.parent_id == Some(edge.source)
                        && edge.target == self.memory_id
                        && !edge.auto_created
                        && weight.to_bits() == 1_f32.to_bits() => {}
                EdgeType::Supersedes
                    if self.supersedes == Some(edge.target)
                        && edge.source == self.memory_id
                        && !edge.auto_created
                        && weight.to_bits() == 1_f32.to_bits() => {}
                EdgeType::Associative | EdgeType::Entity | EdgeType::Temporal
                    if edge.source == self.memory_id
                        && edge.auto_created
                        && self.settings.autolink_enabled =>
                {
                    *kinds.entry(edge.edge_type.as_u8()).or_default() += 1;
                    let target = self
                        .records
                        .iter()
                        .find(|r| r.memory_id == edge.target)
                        .ok_or_else(|| invalid("linked creation target missing"))?;
                    if !matches!(
                        target.after.lifecycle.phase,
                        DecayPhase::Full | DecayPhase::Summary
                    ) {
                        return Err(invalid("linked creation target not live"));
                    }
                    if edge.edge_type == EdgeType::Temporal {
                        let expected = select_temporal_links(
                            &[(edge.target, target.after.lifecycle.created_at)],
                            primary.after.lifecycle.created_at,
                            &self.settings,
                        );
                        if expected
                            .first()
                            .is_none_or(|e| e.2.to_bits() != edge.weight_bits)
                        {
                            return Err(invalid("linked creation temporal weight differs"));
                        }
                    }
                }
                _ => return Err(invalid("unsupported linked creation edge")),
            }
            required.extend([edge.source, edge.target]);
            *counts.entry(edge.source).or_default() += 1;
        }
        if self.parent_id.is_some()
            != self
                .edges
                .iter()
                .any(|e| e.edge_type == EdgeType::ParentChild)
            || self.supersedes.is_some()
                != self
                    .edges
                    .iter()
                    .any(|e| e.edge_type == EdgeType::Supersedes)
            || kinds
                .get(&EdgeType::Associative.as_u8())
                .copied()
                .unwrap_or(0)
                > self.settings.max_auto_links as usize
            || kinds.get(&EdgeType::Entity.as_u8()).copied().unwrap_or(0)
                > self.settings.max_entity_links as usize
            || kinds.get(&EdgeType::Temporal.as_u8()).copied().unwrap_or(0)
                > self.settings.max_temporal_links as usize
        {
            return Err(invalid("linked creation edge allowance mismatch"));
        }
        if let Some(advisory) = &self.advisory {
            if advisory.len() > crate::model::constants::MAX_NEAR_DUPLICATE_MATCHES {
                return Err(invalid("linked advisory limit"));
            }
            let mut seen = BTreeSet::new();
            let mut previous: Option<(MemoryId, f32)> = None;
            for m in advisory {
                let score = f32::from_bits(m.score_bits);
                if !seen.insert(m.memory_id)
                    || m.memory_id == self.memory_id
                    || !score.is_finite()
                    || !(crate::model::constants::DUPLICATE_SIMILARITY_THRESHOLD as f32..=1.)
                        .contains(&score)
                    || previous.is_some_and(|(id, s)| {
                        s.total_cmp(&score).is_lt()
                            || (s.total_cmp(&score).is_eq() && id >= m.memory_id)
                    })
                {
                    return Err(invalid("invalid linked advisory outcome"));
                }
                let r = self
                    .records
                    .iter()
                    .find(|r| r.memory_id == m.memory_id)
                    .ok_or_else(|| invalid("advisory peer proof missing"))?;
                if !matches!(
                    r.after.lifecycle.phase,
                    DecayPhase::Full | DecayPhase::Summary
                ) {
                    return Err(invalid("advisory peer not live"));
                }
                required.insert(m.memory_id);
                previous = Some((m.memory_id, score));
            }
        }
        let mut ids = BTreeSet::new();
        for change in &self.records {
            if !ids.insert(change.memory_id)
                || !required.contains(&change.memory_id)
                || change.memory_id == MemoryId::nil()
            {
                return Err(invalid("invalid linked creation peer identity"));
            }
            let after = &change.after;
            after.lifecycle.validate()?;
            after.content.validate_profile_commitment()?;
            if after.lifecycle.lifecycle.lifecycle_version != self.lifecycle_version
                || after.lifecycle.lifecycle.policy_revision != self.policy_revision
            {
                return Err(invalid("linked creation peer policy mismatch"));
            }
            if change.memory_id == self.memory_id {
                if after.edge_count != counts.get(&self.memory_id).copied().unwrap_or(0) {
                    return Err(invalid("linked creation outgoing count mismatch"));
                }
            } else {
                let before = change
                    .before
                    .as_ref()
                    .ok_or_else(|| invalid("linked creation prior peer missing"))?;
                before.lifecycle.validate()?;
                before.content.validate_profile_commitment()?;
                let mut expected = before.clone();
                if let Some(count) = counts.get(&change.memory_id) {
                    expected.edge_count = expected
                        .edge_count
                        .checked_add(*count)
                        .ok_or_else(|| invalid("linked peer count overflow"))?;
                    expected.lifecycle.lifecycle.record_generation =
                        expected.lifecycle.lifecycle.record_generation.next()?;
                }
                if after != &expected {
                    return Err(invalid("linked creation peer transition mismatch"));
                }
            }
        }
        if ids != required {
            return Err(invalid("linked creation incomplete peer proof"));
        }
        Ok(())
    }
    pub(crate) fn rewind(&self, rows: &mut Vec<ProofRecord>) -> Result<(), StorageError> {
        for change in &self.records {
            let index = rows
                .iter()
                .position(|r| r.id == change.memory_id)
                .ok_or_else(|| invalid("linked creation retained peer absent"))?;
            if rows[index].namespace != self.namespace_id.get() || rows[index].state != change.after
            {
                return Err(invalid("linked creation retained state mismatch"));
            }
            if let Some(before) = &change.before {
                rows[index].state = before.clone();
            } else {
                rows.remove(index);
            }
        }
        Ok(())
    }
}

impl super::native_lifecycle::NativeLifecycleSession {
    pub fn lookup_store(
        &self,
        ctx: &MutationContext,
        input: &NativeStoreInput,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        self.metadata().lookup_native_store(ctx, input, None)
    }
    pub fn capture_store(
        &self,
        scope: super::native_lifecycle::LifecycleScope,
        input: &NativeStoreInput,
        settings: &NativeStoreSettings,
    ) -> Result<NativeStoreIntent, StorageError> {
        self.metadata()
            .capture_native_store(scope, input, settings, &self.policy)
    }
    pub fn prepare_store_links(
        &self,
        intent: &NativeStoreIntent,
        links: &FrozenInsertLinks,
    ) -> Result<PreparedNativeStore, StorageError> {
        self.metadata()
            .prepare_native_store_links(intent, links, &self.policy)
    }
    pub fn store_prepared(
        &mut self,
        ctx: &MutationContext,
        prepared: &PreparedNativeStore,
        generated: &crate::embedding::runtime::GeneratedEmbedding,
    ) -> Result<WriteReceipt, StorageError> {
        self.engine.store_native_prepared(
            ctx,
            prepared,
            generated,
            &self.policy,
            self.clock.as_ref(),
            None,
        )
    }
}
