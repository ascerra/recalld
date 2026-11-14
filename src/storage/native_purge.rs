//! Native purge proof shapes on the existing progress/outbox state machine.
//! Draft wiring consumes the validated maintenance witness; no serving activation.
use serde::{Deserialize, Serialize};

use super::StorageError;
use super::control::{Generation, WriteReceipt};
use super::native_catalog::NamespacePolicyState;
use super::native_lifecycle::invalid;
use crate::model::{NamespaceConfig, NamespaceId, schema::NamespaceSchemaV2};

/// Retained namespace identity/policy for history validation after live catalog
/// removal. This contains configuration only, never memory payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeNamespaceProof {
    pub namespace_id: NamespaceId,
    pub name: String,
    pub embedding_dim: u32,
    pub created_at: i64,
    pub policy: NamespacePolicyState,
    pub schema: NamespaceSchemaV2,
}
impl NativeNamespaceProof {
    pub(crate) fn capture(config: &NamespaceConfig) -> Result<Self, StorageError> {
        let proof = Self {
            namespace_id: config.id,
            name: config.name.clone(),
            embedding_dim: config.embedding_dim,
            created_at: config.created_at,
            policy: NamespacePolicyState::capture(config)?,
            schema: config
                .schema
                .clone()
                .ok_or_else(|| invalid("native purge namespace schema missing"))?,
        };
        proof.namespace()?;
        Ok(proof)
    }
    pub(crate) fn namespace(&self) -> Result<NamespaceConfig, StorageError> {
        if self.namespace_id.is_unset()
            || !(1..=8192).contains(&self.embedding_dim)
            || chrono::DateTime::from_timestamp_millis(self.created_at).is_none()
        {
            return Err(invalid("invalid retained native namespace identity"));
        }
        crate::model::validation::validate_namespace_name(&self.name)
            .map_err(|_| invalid("invalid retained native namespace name"))?;
        let mut config = NamespaceConfig::default_namespace(self.created_at);
        config.id = self.namespace_id;
        config.name = self.name.clone();
        config.embedding_dim = self.embedding_dim;
        self.policy.apply(&mut config);
        config.schema = Some(self.schema.clone());
        crate::storage::catalog::NamespacePolicy::from(&config).validate()?;
        self.schema
            .validate(&config)
            .map_err(StorageError::InvalidControl)?;
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePurgeStarted {
    pub previous_generation: Generation,
    pub before_namespace: NativeNamespaceProof,
}
impl NativePurgeStarted {
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        if self.previous_generation.next()? != receipt.committed_generation
            || self
                .before_namespace
                .name
                .eq_ignore_ascii_case(crate::model::constants::DEFAULT_NAMESPACE_NAME)
        {
            return Err(invalid("invalid native purge start proof"));
        }
        self.before_namespace.namespace()?;
        Ok(())
    }
}

use super::native_maintenance::{NativeDeletionWitness, NativePeerChange};
use super::native_records::{NativeEdgeProof, ProofRecord};
use super::purge::PurgeStage;
use crate::model::{DecayPhase, MemoryId};
use std::collections::{HashMap, HashSet};

/// Each chunk retains commitments and numeric state, never deleted payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePurgeChunk {
    #[serde(deserialize_with = "deserialize_deletions")]
    pub deletions: Vec<NativeDeletionWitness>,
    pub peers: Vec<NativePeerChange>,
    pub removed_edges: Vec<NativeEdgeProof>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePurgeAdvanced {
    pub parent_operation_id: uuid::Uuid,
    pub namespace_id: NamespaceId,
    pub previous_generation: Generation,
    pub sequence: u64,
    pub stage: PurgeStage,
    pub chunk: Option<NativePurgeChunk>,
}
impl NativePurgeAdvanced {
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        if self.parent_operation_id.is_nil()
            || self.namespace_id.is_unset()
            || self.sequence == 0
            || self.previous_generation.next()? != receipt.committed_generation
        {
            return Err(invalid("invalid native purge child boundary"));
        }
        match (&self.chunk, self.stage) {
            (Some(chunk), PurgeStage::Deleting) => chunk.validate(self.namespace_id),
            (None, PurgeStage::Cleanup | PurgeStage::Complete) => Ok(()),
            _ => Err(invalid("native purge child stage/proof mismatch")),
        }
    }
    pub(crate) fn counts(&self) -> (u64, u64, u64, Option<MemoryId>) {
        self.chunk.as_ref().map_or((0, 0, 0, None), |c| {
            (
                c.deletions.len() as u64,
                c.deletions
                    .iter()
                    .filter(|d| d.before.lifecycle.phase == DecayPhase::Tombstone)
                    .count() as u64,
                c.removed_edges.len() as u64,
                c.deletions.last().map(|d| d.memory_id),
            )
        })
    }
}
impl NativePurgeChunk {
    pub(crate) fn validate(&self, namespace: NamespaceId) -> Result<(), StorageError> {
        if self.deletions.is_empty()
            || self.deletions.len() > super::metadata::purge::CHUNK_LIMIT
            || self
                .deletions
                .windows(2)
                .any(|p| p[0].memory_id >= p[1].memory_id)
            || self
                .peers
                .windows(2)
                .any(|p| p[0].change.memory_id >= p[1].change.memory_id)
            || self.removed_edges.windows(2).any(|p| {
                (p[0].source, p[0].target, p[0].edge_type.as_u8())
                    >= (p[1].source, p[1].target, p[1].edge_type.as_u8())
            })
        {
            return Err(invalid("native purge chunk bounds/order"));
        }
        let ids: HashSet<_> = self.deletions.iter().map(|d| d.memory_id).collect();
        let mut decrements = HashMap::<MemoryId, u16>::new();
        for e in &self.removed_edges {
            let weight = f32::from_bits(e.weight_bits);
            if e.source == MemoryId::nil()
                || e.target == MemoryId::nil()
                || e.source == e.target
                || (!ids.contains(&e.source) && !ids.contains(&e.target))
                || !weight.is_finite()
                || !(0. ..=1.).contains(&weight)
            {
                return Err(invalid("native purge removed edge mismatch"));
            }
            let n = decrements.entry(e.source).or_default();
            *n = n
                .checked_add(1)
                .ok_or_else(|| invalid("native purge edge count overflow"))?;
        }
        for d in &self.deletions {
            d.validate()?;
            if d.namespace_id != namespace
                || d.before.edge_count != decrements.remove(&d.memory_id).unwrap_or(0)
            {
                return Err(invalid(
                    "native purge deletion namespace/outgoing count mismatch",
                ));
            }
        }
        for p in &self.peers {
            let c = &p.change;
            let before = c
                .before
                .as_ref()
                .ok_or_else(|| invalid("native purge peer before missing"))?;
            before.lifecycle.validate()?;
            before.content.validate_profile_commitment()?;
            let n = decrements
                .remove(&c.memory_id)
                .ok_or_else(|| invalid("native purge extra peer"))?;
            let mut after = before.clone();
            after.edge_count = after
                .edge_count
                .checked_sub(n)
                .ok_or_else(|| invalid("native purge peer count underflow"))?;
            after.lifecycle.lifecycle.record_generation =
                after.lifecycle.lifecycle.record_generation.next()?;
            if p.namespace_id.is_unset() || ids.contains(&c.memory_id) || c.after != after {
                return Err(invalid("native purge peer state mismatch"));
            }
        }
        if !decrements.is_empty() {
            return Err(invalid("native purge missing peer"));
        }
        Ok(())
    }
    pub(crate) fn rewind(
        &self,
        rows: &mut HashMap<u32, Vec<ProofRecord>>,
    ) -> Result<(), StorageError> {
        for p in &self.peers {
            let row = rows
                .get_mut(&p.namespace_id.get())
                .and_then(|rs| rs.iter_mut().find(|r| r.id == p.change.memory_id))
                .ok_or_else(|| invalid("native purge retained peer missing"))?;
            if row.state != p.change.after {
                return Err(invalid("native purge retained peer differs"));
            }
            row.state = p
                .change
                .before
                .clone()
                .ok_or_else(|| invalid("native purge peer before missing"))?;
        }
        for d in &self.deletions {
            if rows.values().flatten().any(|r| r.id == d.memory_id) {
                return Err(invalid("purged record still present"));
            }
            rows.entry(d.namespace_id.get())
                .or_default()
                .push(d.proof_record()?);
        }
        Ok(())
    }
}

