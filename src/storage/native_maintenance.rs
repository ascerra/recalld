//! Native maintenance proofs contain numeric state and commitments, never lost payloads.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::StorageError;
use super::control::{Generation, GenerationRef, WriteReceipt};
use super::native_lifecycle::{FiniteF32, history_hash, invalid};
use super::native_records::{NativeEdgeProof, NativeRecordChange, NativeRecordState, ProofRecord};
use crate::decay::{
    DecayConfig,
    lifecycle::{EffectiveLifecyclePolicy, project},
};
use crate::model::schema::LifecycleVersion;
use crate::model::{AccessEvent, DecayPhase, MemoryId, NamespaceConfig, NamespaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDecaySkip {
    Unchanged,
    Stale,
    Exempt,
    MissingSummary,
}
#[derive(Debug, Clone)]
pub enum NativeDecayResult {
    Committed(WriteReceipt),
    Skipped(NativeDecaySkip),
}

#[derive(Debug, Clone)]
pub struct NativeDecayIntent {
    pub(crate) scope: super::native_lifecycle::LifecycleScope,
    pub(crate) memory_id: MemoryId,
    pub(crate) capture: super::native_lifecycle::CapturedLifecycle,
    pub(crate) sampled_at: i64,
}
impl NativeDecayIntent {
    pub fn capture_generation(&self) -> GenerationRef {
        self.capture.generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeDecayEvidence {
    pub multiplier: FiniteF32,
    pub max_bonus: FiniteF32,
    pub thresholds: [FiniteF32; 3],
    pub bonus: FiniteF32,
}
impl NativeDecayEvidence {
    pub(crate) fn capture(p: &EffectiveLifecyclePolicy, bonus: f32) -> Result<Self, StorageError> {
        let finite = |v: f32| v.try_into().map_err(StorageError::InvalidMutation);
        Ok(Self {
            multiplier: finite(p.decay_rate_multiplier)?,
            max_bonus: finite(p.config.max_connection_bonus)?,
            thresholds: [
                finite(p.config.phase_1_threshold)?,
                finite(p.config.phase_2_threshold)?,
                finite(p.config.phase_3_threshold)?,
            ],
            bonus: finite(bonus)?,
        })
    }
    fn policy(&self) -> EffectiveLifecyclePolicy {
        EffectiveLifecyclePolicy {
            decay_rate_multiplier: self.multiplier.get(),
            config: DecayConfig {
                max_connection_bonus: self.max_bonus.get(),
                phase_1_threshold: self.thresholds[0].get(),
                phase_2_threshold: self.thresholds[1].get(),
                phase_3_threshold: self.thresholds[2].get(),
                ..DecayConfig::default()
            },
        }
    }
    pub(crate) fn matches_namespace(&self, ns: &NamespaceConfig) -> bool {
        self.thresholds.map(FiniteF32::get)
            == [
                ns.phase_thresholds.full_to_summary,
                ns.phase_thresholds.summary_to_ghost,
                ns.phase_thresholds.ghost_to_delete,
            ]
            && ns
                .decay_rate_multiplier
                .is_none_or(|m| m == self.multiplier.get())
    }
}

pub(crate) enum PlannedDecay {
    Phase(NativeRecordState),
    Delete,
    Skipped(NativeDecaySkip),
}
pub(crate) fn plan(
    before: &NativeRecordState,
    e: &NativeDecayEvidence,
    sampled_at: i64,
) -> Result<PlannedDecay, StorageError> {
    before.lifecycle.validate()?;
    before.content.validate_profile_commitment()?;
    if chrono::DateTime::from_timestamp_millis(sampled_at).is_none() {
        return Err(invalid("invalid native maintenance clock"));
    }
    let b = &before.lifecycle;
    let time = sampled_at.max(b.lifecycle.lifecycle_updated_at);
    let p = project(
        b.stability.get(),
        b.lifecycle.decay_anchor_at,
        &e.policy(),
        time,
        e.bonus.get(),
    )
    .map_err(|x| StorageError::InvalidMutation(x.to_string()))?;
    if e.bonus.get() > e.max_bonus.get() || (b.phase == DecayPhase::Ghost && e.bonus.get() != 0.) {
        return Err(invalid("invalid native maintenance bonus"));
    }
    if b.phase == DecayPhase::Tombstone || b.is_permastore || e.multiplier.get() == 0. {
        return Ok(PlannedDecay::Skipped(NativeDecaySkip::Exempt));
    }
    let phase = match b.phase {
        DecayPhase::Full if p.decay_strength <= e.thresholds[0].get() => {
            if !b.has_summary {
                return Ok(PlannedDecay::Skipped(NativeDecaySkip::MissingSummary));
            }
            DecayPhase::Summary
        }
        DecayPhase::Summary if p.decay_strength <= e.thresholds[1].get() => DecayPhase::Ghost,
        DecayPhase::Ghost if p.decay_strength <= e.thresholds[2].get() => {
            return Ok(PlannedDecay::Delete);
        }
        phase => phase,
    };
    if phase == b.phase
        && p.strength == b.strength.get()
        && p.decay_strength == b.decay_strength.get()
    {
        return Ok(PlannedDecay::Skipped(NativeDecaySkip::Unchanged));
    }
    let mut after = before.clone();
    after.lifecycle.strength = p
        .strength
        .try_into()
        .map_err(StorageError::InvalidMutation)?;
    after.lifecycle.decay_strength = p
        .decay_strength
        .try_into()
        .map_err(StorageError::InvalidMutation)?;
    after.lifecycle.lifecycle.lifecycle_updated_at = time;
    after.lifecycle.lifecycle.record_generation = b.lifecycle.record_generation.next()?;
    if phase != b.phase {
        // Unknown extensions may carry retained payload: require their consumer.
        let erased = before.content.erased()?;
        after.lifecycle.phase = phase;
        after.lifecycle.lifecycle.phase_changed_at = time.max(b.lifecycle.phase_changed_at);
        after.lifecycle.has_text = false;
        after.content.text_sha256 = None;
        if phase == DecayPhase::Ghost {
            after.lifecycle.has_summary = false;
            after.content = erased;
            after.content.vector = before.content.vector.clone();
        }
    }
    Ok(PlannedDecay::Phase(after))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePhaseWrite {
    pub namespace_id: NamespaceId,
    pub memory_id: MemoryId,
    pub lifecycle_version: LifecycleVersion,
    pub policy_revision: Generation,
    pub previous_generation: Generation,
    pub capture_generation: GenerationRef,
    pub sampled_at: i64,
    pub policy: NativeDecayEvidence,
    pub change: NativeRecordChange,
}
fn boundary(
    receipt: &WriteReceipt,
    namespace: NamespaceId,
    id: MemoryId,
    previous: Generation,
    capture: GenerationRef,
    before: &NativeRecordState,
    version: LifecycleVersion,
    revision: Generation,
) -> Result<(), StorageError> {
    before.lifecycle.validate()?;
    if namespace.is_unset()
        || id == MemoryId::nil()
        || capture.store_id != receipt.store_id
        || capture.generation > previous
        || previous.next()? != receipt.committed_generation
        || before.lifecycle.lifecycle.lifecycle_version != version
        || before.lifecycle.lifecycle.policy_revision != revision
    {
        return Err(invalid("invalid native maintenance boundary"));
    }
    Ok(())
}
impl NativePhaseWrite {
    pub(crate) fn validate(&self, r: &WriteReceipt) -> Result<(), StorageError> {
        let before = self
            .change
            .before
            .as_ref()
            .ok_or_else(|| invalid("phase prior state missing"))?;
        boundary(
            r,
            self.namespace_id,
            self.memory_id,
            self.previous_generation,
            self.capture_generation,
            before,
            self.lifecycle_version,
            self.policy_revision,
        )?;
        if self.change.memory_id != self.memory_id {
            return Err(invalid("phase identity mismatch"));
        }
        match plan(before, &self.policy, self.sampled_at)? {
            PlannedDecay::Phase(expected) if expected == self.change.after => Ok(()),
            _ => Err(invalid("invalid native phase transition")),
        }
    }
    pub(crate) fn rewind(&self, rows: &mut [ProofRecord]) -> Result<(), StorageError> {
        let row = rows
            .iter_mut()
            .find(|r| r.id == self.memory_id)
            .ok_or_else(|| invalid("phase record absent"))?;
        if row.namespace != self.namespace_id.get()
            || row.state != self.change.after
            || history_hash(&row.history)? != row.state.history_sha256
        {
            return Err(invalid("phase retained state mismatch"));
        }
        row.state = self
            .change
            .before
            .clone()
            .ok_or_else(|| invalid("phase prior state missing"))?;
        Ok(())
    }
    pub(crate) fn loss_mask(&self) -> u8 {
        use super::metadata::removal_ledger::{FULL_TEXT, SUMMARY, TAGS};
        match (
            self.change.before.as_ref().unwrap().lifecycle.phase,
            self.change.after.lifecycle.phase,
        ) {
            (DecayPhase::Full, DecayPhase::Summary) => FULL_TEXT,
            (DecayPhase::Summary, DecayPhase::Ghost) => FULL_TEXT | SUMMARY | TAGS,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeDeletionWitness {
    pub memory_id: MemoryId,
    pub namespace_id: NamespaceId,
    pub before: NativeRecordState,
    #[serde(deserialize_with = "deserialize_witness_history")]
    pub history: Vec<AccessEvent>,
}
fn deserialize_witness_history<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<AccessEvent>, D::Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Event {
        timestamp: i64,
        kind: crate::model::AccessKind,
    }
    struct History;
    impl<'de> serde::de::Visitor<'de> for History {
        type Value = Vec<AccessEvent>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("bounded numeric access history")
        }
        fn visit_seq<S: serde::de::SeqAccess<'de>>(
            self,
            mut seq: S,
        ) -> Result<Self::Value, S::Error> {
            let mut history = Vec::new();
            while let Some(e) = seq.next_element::<Event>()? {
                if history.len() == crate::model::constants::ACCESS_HISTORY_MAX {
                    return Err(serde::de::Error::custom(
                        "deletion history exceeds canonical bound",
                    ));
                }
                history.push(AccessEvent {
                    timestamp: e.timestamp,
                    kind: e.kind,
                });
            }
            Ok(history)
        }
    }
    d.deserialize_seq(History)
}
impl NativeDeletionWitness {
    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        self.before.lifecycle.validate()?;
        self.before.content.validate_profile_commitment()?;
        if self.memory_id == MemoryId::nil()
            || self.namespace_id.is_unset()
            || self.history.len() > crate::model::constants::ACCESS_HISTORY_MAX
            || self
                .history
                .iter()
                .any(|e| chrono::DateTime::from_timestamp_millis(e.timestamp).is_none())
            || history_hash(&self.history)? != self.before.history_sha256
        {
            return Err(invalid("invalid native deletion witness"));
        }
        // Destructive writers cannot silently drop unknown extension payloads.
        self.before.content.erased()?;
        Ok(())
    }
    pub(crate) fn proof_record(&self) -> Result<ProofRecord, StorageError> {
        self.validate()?;
        Ok(ProofRecord {
            id: self.memory_id,
            namespace: self.namespace_id.get(),
            state: self.before.clone(),
            history: self.history.clone(),
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePeerChange {
    pub namespace_id: NamespaceId,
    pub change: NativeRecordChange,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeHardDeleteWrite {
    pub namespace_id: NamespaceId,
    pub memory_id: MemoryId,
    pub lifecycle_version: LifecycleVersion,
    pub policy_revision: Generation,
    pub previous_generation: Generation,
    pub capture_generation: GenerationRef,
    pub sampled_at: i64,
    pub policy: NativeDecayEvidence,
    pub deletion: NativeDeletionWitness,
    pub peers: Vec<NativePeerChange>,
    pub removed_edges: Vec<NativeEdgeProof>,
}
impl NativeHardDeleteWrite {
    pub(crate) fn validate(&self, r: &WriteReceipt) -> Result<(), StorageError> {
        self.deletion.validate()?;
        boundary(
            r,
            self.namespace_id,
            self.memory_id,
            self.previous_generation,
            self.capture_generation,
            &self.deletion.before,
            self.lifecycle_version,
            self.policy_revision,
        )?;
        if self.deletion.memory_id != self.memory_id
            || self.deletion.namespace_id != self.namespace_id
            || !matches!(
                plan(&self.deletion.before, &self.policy, self.sampled_at)?,
                PlannedDecay::Delete
            )
        {
            return Err(invalid("invalid native hard deletion"));
        }
        if self.removed_edges.windows(2).any(|p| {
            (p[0].source, p[0].target, p[0].edge_type.as_u8())
                >= (p[1].source, p[1].target, p[1].edge_type.as_u8())
        }) || self
            .peers
            .windows(2)
            .any(|p| p[0].change.memory_id >= p[1].change.memory_id)
        {
            return Err(invalid("native deletion proof order/duplicates"));
        }
        let mut keys = HashSet::new();
        let mut decrements = HashMap::<MemoryId, u16>::new();
        for e in &self.removed_edges {
            let weight = f32::from_bits(e.weight_bits);
            if (e.source != self.memory_id && e.target != self.memory_id)
                || e.source == MemoryId::nil()
                || e.target == MemoryId::nil()
                || e.source == e.target
                || !weight.is_finite()
                || !(0. ..=1.).contains(&weight)
                || !keys.insert((e.source, e.target, e.edge_type.as_u8()))
            {
                return Err(invalid("invalid removed native edge"));
            }
            if e.source != self.memory_id {
                let count = decrements.entry(e.source).or_default();
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| invalid("peer decrement overflow"))?;
            }
        }
        let outgoing = self
            .removed_edges
            .iter()
            .filter(|e| e.source == self.memory_id)
            .count();
        if usize::from(self.deletion.before.edge_count) != outgoing {
            return Err(invalid("deleted outgoing count mismatch"));
        }
        let mut ids = HashSet::new();
        for peer in &self.peers {
            let c = &peer.change;
            let before = c
                .before
                .as_ref()
                .ok_or_else(|| invalid("deleted edge peer prior state absent"))?;
            before.lifecycle.validate()?;
            before.content.validate_profile_commitment()?;
            c.after.content.validate_profile_commitment()?;
            let count = decrements
                .remove(&c.memory_id)
                .ok_or_else(|| invalid("peer without removed outgoing edge"))?;
            let mut expected = before.clone();
            expected.edge_count = before
                .edge_count
                .checked_sub(count)
                .ok_or_else(|| invalid("peer count underflow"))?;
            expected.lifecycle.lifecycle.record_generation =
                before.lifecycle.lifecycle.record_generation.next()?;
            if peer.namespace_id.is_unset()
                || c.memory_id == MemoryId::nil()
                || !ids.insert(c.memory_id)
                || c.memory_id == self.memory_id
                || expected != c.after
            {
                return Err(invalid("native deletion peer transition mismatch"));
            }
        }
        if !decrements.is_empty() {
            return Err(invalid("native deletion peer proof missing"));
        }
        Ok(())
    }
    pub(crate) fn rewind(
        &self,
        rows: &mut HashMap<u32, Vec<ProofRecord>>,
    ) -> Result<(), StorageError> {
        if rows.values().flatten().any(|r| r.id == self.memory_id) {
            return Err(invalid("hard deleted record still present"));
        }
        for peer in &self.peers {
            let row = rows
                .get_mut(&peer.namespace_id.get())
                .and_then(|rs| rs.iter_mut().find(|r| r.id == peer.change.memory_id))
                .ok_or_else(|| invalid("hard deletion peer absent"))?;
            if row.state != peer.change.after {
                return Err(invalid("hard deletion peer retained state mismatch"));
            }
            row.state = peer
                .change
                .before
                .clone()
                .ok_or_else(|| invalid("peer prior state missing"))?;
        }
        rows.entry(self.namespace_id.get())
            .or_default()
            .push(self.deletion.proof_record()?);
        Ok(())
    }
}

impl super::native_lifecycle::NativeLifecycleSession {
    /// One read snapshot and clock sample per explicit pass. Applying an intent
    /// again with a new key is stale after its first mutation, never a second loss.
    pub fn capture_decay_pass(
        &self,
        namespace: NamespaceId,
    ) -> Result<Vec<NativeDecayIntent>, StorageError> {
        self.metadata()
            .capture_native_decay_pass(namespace, &self.policy, self.clock.now_millis())
    }

    /// Capture internal preconditions and exactly one clock sample. Reuse this
    /// intent on retry; caller IDs, not captured tokens/time, identify the request.
    pub fn capture_decay(
        &self,
        id: MemoryId,
        namespace: NamespaceId,
    ) -> Result<NativeDecayIntent, StorageError> {
        self.metadata()
            .capture_native_decay(id, namespace, &self.policy, self.clock.now_millis())
    }
    pub fn apply_decay(
        &mut self,
        ctx: &super::control::MutationContext,
        intent: &NativeDecayIntent,
    ) -> Result<NativeDecayResult, StorageError> {
        self.engine
            .apply_native_decay(ctx, intent, &self.policy, None)
    }
    /// Original-ID replay is checked before a fresh capture/clock/policy lookup.
    pub fn maintain(
        &mut self,
        ctx: &super::control::MutationContext,
        id: MemoryId,
    ) -> Result<NativeDecayResult, StorageError> {
        if let Some(receipt) = self.engine.lookup_native_decay(ctx, id, None)? {
            return Ok(NativeDecayResult::Committed(receipt));
        }
        let super::control::MutationScope::Namespace(ns) = ctx.scope else {
            return Err(StorageError::MutationNamespaceMismatch);
        };
        let intent = self.capture_decay(id, ns)?;
        self.apply_decay(ctx, &intent)
    }
}

pub(crate) fn validate_ledger(
    receipt: &WriteReceipt,
    ledger: Option<&super::metadata::removal_ledger::LedgerExport>,
) -> Result<(), StorageError> {
    use super::control::MutationOutcome;
    use super::metadata::removal_ledger::{ALL, TerminalKind};
    let (ns, id, mask, terminal) = match &receipt.outcome {
        MutationOutcome::NativePhaseMaterialized { transition: t } => {
            (t.namespace_id, t.memory_id, t.loss_mask(), false)
        }
        MutationOutcome::NativeMemoryHardDeleted { transition: t } => {
            (t.namespace_id, t.memory_id, ALL, true)
        }
        _ => return Ok(()),
    };
    if mask == 0 {
        return Ok(());
    }
    if !ledger.into_iter().flat_map(|l| &l.entries).any(|e| {
        e.origin_lineage == receipt.store_id
            && e.namespace_id == ns.get()
            && e.memory_uuid == id.into_inner()
            && e.removed_fields_mask & mask == mask
            && if terminal {
                e.terminal_kind == Some(TerminalKind::HardDelete)
                    && e.generation == receipt.committed_generation
                    && e.operation_id == receipt.operation_id
            } else {
                e.generation > receipt.committed_generation
                    || (e.generation == receipt.committed_generation
                        && e.operation_id == receipt.operation_id)
            }
    }) {
        return Err(invalid("native maintenance removal ledger proof missing"));
    }
    Ok(())
}

pub(crate) fn validate_namespace_profiles(
    receipt: &WriteReceipt,
    configs: &HashMap<u32, NamespaceConfig>,
) -> Result<(), StorageError> {
    let check = |ns: NamespaceId, state: &NativeRecordState| -> Result<(), StorageError> {
        let cfg = configs
            .get(&ns.get())
            .ok_or_else(|| invalid("maintenance proof namespace missing"))?;
        if state.content.profile_id != super::native_records::profile_id(cfg) {
            return Err(invalid("maintenance proof namespace/profile mismatch"));
        }
        Ok(())
    };
    match &receipt.outcome {
        super::control::MutationOutcome::NativePhaseMaterialized { transition: t } => {
            check(
                t.namespace_id,
                t.change
                    .before
                    .as_ref()
                    .ok_or_else(|| invalid("phase before missing"))?,
            )?;
            check(t.namespace_id, &t.change.after)?;
        }
        super::control::MutationOutcome::NativeMemoryHardDeleted { transition: t } => {
            check(t.namespace_id, &t.deletion.before)?;
            for p in &t.peers {
                check(
                    p.namespace_id,
                    p.change
                        .before
                        .as_ref()
                        .ok_or_else(|| invalid("peer before missing"))?,
                )?;
                check(p.namespace_id, &p.change.after)?;
            }
        }
        _ => {}
    }
    Ok(())
}
