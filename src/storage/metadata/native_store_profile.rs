//! Single-store wire profile identity. The canonical namespace binding is the
//! only fresh descriptor authority. Completed replay requires only its old hash.
use super::*;
use crate::model::schema::EmbeddingBinding;
use crate::storage::native_store::NativeStoreInput;

pub(crate) enum NativeStoreProfileResolution {
    Replay(WriteReceipt),
    Fresh(NativeStoreInput),
}

impl MetadataStore {
    pub(crate) fn resolve_native_store_profile(
        &self,
        ctx: &MutationContext,
        input: &NativeStoreInput,
        supplied_profile_id: Option<&str>,
        parent: Option<&WorkTicket>,
    ) -> Result<NativeStoreProfileResolution, StorageError> {
        ctx.validate()?;
        let MutationScope::Namespace(namespace_id) = ctx.scope else {
            return Err(StorageError::MutationNamespaceMismatch);
        };
        if ctx.contract_version != 2
            || input.insert.embedding_profile.is_some()
            || input.insert.memory.embedding.is_some() != supplied_profile_id.is_some()
        {
            return Err(StorageError::InvalidMutation("native wire vector requires only its profile ID; generated vectors omit the selector".into()));
        }
        let _guard = self.native_guard_with_work(parent)?;
        let tx = self.db.begin_write()?;
        if let Some(stored) = control::keyed_receipt(&tx, ctx)? {
            let MutationOutcome::NativeMemoryInsertedWithLinks { transition } =
                &stored.receipt.outcome
            else {
                return Err(StorageError::IdempotencyConflict);
            };
            let primary = &transition.primary()?.after;
            let hash = input.request_hash_with_profile_id(
                ctx,
                primary.lifecycle.stability.get(),
                primary.lifecycle.difficulty.get(),
                transition.embedding_dim,
                primary.content.profile_id.as_deref().ok_or_else(|| {
                    StorageError::InvalidControl("linked creation profile missing".into())
                })?,
                &transition.settings,
                supplied_profile_id,
            )?;
            // Includes all original fields and expected G; matching replay must
            // precede fresh CAS, namespace/config reads and provider calls.
            let receipt = control::replay_or_check(&tx, ctx, hash)?.ok_or_else(|| {
                StorageError::InvalidControl("native profile replay disappeared".into())
            })?;
            return Ok(NativeStoreProfileResolution::Replay(receipt));
        }
        let mut resolved = input.clone();
        if let Some(requested) = supplied_profile_id {
            let table = tx.open_table(NAMESPACE_TABLE)?;
            let row = table
                .get(namespace_id.get())?
                .ok_or(StorageError::NamespaceNotFound(namespace_id.get()))?;
            let namespace = NamespaceConfig::decode(row.value())?;
            let schema = namespace.schema.as_ref().ok_or_else(|| {
                StorageError::InvalidMutation(
                    "native supplied vector requires a bound namespace".into(),
                )
            })?;
            schema
                .validate(&namespace)
                .map_err(StorageError::InvalidControl)?;
            let EmbeddingBinding::Bound {
                profile_id,
                descriptor,
                ..
            } = &schema.embedding_binding
            else {
                return Err(StorageError::InvalidMutation(
                    "native supplied vector has unknown profile provenance".into(),
                ));
            };
            if requested != profile_id {
                return Err(StorageError::InvalidMutation(
                    "supplied profile ID differs from canonical namespace binding".into(),
                ));
            }
            // Validation above checks dimension and descriptor-derived ID. The
            // actual insert rechecks the binding under its final write guard.
            resolved.insert.embedding_profile = Some(descriptor.clone());
        }
        Ok(NativeStoreProfileResolution::Fresh(resolved))
    }
}
