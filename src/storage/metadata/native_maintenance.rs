use super::*;
use crate::storage::EdgeStore;
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs};
use crate::storage::native_maintenance::*;
use crate::storage::native_records::{NativeEdgeProof, NativeRecordChange, NativeRecordState};

pub(crate) fn request_hash(ctx: &MutationContext, id: MemoryId) -> Result<[u8; 32], StorageError> {
    ctx.validate()?;
    if !matches!(ctx.scope, MutationScope::Namespace(_)) {
        return Err(StorageError::InvalidMutationContext(
            "native maintenance requires namespace scope".into(),
        ));
    }
    Ok(control::request_hash(
        ctx,
        "native_maintenance",
        &[id.as_bytes()],
    ))
}
impl MetadataStore {
    pub(crate) fn capture_native_decay_pass(
        &self,
        namespace: NamespaceId,
        policy: &NativePolicyInputs,
        sampled_at: i64,
    ) -> Result<Vec<NativeDecayIntent>, StorageError> {
        self.ensure_available()?;
        let _guard = self.publication.read().unwrap_or_else(|e| e.into_inner());
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        let ns = NamespaceConfig::decode(
            read.open_table(NAMESPACE_TABLE)?
                .get(namespace.get())?
                .ok_or(StorageError::NamespaceNotFound(namespace.get()))?
                .value(),
        )?;
        let scope = LifecycleScope {
            namespace_id: namespace,
            namespace_name: ns.name,
            source: None,
        };
        let mut ids = Vec::new();
        let mut phases = HashMap::new();
        for row in read.open_table(META_TABLE)?.iter()? {
            let (_, v) = row?;
            let record = DiskRecord::from_bytes(v.value())?;
            if record.namespace_id == namespace.get() {
                ids.push(MemoryId::from_bytes(record.id));
                phases.insert(MemoryId::from_bytes(record.id), record.phase);
            }
        }
        let captured = self.capture_at(&read, &scope, &ids, policy, false)?;
        if chrono::DateTime::from_timestamp_millis(sampled_at).is_none() {
            return Err(StorageError::InvalidMutation(
                "invalid native maintenance clock".into(),
            ));
        }
        Ok(captured
            .records
            .iter()
            .map(|(id, token)| {
                let capture = crate::storage::native_lifecycle::CapturedLifecycle {
                    generation: captured.generation,
                    version: captured.version,
                    revision: captured.revision,
                    policy_hash: captured.policy_hash,
                    records: vec![(*id, *token)],
                };
                NativeDecayIntent {
                    scope: scope.clone(),
                    memory_id: *id,
                    captured_phase: phases[id],
                    capture,
                    sampled_at,
                }
            })
            .collect())
    }

    pub(crate) fn lookup_native_decay(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        id: MemoryId,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        guard.check(self)?;
        control::replay_or_check(&self.db.begin_write()?, ctx, request_hash(ctx, id)?)
    }
    pub(crate) fn capture_native_decay(
        &self,
        id: MemoryId,
        namespace: NamespaceId,
        policy: &NativePolicyInputs,
        sampled_at: i64,
    ) -> Result<NativeDecayIntent, StorageError> {
        self.ensure_available()?;
        let _guard = self.publication.read().unwrap_or_else(|e| e.into_inner());
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        let ns = NamespaceConfig::decode(
            read.open_table(NAMESPACE_TABLE)?
                .get(namespace.get())?
                .ok_or(StorageError::NamespaceNotFound(namespace.get()))?
                .value(),
        )?;
        let scope = LifecycleScope {
            namespace_id: namespace,
            namespace_name: ns.name,
            source: None,
        };
        let capture = self.capture_at(&read, &scope, &[id], policy, false)?;
        let captured_phase = DiskRecord::from_bytes(
            read.open_table(META_TABLE)?
                .get(id.as_bytes().as_slice())?
                .ok_or(StorageError::NotFound(id.into_inner()))?
                .value(),
        )?
        .phase;
        if chrono::DateTime::from_timestamp_millis(sampled_at).is_none() {
            return Err(StorageError::InvalidMutation(
                "invalid native maintenance clock".into(),
            ));
        }
        Ok(NativeDecayIntent {
            scope,
            captured_phase,
            memory_id: id,
            capture,
            sampled_at,
        })
    }
    pub(crate) fn decay_native_with_effects(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        intent: &NativeDecayIntent,
        policy: &NativePolicyInputs,
        edge_snapshot: &redb::ReadTransaction,
        text_hashes: &HashMap<MemoryId, Option<[u8; 32]>>,
    ) -> Result<NativeDecayResult, StorageError> {
        self.decay_native_with_effects_bounded(
            guard,
            ctx,
            intent,
            policy,
            edge_snapshot,
            text_hashes,
            None,
        )
    }

