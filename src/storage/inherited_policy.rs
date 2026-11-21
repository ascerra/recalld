//! Explicit adoption of the inherited multiplier. Old absence remains unknown.
//! This administrative API does not activate ordinary native serving.
use redb::ReadableTable;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use uuid::Uuid;

use super::control::{
    self, Generation, GenerationRef, MutationContext, MutationScope, WriteReceipt,
};
use super::native_lifecycle::invalid;
use super::native_maintenance::{NativeDecayEvidence, PlannedDecay};
use super::native_records::NativeRecordState;
use super::offline_snapshot::OfflineSnapshot;
use super::{PersistedEdge, StorageError};
use crate::decay::{DecayConfig, lifecycle::EffectiveLifecyclePolicy};
use crate::model::schema::LifecycleVersion;
use crate::model::{DecayPhase, DiskRecord, MemoryId, NamespaceConfig, NamespaceId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InheritedDecayBinding {
    pub version: u32,
    /// The effective f32, not the spelling of a wider configuration number.
    pub multiplier_bits: u32,
    pub adoption: GenerationRef,
    pub operation_id: Uuid,
}
impl InheritedDecayBinding {
    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        let value = f32::from_bits(self.multiplier_bits);
        if self.version != 1
            || !value.is_finite()
            || value < 0.
            || (value == 0. && self.multiplier_bits != 0)
            || self.adoption.store_id.is_nil()
            || self.adoption.generation == Generation::default()
            || self.operation_id.is_nil()
        {
            return Err(invalid("invalid inherited decay binding"));
        }
        Ok(())
    }
    pub(crate) fn matches_receipt(&self, receipt: &WriteReceipt) -> bool {
        self.adoption
            == GenerationRef {
                store_id: receipt.store_id,
                generation: receipt.committed_generation,
            }
            && self.operation_id == receipt.operation_id
    }
}

pub(crate) fn normalize(multiplier: f64) -> Result<u32, StorageError> {
    let value = multiplier as f32;
    if !multiplier.is_finite()
        || multiplier < 0.
        || !value.is_finite()
        || (multiplier != 0. && value == 0.)
    {
        return Err(StorageError::InvalidMutation(
            "inherited multiplier must be finite nonnegative f32 without underflow".into(),
        ));
    }
    Ok(if value == 0. { 0 } else { value.to_bits() })
}
pub(crate) fn supported_inputs(global: &DecayConfig) -> Result<(), StorageError> {
    let fields = |c: &DecayConfig| {
        [
            c.initial_stability,
            c.difficulty,
            c.phase_1_threshold,
            c.phase_2_threshold,
            c.phase_3_threshold,
            c.permastore_threshold,
            c.max_connection_bonus,
            c.partial_access_weight,
        ]
        .map(f32::to_bits)
    };
    if fields(global) != fields(&DecayConfig::default()) {
        return Err(StorageError::InvalidMutation(
            "inherited policy v1 supports only the built-in shared planner inputs".into(),
        ));
    }
    Ok(())
}
/// Resolve only after reading the canonical namespace under the operation's guard.
/// Historical replay callers must return their original receipt before this check.
pub(crate) fn resolve_bound(
    ns: &NamespaceConfig,
    global: &DecayConfig,
    multiplier: f64,
) -> Result<EffectiveLifecyclePolicy, StorageError> {
    supported_inputs(global)?;
    let legacy_compatibility = ns.schema.as_ref().is_some_and(|schema| {
        schema.lifecycle_version == LifecycleVersion::LegacyAccessV1
            && matches!(
                schema.embedding_binding,
                crate::model::schema::EmbeddingBinding::UnknownLegacy
            )
            && schema.inherited_decay.is_none()
    });
    // Legacy operations used the current runtime multiplier. Preserve that
    // behavior without recording an adoption or claiming a historical value.
    let mut multiplier = multiplier;
    if ns.schema.is_some() && ns.decay_rate_multiplier.is_none() && !legacy_compatibility {
        let binding = ns.schema.as_ref().unwrap().inherited_decay.as_ref().ok_or_else(||
            StorageError::InvalidMutation("inherited policy is unknown; explicitly preview/adopt before dependent operations".into()))?;
        binding.validate()?;
        normalize(multiplier)?;
        // The adopted value governs. Startup adopts a changed configuration;
        // until that succeeds the namespace keeps its stored value.
        multiplier = f32::from_bits(binding.multiplier_bits) as f64;
    }
    crate::decay::lifecycle::resolve_policy(ns, global, multiplier)
        .map_err(|e| StorageError::InvalidMutation(e.to_string()))
}

