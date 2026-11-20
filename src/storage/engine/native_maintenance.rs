use super::*;
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::storage::native_maintenance::{NativeDecayIntent, NativeDecayResult};
use crate::storage::work::WorkTicket;
use std::collections::HashMap;
impl RedbStorageEngine {
    pub(crate) fn lookup_native_decay(
        &self,
        ctx: &MutationContext,
        id: MemoryId,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let guard = self.meta_store.native_guard_with_work(parent)?;
        self.meta_store.lookup_native_decay(&guard, ctx, id)
    }
    pub(crate) fn apply_native_decay(
        &mut self,
        ctx: &MutationContext,
        intent: &NativeDecayIntent,
        policy: &NativePolicyInputs,
        parent: Option<&WorkTicket>,
    ) -> Result<NativeDecayResult, StorageError> {
        self.apply_native_decay_bounded(ctx, intent, policy, parent, None)
    }

    pub(crate) fn apply_native_decay_bounded(
        &mut self,
        ctx: &MutationContext,
        intent: &NativeDecayIntent,
        policy: &NativePolicyInputs,
        parent: Option<&WorkTicket>,
        admission: Option<&crate::storage::maintenance::ReceiptAdmission>,
    ) -> Result<NativeDecayResult, StorageError> {
        self.meta_store.check_receipt_admission(admission)?;
        let guard = self.meta_store.native_guard_with_work(parent)?;
        if let Some(receipt) = self
            .meta_store
            .lookup_native_decay(&guard, ctx, intent.memory_id)?
        {
            return Ok(NativeDecayResult::Committed(receipt));
        }
        // One edge snapshot, read per node: a commit reads the memory's
        // neighbourhood, not every edge.
        let result = {
            let snapshot = self.edge_store.read_transaction()?;
            let mut ids = vec![intent.memory_id];
            ids.extend(
                EdgeStore::incident_edges(&snapshot, intent.memory_id)?
                    .iter()
                    .filter(|e| e.target == intent.memory_id)
                    .map(|e| e.source),
            );
            ids.sort();
            ids.dedup();
            let mut hashes = HashMap::new();
            for id in ids {
                if self.meta_store.get(id)?.is_some() {
                    hashes.insert(id, self.native_text_hash(id)?);
                }
            }
            self.meta_store.decay_native_with_effects_bounded(
                &guard, ctx, intent, policy, &snapshot, &hashes, admission,
            )?
        };
        if let NativeDecayResult::Committed(receipt) = &result {
            if !receipt.replayed {
                effects::apply_pending(
                    &self.meta_store,
                    &guard,
                    &self.edge_store,
                    Some(&mut self.vector_manager),
                )
                .map_err(|source| StorageError::CommittedMutation {
                    receipt: Box::new(receipt.clone()),
                    source: Box::new(source),
                })?;
            }
        }
        Ok(result)
    }
}
