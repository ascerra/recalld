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
        let guard = self.meta_store.native_guard_with_work(parent)?;
        if let Some(receipt) = self
            .meta_store
            .lookup_native_decay(&guard, ctx, intent.memory_id)?
        {
            return Ok(NativeDecayResult::Committed(receipt));
        }
        let edges = self.edge_store.load_all_edges()?;
        let mut ids = vec![intent.memory_id];
        ids.extend(
            edges
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
        let result = self
            .meta_store
            .decay_native_with_effects(&guard, ctx, intent, policy, &edges, &hashes)?;
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
