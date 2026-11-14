use super::*;
use crate::storage::native_graph::*;
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs, invalid};
use crate::storage::native_records::{NativeEdgeProof, NativeRecordChange, NativeRecordState};
use crate::time::Clock;

fn context(ctx: &MutationContext, namespace: NamespaceId) -> Result<(), StorageError> {
    ctx.validate()?;
    if ctx.contract_version != 2 || ctx.scope != MutationScope::Namespace(namespace) {
        return Err(StorageError::InvalidMutationContext(
            "native graph requires contract 2 and its captured namespace".into(),
        ));
    }
    Ok(())
}
impl MetadataStore {
    pub(crate) fn capture_native_graph(
        &self,
        scope: LifecycleScope,
        original: &NativeGraphInput,
        policy: &NativePolicyInputs,
        parent: Option<&WorkTicket>,
    ) -> Result<NativeGraphIntent, StorageError> {
        original.validate()?;
        let _guard = self.native_guard_with_work(parent)?;
        let ids: std::collections::BTreeSet<_> = original
            .edges
            .iter()
            .flat_map(|e| [e.source, e.target])
            .collect();
        let capture = self.capture_at(
            &self.db.begin_read()?,
            &scope,
            &ids.into_iter().collect::<Vec<_>>(),
            policy,
            false,
        )?;
        Ok(NativeGraphIntent {
            scope,
            original: original.clone(),
            capture,
        })
    }
    pub(crate) fn lookup_native_graph(
        &self,
        ctx: &MutationContext,
        original: &NativeGraphInput,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let MutationScope::Namespace(namespace) = ctx.scope else {
            return Err(StorageError::MutationNamespaceMismatch);
        };
        context(ctx, namespace)?;
        let hash = original.request_hash(ctx)?;
        let _guard = self.native_guard_with_work(parent)?;
        control::replay_or_check(&self.db.begin_write()?, ctx, hash)
    }
    pub(crate) fn apply_native_graph(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        intent: &NativeGraphIntent,
        policy: &NativePolicyInputs,
        clock: &dyn Clock,
        existing: &[crate::storage::PersistedEdge],
        hashes: &HashMap<MemoryId, Option<[u8; 32]>>,
    ) -> Result<Option<effects::EffectResult>, StorageError> {
        guard.check(self)?;
        context(ctx, intent.scope.namespace_id)?;
        let hash = intent.original.request_hash(ctx)?;
        let tx = self.db.begin_write()?;
        if let Some(receipt) = control::replay_or_check(&tx, ctx, hash)? {
            return replay_result(receipt).map(Some);
        }
        let (state, ns, _, current) =
            self.check_native_capture(&tx, &intent.scope, &intent.capture, policy, false)?;
        let mut changes = Vec::new();
        for e in &intent.original.edges {
            for (id, source) in [(e.source, true), (e.target, false)] {
                let row = current
                    .get(&id)
                    .ok_or_else(|| invalid("native graph capture endpoint missing"))?;
                if !endpoint_allowed(e.edge_type, source, row.phase) {
                    return Err(StorageError::LifecycleContentUnavailable);
                }
            }
            let before = existing.iter().find(|old| {
                old.source == e.source && old.target == e.target && old.edge_type == e.edge_type
            });
            if before.is_some_and(|old| {
                old.weight.to_bits() == e.weight.to_bits() && old.auto_created == e.auto_created
            }) {
                continue;
            }
            changes.push(NativeEdgeChange {
                before: before.map(NativeEdgeProof::from),
                after: NativeEdgeProof {
                    source: e.source,
                    target: e.target,
                    edge_type: e.edge_type,
                    weight_bits: e.weight.to_bits(),
                    auto_created: e.auto_created,
                    created_at: 0,
                },
            });
        }
        // A keyed no-op reserves its outcome in the same receipt/outbox path.
        // Witnesses preserve the existing timestamp; no clock or peer token changes.
        if changes.is_empty() {
            if ctx.idempotency_key.is_none() {
                return Ok(None);
            }
            let mut records = Vec::with_capacity(current.len());
            for (id, row) in &current {
                let before = NativeRecordState::capture(
                    row,
                    &ns,
                    *hashes
                        .get(id)
                        .ok_or_else(|| invalid("native graph text commitment missing"))?,
                )?;
                records.push(NativeRecordChange {
                    memory_id: *id,
                    before: Some(before.clone()),
                    after: before,
                });
            }
            records.sort_by_key(|r| r.memory_id);
            let edges = intent
                .original
                .edges
                .iter()
                .map(|e| {
                    existing
                        .iter()
                        .find(|old| {
                            old.source == e.source
                                && old.target == e.target
                                && old.edge_type == e.edge_type
                        })
                        .map(NativeEdgeProof::from)
                        .ok_or_else(|| invalid("native graph no-change edge missing"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let transition = NativeGraphNoChange {
                namespace_id: ns.id,
                lifecycle_version: intent.capture.version,
                policy_revision: intent.capture.revision,
                capture_generation: intent.capture.generation,
                previous_generation: state.committed_generation,
                edges,
                records,
            };
            return self
                .finish_record_effects(
                    guard,
                    tx,
                    ctx,
                    hash,
                    existing,
                    &[],
                    &[],
                    false,
                    false,
                    effects::RecordMutation::NativeGraphUnchanged(transition),
                    None,
                )
                .map(Some);
        }
        let sampled = clock.now_millis();
        if chrono::DateTime::from_timestamp_millis(sampled).is_none() {
            return Err(StorageError::InvalidMutation(
                "invalid native graph clock sample".into(),
            ));
        }
        for edge in &mut changes {
            edge.after.created_at = sampled.max(0) as u64;
        }
        let endpoints: std::collections::BTreeSet<_> = changes
            .iter()
            .flat_map(|e| [e.after.source, e.after.target])
            .collect();
        let mut records = Vec::with_capacity(endpoints.len());
        for id in endpoints {
            let before = NativeRecordState::capture(
                &current[&id],
                &ns,
                *hashes
                    .get(&id)
                    .ok_or_else(|| invalid("native graph text commitment missing"))?,
            )?;
            records.push(NativeRecordChange {
                memory_id: id,
                before: Some(before.clone()),
                after: before,
            });
        }
        let transition = NativeGraphWrite {
            namespace_id: ns.id,
            lifecycle_version: intent.capture.version,
            policy_revision: intent.capture.revision,
            capture_generation: intent.capture.generation,
            previous_generation: state.committed_generation,
            sampled_at: sampled,
            edges: changes,
            records,
        };
        let puts = transition.puts();
        self.finish_record_effects(
            guard,
            tx,
            ctx,
            hash,
            existing,
            &puts,
            &[],
            false,
            false,
            effects::RecordMutation::NativeGraph(transition),
                    None,
        )
        .map(Some)
    }
}
pub(crate) fn replay_result(receipt: WriteReceipt) -> Result<effects::EffectResult, StorageError> {
    let (peer_counts, edges_put) = match &receipt.outcome {
        MutationOutcome::NativeGraphChanged { transition } => {
            (transition.peer_counts(), transition.edges.len())
        }
        MutationOutcome::NativeGraphUnchanged { .. } => (vec![], 0),
        _ => return Err(StorageError::IdempotencyConflict),
    };
    Ok(effects::EffectResult {
        peer_counts,
        edges_put,
        edges_removed: 0,
        removed: vec![],
        receipt,
    })
}
