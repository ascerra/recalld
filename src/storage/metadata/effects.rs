//! Durable metadata/edge effects. Canonical application and derived publication
//! are separate acknowledgements; a committed operation is never rolled back.
use super::*;
use crate::model::EdgeType;
use crate::storage::edges::PersistedEdge;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;

#[cfg(test)]
thread_local! {
    pub(crate) static FAIL_BEFORE_RECORD_COMMIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static FAIL_AFTER_RECORD_COMMIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Ephemeral inputs only; receipts and effects never retain stripped content.
pub(super) enum RecordMutation<'a> {
    NativePhase {
        before: &'a DiskRecord,
        after: &'a DiskRecord,
        transition: crate::storage::native_maintenance::NativePhaseWrite,
    },
    NativeHardDelete(crate::storage::native_maintenance::NativeHardDeleteWrite),
    NativePurgeChunk(
        &'a crate::storage::purge::PurgeProgress,
        crate::storage::native_purge::NativePurgeChunk,
    ),
    GraphOnly,
    LifecyclePatched {
        before: &'a DiskRecord,
        after: &'a DiskRecord,
    },
    NativeGraphUnchanged(crate::storage::native_graph::NativeGraphNoChange),
    NativeGraph(crate::storage::native_graph::NativeGraphWrite),
    PurgeChunk(&'a crate::storage::purge::PurgeProgress),
    Inserted(&'a DiskRecord, u32),
    NativeStored(
        &'a DiskRecord,
        crate::storage::native_store::NativeStoreWrite,
    ),
    NativeInserted(
        &'a DiskRecord,
        crate::storage::native_records::NativeRecordWrite,
    ),
    NativeTombstoned {
        record: &'a DiskRecord,
        old_phase: DecayPhase,
        transition: crate::storage::native_records::NativeRecordWrite,
    },
    Tombstoned {
        record: &'a DiskRecord,
        old_phase: DecayPhase,
    },
}

pub(crate) const SCHEMA: TableDefinition<&str, u32> = TableDefinition::new("effects_schema");
pub(crate) const EFFECTS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("mutation_effects");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VectorRelease {
    pub namespace_id: NamespaceId,
    pub slot: u32,
    pub memory_id: MemoryId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PendingEffects {
    pub version: u32,
    pub receipt: WriteReceipt,
    pub scope: MutationScope,
    pub puts: Vec<PersistedEdge>,
    pub deletes: Vec<PersistedEdge>,
    pub releases: Vec<VectorRelease>,
    pub invalidated_ids: Vec<MemoryId>,
    pub removed_ids: Vec<MemoryId>,
    pub peer_counts: Vec<(MemoryId, u16)>,
    pub canonical_applied: bool,
}

/// Result data for the caller's graph/cache publication. Removed records are
/// returned in memory only; durable effects/receipts never retain their content.
#[derive(Debug)]
pub(crate) struct EffectResult {
    pub receipt: WriteReceipt,
    pub peer_counts: Vec<(MemoryId, u16)>,
    pub removed: Vec<(MemoryId, DiskRecord)>,
    pub edges_removed: usize,
    pub edges_put: usize,
}

pub(crate) fn initialize(tx: &redb::WriteTransaction) -> Result<(), StorageError> {
    tx.open_table(SCHEMA)?.insert("version", 1)?;
    tx.open_table(EFFECTS)?;
    Ok(())
}

pub(crate) fn validate_existing(
    tx: &redb::ReadTransaction,
) -> Result<Vec<PendingEffects>, StorageError> {
    use redb::{MultimapTableHandle, TableHandle};
    let reserved = [SCHEMA.name(), EFFECTS.name()];
    if tx
        .list_multimap_tables()?
        .any(|t| reserved.contains(&t.name()))
    {
        return Err(StorageError::InvalidControl(
            "effects schema table is a multimap".into(),
        ));
    }
    let names: Vec<_> = tx.list_tables()?.map(|t| t.name().to_owned()).collect();
    let present = reserved.map(|n| names.iter().any(|s| s == n));
    if !present.iter().any(|p| *p) {
        if names.iter().any(|n| n == control::RECEIPTS.name()) {
            for entry in tx.open_table(control::RECEIPTS)?.iter()? {
                let (_, value) = entry?;
                if matches!(
                    control::validate_receipt(value.value())?.receipt.outcome,
                    MutationOutcome::GraphChanged { .. }
                        | MutationOutcome::MemoryInserted { .. }
                        | MutationOutcome::MemoryTombstoned { .. }
                        | MutationOutcome::LifecycleMaterialized { .. }
                        | MutationOutcome::NativePhaseMaterialized { .. }
                        | MutationOutcome::NativeMemoryHardDeleted { .. }
                        | MutationOutcome::NativeMemoryInserted { .. }
                        | MutationOutcome::NativeMemoryTombstoned { .. }
                        | MutationOutcome::NativeNamespacePurgeStarted { .. }
                        | MutationOutcome::NativeNamespacePurgeAdvanced { .. }
                        | MutationOutcome::NativeMemoryInsertedWithLinks { .. }
                        | MutationOutcome::NativeGraphChanged { .. }
                        | MutationOutcome::NativeGraphUnchanged { .. }
                        | MutationOutcome::NamespacePurgeStarted { .. }
                        | MutationOutcome::NamespacePurgeAdvanced { .. }
                ) {
                    return Err(StorageError::InvalidControl(
                        "effects schema missing from adopted store".into(),
                    ));
                }
            }
        }
        return Ok(Vec::new());
    }
    if !present.iter().all(|p| *p) {
        return Err(StorageError::InvalidControl(
            "incomplete effects schema".into(),
        ));
    }
    let schema = tx.open_table(SCHEMA)?;
    if schema.len()? != 1 || schema.get("version")?.is_none_or(|v| v.value() != 1) {
        return Err(StorageError::InvalidControl(
            "unsupported effects schema".into(),
        ));
    }
    let state = control::ControlState::decode(
        tx.open_table(control::CONTROL)?
            .get("state")?
            .ok_or_else(|| StorageError::InvalidControl("effects without control state".into()))?
            .value(),
    )?;
    let receipts = tx.open_table(control::RECEIPTS)?;
    let table = tx.open_table(EFFECTS)?;
    let mut effects = Vec::new();
    for entry in table.iter()? {
        let (key, value) = entry?;
        let effect: PendingEffects = control::decode(value.value())?;
        if effect.version != 1
            || key.value() != effect.receipt.operation_id.as_bytes()
            || effect.receipt.store_id != state.store_id
            || effect.receipt.replayed
            || effect.receipt.committed_generation > state.committed_generation
        {
            return Err(StorageError::InvalidControl(
                "invalid effects identity/version".into(),
            ));
        }
        let receipt_key = control::scoped_key(
            state.store_id,
            effect.scope,
            effect.receipt.operation_id.as_bytes(),
        );
        let stored = receipts
            .get(receipt_key.as_slice())?
            .ok_or_else(|| StorageError::InvalidControl("effects without receipt".into()))?;
        if control::validate_receipt(stored.value())?.receipt != effect.receipt {
            return Err(StorageError::InvalidControl(
                "effects receipt mismatch".into(),
            ));
        }
        match &effect.receipt.outcome {
            MutationOutcome::NativeNamespacePurgeAdvanced { .. } => {
                crate::storage::native_purge::validate_effect(&effect)?
            }
            MutationOutcome::NativePhaseMaterialized { .. }
            | MutationOutcome::NativeMemoryHardDeleted { .. } => {
                super::native_maintenance::validate_effect(&effect)?
            }
            MutationOutcome::NativeGraphUnchanged { transition } => {
                validate_native_graph_nochange_effect(&effect, transition)?
            }
            MutationOutcome::NativeGraphChanged { transition } => {
                validate_native_graph_effect(&effect, transition)?
            }
            MutationOutcome::NativeMemoryInsertedWithLinks { transition } => {
                validate_native_store_effect(&effect, transition)?
            }
            MutationOutcome::NativeMemoryInserted { .. } => validate_native_effect(&effect, true)?,
            MutationOutcome::NativeMemoryTombstoned { .. } => {
                validate_native_effect(&effect, false)?
            }
            _ => {}
        }
        match effect.receipt.outcome {
            MutationOutcome::LifecycleMaterialized { memory_id, .. }
                if matches!(effect.scope, MutationScope::Namespace(_))
                    && effect.puts.is_empty()
                    && effect.deletes.is_empty()
                    && effect.removed_ids.is_empty()
                    && effect.releases.is_empty()
                    && effect.peer_counts.is_empty()
                    && effect.invalidated_ids == [memory_id] => {}
            MutationOutcome::NativeNamespacePurgeAdvanced { .. } => {}
            MutationOutcome::NativePhaseMaterialized { .. }
            | MutationOutcome::NativeMemoryHardDeleted { .. } => {}
            MutationOutcome::NativeMemoryInserted { .. }
            | MutationOutcome::NativeMemoryTombstoned { .. } => {}
            MutationOutcome::NativeMemoryInsertedWithLinks { .. }
            | MutationOutcome::NativeGraphChanged { .. } => {}
            MutationOutcome::NativeGraphUnchanged { .. } => {}
            MutationOutcome::MemoryTombstoned { memory_id, status }
                if matches!(effect.scope, MutationScope::Namespace(_))
                    && effect.puts.is_empty()
                    && effect.deletes.is_empty()
                    && effect.removed_ids.is_empty()
                    && effect.peer_counts.is_empty()
                    && effect.invalidated_ids == [memory_id]
                    && match status {
                        crate::storage::tombstone::TombstoneStatus::Tombstoned => {
                            effect.releases.len() == 1
                                && effect.releases.iter().all(|r| {
                                    r.memory_id == memory_id
                                        && effect.scope == MutationScope::Namespace(r.namespace_id)
                                })
                        }
                        crate::storage::tombstone::TombstoneStatus::AlreadyTombstoned => {
                            effect.releases.is_empty()
                        }
                    } => {}
            MutationOutcome::MemoryInserted {
                memory_id,
                edges_put,
                ..
            } if matches!(effect.scope, MutationScope::Namespace(_))
                && edges_put == effect.puts.len()
                && effect.deletes.is_empty()
                && effect.removed_ids.is_empty()
                && effect.releases.is_empty()
                && effect.invalidated_ids.contains(&memory_id) => {}
            MutationOutcome::NamespacePurgeAdvanced {
                stage,
                removed,
                edges_removed,
                ..
            } if effect.puts.is_empty()
                && effect.releases.is_empty()
                && removed == effect.removed_ids.len() as u64
                && edges_removed == effect.deletes.len() as u64
                && (stage == crate::storage::purge::PurgeStage::Deleting
                    || (removed == 0
                        && edges_removed == 0
                        && effect.invalidated_ids.is_empty()
                        && effect.peer_counts.is_empty())) => {}
            MutationOutcome::GraphChanged {
                edges_put,
                edges_removed,
                memories_removed,
            } if edges_put == effect.puts.len()
                && edges_removed == effect.deletes.len()
                && memories_removed == effect.removed_ids.len() => {}
            _ => {
                return Err(StorageError::InvalidControl(
                    "effects outcome disagrees with actions".into(),
                ));
            }
        }
        if effect.releases.iter().any(|r| {
            !effect.removed_ids.contains(&r.memory_id) && !terminal_effect(&effect, r.memory_id)
        }) {
            return Err(StorageError::InvalidControl(
                "release without removed owner".into(),
            ));
        }
        // Deletes may clean up legacy invalid edges; only new puts must
        // satisfy current admission rules (including no self edge).
        for edge in &effect.puts {
            if !edge.weight.is_finite()
                || !(0.0..=1.0).contains(&edge.weight)
                || edge.source == edge.target
            {
                return Err(StorageError::InvalidControl("invalid effect edge".into()));
            }
        }
        effects.push(effect);
    }
    effects.sort_by_key(|e| e.receipt.committed_generation);
    Ok(effects)
}

impl MetadataStore {
    pub(crate) fn effects_stamp(&self) -> Result<u64, StorageError> {
        let before = self.effects_epoch.load(Ordering::SeqCst);
        self.ensure_available()?;
        if self.derived_dirty.load(Ordering::SeqCst) {
            return Err(StorageError::RecoveryRequired(
                "derived graph effects pending publication".into(),
            ));
        }
        if before != self.effects_epoch.load(Ordering::SeqCst) {
            return Err(StorageError::RecoveryRequired(
                "effects changed during admission".into(),
            ));
        }
        Ok(before)
    }

    pub(crate) fn needs_effects_rebuild(&self) -> bool {
        self.derived_dirty.load(Ordering::SeqCst)
    }

    /// Called after every derived index is rebuilt, either during startup or
    /// under the online coordinator's publication/index/storage guards.
    pub(crate) fn acknowledge_rebuilt_effects(&self) -> Result<(), StorageError> {
        self.acknowledge_rebuilt_effects_with_work(None)
    }

    pub(crate) fn acknowledge_rebuilt_effects_with_work(
        &self,
        parent: Option<&WorkTicket>,
    ) -> Result<(), StorageError> {
        let work = self.work_tracker.admit(parent)?;
        let result = (|| {
            let ids: Vec<_> = self
                .pending_effects()?
                .iter()
                .map(|e| e.receipt.operation_id)
                .collect();
            for id in ids {
                self.acknowledge_derived_effects_with_work(id, Some(&work))?;
            }
            Ok(())
        })();
        self.finish_work(work);
        result
    }

    pub(crate) fn pending_effects(&self) -> Result<Vec<PendingEffects>, StorageError> {
        validate_existing(&self.db.begin_read()?)
    }

    /// Caller holds the publication write gate, including the edge snapshot.
    pub(crate) fn commit_graph_effects(
        &self,
        _guard: &MutationGuard<'_>,
        namespace: NamespaceId,
        existing_edges: &[PersistedEdge],
        puts: &[PersistedEdge],
        incident_ids: &[MemoryId],
        delete_records: bool,
        release_vectors: bool,
    ) -> Result<EffectResult, StorageError> {
        self.commit_graph_effects_with_origin(
            _guard,
            namespace,
            existing_edges,
            puts,
            incident_ids,
            delete_records,
            release_vectors,
            1,
        )
    }

    pub(crate) fn commit_graph_effects_with_origin(
        &self,
        _guard: &MutationGuard<'_>,
        namespace: NamespaceId,
        existing_edges: &[PersistedEdge],
        puts: &[PersistedEdge],
        incident_ids: &[MemoryId],
        delete_records: bool,
        release_vectors: bool,
        original_contract_version: u32,
    ) -> Result<EffectResult, StorageError> {
        _guard.check(self)?;
        self.ensure_available()?;
        let mut ctx = MutationContext::unkeyed(namespace);
        ctx.contract_version = original_contract_version;
        ctx.validate()?;
        let tx = self.db.begin_write()?;
        let request = control::encode(&(puts, incident_ids, delete_records, release_vectors))?;
        let hash = control::request_hash(&ctx, "graph_effects", &[&request]);
        self.finish_record_effects(
            _guard,
            tx,
            &ctx,
            hash,
            existing_edges,
            puts,
            incident_ids,
            delete_records,
            release_vectors,
            RecordMutation::GraphOnly,
            None,
        )
    }

    /// Shared transaction tail for graph mutations and canonical insertion.
    /// Insertion has already staged its new row/indexes in this transaction.
    pub(super) fn finish_record_effects(
        &self,
        _guard: &MutationGuard<'_>,
        tx: redb::WriteTransaction,
        ctx: &MutationContext,
        hash: [u8; 32],
        existing_edges: &[PersistedEdge],
        puts: &[PersistedEdge],
        incident_ids: &[MemoryId],
        delete_records: bool,
        release_vectors: bool,
        mut mutation: RecordMutation<'_>,
        child: Option<&crate::storage::batch::BatchChild>,
    ) -> Result<EffectResult, StorageError> {
        _guard.check(self)?;
        let MutationScope::Namespace(namespace) = ctx.scope else {
            return Err(StorageError::InvalidMutationContext(
                "record effects require namespace scope".into(),
            ));
        };
        if tx
            .open_table(NAMESPACE_TABLE)?
            .get(namespace.get())?
            .is_none()
        {
            return Err(StorageError::NamespaceNotFound(namespace.get()));
        }

        if !matches!(
            mutation,
            RecordMutation::PurgeChunk(_) | RecordMutation::NativePurgeChunk(_, _)
        ) {
            super::namespace_state::require_namespace_active(&tx, namespace)?;
        }

        let mut removed = Vec::new();
        let mut releases = Vec::new();
        let mut invalidated: std::collections::HashSet<_> = incident_ids.iter().copied().collect();
        match &mutation {
            RecordMutation::NativePhase { after: record, .. }
            | RecordMutation::Inserted(record, _)
            | RecordMutation::Tombstoned { record, .. }
            | RecordMutation::LifecyclePatched { after: record, .. }
            | RecordMutation::NativeInserted(record, _)
            | RecordMutation::NativeStored(record, _)
            | RecordMutation::NativeTombstoned { record, .. } => {
                invalidated.insert(MemoryId::from_bytes(record.id));
            }
            RecordMutation::NativeHardDelete(_)
            | RecordMutation::GraphOnly
            | RecordMutation::PurgeChunk(_)
            | RecordMutation::NativeGraph(_)
            | RecordMutation::NativePurgeChunk(_, _)
            | RecordMutation::NativeGraphUnchanged(_) => {}
        }
        if let RecordMutation::Tombstoned { record, old_phase }
        | RecordMutation::NativeTombstoned {
            record, old_phase, ..
        } = &mutation
        {
            if *old_phase != DecayPhase::Tombstone {
                releases.push(VectorRelease {
                    namespace_id: namespace,
                    slot: record.vector_slot,
                    memory_id: MemoryId::from_bytes(record.id),
                });
            }
        }
        let mut final_edges: Vec<_> = existing_edges
            .iter()
            .filter(|e| !incident_ids.contains(&e.source) && !incident_ids.contains(&e.target))
            .cloned()
            .collect();
        let deletes: Vec<_> = existing_edges
            .iter()
            .filter(|e| incident_ids.contains(&e.source) || incident_ids.contains(&e.target))
            .cloned()
            .collect();
        let mut affected: std::collections::HashSet<_> = deletes.iter().map(|e| e.source).collect();
        let mut actual_puts = Vec::new();
        let mut peer_counts = Vec::new();
        {
            let mut records = tx.open_table(META_TABLE)?;
            for edge in puts {
                if !edge.weight.is_finite()
                    || !(0.0..=1.0).contains(&edge.weight)
                    || edge.source == edge.target
                {
                    return Err(StorageError::InvalidMutation(
                        "invalid edge weight or self edge".into(),
                    ));
                }
                for id in [edge.source, edge.target] {
                    let record = DiskRecord::from_bytes(
                        records
                            .get(id.as_bytes().as_slice())?
                            .ok_or(StorageError::NotFound(id.into_inner()))?
                            .value(),
                    )?;
                    // V1 structural compatibility: correcting a retained old
                    // identity and naming a retained parent are allowed. New
                    // children/replacements and associative endpoints must live.
                    let retained_structural = match edge.edge_type {
                        EdgeType::Supersedes => id == edge.target || delete_records,
                        EdgeType::ParentChild => id == edge.source || delete_records,
                        _ => false,
                    };
                    if record.id != *id.as_bytes()
                        || record.namespace_id != namespace.get()
                        || (matches!(record.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
                            && !retained_structural)
                        || (delete_records && incident_ids.contains(&id))
                    {
                        return Err(StorageError::InvalidMutation(
                            "edge endpoint is unavailable or in another namespace".into(),
                        ));
                    }
                }
                if edge.edge_type == EdgeType::Supersedes {
                    if final_edges.iter().any(|old| {
                        old.edge_type == EdgeType::Supersedes
                            && old.target == edge.target
                            && old.source != edge.source
                    }) {
                        return Err(StorageError::EdgeConflict("supersedes branch".into()));
                    }
                    let mut seen = std::collections::HashSet::new();
                    let mut visit = vec![edge.target];
                    while let Some(id) = visit.pop() {
                        if id == edge.source {
                            return Err(StorageError::EdgeConflict("supersedes cycle".into()));
                        }
                        if seen.insert(id) {
                            visit.extend(
                                final_edges
                                    .iter()
                                    .filter(|e| {
                                        e.edge_type == EdgeType::Supersedes && e.source == id
                                    })
                                    .map(|e| e.target),
                            );
                        }
                    }
                }
                if let Some(old) = final_edges.iter_mut().find(|old| same_edge(old, edge)) {
                    // Native receipt identity preserves f32 bits, including
                    // signed zero. Keep legacy numeric equality unchanged.
                    let unchanged = if matches!(&mutation, RecordMutation::NativeGraph(_)) {
                        crate::storage::native_records::NativeEdgeProof::from(&*old)
                            == crate::storage::native_records::NativeEdgeProof::from(edge)
                    } else {
                        old == edge
                    };
                    if unchanged {
                        continue;
                    }
                    *old = edge.clone();
                } else {
                    final_edges.push(edge.clone());
                }
                affected.insert(edge.source);
                invalidated.insert(edge.source);
                invalidated.insert(edge.target);
                actual_puts.retain(|old| !same_edge(old, edge));
                actual_puts.push(edge.clone());
            }
            if delete_records {
                let mut tags = tx.open_multimap_table(TAG_INDEX)?;
                let mut namespaces = tx.open_multimap_table(NAMESPACE_INDEX)?;
                for id in incident_ids {
                    let Some(bytes) = records.remove(id.as_bytes().as_slice())? else {
                        continue;
                    };
                    let record = DiskRecord::from_bytes(bytes.value())?;
                    if record.namespace_id != namespace.get() {
                        return Err(StorageError::MutationNamespaceMismatch);
                    }
                    for tag in &record.tags {
                        tags.remove(tag.as_str(), id.as_bytes().as_slice())?;
                    }
                    namespaces.remove(record.namespace_id, id.as_bytes().as_slice())?;
                    if release_vectors && record.phase != DecayPhase::Tombstone {
                        releases.push(VectorRelease {
                            namespace_id: namespace,
                            slot: record.vector_slot,
                            memory_id: *id,
                        });
                    }
                    removed.push((*id, record));
                }
            }
            for id in affected {
                let Some(bytes) = records.get(id.as_bytes().as_slice())? else {
                    continue;
                };
                let mut record = DiskRecord::from_bytes(bytes.value())?;
                drop(bytes);
                let count = u16::try_from(final_edges.iter().filter(|e| e.source == id).count())
                    .map_err(|_| {
                        StorageError::InvalidMutation(
                            "outgoing edge count exceeds record capacity".into(),
                        )
                    })?;
                let insertion_changes = match &mut mutation {
                    RecordMutation::NativeInserted(_, proof) => Some(&mut proof.records),
                    RecordMutation::NativeStored(_, proof) => Some(&mut proof.records),
                    _ => None,
                };
                if let Some(changes) = insertion_changes {
                    let change =
                        changes
                            .iter_mut()
                            .find(|r| r.memory_id == id)
                            .ok_or_else(|| {
                                StorageError::InvalidControl(
                                    "native edge source proof missing".into(),
                                )
                            })?;
                    if let Some(before) = &change.before {
                        if before.lifecycle
                            != crate::storage::native_lifecycle::LifecycleState::capture(&record)?
                            || before.edge_count != record.edge_count
                        {
                            return Err(StorageError::InvalidControl(
                                "native peer changed within transaction".into(),
                            ));
                        }
                        record
                            .schema
                            .as_mut()
                            .ok_or_else(|| {
                                StorageError::InvalidControl("native peer schema missing".into())
                            })?
                            .lifecycle
                            .record_generation =
                            before.lifecycle.lifecycle.record_generation.next()?;
                        change.after.lifecycle.lifecycle.record_generation =
                            record.schema.as_ref().unwrap().lifecycle.record_generation;
                    }
                    change.after.edge_count = count;
                }
                let deletion_peers = match &mut mutation {
                    RecordMutation::NativeHardDelete(proof) => Some(&mut proof.peers),
                    RecordMutation::NativePurgeChunk(_, proof) => Some(&mut proof.peers),
                    _ => None,
                };
                if let Some(peers) = deletion_peers {
                    let peer = peers
                        .iter_mut()
                        .find(|p| p.change.memory_id == id)
                        .ok_or_else(|| {
                            StorageError::InvalidControl("native deleted peer proof missing".into())
                        })?;
                    let before = peer.change.before.as_ref().ok_or_else(|| {
                        StorageError::InvalidControl("native deleted peer before missing".into())
                    })?;
                    if record.namespace_id != peer.namespace_id.get()
                        || !before.matches_record(&record)?
                    {
                        return Err(StorageError::InvalidControl(
                            "native deleted peer changed inside transaction".into(),
                        ));
                    }
                    record.schema.as_mut().unwrap().lifecycle.record_generation =
                        before.lifecycle.lifecycle.record_generation.next()?;
                    peer.change.after.lifecycle.lifecycle.record_generation =
                        record.schema.as_ref().unwrap().lifecycle.record_generation;
                    peer.change.after.edge_count = count;
                }
                if let RecordMutation::NativeGraph(proof) = &mut mutation {
                    let change = proof
                        .records
                        .iter_mut()
                        .find(|r| r.memory_id == id)
                        .ok_or_else(|| {
                            StorageError::InvalidControl("native graph source proof missing".into())
                        })?;
                    let before = change.before.as_ref().ok_or_else(|| {
                        StorageError::InvalidControl(
                            "native graph source prior state missing".into(),
                        )
                    })?;
                    if !before.matches_record(&record)? {
                        return Err(StorageError::InvalidControl(
                            "native graph source changed within transaction".into(),
                        ));
                    }
                    let token = before.lifecycle.lifecycle.record_generation.next()?;
                    record.schema.as_mut().unwrap().lifecycle.record_generation = token;
                    change.after.lifecycle.lifecycle.record_generation = token;
                    change.after.edge_count = count;
                }
                record.edge_count = count;
                records.insert(id.as_bytes().as_slice(), record.to_bytes().as_slice())?;
                peer_counts.push((id, count));
                invalidated.insert(id);
            }
        }
        peer_counts.sort_by_key(|(id, _)| *id);
        let mut invalidated_ids: Vec<_> = invalidated.into_iter().collect();
        invalidated_ids.sort();
        if let RecordMutation::NativeInserted(_, proof) = &mut mutation {
            proof.edges = actual_puts
                .iter()
                .map(crate::storage::native_records::NativeEdgeProof::from)
                .collect();
        }
        let outcome = match &mutation {
            RecordMutation::LifecyclePatched { before, after } => {
                MutationOutcome::LifecycleMaterialized {
                    memory_id: MemoryId::from_bytes(after.id),
                    from_phase: before.phase,
                    to_phase: after.phase,
                    strength_bits: after.strength.to_bits(),
                    decay_strength_bits: after.decay_strength.to_bits(),
                    removed_fields_mask: super::removal_ledger::lifecycle_removed_fields(
                        before, after,
                    ),
                }
            }
            RecordMutation::NativePhase { transition, .. } => {
                MutationOutcome::NativePhaseMaterialized {
                    transition: transition.clone(),
                }
            }
            RecordMutation::NativeHardDelete(transition) => {
                MutationOutcome::NativeMemoryHardDeleted {
                    transition: transition.clone(),
                }
            }
            RecordMutation::NativeStored(_, transition) => {
                MutationOutcome::NativeMemoryInsertedWithLinks {
                    transition: transition.clone(),
                }
            }
            RecordMutation::NativeGraphUnchanged(transition) => {
                MutationOutcome::NativeGraphUnchanged {
                    transition: transition.clone(),
                }
            }
            RecordMutation::NativeGraph(transition) => MutationOutcome::NativeGraphChanged {
                transition: transition.clone(),
            },
            RecordMutation::NativeInserted(_, transition) => {
                MutationOutcome::NativeMemoryInserted {
                    transition: transition.clone(),
                }
            }
            RecordMutation::NativeTombstoned { transition, .. } => {
                MutationOutcome::NativeMemoryTombstoned {
                    transition: transition.clone(),
                }
            }
            RecordMutation::Inserted(record, embedding_dim) => MutationOutcome::MemoryInserted {
                memory_id: MemoryId::from_bytes(record.id),
                created_at: record.created_at,
                initial_stability_bits: record.stability.to_bits(),
                embedding_dim: *embedding_dim,
                edges_put: actual_puts.len(),
            },
            RecordMutation::Tombstoned { record, old_phase } => MutationOutcome::MemoryTombstoned {
                memory_id: MemoryId::from_bytes(record.id),
                status: if *old_phase == DecayPhase::Tombstone {
                    crate::storage::tombstone::TombstoneStatus::AlreadyTombstoned
                } else {
                    crate::storage::tombstone::TombstoneStatus::Tombstoned
                },
            },
            RecordMutation::GraphOnly
            | RecordMutation::PurgeChunk(_)
            | RecordMutation::NativePurgeChunk(_, _) => MutationOutcome::GraphChanged {
                edges_put: actual_puts.len(),
                edges_removed: deletes.len(),
                memories_removed: removed.len(),
            },
        };
        let receipt = if let RecordMutation::NativePurgeChunk(progress, chunk) = &mutation {
            super::purge::record_next_native(
                &tx,
                progress,
                self.ids.as_ref(),
                crate::storage::purge::PurgeStage::Deleting,
                &removed,
                deletes.len(),
                Some(chunk.clone()),
            )?
        } else if let RecordMutation::PurgeChunk(progress) = &mutation {
            super::purge::record_next(
                &tx,
                progress,
                self.ids.as_ref(),
                crate::storage::purge::PurgeStage::Deleting,
                &removed,
                deletes.len(),
            )?
        } else if let Some(child) = child {
            super::batch::record_child(&tx, child, ctx, hash, outcome)?
        } else {
            self.record_receipt(&tx, ctx, hash, outcome)?
        };
        if let RecordMutation::LifecyclePatched { before, after } = &mutation {
            super::removal_ledger::record_change(
                &tx,
                crate::storage::control::GenerationRef {
                    store_id: receipt.store_id,
                    generation: receipt.committed_generation,
                },
                before,
                after,
                receipt.operation_id,
            )?;
        }
        if let RecordMutation::Tombstoned { record, old_phase }
        | RecordMutation::NativeTombstoned {
            record, old_phase, ..
        } = &mutation
        {
            if *old_phase != DecayPhase::Tombstone {
                super::removal_ledger::record_removal(
                    &tx,
                    crate::storage::control::GenerationRef {
                        store_id: receipt.store_id,
                        generation: receipt.committed_generation,
                    },
                    record.namespace_id,
                    MemoryId::from_bytes(record.id),
                    Some(super::removal_ledger::TerminalKind::Tombstone),
                    super::removal_ledger::ALL,
                    receipt.operation_id,
                )?;
            }
        }
        if let RecordMutation::NativePhase { transition, .. } = &mutation {
            let mask = transition.loss_mask();
            if mask != 0 {
                super::removal_ledger::record_removal(
                    &tx,
                    crate::storage::control::GenerationRef {
                        store_id: receipt.store_id,
                        generation: receipt.committed_generation,
                    },
                    namespace.get(),
                    transition.memory_id,
                    None,
                    mask,
                    receipt.operation_id,
                )?;
            }
        }
        match &receipt.outcome {
            MutationOutcome::NativePhaseMaterialized { transition } => {
                transition.validate(&receipt)?
            }
            MutationOutcome::NativeMemoryHardDeleted { transition } => {
                transition.validate(&receipt)?
            }
            MutationOutcome::NativeMemoryInsertedWithLinks { transition } => {
                transition.validate(&receipt)?
            }
            MutationOutcome::NativeGraphChanged { transition } => transition.validate(&receipt)?,
            MutationOutcome::NativeGraphUnchanged { transition } => {
                transition.validate(&receipt)?
            }
            MutationOutcome::NativeMemoryInserted { transition } => {
                transition.validate(&receipt, true)?
            }
            MutationOutcome::NativeMemoryTombstoned { transition } => {
                transition.validate(&receipt, false)?
            }
            _ => {}
        }
        // Removal evidence and effect receipt share this canonical transaction.
        for (id, record) in &removed {
            super::removal_ledger::record_removal(
                &tx,
                crate::storage::control::GenerationRef {
                    store_id: receipt.store_id,
                    generation: receipt.committed_generation,
                },
                record.namespace_id,
                *id,
                Some(super::removal_ledger::TerminalKind::HardDelete),
                super::removal_ledger::ALL,
                receipt.operation_id,
            )?;
        }
        let pending = PendingEffects {
            version: 1,
            receipt: receipt.clone(),
            scope: ctx.scope,
            puts: actual_puts,
            deletes,
            releases,
            removed_ids: removed.iter().map(|(id, _)| *id).collect(),
            invalidated_ids,
            peer_counts: peer_counts.clone(),
            canonical_applied: false,
        };
        tx.open_table(EFFECTS)?.insert(
            receipt.operation_id.as_bytes().as_slice(),
            control::encode(&pending)?.as_slice(),
        )?;
        #[cfg(test)]
        if FAIL_BEFORE_RECORD_COMMIT.with(|flag| flag.replace(false)) {
            return Err(StorageError::InvalidMutation(
                "injected precommit abandonment".into(),
            ));
        }
        let certification = self.prepare_vector_validation_commit(&tx, &receipt)?;
        crash_point("before_metadata_commit");
        // Publish dirty before advancing the epoch. Admission samples the
        // epoch twice; it cannot accept a new stamp with an old clean flag.
        let next = self
            .effects_epoch
            .load(Ordering::SeqCst)
            .checked_add(1)
            .ok_or(StorageError::GenerationExhausted)?;
        self.derived_dirty.store(true, Ordering::SeqCst);
        self.canonical_pending.store(true, Ordering::SeqCst);
        self.effects_epoch.store(next, Ordering::SeqCst);
        crate::performance::event!(
            kind = "commit_call",
            resource = "metadata_effects",
            phase = "attempt",
            sync_coverage = "opaque_backend"
        );
        let timer = crate::performance::Timer::new("metadata.effects_commit");
        let committed = tx.commit();
        crate::performance::event!(
            kind = "commit_call",
            resource = "metadata_effects",
            phase = "return",
            success = committed.is_ok(),
            sync_coverage = "opaque_backend"
        );
        timer.outcome(committed.is_ok());
        // Synthetic I/O ambiguity: the commit succeeded, but the caller must
        // not assume failure means its staged bytes can be reclaimed.
        #[cfg(test)]
        let committed = committed.and_then(|()| {
            if FAIL_AFTER_RECORD_COMMIT.with(|flag| flag.replace(false))
                || child.is_some()
                    && self
                        .fail_batch_commit
                        .compare_exchange(3, 0, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
            {
                Err(redb::CommitError::Storage(redb::StorageError::Io(
                    std::io::Error::other("injected unknown record commit"),
                )))
            } else {
                Ok(())
            }
        });
        committed.map_err(|source| {
            self.mark_unavailable();
            StorageError::CommitOutcomeUnknown {
                operation_id: receipt.operation_id,
                source,
            }
        })?;
        self.publish_vector_validation_commit(certification, &receipt)?;
        self.view_publication.canonical_committed();
        crash_point("after_metadata_commit");
        let mut phases = self.phase_index.write().unwrap_or_else(|e| e.into_inner());
        if let RecordMutation::LifecyclePatched { before, after }
        | RecordMutation::NativePhase { before, after, .. } = &mutation
        {
            phases.transition(
                after.namespace_id,
                after.vector_slot,
                before.phase,
                after.phase,
            );
        }
        if let RecordMutation::Inserted(record, _)
        | RecordMutation::NativeInserted(record, _)
        | RecordMutation::NativeStored(record, _) = &mutation
        {
            phases.transition(
                record.namespace_id,
                record.vector_slot,
                DecayPhase::Full,
                record.phase,
            );
        }
        if let RecordMutation::Tombstoned { record, old_phase }
        | RecordMutation::NativeTombstoned {
            record, old_phase, ..
        } = &mutation
        {
            if *old_phase != DecayPhase::Tombstone {
                phases.remove(record.namespace_id, record.vector_slot, *old_phase);
            }
        }
        for (_, record) in &removed {
            if record.phase != DecayPhase::Tombstone {
                phases.remove(record.namespace_id, record.vector_slot, record.phase);
            }
        }
        Ok(EffectResult {
            receipt,
            peer_counts,
            removed,
            edges_removed: pending.deletes.len(),
            edges_put: pending.puts.len(),
        })
    }

    pub(crate) fn acknowledge_canonical_effects(
        &self,
        guard: &MutationGuard<'_>,
        effect: &PendingEffects,
    ) -> Result<(), StorageError> {
        guard.check(self)?;
        let mut completed = effect.clone();
        completed.canonical_applied = true;
        let tx = self.db.begin_write()?;
        tx.open_table(EFFECTS)?.insert(
            completed.receipt.operation_id.as_bytes().as_slice(),
            control::encode(&completed)?.as_slice(),
        )?;
        tx.commit().map_err(|error| {
            self.mark_unavailable();
            StorageError::RedbCommit(error)
        })?;
        Ok(())
    }

    pub(crate) fn acknowledge_derived_effects(
        &self,
        operation_id: uuid::Uuid,
    ) -> Result<(), StorageError> {
        self.acknowledge_derived_effects_with_work(operation_id, None)
    }

    pub(crate) fn acknowledge_derived_effects_with_work(
        &self,
        operation_id: uuid::Uuid,
        parent: Option<&WorkTicket>,
    ) -> Result<(), StorageError> {
        // This narrow checkpoint consumes an already validated outbox only
        // after the caller rebuilt the real views. It does not author canonical
        // records, receipts or generations, and cannot admit legacy writers to
        // a native owner. Both paths retain the same owner-instance work ticket.
        let publication = if self.native_lifecycle_only {
            self.native_guard_with_work(parent)?
        } else {
            self.mutation_guard_with_work(parent)?
        };
        publication.check(self)?;
        self.pending_effects()?;
        let tx = self.db.begin_write()?;
        {
            let mut effects = tx.open_table(EFFECTS)?;
            if let Some(value) = effects.get(operation_id.as_bytes().as_slice())? {
                let effect: PendingEffects = control::decode(value.value())?;
                if !effect.canonical_applied {
                    return Err(StorageError::RecoveryRequired(
                        "canonical effects pending".into(),
                    ));
                }
            }
            effects.remove(operation_id.as_bytes().as_slice())?;
        }
        tx.commit().map_err(|error| {
            self.mark_unavailable();
            StorageError::RedbCommit(error)
        })?;
        self.derived_dirty
            .store(!self.pending_effects()?.is_empty(), Ordering::SeqCst);
        Ok(())
    }
}

fn same_edge(a: &PersistedEdge, b: &PersistedEdge) -> bool {
    a.source == b.source && a.target == b.target && a.edge_type == b.edge_type
}

pub(crate) fn crash_point(_point: &str) {
    #[cfg(test)]
    if std::env::var("RECALLD_TEST_EFFECT_CRASH").as_deref() == Ok(_point) {
        std::process::exit(91);
    }
}

pub(crate) fn validate_plan(
    effect: &PendingEffects,
    records: &[(MemoryId, DiskRecord)],
) -> Result<(), StorageError> {
    let graph_records = match &effect.receipt.outcome {
        MutationOutcome::NativeGraphChanged { transition } => {
            validate_native_graph_effect(effect, transition)?;
            Some((transition.namespace_id, &transition.records))
        }
        MutationOutcome::NativeGraphUnchanged { transition } => {
            validate_native_graph_nochange_effect(effect, transition)?;
            Some((transition.namespace_id, &transition.records))
        }
        _ => None,
    };
    if let Some((namespace, changes)) = graph_records {
        for change in changes {
            let record = records
                .iter()
                .find(|(id, _)| *id == change.memory_id)
                .map(|(_, r)| r)
                .ok_or_else(|| {
                    StorageError::InvalidControl("native graph pending endpoint missing".into())
                })?;
            if record.namespace_id != namespace.get() || !change.after.matches_record(record)? {
                return Err(StorageError::InvalidControl(
                    "native graph pending peer differs from canonical state".into(),
                ));
            }
        }
    }
    if effect
        .removed_ids
        .iter()
        .any(|id| records.iter().any(|(key, _)| key == id))
    {
        return Err(StorageError::InvalidControl(
            "removed effect ID still exists".into(),
        ));
    }
    super::native_maintenance::validate_pending_records(effect, records)?;
    crate::storage::native_purge::validate_pending_records(effect, records)?;
    let record_proof = if let MutationOutcome::NativeMemoryInsertedWithLinks { transition } =
        &effect.receipt.outcome
    {
        validate_native_store_effect(effect, transition)?;
        Some((transition.namespace_id, &transition.records))
    } else {
        native_proof(effect).map(|p| (p.namespace_id, &p.records))
    };
    if let Some((namespace, changes)) = record_proof {
        for change in changes {
            let record = records
                .iter()
                .find(|(id, _)| *id == change.memory_id)
                .map(|(_, r)| r)
                .ok_or_else(|| {
                    StorageError::InvalidControl("native pending record missing".into())
                })?;
            if record.namespace_id != namespace.get() || !change.after.matches_record(record)? {
                return Err(StorageError::InvalidControl(
                    "native pending proof differs from canonical record".into(),
                ));
            }
        }
    }
    if let MutationOutcome::MemoryInserted {
        memory_id,
        created_at,
        ..
    } = effect.receipt.outcome
    {
        if records
            .iter()
            .find(|(id, _)| *id == memory_id)
            .is_none_or(|(_, record)| {
                record.phase != DecayPhase::Full
                    || record.created_at != created_at
                    || effect.scope
                        != MutationScope::Namespace(NamespaceId::new(record.namespace_id))
            })
        {
            return Err(StorageError::InvalidControl(
                "pending insertion disagrees with canonical record".into(),
            ));
        }
    }
    if let MutationOutcome::LifecycleMaterialized {
        memory_id,
        to_phase,
        strength_bits,
        decay_strength_bits,
        removed_fields_mask,
        ..
    } = effect.receipt.outcome
    {
        if records
            .iter()
            .find(|(id, _)| *id == memory_id)
            .is_none_or(|(_, r)| {
                r.phase != to_phase
                    || r.strength.to_bits() != strength_bits
                    || r.decay_strength.to_bits() != decay_strength_bits
                    || effect.scope != MutationScope::Namespace(NamespaceId::new(r.namespace_id))
                    || (removed_fields_mask & super::removal_ledger::FULL_TEXT != 0
                        && (r.text_offset != 0 || r.text_length != 0))
                    || (removed_fields_mask & super::removal_ledger::SUMMARY != 0
                        && !r.summary.is_empty())
            })
        {
            return Err(StorageError::InvalidControl(
                "pending lifecycle patch disagrees with canonical record".into(),
            ));
        }
    }
    if let MutationOutcome::MemoryTombstoned { memory_id, .. } = effect.receipt.outcome {
        if records
            .iter()
            .find(|(id, _)| *id == memory_id)
            .is_none_or(|(_, record)| {
                record.phase != DecayPhase::Tombstone
                    || !record.summary.is_empty()
                    || !record.tags.is_empty()
                    || record.text_offset != 0
                    || record.text_length != 0
                    || effect.scope
                        != MutationScope::Namespace(NamespaceId::new(record.namespace_id))
                    || effect.releases.iter().any(|r| r.slot != record.vector_slot)
            })
        {
            return Err(StorageError::InvalidControl(
                "pending tombstone disagrees with canonical record".into(),
            ));
        }
    }
    for release in &effect.releases {
        if records.iter().any(|(id, r)| {
            (*id == release.memory_id && !terminal_effect(effect, *id))
                || (r.phase != DecayPhase::Tombstone
                    && r.namespace_id == release.namespace_id.get()
                    && r.vector_slot == release.slot)
        }) {
            return Err(StorageError::OwnershipMismatch(
                "effect attempts to release an owned vector".into(),
            ));
        }
    }
    for (id, count) in &effect.peer_counts {
        if records
            .iter()
            .find(|(key, _)| key == id)
            .is_none_or(|(_, r)| r.edge_count != *count)
        {
            return Err(StorageError::InvalidControl(
                "effect peer count disagrees with canonical metadata".into(),
            ));
        }
    }
    for edge in &effect.puts {
        let MutationScope::Namespace(namespace) = effect.scope else {
            return Err(StorageError::InvalidControl(
                "graph effect has catalog scope".into(),
            ));
        };
        if [edge.source, edge.target].iter().any(|id| {
            records
                .iter()
                .find(|(key, _)| key == id)
                .is_none_or(|(_, record)| record.namespace_id != namespace.get())
        }) {
            return Err(StorageError::InvalidControl(
                "effect put has missing or foreign endpoint".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
impl MetadataStore {
    /// Instance-local probe, reached only after actual exposure commit/publication.
    pub(crate) fn after_observation_probe(&self) {
        match self.observation_fault.swap(0, Ordering::SeqCst) {
            1 => self.mark_unavailable(),
            2 => {
                self.effects_epoch.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        }
    }
}
fn native_proof(
    effect: &PendingEffects,
) -> Option<&crate::storage::native_records::NativeRecordWrite> {
    match &effect.receipt.outcome {
        MutationOutcome::NativeMemoryInserted { transition }
        | MutationOutcome::NativeMemoryTombstoned { transition } => Some(transition),
        _ => None,
    }
}
fn validate_native_graph_nochange_effect(
    effect: &PendingEffects,
    proof: &crate::storage::native_graph::NativeGraphNoChange,
) -> Result<(), StorageError> {
    proof.validate(&effect.receipt)?;
    if effect.scope != MutationScope::Namespace(proof.namespace_id)
        || !effect.puts.is_empty()
        || !effect.deletes.is_empty()
        || !effect.releases.is_empty()
        || !effect.invalidated_ids.is_empty()
        || !effect.removed_ids.is_empty()
        || !effect.peer_counts.is_empty()
    {
        return Err(StorageError::InvalidControl(
            "native graph no-change has effects".into(),
        ));
    }
    Ok(())
}
fn validate_native_graph_effect(
    effect: &PendingEffects,
    proof: &crate::storage::native_graph::NativeGraphWrite,
) -> Result<(), StorageError> {
    proof.validate(&effect.receipt)?;
    let mut invalidated: Vec<_> = proof
        .edges
        .iter()
        .flat_map(|e| [e.after.source, e.after.target])
        .collect();
    invalidated.sort();
    invalidated.dedup();
    if effect.scope != MutationScope::Namespace(proof.namespace_id)
        || !effect.deletes.is_empty()
        || !effect.removed_ids.is_empty()
        || !effect.releases.is_empty()
        || effect
            .puts
            .iter()
            .map(crate::storage::native_records::NativeEdgeProof::from)
            .collect::<Vec<_>>()
            != proof
                .edges
                .iter()
                .map(|e| e.after.clone())
                .collect::<Vec<_>>()
        || effect.peer_counts != proof.peer_counts()
        || effect.invalidated_ids != invalidated
    {
        return Err(StorageError::InvalidControl(
            "native graph effects disagree with receipt".into(),
        ));
    }
    Ok(())
}
fn terminal_effect(effect: &PendingEffects, id: MemoryId) -> bool {
    matches!(effect.receipt.outcome,MutationOutcome::MemoryTombstoned {memory_id,..} if memory_id==id)
        || matches!(&effect.receipt.outcome,MutationOutcome::NativeMemoryTombstoned {transition} if transition.memory_id==id)
}
fn validate_native_effect(effect: &PendingEffects, insertion: bool) -> Result<(), StorageError> {
    use crate::storage::native_records::NativeEdgeProof;
    let p = native_proof(effect).unwrap();
    p.validate(&effect.receipt, insertion)?;
    let mut invalidated = vec![p.memory_id];
    for e in &effect.puts {
        invalidated.extend([e.source, e.target]);
    }
    invalidated.sort();
    invalidated.dedup();
    let mut peers = effect.puts.iter().map(|e| e.source).collect::<Vec<_>>();
    peers.sort();
    peers.dedup();
    let expected_peers = peers
        .into_iter()
        .map(|id| {
            p.records
                .iter()
                .find(|r| r.memory_id == id)
                .map(|r| (id, r.after.edge_count))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| StorageError::InvalidControl("native effect peer proof missing".into()))?;
    let before = p.primary()?.before.as_ref();
    let release = if insertion {
        None
    } else {
        before
            .filter(|b| b.lifecycle.phase != DecayPhase::Tombstone)
            .and_then(|b| b.content.vector.as_ref())
            .map(|v| v.slot)
    };
    if effect.scope != MutationScope::Namespace(p.namespace_id)
        || !effect.removed_ids.is_empty()
        || !effect.deletes.is_empty()
        || effect
            .puts
            .iter()
            .map(NativeEdgeProof::from)
            .collect::<Vec<_>>()
            != p.edges
        || effect.peer_counts != expected_peers
        || effect.invalidated_ids != invalidated
        || effect.releases.len() != usize::from(release.is_some())
        || effect.releases.iter().any(|r| {
            Some(r.slot) != release
                || r.memory_id != p.memory_id
                || r.namespace_id != p.namespace_id
        })
    {
        return Err(StorageError::InvalidControl(
            "native effect actions disagree with receipt".into(),
        ));
    }
    Ok(())
}
fn validate_native_store_effect(
    effect: &PendingEffects,
    proof: &crate::storage::native_store::NativeStoreWrite,
) -> Result<(), StorageError> {
    proof.validate(&effect.receipt)?;
    let mut invalidated = vec![proof.memory_id];
    let mut sources = std::collections::BTreeSet::new();
    for edge in &proof.edges {
        invalidated.extend([edge.source, edge.target]);
        sources.insert(edge.source);
    }
    invalidated.sort();
    invalidated.dedup();
    let peers: Vec<_> = sources
        .into_iter()
        .map(|id| {
            (
                id,
                proof
                    .records
                    .iter()
                    .find(|r| r.memory_id == id)
                    .unwrap()
                    .after
                    .edge_count,
            )
        })
        .collect();
    if effect.scope != MutationScope::Namespace(proof.namespace_id)
        || !effect.deletes.is_empty()
        || !effect.removed_ids.is_empty()
        || !effect.releases.is_empty()
        || effect
            .puts
            .iter()
            .map(crate::storage::native_records::NativeEdgeProof::from)
            .collect::<Vec<_>>()
            != proof.edges
        || effect.peer_counts != peers
        || effect.invalidated_ids != invalidated
    {
        return Err(StorageError::InvalidControl(
            "linked creation effects disagree with receipt".into(),
        ));
    }
    Ok(())
}