    pub(crate) fn decay_native_with_effects_bounded(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        intent: &NativeDecayIntent,
        policy: &NativePolicyInputs,
        // One snapshot of the edge store, read per node.
        edge_snapshot: &redb::ReadTransaction,
        text_hashes: &HashMap<MemoryId, Option<[u8; 32]>>,
        admission: Option<&crate::storage::maintenance::ReceiptAdmission>,
    ) -> Result<NativeDecayResult, StorageError> {
        guard.check(self)?;
        self.check_receipt_admission(admission)?;
        let hash = request_hash(ctx, intent.memory_id)?;
        if ctx.scope != MutationScope::Namespace(intent.scope.namespace_id) {
            return Err(StorageError::MutationNamespaceMismatch);
        }
        let tx = self.db.begin_write()?;
        if let Some(receipt) = control::replay_or_check(&tx, ctx, hash)? {
            return Ok(NativeDecayResult::Committed(receipt));
        }
        let (control, ns, resolved, mut current) =
            match self.check_native_capture(&tx, &intent.scope, &intent.capture, policy, false) {
                Ok(v) => v,
                Err(
                    StorageError::NotFound(_)
                    | StorageError::NamespaceNotFound(_)
                    | StorageError::LifecycleConflict(_),
                ) => return Ok(NativeDecayResult::Skipped(NativeDecaySkip::Stale)),
                Err(e) => return Err(e),
            };
        let record = current
            .remove(&intent.memory_id)
            .ok_or(StorageError::NotFound(intent.memory_id.into_inner()))?;
        let edges = EdgeStore::incident_edges(edge_snapshot, intent.memory_id)?;
        let before = NativeRecordState::capture(
            &record,
            &ns,
            text_hashes.get(&intent.memory_id).copied().flatten(),
        )?;
        // Canonical graph and current donor rows, under the same native publication
        // guard/write transaction as the patch. No cached graph or caller bonus.
        let bonus = if record.phase == DecayPhase::Ghost {
            0.
        } else {
            canonical_bonus(
                &tx,
                edge_snapshot,
                &ns,
                &resolved,
                intent.memory_id,
                intent.sampled_at,
            )?
        };
        let evidence = NativeDecayEvidence::capture(
            &resolved,
            bonus.min(resolved.config.max_connection_bonus),
        )?;
        match plan(&before, &evidence, intent.sampled_at)? {
            PlannedDecay::Skipped(reason) => Ok(NativeDecayResult::Skipped(reason)),
            // Strength is recomputed at read time from stability, the decay anchor
            // and policy, so a pass commits (and keeps a receipt) only for a phase
            // change (#239). `plan` still refreshes strength: retained receipts
            // that did so validate against it.
            PlannedDecay::Phase(after) if after.lifecycle.phase == before.lifecycle.phase => {
                Ok(NativeDecayResult::Skipped(NativeDecaySkip::Unchanged))
            }
            PlannedDecay::Phase(after) => {
                let transition = NativePhaseWrite {
                    namespace_id: ns.id,
                    memory_id: intent.memory_id,
                    lifecycle_version: intent.capture.version,
                    policy_revision: intent.capture.revision,
                    previous_generation: control.committed_generation,
                    capture_generation: intent.capture.generation,
                    sampled_at: intent.sampled_at,
                    policy: evidence,
                    change: NativeRecordChange {
                        memory_id: intent.memory_id,
                        before: Some(before),
                        after: after.clone(),
                    },
                };
                let mut patched = record.clone();
                after.lifecycle.apply(&mut patched);
                if patched.phase != record.phase {
                    patched.text_offset = 0;
                    patched.text_length = 0;
                    if patched.phase == DecayPhase::Ghost {
                        let mut tags = tx.open_multimap_table(TAG_INDEX)?;
                        for tag in &patched.tags {
                            tags.remove(tag.as_str(), intent.memory_id.as_bytes().as_slice())?;
                        }
                        patched.summary.clear();
                        patched.tags.clear();
                        let labels = &mut patched.schema.as_mut().unwrap().labels;
                        labels.entities.clear();
                        labels.topics.clear();
                        labels.emotions.clear();
                    }
                }
                if !after.matches_record(&patched)? {
                    return Err(StorageError::InvalidControl(
                        "native phase patch differs from proof".into(),
                    ));
                }
                tx.open_table(META_TABLE)?.insert(
                    intent.memory_id.as_bytes().as_slice(),
                    patched.encode()?.as_slice(),
                )?;
                self.finish_record_effects_bounded(
                    guard,
                    tx,
                    ctx,
                    hash,
                    &edges,
                    &[],
                    &[],
                    false,
                    false,
                    effects::RecordMutation::NativePhase {
                        before: &record,
                        after: &patched,
                        transition,
                    },
                    None,
                    admission
                        .map(crate::storage::maintenance::ReceiptAdmission::limit)
                        .transpose()?,
                )
                .map(|r| NativeDecayResult::Committed(r.receipt))
            }
            PlannedDecay::Delete => {
                let mut removed_edges = edges
                    .iter()
                    .filter(|e| e.source == intent.memory_id || e.target == intent.memory_id)
                    .map(NativeEdgeProof::from)
                    .collect::<Vec<_>>();
                removed_edges.sort_by_key(|e| (e.source, e.target, e.edge_type.as_u8()));
                let mut ids = removed_edges
                    .iter()
                    .filter(|e| e.source != intent.memory_id)
                    .map(|e| e.source)
                    .collect::<Vec<_>>();
                ids.sort();
                ids.dedup();
                // The effect recounts each peer's outgoing edges, so it gets
                // every one of them besides the memory's own.
                let mut edges = edges;
                let mut peers = Vec::new();
                for id in ids {
                    edges.extend(
                        EdgeStore::incident_edges(edge_snapshot, id)?
                            .into_iter()
                            .filter(|e| e.source == id && e.target != intent.memory_id),
                    );
                    let peer = DiskRecord::from_bytes(
                        tx.open_table(META_TABLE)?
                            .get(id.as_bytes().as_slice())?
                            .ok_or(StorageError::NotFound(id.into_inner()))?
                            .value(),
                    )?;
                    let cfg = NamespaceConfig::decode(
                        tx.open_table(NAMESPACE_TABLE)?
                            .get(peer.namespace_id)?
                            .ok_or(StorageError::NamespaceNotFound(peer.namespace_id))?
                            .value(),
                    )?;
                    let state = NativeRecordState::capture(
                        &peer,
                        &cfg,
                        text_hashes.get(&id).copied().flatten(),
                    )?;
                    peers.push(NativePeerChange {
                        namespace_id: cfg.id,
                        change: NativeRecordChange {
                            memory_id: id,
                            before: Some(state.clone()),
                            after: state,
                        },
                    });
                }
                let release = before.content.vector.is_some();
                let transition = NativeHardDeleteWrite {
                    namespace_id: ns.id,
                    memory_id: intent.memory_id,
                    lifecycle_version: intent.capture.version,
                    policy_revision: intent.capture.revision,
                    previous_generation: control.committed_generation,
                    capture_generation: intent.capture.generation,
                    sampled_at: intent.sampled_at,
                    policy: evidence,
                    deletion: NativeDeletionWitness {
                        memory_id: intent.memory_id,
                        namespace_id: ns.id,
                        before,
                        history: record.access_history.clone(),
                    },
                    peers,
                    removed_edges,
                };
                self.finish_record_effects_bounded(
                    guard,
                    tx,
                    ctx,
                    hash,
                    &edges,
                    &[],
                    &[intent.memory_id],
                    true,
                    release,
                    effects::RecordMutation::NativeHardDelete(transition),
                    None,
                    admission
                        .map(crate::storage::maintenance::ReceiptAdmission::limit)
                        .transpose()?,
                )
                .map(|r| NativeDecayResult::Committed(r.receipt))
            }
        }
    }
}
/// The connection bonus of `id` over its namespace graph, read from the part
/// of that graph the bonus reaches instead of all of it: `id` with every
/// edge, its neighbours, and the neighbours of each neighbour whose fan lets
/// it contribute. Every node whose fan the bonus reads gets every
/// in-namespace edge, or enough of them that its fan contributes nothing,
/// so the result is the bonus over the whole namespace graph.
pub(super) fn canonical_bonus(
    tx: &redb::WriteTransaction,
    edge_snapshot: &redb::ReadTransaction,
    ns: &NamespaceConfig,
    policy: &crate::decay::lifecycle::EffectiveLifecyclePolicy,
    id: MemoryId,
    now: i64,
) -> Result<f32, StorageError> {
    let config = crate::graph::activation::ActivationConfig::default();
    let mut local = Neighbourhood {
        rows: tx.open_table(META_TABLE)?,
        edge_snapshot,
        ns,
        policy,
        now,
        s_max: config.s_max,
        graph: crate::graph::RelationshipGraph::new(),
        donors: HashMap::new(),
        members: HashMap::new(),
        added: std::collections::HashSet::new(),
        expanded: HashMap::new(),
    };
    if !local.member(id)? {
        return Ok(0.);
    }
    for j in local.expand(id, false)? {
        let next = local.expand(j, true)?;
        if config.include_2hop && local.contributes(j) {
            for k in next {
                local.expand(k, true)?;
            }
        }
    }
    Ok(crate::graph::activation::connection_bonus_with_strengths(
        id,
        &local.graph,
        &config,
        &local.donors,
    ))
}

