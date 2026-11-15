use super::*;
use crate::storage::native_graph::NativeGraphIntent;
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::storage::work::WorkTicket;
use crate::time::Clock;

impl RedbStorageEngine {
    /// Existing owner only. Caller holds graph -> storage ownership through
    /// publication. This applies canonical edges, not derived readiness.
    pub(crate) fn apply_native_graph(
        &mut self,
        ctx: &MutationContext,
        intent: &NativeGraphIntent,
        policy: &NativePolicyInputs,
        clock: &dyn Clock,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<crate::storage::metadata::effects::EffectResult>, StorageError> {
        if let Some(receipt) = self
            .meta_store
            .lookup_native_graph(ctx, &intent.original, parent)?
        {
            return crate::storage::metadata::native_graph::replay_result(receipt).map(Some);
        }
        let guard = self.meta_store.native_guard_with_work(parent)?;
        let edges = self.edge_store.load_all_edges()?;
        let mut hashes = std::collections::HashMap::new();
        for (id, _) in &intent.capture.records {
            hashes.insert(*id, self.native_text_hash(*id)?);
        }
        let result = self
            .meta_store
            .apply_native_graph(&guard, ctx, intent, policy, clock, &edges, &hashes)?;
        if let Some(result) = &result {
            if !result.receipt.replayed {
                effects::apply_pending(&self.meta_store, &guard, &self.edge_store, None).map_err(
                    |source| StorageError::CommittedMutation {
                        receipt: Box::new(result.receipt.clone()),
                        source: Box::new(source),
                    },
                )?;
            }
        }
        Ok(result)
    }
}
