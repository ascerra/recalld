use super::*;
use crate::model::schema::EmbeddingProfile;
use crate::storage::native_catalog::NativeNamespaceInput;
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::storage::work::WorkTicket;
use crate::time::Clock;

impl RedbStorageEngine {
    /// Preserve original input identity and replay before sampling active
    /// provider configuration. Shared by the native session and wire adapter.
    pub(crate) fn create_native_namespace_with_provider(
        &mut self,
        ctx: &MutationContext,
        input: &NativeNamespaceInput,
        provider: &dyn crate::embedding::EmbeddingProvider,
        policy: &NativePolicyInputs,
        clock: &dyn Clock,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        if let Some(receipt) = self
            .meta_store
            .lookup_native_namespace_replay(ctx, input, parent)?
        {
            return Ok(receipt);
        }
        let profile = provider
            .profile()
            .ok_or(StorageError::ProviderConfigurationUnavailable)?;
        let profile_id = profile
            .profile_id()
            .map_err(|_| StorageError::ProviderConfigurationUnavailable)?;
        if provider.dimensions() != profile.dimension as usize {
            return Err(StorageError::ProviderConfigurationUnavailable);
        }
        if input
            .embedding_dim
            .is_some_and(|dim| dim != profile.dimension)
        {
            return Err(StorageError::EmbeddingProfileMismatch {
                field: "embeddingDim",
            });
        }
        if input
            .embedding_profile_id
            .as_ref()
            .is_some_and(|id| *id != profile_id)
        {
            return Err(StorageError::EmbeddingProfileMismatch {
                field: "embeddingProfileId",
            });
        }
        self.create_native_namespace(ctx, input, &profile, policy, clock, parent)
    }

    pub(crate) fn create_native_namespace(
        &mut self,
        ctx: &MutationContext,
        input: &NativeNamespaceInput,
        active_profile: &EmbeddingProfile,
        policy: &NativePolicyInputs,
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
                meta.create_native_catalog(
                    &guard,
                    ctx,
                    input,
                    active_profile,
                    policy,
                    clock,
                    prepare,
                )
            },
        )
    }
}
