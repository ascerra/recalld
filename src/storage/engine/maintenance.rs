use super::*;
use crate::storage::maintenance::DecayMutationResult;

impl RedbStorageEngine {
    /// Called after the sweep's final graph/policy reprojection, under graph ->
    /// storage guards. No phase loss frees a vector in the legacy planner.
    /// Committed records still require awaited derived publication.
    pub fn apply_legacy_decay_with_receipt(
        &self,
        ctx: &MutationContext,
        id: MemoryId,
        expected: &DiskRecord,
        namespace: &NamespaceConfig,
        phase: DecayPhase,
        strength: f32,
        decay_strength: f32,
    ) -> Result<DecayMutationResult, StorageError> {
        self.apply_legacy_decay_with_work(
            ctx,
            id,
            expected,
            namespace,
            phase,
            strength,
            decay_strength,
            None,
        )
    }

    pub(crate) fn apply_legacy_decay_with_work(
        &self,
        ctx: &MutationContext,
        id: MemoryId,
        expected: &DiskRecord,
        namespace: &NamespaceConfig,
        phase: DecayPhase,
        strength: f32,
        decay_strength: f32,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<DecayMutationResult, StorageError> {
        self.ensure_available()?;
        let guard = self.meta_store.mutation_guard_with_work(parent)?;
        let result = self.meta_store.decay_with_effects(
            &guard,
            ctx,
            id,
            expected,
            namespace,
            phase,
            strength,
            decay_strength,
            &self.edge_store.load_all_edges()?,
        )?;
        if let DecayMutationResult::Committed(receipt) = &result {
            if !receipt.replayed {
                effects::apply_pending(&self.meta_store, &guard, &self.edge_store, None).map_err(
                    |source| StorageError::CommittedMutation {
                        receipt: Box::new(receipt.clone()),
                        source: Box::new(source),
                    },
                )?;
            }
        }
        Ok(result)
    }
}
