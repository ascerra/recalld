use super::*;
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::storage::native_policy::NativePolicyIntent;
use crate::storage::work::WorkTicket;

impl RedbStorageEngine {
    pub(crate) fn apply_native_policy(
        &mut self,
        ctx: &MutationContext,
        intent: &NativePolicyIntent,
        policy: &NativePolicyInputs,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let guard = self.meta_store.native_guard_with_work(parent)?;
        self.meta_store
            .apply_native_policy(&guard, ctx, intent, policy)
    }
}