/// Canonical rows (read by ID) and edges around one memory, as the
/// namespace graph and donor strengths hold them.
struct Neighbourhood<'a, 't> {
    rows: redb::Table<'t, &'static [u8], &'static [u8]>,
    edge_snapshot: &'a redb::ReadTransaction,
    ns: &'a NamespaceConfig,
    policy: &'a crate::decay::lifecycle::EffectiveLifecyclePolicy,
    now: i64,
    s_max: f32,
    graph: crate::graph::RelationshipGraph,
    donors: HashMap<MemoryId, f32>,
    members: HashMap<MemoryId, bool>,
    added: std::collections::HashSet<(MemoryId, crate::model::EdgeType, MemoryId)>,
    expanded: HashMap<MemoryId, Vec<MemoryId>>,
}

impl Neighbourhood<'_, '_> {
    /// Whether `id` is a row of the namespace; adds it as a node (and donor)
    /// the first time.
    fn member(&mut self, id: MemoryId) -> Result<bool, StorageError> {
        if let Some(&member) = self.members.get(&id) {
            return Ok(member);
        }
        let row = match self.rows.get(id.as_bytes().as_slice())? {
            Some(bytes) => Some(DiskRecord::from_bytes(bytes.value())?),
            None => None,
        };
        let r = row.filter(|r| r.namespace_id == self.ns.id.get());
        self.members.insert(id, r.is_some());
        let Some(r) = r else {
            return Ok(false);
        };
        self.graph
            .add_node(id, self.ns.id, r.phase, r.strength, r.vector_slot)
            .map_err(|e| StorageError::InvalidControl(e.to_string()))?;
        let state = crate::storage::native_lifecycle::LifecycleState::capture(&r)?;
        if state.eligible() {
            let p = crate::decay::lifecycle::project(
                r.stability,
                state.lifecycle.decay_anchor_at,
                self.policy,
                self.now.max(state.lifecycle.lifecycle_updated_at),
                0.,
            )
            .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
            self.donors.insert(id, p.strength);
        }
        Ok(true)
    }

