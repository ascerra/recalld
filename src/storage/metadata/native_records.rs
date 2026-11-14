//! Native record transactions use the existing publication, payload staging and
//! canonical effect tail. No public serving or derived-readiness boundary.
use super::*;
use crate::model::schema::{LifecycleFields, RecordSchemaV2, TimestampOrigin, VectorReference};
use crate::storage::batch::BatchChild;
use crate::storage::control::Generation;
use crate::storage::edges::PersistedEdge;
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs, invalid};
use crate::storage::native_records::*;
use crate::time::Clock;

fn context(ctx: &MutationContext, namespace: NamespaceId) -> Result<(), StorageError> {
    ctx.validate()?;
    if ctx.scope != MutationScope::Namespace(namespace) {
        return Err(StorageError::MutationNamespaceMismatch);
    }
    Ok(())
}

fn replay_tombstone(
    tx: &redb::WriteTransaction,
    ctx: &MutationContext,
    id: MemoryId,
) -> Result<Option<WriteReceipt>, StorageError> {
    let MutationScope::Namespace(namespace) = ctx.scope else {
        return Err(StorageError::MutationNamespaceMismatch);
    };
    context(ctx, namespace)?;
    let hash = control::request_hash(ctx, "native_tombstone", &[id.as_bytes()]);
    control::replay_or_check(tx, ctx, hash)
}

pub(super) fn replay_insert(
    tx: &redb::WriteTransaction,
    ctx: &MutationContext,
    input: &NativeInsert,
) -> Result<Option<WriteReceipt>, StorageError> {
    let MutationScope::Namespace(namespace) = ctx.scope else {
        return Err(StorageError::MutationNamespaceMismatch);
    };
    context(ctx, namespace)?;
    let Some(stored) = control::keyed_receipt(tx, ctx)? else {
        return Ok(None);
    };
    let MutationOutcome::NativeMemoryInserted { transition } = &stored.receipt.outcome else {
        return Err(StorageError::IdempotencyConflict);
    };
    let original = &transition.primary()?.after;
    let hash = input.request_hash(
        ctx,
        original.lifecycle.stability.get(),
        original.lifecycle.difficulty.get(),
        transition.embedding_dim,
        original
            .content
            .profile_id
            .as_deref()
            .ok_or_else(|| invalid("creation profile missing"))?,
    )?;
    control::replay_or_check(tx, ctx, hash)
}