pub(crate) fn validate_evidence(
    receipt: &WriteReceipt,
    configs: &HashMap<u32, NamespaceConfig>,
    ledger: Option<&super::metadata::removal_ledger::LedgerExport>,
) -> Result<(), StorageError> {
    let super::control::MutationOutcome::NativeNamespacePurgeAdvanced { transition: t } =
        &receipt.outcome
    else {
        return Ok(());
    };
    let Some(chunk) = &t.chunk else {
        return Ok(());
    };
    let check = |ns: NamespaceId,
                 state: &super::native_records::NativeRecordState|
     -> Result<(), StorageError> {
        let config = configs
            .get(&ns.get())
            .ok_or_else(|| invalid("purge witness namespace missing"))?;
        if state.content.profile_id != super::native_records::profile_id(config) {
            return Err(invalid("purge witness profile mismatch"));
        }
        Ok(())
    };
    for d in &chunk.deletions {
        check(d.namespace_id, &d.before)?;
        let ns = configs
            .get(&d.namespace_id.get())
            .unwrap()
            .schema
            .as_ref()
            .ok_or_else(|| invalid("purge schema missing"))?;
        if d.before.lifecycle.lifecycle.policy_revision != ns.policy_revision
            || d.before.lifecycle.lifecycle.lifecycle_version != ns.lifecycle_version
        {
            return Err(invalid("purge deletion policy mismatch"));
        }
        if !ledger.into_iter().flat_map(|l| &l.entries).any(|e| {
            e.origin_lineage == receipt.store_id
                && e.namespace_id == d.namespace_id.get()
                && e.memory_uuid == d.memory_id.into_inner()
                && e.generation == receipt.committed_generation
                && e.operation_id == receipt.operation_id
                && e.removed_fields_mask == super::metadata::removal_ledger::ALL
                && e.terminal_kind
                    == Some(super::metadata::removal_ledger::TerminalKind::HardDelete)
        }) {
            return Err(invalid("purge deletion ledger mismatch"));
        }
    }
    for p in &chunk.peers {
        check(
            p.namespace_id,
            p.change
                .before
                .as_ref()
                .ok_or_else(|| invalid("purge peer missing"))?,
        )?;
        check(p.namespace_id, &p.change.after)?;
    }
    Ok(())
}