/// Fixed-size, content-free counts. A record contributes to only one next step.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhaseCounts {
    pub full_to_summary: u64,
    pub summary_to_ghost: u64,
    pub ghost_to_delete: u64,
    pub retained: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InheritedDecayPreview {
    pub version: u32,
    pub source: GenerationRef,
    pub namespace_id: NamespaceId,
    pub policy_revision: Generation,
    pub lifecycle_version: LifecycleVersion,
    /// Required nullable: unknown is not an inferred historical value.
    #[serde(deserialize_with = "required_prior")]
    pub prior: Option<InheritedDecayBinding>,
    pub multiplier_bits: u32,
    pub sampled_at: i64,
    pub examined_rows: u64,
    pub record_count: u64,
    pub examined_edges: u64,
    pub canonical_digest: [u8; 32],
    pub candidate: PhaseCounts,
    pub token: [u8; 32],
}
impl InheritedDecayPreview {
    fn token(&self) -> Result<[u8; 32], StorageError> {
        let mut value = self.clone();
        value.token = [0; 32];
        Ok(Sha256::digest(control::encode(&value)?).into())
    }
    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        if self.version != 1
            || self.source.store_id.is_nil()
            || self.namespace_id.is_unset()
            || normalize(f32::from_bits(self.multiplier_bits) as f64)? != self.multiplier_bits
            || chrono::DateTime::from_timestamp_millis(self.sampled_at).is_none()
            || self.record_count > self.examined_rows
            || self.examined_rows > MAX_ROWS
            || self.examined_edges > MAX_EDGES
            || self.record_count > MAX_NAMESPACE_ROWS
            || self
                .candidate
                .full_to_summary
                .checked_add(self.candidate.summary_to_ghost)
                .and_then(|v| v.checked_add(self.candidate.ghost_to_delete))
                .and_then(|v| v.checked_add(self.candidate.retained))
                != Some(self.record_count)
            || self.token != self.token()?
        {
            return Err(invalid("invalid or incomplete inherited policy preview"));
        }
        if let Some(prior) = &self.prior {
            prior.validate()?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdoptInheritedDecay {
    pub context: MutationContext,
    pub preview: InheritedDecayPreview,
}
impl AdoptInheritedDecay {
    pub(crate) fn hash(&self) -> Result<[u8; 32], StorageError> {
        self.context.validate()?;
        self.preview.validate()?;
        if self.context.scope != MutationScope::Namespace(self.preview.namespace_id)
            || self.context.store_id != Some(self.preview.source.store_id)
            || self.context.expected_generation != Some(self.preview.source.generation)
        {
            return Err(StorageError::InvalidMutationContext(
                "adoption requires the full original preview source and namespace".into(),
            ));
        }
        Ok(control::request_hash(
            &self.context,
            "adopt_inherited_decay_v1",
            &[&control::encode(&self.preview)?],
        ))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InheritedDecayAdopted {
    pub contract_version: u32,
    pub preview: InheritedDecayPreview,
    pub after: InheritedDecayBinding,
    pub next_revision: Generation,
    pub record_token_digest: String,
}
impl InheritedDecayAdopted {
    pub(crate) fn changed(&self) -> bool {
        self.preview.prior.as_ref().map(|p| p.multiplier_bits) != Some(self.preview.multiplier_bits)
    }
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        self.preview.validate()?;
        self.after.validate()?;
        if !matches!(self.contract_version, 1 | 2)
            || self.preview.source.store_id != receipt.store_id
            || self.preview.source.generation.next()? != receipt.committed_generation
            || self.after.multiplier_bits != self.preview.multiplier_bits
            || self.record_token_digest.len() != 64
            || !self
                .record_token_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || if self.changed() {
                self.next_revision != self.preview.policy_revision.next()?
                    || !self.after.matches_receipt(receipt)
            } else {
                self.next_revision != self.preview.policy_revision
                    || self.preview.prior.as_ref() != Some(&self.after)
            }
        {
            return Err(invalid("invalid inherited policy adoption proof"));
        }
        Ok(())
    }
    pub(crate) fn revision_transition(&self) -> super::lifecycle_migration::PolicyTransition {
        super::lifecycle_migration::PolicyTransition {
            namespace_id: self.preview.namespace_id,
            from: self.preview.lifecycle_version,
            to: self.preview.lifecycle_version,
            previous_revision: self.preview.policy_revision,
            next_revision: self.next_revision,
            previous_generation: self.preview.source.generation,
            record_count: self.preview.record_count,
            record_token_digest: self.record_token_digest.clone(),
        }
    }
}

// Bounds are checked before decoding/copying each raw row, including excluded rows.
// Hitting any limit returns an error, never an applicable partial preview token.
const MAX_ROWS: u64 = 1_000_000;
const MAX_NAMESPACE_ROWS: u64 = 100_000;
const MAX_EDGES: u64 = 1_000_000;
const MAX_ROW_BYTES: usize = 1024 * 1024;
pub(crate) fn decode_row(key: &[u8], bytes: &[u8]) -> Result<DiskRecord, StorageError> {
    if bytes.len() > MAX_ROW_BYTES {
        return Err(invalid("policy preview record byte budget exceeded"));
    }
    let row = DiskRecord::from_bytes(bytes)?;
    if key != row.id {
        return Err(invalid("policy preview row identity mismatch"));
    }
    Ok(row)
}

pub(crate) fn preview(
    source: GenerationRef,
    ns: &NamespaceConfig,
    multiplier_bits: u32,
    sampled_at: i64,
    rows: impl Iterator<Item = Result<DiskRecord, StorageError>>,
    edges: impl Iterator<Item = Result<PersistedEdge, StorageError>>,
) -> Result<InheritedDecayPreview, StorageError> {
    if ns.decay_rate_multiplier.is_some() {
        return Err(StorageError::InvalidMutation(
            "explicit namespace policy does not inherit; set inheritance before adoption".into(),
        ));
    }
    let schema = ns
        .schema
        .as_ref()
        .ok_or_else(|| invalid("native namespace required"))?;
    let p = crate::decay::lifecycle::resolve_policy(
        ns,
        &DecayConfig::default(),
        f32::from_bits(multiplier_bits) as f64,
    )
    .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
    let mut graph = crate::graph::RelationshipGraph::new();
    let mut donors = HashMap::new();
    let mut states = BTreeMap::new();
    let mut digest = Sha256::new();
    digest.update(b"recalld-inherited-policy-preview-v1");
    digest.update(control::encode(ns)?);
    let mut examined_rows = 0;
    for row in rows {
        if examined_rows == MAX_ROWS {
            return Err(invalid("policy preview row budget exceeded"));
        }
        examined_rows += 1;
        let row = row?;
        digest.update(row.id);
        digest.update(Sha256::digest(row.encode()?));
        if row.namespace_id != ns.id.get() {
            continue;
        }
        if states.len() as u64 == MAX_NAMESPACE_ROWS {
            return Err(invalid("policy preview namespace budget exceeded"));
        }
        let state = NativeRecordState::capture(&row, ns, None)?;
        if state.lifecycle.lifecycle.policy_revision != schema.policy_revision
            || state.lifecycle.lifecycle.lifecycle_version != schema.lifecycle_version
        {
            return Err(invalid("policy preview row revision mismatch"));
        }
        let id = MemoryId::from_bytes(row.id);
        graph
            .add_node(id, ns.id, row.phase, row.strength, row.vector_slot)
            .map_err(|e| invalid(&e.to_string()))?;
        if state.lifecycle.eligible() {
            let projected = crate::decay::lifecycle::project(
                row.stability,
                state.lifecycle.lifecycle.decay_anchor_at,
                &p,
                sampled_at.max(state.lifecycle.lifecycle.lifecycle_updated_at),
                0.,
            )
            .map_err(|e| invalid(&e.to_string()))?;
            donors.insert(id, projected.strength);
        }
        states.insert(id, state);
    }
    let mut examined_edges = 0;
    for edge in edges {
        if examined_edges == MAX_EDGES {
            return Err(invalid("policy preview edge budget exceeded"));
        }
        examined_edges += 1;
        let edge = edge?;
        digest.update(control::encode(
            &super::native_records::NativeEdgeProof::from(&edge),
        )?);
        if states.contains_key(&edge.source) && states.contains_key(&edge.target) {
            graph
                .add_edge(
                    edge.source,
                    edge.target,
                    edge.edge_type,
                    edge.weight,
                    edge.auto_created,
                )
                .map_err(|e| invalid(&e.to_string()))?;
        }
    }
    let mut candidate = PhaseCounts::default();
    for (id, state) in &states {
        // Match the actual maintenance writer: retained Ghost edges never
        // contribute activation to the Ghost's terminal deletion decision.
        let bonus = if state.lifecycle.phase == DecayPhase::Ghost {
            0.
        } else {
            crate::graph::activation::connection_bonus_with_strengths(
                *id,
                &graph,
                &Default::default(),
                &donors,
            )
        };
        match super::native_maintenance::plan(
            state,
            &NativeDecayEvidence::capture(&p, bonus)?,
            sampled_at,
        )? {
            PlannedDecay::Delete => candidate.ghost_to_delete += 1,
            PlannedDecay::Phase(after)
                if state.lifecycle.phase == DecayPhase::Full
                    && after.lifecycle.phase == DecayPhase::Summary =>
            {
                candidate.full_to_summary += 1
            }
            PlannedDecay::Phase(after)
                if state.lifecycle.phase == DecayPhase::Summary
                    && after.lifecycle.phase == DecayPhase::Ghost =>
            {
                candidate.summary_to_ghost += 1
            }
            _ => candidate.retained += 1,
        }
    }
    let mut result = InheritedDecayPreview {
        version: 1,
        source,
        namespace_id: ns.id,
        policy_revision: schema.policy_revision,
        lifecycle_version: schema.lifecycle_version,
        prior: schema.inherited_decay.clone(),
        multiplier_bits,
        sampled_at,
        examined_rows,
        record_count: states.len() as u64,
        examined_edges,
        canonical_digest: digest.finalize().into(),
        candidate,
        token: [0; 32],
    };
    result.token = result.token()?;
    result.validate()?;
    Ok(result)
}

// This administrative command bounds disposable control copies before any
// inspector can allocate them. Payload files are validated/read, never copied.
const MAX_CONTROL_BYTES: u64 = 512 * 1024 * 1024;
fn bounded_snapshot(path: &Path) -> Result<OfflineSnapshot, StorageError> {
    if !path.is_dir() {
        return Err(StorageError::InvalidPath);
    }
    let owner = super::ownership::StoreOwnership::acquire(path)?;
    let mut total = 0u64;
    for name in ["meta.db", "edges.db"] {
        let file = owner.path.join(name);
        super::validation::require_file(&file)?;
        total = total
            .checked_add(std::fs::metadata(&file)?.len())
            .ok_or_else(|| invalid("policy inspection copy byte budget exceeded"))?;
        if total > MAX_CONTROL_BYTES {
            return Err(invalid(
                "policy inspection copy byte budget exceeded; no complete preview produced",
            ));
        }
    }
    // OfflineSnapshot and each inspector own TempDirs: every failure drops its
    // private copies. This same source owner remains held until apply finishes.
    OfflineSnapshot::open_owned(owner)
}

/// Read-only ownership and disposable validated metadata; no effects recovery.
pub fn preview_inherited_decay(
    path: &Path,
    namespace: NamespaceId,
    multiplier: f64,
    source: GenerationRef,
    clock: &dyn crate::time::Clock,
) -> Result<InheritedDecayPreview, StorageError> {
    let bits = normalize(multiplier)?;
    let snapshot = bounded_snapshot(path)?;
    if snapshot.control().generation_ref() != source {
        return Err(invalid("policy preview source changed"));
    }
    preview_snapshot(&snapshot, namespace, bits, clock.now_millis())
}
fn preview_snapshot(
    snapshot: &OfflineSnapshot,
    namespace: NamespaceId,
    bits: u32,
    sampled_at: i64,
) -> Result<InheritedDecayPreview, StorageError> {
    let source = snapshot.control().generation_ref();
    let ns = snapshot
        .namespaces()
        .iter()
        .find(|n| n.id == namespace)
        .ok_or(StorageError::NamespaceNotFound(namespace.get()))?;
    let read = snapshot.metadata.begin_read()?;
    let rows = read.open_table(super::metadata::META_TABLE)?;
    let edge_read = snapshot.edges.begin_read()?;
    let edges = edge_read.open_table(super::edges::FORWARD_EDGES)?;
    preview(
        source,
        ns,
        bits,
        sampled_at,
        rows.iter()?.map(|r| {
            let (k, v) = r?;
            decode_row(k.value(), v.value())
        }),
        edges.iter()?.map(|r| {
            let (k, v) = r?;
            PersistedEdge::from_forward(k.value(), v.value())
        }),
    )
}

/// Offline exclusive native owner. The returned receipt is canonical; the caller
/// must reopen live consumers. This does not claim live derived readiness.
pub fn adopt_inherited_decay(
    path: &Path,
    request: &AdoptInheritedDecay,
) -> Result<WriteReceipt, StorageError> {
    let hash = request.hash()?;
    let snapshot = bounded_snapshot(path)?;
    {
        // Replay/CAS and namespace admission on a disposable copy first: rejected
        // requests must not turn an ordinary writable redb open into a source edit.
        let tx = snapshot.metadata.begin_write()?;
        if let Some(receipt) = control::replay_or_check(&tx, &request.context, hash)? {
            return Ok(receipt);
        }
        super::metadata::namespace_state::require_namespace_active(
            &tx,
            request.preview.namespace_id,
        )?;
    }
    if preview_snapshot(
        &snapshot,
        request.preview.namespace_id,
        request.preview.multiplier_bits,
        request.preview.sampled_at,
    )? != request.preview
    {
        return Err(StorageError::LifecycleConflict(
            "adoption preview canonical evidence changed".into(),
        ));
    }
    let mut engine = super::RedbStorageEngine::open_owned_with_ids(
        snapshot.into_owner(),
        true,
        Box::new(super::ids::SystemIds),
    )?;
    engine.adopt_inherited_decay(request, None)
}

fn required_prior<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<InheritedDecayBinding>, D::Error> {
    Option::<InheritedDecayBinding>::deserialize(deserializer)
}
