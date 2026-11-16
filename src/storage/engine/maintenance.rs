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
        self.apply_legacy_decay_with_work_bounded(
            ctx,
            id,
            expected,
            namespace,
            phase,
            strength,
            decay_strength,
            parent,
            None,
        )
    }

    pub(crate) fn apply_legacy_decay_with_work_bounded(
        &self,
        ctx: &MutationContext,
        id: MemoryId,
        expected: &DiskRecord,
        namespace: &NamespaceConfig,
        phase: DecayPhase,
        strength: f32,
        decay_strength: f32,
        parent: Option<&crate::storage::work::WorkTicket>,
        admission: Option<&crate::storage::maintenance::ReceiptAdmission>,
    ) -> Result<DecayMutationResult, StorageError> {
        self.meta_store.check_receipt_admission(admission)?;
        self.ensure_available()?;
        let guard = self.meta_store.mutation_guard_with_work(parent)?;
        let result = self.meta_store.decay_with_effects_bounded(
            &guard,
            ctx,
            id,
            expected,
            namespace,
            phase,
            strength,
            decay_strength,
            &self.edge_store.load_all_edges()?,
            admission,
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