impl MetadataStore {
    pub(crate) fn capture_native_insert(
        &self,
        scope: LifecycleScope,
        input: &NativeInsert,
        policy: &NativePolicyInputs,
    ) -> Result<NativeInsertIntent, StorageError> {
        self.ensure_available()?;
        let _publication = self.publication.read().unwrap_or_else(|e| e.into_inner());
        self.ensure_available()?;
        input.validate()?;
        let read = self.db.begin_read()?;
        let mut ids = Vec::new();
        for id in [input.memory.parent_id, input.memory.supersedes]
            .into_iter()
            .flatten()
        {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        let capture = self.capture_at(&read, &scope, &ids, policy, false)?;
        let namespace = NamespaceConfig::decode(
            read.open_table(NAMESPACE_TABLE)?
                .get(scope.namespace_id.get())?
                .ok_or(StorageError::NamespaceNotFound(scope.namespace_id.get()))?
                .value(),
        )?;
        if profile_id(&namespace).is_none() {
            return Err(StorageError::InvalidMutation(
                "native insertion requires an explicitly bound namespace profile".into(),
            ));
        }
        Ok(NativeInsertIntent {
            scope,
            original: input.clone(),
            namespace,
            capture,
        })
    }
    pub(crate) fn capture_native_tombstone(
        &self,
        id: MemoryId,
        namespace: Option<NamespaceId>,
        policy: &NativePolicyInputs,
    ) -> Result<NativeTombstoneIntent, StorageError> {
        self.ensure_available()?;
        let _publication = self.publication.read().unwrap_or_else(|e| e.into_inner());
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        let record = DiskRecord::from_bytes(
            read.open_table(META_TABLE)?
                .get(id.as_bytes().as_slice())?
                .ok_or(StorageError::NotFound(id.into_inner()))?
                .value(),
        )?;
        let ns = NamespaceId::new(record.namespace_id);
        if namespace.is_some_and(|n| n != ns) {
            return Err(StorageError::MutationNamespaceMismatch);
        }
        let cfg = NamespaceConfig::decode(
            read.open_table(NAMESPACE_TABLE)?
                .get(ns.get())?
                .ok_or(StorageError::NamespaceNotFound(ns.get()))?
                .value(),
        )?;
        let scope = LifecycleScope {
            namespace_id: ns,
            namespace_name: cfg.name,
            source: None,
        };
        let capture = self.capture_at(&read, &scope, &[id], policy, false)?;
        Ok(NativeTombstoneIntent {
            scope,
            memory_id: id,
            capture,
        })
    }
    pub(crate) fn lookup_native_insert(
        &self,
        ctx: &MutationContext,
        input: &NativeInsert,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let _guard = self.native_guard_with_work(parent)?;
        replay_insert(&self.db.begin_write()?, ctx, input)
    }
    pub(crate) fn lookup_native_insert_child(
        &self,
        child: &BatchChild,
        input: &NativeInsert,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let _guard = self.native_guard_with_work(parent)?;
        let tx = self.db.begin_write()?;
        if child.version != 2 {
            return Err(StorageError::InvalidMutationContext(
                "historical native insert requires batch 2".into(),
            ));
        }
        let hash =
            crate::storage::batch::native::insert_hash(&child.context(), input, &child.defaults)?;
        let replay = super::batch::replay_child(&tx, child, hash)?;
        if replay.is_none() {
            super::batch::check_policy(&tx, child)?;
        }
        Ok(replay)
    }
    pub(crate) fn lookup_native_forget_child(
        &self,
        child: &BatchChild,
        id: MemoryId,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let _guard = self.native_guard_with_work(parent)?;
        let tx = self.db.begin_write()?;
        if child.version != 2 {
            return Err(StorageError::InvalidMutationContext(
                "native forget requires batch 2".into(),
            ));
        }
        let hash = control::request_hash(&child.context(), "native_tombstone", &[id.as_bytes()]);
        let replay = super::batch::replay_child(&tx, child, hash)?;
        if replay.is_none() {
            super::batch::check_policy(&tx, child)?;
        }
        Ok(replay)
    }
    pub(crate) fn insert_native_with_effects(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        intent: &NativeInsertIntent,
        prepared: Option<&crate::storage::native_store::PreparedNativeStore>,
        policy: &NativePolicyInputs,
        clock: &dyn Clock,
        embedding: &[f32],
        profile: &crate::model::schema::EmbeddingProfile,
        existing_edges: &[PersistedEdge],
        text_hashes: &HashMap<MemoryId, Option<[u8; 32]>>,
        stage: impl FnOnce(&NamespaceConfig, &mut DiskRecord, &[f32]) -> Result<(), StorageError>,
        child: Option<&BatchChild>,
    ) -> Result<WriteReceipt, StorageError> {
        guard.check(self)?;
        context(ctx, intent.scope.namespace_id)?;
        let tx = self.db.begin_write()?;
        let store_input = prepared.map(|p| crate::storage::native_store::NativeStoreInput {
            insert: intent.original.clone(),
            check_duplicates: p.intent.check_duplicates,
        });
        if let Some(child) = child {
            check_child_context(ctx, child)?;
            let hash = match (&store_input, prepared, child.version) {
                (Some(input), Some(p), 3) => {
                    if child
                        .defaults
                        .native
                        .as_ref()
                        .and_then(|n| n.store_settings.as_ref())
                        != Some(&p.intent.settings)
                    {
                        return Err(StorageError::IdempotencyConflict);
                    }
                    crate::storage::batch::native::store_hash(ctx, input, &child.defaults)?
                }
                (None, None, 2) => crate::storage::batch::native::insert_hash(
                    ctx,
                    &intent.original,
                    &child.defaults,
                )?,
                _ => {
                    return Err(StorageError::InvalidMutationContext(
                        "native writer/batch version mismatch".into(),
                    ));
                }
            };
            if let Some(receipt) = super::batch::replay_child(&tx, child, hash)? {
                return Ok(receipt);
            }
            super::batch::check_policy(&tx, child)?;
        }
        let replay = if let Some(input) = &store_input {
            super::native_store::replay_store(&tx, ctx, input)?
        } else {
            replay_insert(&tx, ctx, &intent.original)?
        };
        if let Some(receipt) = replay {
            return Ok(receipt);
        }
        let (state, ns, _, current) =
            self.check_native_capture(&tx, &intent.scope, &intent.capture, policy, false)?;
        if prepared.is_some() && state.generation_ref() != intent.capture.generation {
            return Err(StorageError::LifecycleConflict(
                "linked store generation changed before allocation".into(),
            ));
        }
        let generation = state.committed_generation.next()?;
        let input = &intent.original;
        input.validate()?;
        let binding = crate::embedding::profile::require_compatible(&ns, profile).map_err(|e| {
            if prepared.is_some() && input.memory.embedding.is_none() {
                StorageError::ProviderVectorUnavailable(e.to_string())
            } else {
                StorageError::InvalidMutation(e.to_string())
            }
        })?;
        if let Some(supplied) = &input.embedding_profile {
            crate::embedding::profile::require_compatible(&ns, supplied)
                .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
        }
        let stability = input
            .memory
            .initial_stability
            .unwrap_or(ns.initial_stability);
        if !stability.is_finite() || stability <= 0. {
            return Err(StorageError::InvalidMutation(
                "invalid initial stability".into(),
            ));
        }
        let hash = if let Some(child) = child {
            if let Some(input) = &store_input {
                crate::storage::batch::native::store_hash(ctx, input, &child.defaults)?
            } else {
                crate::storage::batch::native::insert_hash(ctx, input, &child.defaults)?
            }
        } else if let Some(p) = prepared {
            store_input.as_ref().unwrap().request_hash(
                ctx,
                stability,
                ns.default_difficulty,
                ns.embedding_dim,
                &binding,
                &p.intent.settings,
            )?
        } else {
            input.request_hash(
                ctx,
                stability,
                ns.default_difficulty,
                ns.embedding_dim,
                &binding,
            )?
        };
        control::replay_or_check(&tx, ctx, hash)?;
        let mut normalized = embedding.to_vec();
        crate::storage::insertion::normalize_embedding(
            &mut normalized,
            ns.embedding_dim,
            input.memory.embedding.is_some(),
        )?;
        let id = self.allocate_memory_id(input.memory.id);
        if id == MemoryId::nil() {
            return Err(StorageError::InvalidMutation("nil memory ID".into()));
        }
        removal_ledger::ensure_insertable(&tx, id)?;
        if tx
            .open_table(META_TABLE)?
            .get(id.as_bytes().as_slice())?
            .is_some()
        {
            return Err(StorageError::DuplicateId(id.into_inner()));
        }
        // A supplied identity may also survive only in preserved creation history
        // after a fork. No new index/schema is invented for this infrequent path.
        if input.memory.id.is_some() {
            for row in tx.open_table(control::RECEIPTS)?.iter()? {
                let (_, bytes) = row?;
                let historical = control::validate_receipt(bytes.value())?.receipt;
                if matches!(&historical.outcome, MutationOutcome::NativeMemoryInserted { transition } if transition.memory_id==id)
                    || matches!(&historical.outcome, MutationOutcome::NativeMemoryInsertedWithLinks { transition } if transition.memory_id==id)
                {
                    return Err(StorageError::DuplicateId(id.into_inner()));
                }
            }
        }
        super::insertion::validate_insert_targets(&tx, &input.memory, ns.id, id, existing_edges)?;
        let sampled = clock.now_millis();
        let created = input.memory.created_at.unwrap_or(sampled);
        if chrono::DateTime::from_timestamp_millis(sampled).is_none() {
            return Err(StorageError::InvalidMutation("invalid clock sample".into()));
        }
        let mut automatic = Vec::new();
        if let Some(p) = prepared {
            automatic.extend(p.links.static_links.iter().copied());
            automatic.extend(crate::storage::native_store::select_temporal_links(
                &p.links.temporal,
                created,
                &p.intent.settings,
            ));
            if automatic.len()
                + usize::from(input.memory.parent_id.is_some())
                + usize::from(input.memory.supersedes.is_some())
                > crate::storage::native_graph::MAX_NATIVE_GRAPH_EDGES
            {
                return Err(StorageError::InvalidMutation(
                    "linked insertion exceeds edge allowance".into(),
                ));
            }
            if input
                .memory
                .parent_id
                .is_some_and(|id| current.get(&id).is_some_and(|r| r.edge_count == u16::MAX))
            {
                return Err(StorageError::InvalidMutation(
                    "linked parent outgoing count overflow".into(),
                ));
            }
        }
        let vector_hash = hash_bytes(
            &normalized
                .iter()
                .flat_map(|f| f.to_le_bytes())
                .collect::<Vec<_>>(),
        );
        let mut record = DiskRecord {
            version: 2,
            id: *id.as_bytes(),
            namespace_id: ns.id.get(),
            created_at: created,
            last_accessed_at: created,
            phase: DecayPhase::Full,
            strength: 1.,
            decay_strength: 1.,
            stability,
            difficulty: ns.default_difficulty,
            is_permastore: 0,
            vector_slot: 0,
            edge_count: 0,
            summary: input.memory.summary.clone(),
            tags: input.memory.tags.clone(),
            access_history: vec![],
            text_offset: 0,
            text_length: 0,
            schema: Some(RecordSchemaV2 {
                lifecycle: LifecycleFields {
                    observed_at: input.observed_at.unwrap_or(created),
                    retrieved_at: None,
                    reinforced_at: None,
                    decay_anchor_at: created,
                    lifecycle_version: intent.capture.version,
                    lifecycle_updated_at: sampled.max(created),
                    timestamp_origin: TimestampOrigin::Native,
                    phase_changed_at: created,
                    policy_revision: intent.capture.revision,
                    record_generation: Generation::default().next()?,
                },
                labels: input.labels(),
                vector: Some(VectorReference {
                    slot: 0,
                    allocation_epoch: generation,
                    sha256: vector_hash,
                }),
                required_features: vec![],
                extensions: Default::default(),
            }),
        };
        let mut puts = super::insertion::stage_insert(
            &tx,
            &input.memory,
            &ns,
            &mut record,
            &normalized,
            sampled,
            stage,
        )?;
        puts.extend(
            automatic
                .into_iter()
                .map(|(target, edge_type, weight)| PersistedEdge {
                    source: id,
                    target,
                    edge_type,
                    weight,
                    auto_created: true,
                    created_at: sampled.max(0) as u64,
                }),
        );
        let after = NativeRecordState::capture(
            &record,
            &ns,
            input
                .memory
                .full_text
                .as_ref()
                .map(|s| hash_bytes(s.as_bytes())),
        )?;
        let mut records = vec![NativeRecordChange {
            memory_id: id,
            before: None,
            after,
        }];
        if let Some(parent) = input.memory.parent_id {
            let before = NativeRecordState::capture(
                current
                    .get(&parent)
                    .ok_or_else(|| invalid("parent capture missing"))?,
                &ns,
                *text_hashes
                    .get(&parent)
                    .ok_or_else(|| invalid("parent text snapshot missing"))?,
            )?;
            records.push(NativeRecordChange {
                memory_id: parent,
                before: Some(before.clone()),
                after: before,
            });
        }
        let mutation = if let Some(p) = prepared {
            let mut peers = std::collections::BTreeSet::new();
            for edge in &puts {
                peers.extend([edge.source, edge.target]);
            }
            if let Some(a) = &p.links.advisory {
                peers.extend(a.iter().map(|&(id, _)| id));
            }
            peers.remove(&id);
            records.truncate(1);
            for peer in peers {
                let before = NativeRecordState::capture(
                    current
                        .get(&peer)
                        .ok_or_else(|| invalid("linked store peer capture missing"))?,
                    &ns,
                    *text_hashes
                        .get(&peer)
                        .ok_or_else(|| invalid("linked store peer text commitment missing"))?,
                )?;
                records.push(NativeRecordChange {
                    memory_id: peer,
                    before: Some(before.clone()),
                    after: before,
                });
            }
            effects::RecordMutation::NativeStored(
                &record,
                crate::storage::native_store::NativeStoreWrite {
                    namespace_id: ns.id,
                    memory_id: id,
                    lifecycle_version: intent.capture.version,
                    policy_revision: intent.capture.revision,
                    previous_generation: state.committed_generation,
                    capture_generation: intent.capture.generation,
                    sampled_at: sampled,
                    embedding_dim: ns.embedding_dim,
                    records,
                    edges: puts.iter().map(NativeEdgeProof::from).collect(),
                    parent_id: input.memory.parent_id,
                    supersedes: input.memory.supersedes,
                    settings: p.intent.settings.clone(),
                    check_duplicates: p.intent.check_duplicates,
                    advisory: p.links.advisory.as_ref().map(|a| {
                        a.iter()
                            .map(|&(memory_id, score)| {
                                crate::storage::native_store::NativeAdvisoryMatch {
                                    memory_id,
                                    score_bits: score.to_bits(),
                                }
                            })
                            .collect()
                    }),
                },
            )
        } else {
            let transition = NativeRecordWrite {
                namespace_id: ns.id,
                memory_id: id,
                lifecycle_version: intent.capture.version,
                policy_revision: intent.capture.revision,
                previous_generation: state.committed_generation,
                capture_generation: intent.capture.generation,
                sampled_at: sampled,
                embedding_dim: ns.embedding_dim,
                records,
                edges: puts.iter().map(NativeEdgeProof::from).collect(),
                phase_time_rule: None,
            };
            effects::RecordMutation::NativeInserted(&record, transition)
        };
        self.finish_record_effects(
            guard,
            tx,
            ctx,
            hash,
            existing_edges,
            &puts,
            &[],
            false,
            false,
            mutation,
            child,
        )
        .map(|r| r.receipt)
    }
    /// Stable caller identity lookup before resolving current record/policy/name.
    pub(crate) fn lookup_native_tombstone_by_id(
        &self,
        ctx: &MutationContext,
        id: MemoryId,
        asserted_namespace: Option<&str>,
        parent: Option<&super::super::work::WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let guard = self.native_guard_with_work(parent)?;
        let tx = self.db.begin_write()?;
        let receipt = replay_tombstone(&tx, ctx, id)?;
        if let (Some(receipt), Some(name)) = (&receipt, asserted_namespace) {
            let assertion = (|| {
                let MutationScope::Namespace(namespace) = ctx.scope else {
                    return Err(StorageError::MutationNamespaceMismatch);
                };
                let binding = self.replay_namespace_binding_at(&guard, &tx, namespace)?;
                if binding.name != name {
                    return Err(StorageError::NamespaceMismatch {
                        id: id.into_inner(),
                        namespace: name.to_owned(),
                    });
                }
                Ok(())
            })();
            assertion.map_err(|source| StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            })?;
        }
        Ok(receipt)
    }
    pub(crate) fn lookup_native_tombstone(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        intent: &NativeTombstoneIntent,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        guard.check(self)?;
        context(ctx, intent.scope.namespace_id)?;
        replay_tombstone(&self.db.begin_write()?, ctx, intent.memory_id)
    }
    pub(crate) fn tombstone_native_with_effects(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        intent: &NativeTombstoneIntent,
        policy: &NativePolicyInputs,
        clock: &dyn Clock,
        existing_edges: &[PersistedEdge],
        text_hash: Option<[u8; 32]>,
        child: Option<&BatchChild>,
    ) -> Result<WriteReceipt, StorageError> {
        guard.check(self)?;
        context(ctx, intent.scope.namespace_id)?;
        let tx = self.db.begin_write()?;
        let hash = control::request_hash(ctx, "native_tombstone", &[intent.memory_id.as_bytes()]);
        if let Some(child) = child {
            check_child_context(ctx, child)?;
            if let Some(receipt) = super::batch::replay_child(&tx, child, hash)? {
                return Ok(receipt);
            }
            super::batch::check_policy(&tx, child)?;
        } else if let Some(receipt) = control::replay_or_check(&tx, ctx, hash)? {
            return Ok(receipt);
        }
        let (state, ns, _, mut current) =
            self.check_native_capture(&tx, &intent.scope, &intent.capture, policy, false)?;
        state.committed_generation.next()?;
        let mut record = current
            .remove(&intent.memory_id)
            .ok_or_else(|| invalid("tombstone capture missing"))?;
        let before = NativeRecordState::capture(&record, &ns, text_hash)?;
        // Fail unknown extension consumers before time, metadata, or payload changes.
        before.content.erased()?;
        let sampled = clock.now_millis();
        if chrono::DateTime::from_timestamp_millis(sampled).is_none() {
            return Err(StorageError::InvalidMutation("invalid clock sample".into()));
        }
        let phase_time_rule = Some(PhaseTimeRule::MonotonicMarkerV1);
        let after = before.tombstone(sampled, phase_time_rule)?;
        let old_phase = record.phase;
        if old_phase != DecayPhase::Tombstone {
            for tag in &record.tags {
                tx.open_multimap_table(TAG_INDEX)?
                    .remove(tag.as_str(), intent.memory_id.as_bytes().as_slice())?;
            }
            tx.open_multimap_table(NAMESPACE_INDEX)?
                .remove(record.namespace_id, intent.memory_id.as_bytes().as_slice())?;
            after.lifecycle.apply(&mut record);
            record.summary.clear();
            record.tags.clear();
            record.text_offset = 0;
            record.text_length = 0;
            let fields = record.schema.as_mut().unwrap();
            fields.labels.entities.clear();
            fields.labels.topics.clear();
            fields.labels.emotions.clear();
            fields.vector = None;
            tx.open_table(META_TABLE)?.insert(
                intent.memory_id.as_bytes().as_slice(),
                record.encode()?.as_slice(),
            )?;
        }
        let transition = NativeRecordWrite {
            namespace_id: ns.id,
            memory_id: intent.memory_id,
            lifecycle_version: intent.capture.version,
            policy_revision: intent.capture.revision,
            previous_generation: state.committed_generation,
            capture_generation: intent.capture.generation,
            sampled_at: sampled,
            embedding_dim: ns.embedding_dim,
            records: vec![NativeRecordChange {
                memory_id: intent.memory_id,
                before: Some(before),
                after,
            }],
            edges: vec![],
            phase_time_rule,
        };
        self.finish_record_effects(
            guard,
            tx,
            ctx,
            hash,
            existing_edges,
            &[],
            &[],
            false,
            false,
            effects::RecordMutation::NativeTombstoned {
                record: &record,
                old_phase,
                transition,
            },
            child,
        )
        .map(|r| r.receipt)
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn check_child_context(
    ctx: &MutationContext,
    child: &BatchChild,
) -> Result<(), StorageError> {
    if !matches!(child.version, 2 | 3)
        || ctx.contract_version != 2
        || ctx.store_id != Some(child.store_id)
        || ctx.scope != MutationScope::Namespace(child.namespace())
        || ctx.idempotency_key.is_some()
        || ctx.expected_generation.is_some()
    {
        return Err(StorageError::InvalidMutationContext(
            "foreign native batch child context".into(),
        ));
    }
    Ok(())
}
