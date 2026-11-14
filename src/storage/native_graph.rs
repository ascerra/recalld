//! Native standalone edge puts. Store autolinking needs a separate durable
//! insertion plan; these receipts do not make insert-then-link atomic.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::control::{Generation, GenerationRef, MutationContext, WriteReceipt};
use super::native_lifecycle::{CapturedLifecycle, LifecycleScope, invalid};
use super::native_records::{NativeEdgeProof, NativeRecordChange, ProofRecord};
use super::{PersistedEdge, StorageError};
use crate::model::schema::LifecycleVersion;
use crate::model::{DecayPhase, EdgeType, MemoryId, NamespaceId};

/// Explicit internal transaction limits, not retrieval scan limits.
pub const MAX_NATIVE_GRAPH_EDGES: usize = 1024;
pub const MAX_NATIVE_GRAPH_ENDPOINTS: usize = 2 * MAX_NATIVE_GRAPH_EDGES;

#[derive(Debug, Clone)]
pub struct NativeEdgeInput {
    pub source: MemoryId,
    pub target: MemoryId,
    pub edge_type: EdgeType,
    pub weight: f32,
    pub auto_created: bool,
}
#[derive(Debug, Clone)]
pub struct NativeGraphInput {
    /// Ordered caller intent. Generated timestamps are deliberately absent.
    pub edges: Vec<NativeEdgeInput>,
}
pub(crate) type EdgeKey = (MemoryId, u8, MemoryId);
pub(crate) fn edge_key(e: &NativeEdgeProof) -> EdgeKey {
    (e.source, e.edge_type.as_u8(), e.target)
}
impl NativeGraphInput {
    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        if self.edges.len() > MAX_NATIVE_GRAPH_EDGES {
            return Err(StorageError::InvalidMutation(
                "native graph input exceeds 1024 edges".into(),
            ));
        }
        let mut keys = BTreeSet::new();
        for e in &self.edges {
            if e.source == MemoryId::nil()
                || e.target == MemoryId::nil()
                || e.source == e.target
                || !e.weight.is_finite()
                || !(0.0..=1.0).contains(&e.weight)
                || !keys.insert((e.source, e.edge_type.as_u8(), e.target))
            {
                return Err(StorageError::InvalidMutation(
                    "invalid or duplicate native edge input".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn request_hash(&self, ctx: &MutationContext) -> Result<[u8; 32], StorageError> {
        self.validate()?;
        let ordered: Vec<_> = self
            .edges
            .iter()
            .map(|e| {
                (
                    e.source,
                    e.target,
                    e.edge_type,
                    e.weight.to_bits(),
                    e.auto_created,
                )
            })
            .collect();
        Ok(super::control::request_hash(
            ctx,
            "native_graph_put_v1",
            &[&super::control::encode(&ordered)?],
        ))
    }
}
#[derive(Debug, Clone)]
pub struct NativeGraphIntent {
    pub(crate) scope: LifecycleScope,
    pub(crate) original: NativeGraphInput,
    pub(crate) capture: CapturedLifecycle,
}
impl NativeGraphIntent {
    pub fn capture_generation(&self) -> GenerationRef {
        self.capture.generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeEdgeChange {
    pub before: Option<NativeEdgeProof>,
    pub after: NativeEdgeProof,
}
/// Additive receipt type. Historical NativeRecordWrite retains its <=2-edge
/// creation semantics. No payload strings or embeddings are archived here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeGraphWrite {
    pub namespace_id: NamespaceId,
    pub lifecycle_version: LifecycleVersion,
    pub policy_revision: Generation,
    pub capture_generation: GenerationRef,
    pub previous_generation: Generation,
    pub sampled_at: i64,
    pub edges: Vec<NativeEdgeChange>,
    pub records: Vec<NativeRecordChange>,
}

/// A keyed successful no-op binds intent without changing any record or edge.
/// Edges are witnesses, never puts; the empty request has empty witnesses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeGraphNoChange {
    pub namespace_id: NamespaceId,
    pub lifecycle_version: LifecycleVersion,
    pub policy_revision: Generation,
    pub capture_generation: GenerationRef,
    pub previous_generation: Generation,
    pub edges: Vec<NativeEdgeProof>,
    pub records: Vec<NativeRecordChange>,
}
impl NativeGraphNoChange {
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        if self.namespace_id.is_unset()
            || self.edges.len() > MAX_NATIVE_GRAPH_EDGES
            || self.records.len() > MAX_NATIVE_GRAPH_ENDPOINTS
            || self.capture_generation.store_id != receipt.store_id
            || self.capture_generation.generation > self.previous_generation
            || self.previous_generation.next()? != receipt.committed_generation
        {
            return Err(invalid("invalid native graph no-change boundary"));
        }
        let mut keys = BTreeSet::new();
        let mut endpoints = BTreeSet::new();
        for e in &self.edges {
            if !keys.insert(edge_key(e))
                || e.source == e.target
                || e.source == MemoryId::nil()
                || e.target == MemoryId::nil()
                || !f32::from_bits(e.weight_bits).is_finite()
                || !(0.0..=1.0).contains(&f32::from_bits(e.weight_bits))
            {
                return Err(invalid("invalid native graph no-change edge"));
            }
            endpoints.extend([e.source, e.target]);
        }
        let mut ids = BTreeSet::new();
        for change in &self.records {
            if !ids.insert(change.memory_id)
                || !endpoints.contains(&change.memory_id)
                || change.before.as_ref() != Some(&change.after)
            {
                return Err(invalid("native graph no-change peer changed"));
            }
            let state = &change.after;
            state.lifecycle.validate()?;
            state.content.validate_profile_commitment()?;
            if state.lifecycle.lifecycle.lifecycle_version != self.lifecycle_version
                || state.lifecycle.lifecycle.policy_revision != self.policy_revision
            {
                return Err(invalid("native graph no-change peer policy mismatch"));
            }
            for e in &self.edges {
                for (id, source) in [(e.source, true), (e.target, false)] {
                    if id == change.memory_id
                        && !endpoint_allowed(e.edge_type, source, state.lifecycle.phase)
                    {
                        return Err(invalid("native graph no-change endpoint unavailable"));
                    }
                }
            }
        }
        if ids != endpoints {
            return Err(invalid("native graph no-change endpoint missing"));
        }
        Ok(())
    }
    pub(crate) fn validate_namespace_profiles(
        &self,
        namespace: &crate::model::NamespaceConfig,
    ) -> Result<(), StorageError> {
        if namespace.id != self.namespace_id {
            return Err(invalid("native graph no-change namespace mismatch"));
        }
        for change in &self.records {
            change.after.content.validate_namespace_profile(namespace)?;
        }
        Ok(())
    }
    pub(crate) fn rewind(&self, rows: &mut [ProofRecord]) -> Result<(), StorageError> {
        for change in &self.records {
            if !rows.iter().any(|row| {
                row.id == change.memory_id
                    && row.namespace == self.namespace_id.get()
                    && row.state == change.after
            }) {
                return Err(invalid("native graph no-change differs from retained peer"));
            }
        }
        Ok(())
    }
}

pub(crate) fn endpoint_allowed(kind: EdgeType, source: bool, phase: DecayPhase) -> bool {
    matches!(phase, DecayPhase::Full | DecayPhase::Summary)
        || (kind == EdgeType::ParentChild && source)
        || (kind == EdgeType::Supersedes && !source)
}
impl NativeGraphWrite {
    pub(crate) fn validate_namespace_profiles(
        &self,
        namespace: &crate::model::NamespaceConfig,
    ) -> Result<(), StorageError> {
        if namespace.id != self.namespace_id {
            return Err(invalid("native graph proof namespace mismatch"));
        }
        for change in &self.records {
            change
                .before
                .as_ref()
                .ok_or_else(|| invalid("native graph prior peer missing"))?
                .content
                .validate_namespace_profile(namespace)?;
            change.after.content.validate_namespace_profile(namespace)?;
        }
        Ok(())
    }
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        if self.namespace_id.is_unset()
            || self.edges.is_empty()
            || self.edges.len() > MAX_NATIVE_GRAPH_EDGES
            || self.records.len() > MAX_NATIVE_GRAPH_ENDPOINTS
            || self.capture_generation.store_id != receipt.store_id
            || self.capture_generation.generation > self.previous_generation
            || self.previous_generation.next()? != receipt.committed_generation
            || chrono::DateTime::from_timestamp_millis(self.sampled_at).is_none()
        {
            return Err(invalid("invalid native graph receipt boundary"));
        }
        let mut keys = BTreeSet::new();
        let mut endpoints = BTreeSet::new();
        let mut increments = BTreeMap::<MemoryId, u16>::new();
        for change in &self.edges {
            let e = &change.after;
            if !keys.insert(edge_key(e))
                || e.source == e.target
                || e.source == MemoryId::nil()
                || e.target == MemoryId::nil()
                || !f32::from_bits(e.weight_bits).is_finite()
                || !(0.0..=1.0).contains(&f32::from_bits(e.weight_bits))
                || e.created_at != self.sampled_at.max(0) as u64
            {
                return Err(invalid("invalid native graph edge proof"));
            }
            if let Some(before) = &change.before {
                if edge_key(before) != edge_key(e)
                    || !f32::from_bits(before.weight_bits).is_finite()
                    || !(0.0..=1.0).contains(&f32::from_bits(before.weight_bits))
                    || (before.weight_bits == e.weight_bits
                        && before.auto_created == e.auto_created)
                {
                    return Err(invalid("invalid native graph overwrite proof"));
                }
            }
            *increments.entry(e.source).or_default() += u16::from(change.before.is_none());
            endpoints.extend([e.source, e.target]);
        }
        let mut ids = BTreeSet::new();
        for change in &self.records {
            if !ids.insert(change.memory_id) || !endpoints.contains(&change.memory_id) {
                return Err(invalid("invalid native graph peer identity"));
            }
            let before = change
                .before
                .as_ref()
                .ok_or_else(|| invalid("native graph prior peer missing"))?;
            before.lifecycle.validate()?;
            before.content.validate_profile_commitment()?;
            change.after.content.validate_profile_commitment()?;
            let fields = &before.lifecycle.lifecycle;
            if fields.lifecycle_version != self.lifecycle_version
                || fields.policy_revision != self.policy_revision
            {
                return Err(invalid("native graph peer policy mismatch"));
            }
            let mut expected = before.clone();
            if let Some(added) = increments.get(&change.memory_id) {
                expected.edge_count = expected
                    .edge_count
                    .checked_add(*added)
                    .ok_or_else(|| invalid("native graph peer count overflow"))?;
                expected.lifecycle.lifecycle.record_generation = fields.record_generation.next()?;
            }
            if change.after != expected {
                return Err(invalid("native graph peer transition mismatch"));
            }
            for edge in &self.edges {
                for (id, source) in [(edge.after.source, true), (edge.after.target, false)] {
                    if id == change.memory_id
                        && !endpoint_allowed(edge.after.edge_type, source, before.lifecycle.phase)
                    {
                        return Err(invalid("native graph endpoint is not eligible"));
                    }
                }
            }
        }
        if ids != endpoints {
            return Err(invalid("native graph endpoint proof missing"));
        }
        Ok(())
    }
    pub(crate) fn rewind(&self, rows: &mut [ProofRecord]) -> Result<(), StorageError> {
        for change in &self.records {
            let row = rows
                .iter_mut()
                .find(|r| r.id == change.memory_id)
                .ok_or_else(|| invalid("native graph peer absent"))?;
            if row.namespace != self.namespace_id.get() || row.state != change.after {
                return Err(invalid("native graph proof disagrees with retained peer"));
            }
            row.state = change
                .before
                .clone()
                .ok_or_else(|| invalid("native graph prior peer missing"))?;
        }
        Ok(())
    }
    pub(crate) fn puts(&self) -> Vec<PersistedEdge> {
        self.edges
            .iter()
            .map(|e| PersistedEdge {
                source: e.after.source,
                target: e.after.target,
                edge_type: e.after.edge_type,
                weight: f32::from_bits(e.after.weight_bits),
                auto_created: e.after.auto_created,
                created_at: e.after.created_at,
            })
            .collect()
    }
    pub(crate) fn peer_counts(&self) -> Vec<(MemoryId, u16)> {
        let sources: BTreeSet<_> = self.edges.iter().map(|e| e.after.source).collect();
        let mut counts: Vec<_> = self
            .records
            .iter()
            .filter(|r| sources.contains(&r.memory_id))
            .map(|r| (r.memory_id, r.after.edge_count))
            .collect();
        counts.sort_by_key(|(id, _)| *id);
        counts
    }
}

impl super::native_lifecycle::NativeLifecycleSession {
    pub fn capture_graph(
        &self,
        scope: LifecycleScope,
        input: &NativeGraphInput,
    ) -> Result<NativeGraphIntent, StorageError> {
        self.metadata()
            .capture_native_graph(scope, input, &self.policy, None)
    }
    /// Explicit offline primitive; success does not certify derived publication.
    /// Unkeyed no-ops return None. Keyed no-ops commit a typed receipt and
    /// zero-action outbox, preserving record tokens and edge timestamps.
    pub fn put_graph_prepared(
        &mut self,
        ctx: &MutationContext,
        intent: &NativeGraphIntent,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        self.engine
            .apply_native_graph(ctx, intent, &self.policy, self.clock.as_ref(), None)
            .map(|r| r.map(|r| r.receipt))
    }
    /// Matching replay precedes current scope/policy/endpoint capture and clock.
    pub fn put_graph(
        &mut self,
        ctx: &MutationContext,
        scope: LifecycleScope,
        input: &NativeGraphInput,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let root = self.metadata().work_tracker().begin()?;
        let result = (|| {
            if let Some(receipt) = self
                .metadata()
                .lookup_native_graph(ctx, input, Some(&root))?
            {
                return Ok(Some(receipt));
            }
            let intent =
                self.metadata()
                    .capture_native_graph(scope, input, &self.policy, Some(&root))?;
            self.engine
                .apply_native_graph(ctx, &intent, &self.policy, self.clock.as_ref(), Some(&root))
                .map(|r| r.map(|r| r.receipt))
        })();
        self.metadata().finish_work(root);
        result
    }
}

/// Validate the retained active-origin edge suffix against the actual edge DB.
/// Pending canonical puts may be wholly before/after edges.db application; their
/// exact receipt plan supplies the logical committed view during preflight.
pub(crate) fn validate_edge_history(
    read: &redb::ReadTransaction,
    state: &super::control::ControlState,
    baseline: Generation,
    edges: &[PersistedEdge],
    pending: &[super::metadata::effects::PendingEffects],
) -> Result<(), StorageError> {
    use super::control::MutationOutcome;
    use redb::ReadableTable;
    let mut suffix = BTreeMap::new();
    for row in read.open_table(super::control::RECEIPTS)?.iter()? {
        let (_, value) = row?;
        let receipt = super::control::validate_receipt(value.value())?.receipt;
        if receipt.store_id == state.store_id && receipt.committed_generation > baseline {
            suffix.insert(receipt.committed_generation, receipt);
        }
    }
    // Historical foreign-origin receipts were already ancestry/policy checked;
    // deletion-aware restore may have removed their edges. Never replay them.
    if !suffix.values().any(|r| {
        matches!(
            r.outcome,
            MutationOutcome::NativeGraphChanged { .. }
                | MutationOutcome::NativeMemoryInsertedWithLinks { .. }
                | MutationOutcome::NativeGraphUnchanged { .. }
        )
    }) {
        return Ok(());
    }
    let mut current: BTreeMap<_, _> = edges
        .iter()
        .map(|e| {
            let e = NativeEdgeProof::from(e);
            (edge_key(&e), e)
        })
        .collect();
    for effect in pending.iter().filter(|p| !p.canonical_applied) {
        if let MutationOutcome::NativeGraphChanged { transition } = &effect.receipt.outcome {
            for e in &transition.edges {
                let physical = current.get(&edge_key(&e.after));
                if physical != Some(&e.after) && physical != e.before.as_ref() {
                    return Err(invalid(
                        "pending native graph disagrees with physical before/after edge",
                    ));
                }
            }
        }
        for e in &effect.deletes {
            current.remove(&(e.source, e.edge_type.as_u8(), e.target));
        }
        for e in &effect.puts {
            let e = NativeEdgeProof::from(e);
            current.insert(edge_key(&e), e);
        }
    }
    for receipt in suffix.values().rev() {
        match &receipt.outcome {
            MutationOutcome::NativeGraphUnchanged { transition } => {
                for e in &transition.edges {
                    if current.get(&edge_key(e)) != Some(e) {
                        return Err(invalid("native graph no-change differs from retained edge"));
                    }
                }
            }
            MutationOutcome::NativeGraphChanged { transition } => {
                for e in &transition.edges {
                    let key = edge_key(&e.after);
                    if current.get(&key) != Some(&e.after) {
                        return Err(invalid("native graph history differs from retained edge"));
                    }
                    if let Some(before) = &e.before {
                        current.insert(key, before.clone());
                    } else {
                        current.remove(&key);
                    }
                }
            }
            MutationOutcome::NativeMemoryInsertedWithLinks { transition } => {
                for e in &transition.edges {
                    if current.remove(&edge_key(e)).as_ref() != Some(e) {
                        return Err(invalid(
                            "linked creation differs after graph history rewind",
                        ));
                    }
                }
            }
            MutationOutcome::NativeMemoryInserted { transition } => {
                for e in &transition.edges {
                    if current.remove(&edge_key(e)).as_ref() != Some(e) {
                        return Err(invalid(
                            "native creation edge differs after graph history rewind",
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
