use super::*;
use crate::storage::inherited_policy::{AdoptInheritedDecay, PhaseCounts};
use crate::storage::work::WorkTicket;
impl RedbStorageEngine {
    pub(crate) fn adopt_inherited_decay(
        &mut self,
        request: &AdoptInheritedDecay,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        let guard = self.meta_store.native_guard_with_work(parent)?;
        self.meta_store
            .adopt_inherited_decay(&guard, request, &self.edge_store)
    }

    /// Startup adoption of a changed global multiplier for one inheriting
    /// namespace: the same preview and receipted adoption as the operator
    /// commands, run on the exclusive owner before anything is served.
    pub(crate) fn adopt_inherited_multiplier(
        &mut self,
        ns: &NamespaceConfig,
        multiplier_bits: u32,
        sampled_at: i64,
    ) -> Result<PhaseCounts, StorageError> {
        let preview = self.meta_store.preview_inherited_decay(
            ns,
            multiplier_bits,
            sampled_at,
            &self.edge_store,
        )?;
        let candidate = preview.candidate.clone();
        let request = AdoptInheritedDecay {
            context: crate::storage::control::MutationContext {
                store_id: Some(preview.source.store_id),
                scope: crate::storage::control::MutationScope::Namespace(ns.id),
                idempotency_key: None,
                expected_generation: Some(preview.source.generation),
                contract_version: 2,
            },
            preview,
        };
        self.adopt_inherited_decay(&request, None)?;
        Ok(candidate)
    }
}
