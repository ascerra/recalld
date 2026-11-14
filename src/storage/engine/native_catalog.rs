use super::*;
use crate::model::schema::EmbeddingProfile;
use crate::storage::native_catalog::NativeNamespaceInput;
use crate::storage::work::WorkTicket;
use crate::time::Clock;

impl RedbStorageEngine {
    pub(crate) fn create_native_namespace(
        &mut self,
        ctx: &MutationContext,
        input: &NativeNamespaceInput,
        active_profile: &EmbeddingProfile,
        clock: &dyn Clock,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        self.ensure_available()?;
        let meta = &self.meta_store;
        // Hold the actual owner ticket/publication guard through file sync,
        // canonical commit and in-process handle installation.
        let guard = meta.native_guard_with_work(parent)?;
        super::catalog::publish_namespace(
            meta,
            &mut self.vector_manager,
            &self.db_path,
            |prepare| {
                meta.create_native_catalog(&guard, ctx, input, active_profile, clock, prepare)
            },
        )
    }
}
