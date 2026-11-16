use super::*;
use crate::storage::inherited_policy::AdoptInheritedDecay;
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
}