pub(crate) fn validate_effect(
    e: &super::metadata::effects::PendingEffects,
) -> Result<(), StorageError> {
    let super::control::MutationOutcome::NativeNamespacePurgeAdvanced { transition: t } =
        &e.receipt.outcome
    else {
        return Ok(());
    };
    let mut removed = Vec::new();
    let mut peers = Vec::new();
    let mut deleted = Vec::new();
    if let Some(chunk) = &t.chunk {
        removed = chunk.deletions.iter().map(|d| d.memory_id).collect();
        peers = chunk
            .peers
            .iter()
            .map(|p| (p.change.memory_id, p.change.after.edge_count))
            .collect();
        deleted = chunk.removed_edges.clone();
    } else if t.stage != PurgeStage::Complete || !e.canonical_applied {
        return Err(invalid("unexpected purge finalization effects"));
    }
    let mut invalidated = removed.clone();
    invalidated.extend(peers.iter().map(|p| p.0));
    invalidated.sort();
    invalidated.dedup();
    let mut actual = e
        .deletes
        .iter()
        .map(NativeEdgeProof::from)
        .collect::<Vec<_>>();
    actual.sort_by_key(|e| (e.source, e.target, e.edge_type.as_u8()));
    if e.scope != super::control::MutationScope::Namespace(t.namespace_id)
        || !e.puts.is_empty()
        || !e.releases.is_empty()
        || removed != e.removed_ids
        || peers != e.peer_counts
        || deleted != actual
        || invalidated != e.invalidated_ids
    {
        return Err(invalid("native purge outbox differs from witness"));
    }
    Ok(())
}
pub(crate) fn validate_pending_records(
    e: &super::metadata::effects::PendingEffects,
    records: &[(MemoryId, crate::model::DiskRecord)],
) -> Result<(), StorageError> {
    let super::control::MutationOutcome::NativeNamespacePurgeAdvanced { transition: t } =
        &e.receipt.outcome
    else {
        return Ok(());
    };
    if let Some(chunk) = &t.chunk {
        for p in &chunk.peers {
            let r = records
                .iter()
                .find(|r| r.0 == p.change.memory_id)
                .ok_or_else(|| invalid("native purge pending peer missing"))?;
            if r.1.namespace_id != p.namespace_id.get() || !p.change.after.matches_record(&r.1)? {
                return Err(invalid("native purge pending peer differs"));
            }
        }
    }
    Ok(())
}

fn deserialize_deletions<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<NativeDeletionWitness>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<NativeDeletionWitness>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("bounded native purge deletion witnesses")
        }
        fn visit_seq<S: serde::de::SeqAccess<'de>>(
            self,
            mut seq: S,
        ) -> Result<Self::Value, S::Error> {
            let mut rows = Vec::new();
            while rows.len() < super::metadata::purge::CHUNK_LIMIT {
                let Some(row) = seq.next_element()? else {
                    return Ok(rows);
                };
                rows.push(row);
            }
            if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom("native purge chunk exceeds bound"));
            }
            Ok(rows)
        }
    }
    d.deserialize_seq(Bounded)
}