    fn contributes(&self, id: MemoryId) -> bool {
        crate::graph::activation::fan_strength(self.s_max, self.graph.degree(&id)) > 0.
    }

    /// Adds the in-namespace edges of `id` and returns its neighbours. With
    /// `saturate`, stops once its fan contributes nothing: then the
    /// neighbours are partial, but the bonus never expands from `id`.
    fn expand(&mut self, id: MemoryId, saturate: bool) -> Result<Vec<MemoryId>, StorageError> {
        if let Some(neighbours) = self.expanded.get(&id) {
            return Ok(neighbours.clone());
        }
        let mut neighbours = Vec::new();
        for e in EdgeStore::incident_edges(self.edge_snapshot, id)? {
            if saturate && !self.contributes(id) {
                break;
            }
            let other = if e.source == id { e.target } else { e.source };
            if !self.member(other)? {
                continue;
            }
            if self.added.insert((e.source, e.edge_type, e.target)) {
                self.graph
                    .add_edge(e.source, e.target, e.edge_type, e.weight, e.auto_created)
                    .map_err(|e| StorageError::InvalidControl(e.to_string()))?;
            }
            neighbours.push(other);
        }
        neighbours.sort();
        neighbours.dedup();
        self.expanded.insert(id, neighbours.clone());
        Ok(neighbours)
    }
}

pub(super) fn validate_effect(e: &effects::PendingEffects) -> Result<(), StorageError> {
    let bad =
        || StorageError::InvalidControl("native maintenance effect differs from receipt".into());
    match &e.receipt.outcome {
        MutationOutcome::NativePhaseMaterialized { transition: t } => {
            t.validate(&e.receipt)?;
            if e.scope != MutationScope::Namespace(t.namespace_id)
                || !e.puts.is_empty()
                || !e.deletes.is_empty()
                || !e.releases.is_empty()
                || !e.removed_ids.is_empty()
                || !e.peer_counts.is_empty()
                || e.invalidated_ids != [t.memory_id]
            {
                return Err(bad());
            }
        }
        MutationOutcome::NativeMemoryHardDeleted { transition: t } => {
            t.validate(&e.receipt)?;
            let mut edges = e
                .deletes
                .iter()
                .map(NativeEdgeProof::from)
                .collect::<Vec<_>>();
            edges.sort_by_key(|e| (e.source, e.target, e.edge_type.as_u8()));
            let mut peers = t
                .peers
                .iter()
                .map(|p| (p.change.memory_id, p.change.after.edge_count))
                .collect::<Vec<_>>();
            peers.sort();
            let mut invalidated = peers
                .iter()
                .map(|(id, _)| *id)
                .chain([t.memory_id])
                .collect::<Vec<_>>();
            invalidated.sort();
            invalidated.dedup();
            let slot = t.deletion.before.content.vector.as_ref().map(|v| v.slot);
            if e.scope != MutationScope::Namespace(t.namespace_id)
                || !e.puts.is_empty()
                || edges != t.removed_edges
                || e.removed_ids != [t.memory_id]
                || e.peer_counts != peers
                || e.invalidated_ids != invalidated
                || e.releases.len() != usize::from(slot.is_some())
                || e.releases.iter().any(|r| {
                    r.namespace_id != t.namespace_id
                        || r.memory_id != t.memory_id
                        || Some(r.slot) != slot
                })
            {
                return Err(bad());
            }
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn validate_pending_records(
    e: &effects::PendingEffects,
    records: &[(MemoryId, DiskRecord)],
) -> Result<(), StorageError> {
    let check =
        |id: MemoryId, ns: NamespaceId, state: &NativeRecordState| -> Result<(), StorageError> {
            let r = records
                .iter()
                .find(|(key, _)| *key == id)
                .map(|(_, r)| r)
                .ok_or_else(|| {
                    StorageError::InvalidControl("native maintenance pending row absent".into())
                })?;
            if r.namespace_id != ns.get() || !state.matches_record(r)? {
                return Err(StorageError::InvalidControl(
                    "native maintenance pending row mismatch".into(),
                ));
            }
            Ok(())
        };
    match &e.receipt.outcome {
        MutationOutcome::NativePhaseMaterialized { transition: t } => {
            check(t.memory_id, t.namespace_id, &t.change.after)?
        }
        MutationOutcome::NativeMemoryHardDeleted { transition: t } => {
            for edge in &t.removed_edges {
                for id in [edge.source, edge.target] {
                    if id != t.memory_id && !records.iter().any(|(key, _)| *key == id) {
                        return Err(StorageError::InvalidControl(
                            "native deletion has an unproven missing endpoint".into(),
                        ));
                    }
                }
            }
            for p in &t.peers {
                check(p.change.memory_id, p.namespace_id, &p.change.after)?;
            }
        }
        _ => {}
    }
    Ok(())
}
