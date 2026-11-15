use super::*;
use crate::model::schema::EmbeddingProfile;
use crate::storage::native_catalog::{
    NamespacePolicyState, NativeNamespaceCreated, NativeNamespaceInput,
};
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::time::Clock;

fn validate_context(ctx: &MutationContext) -> Result<(), StorageError> {
    ctx.validate()?;
    if ctx.scope != MutationScope::Catalog {
        return Err(StorageError::InvalidMutationContext(
            "native creation requires catalog scope".into(),
        ));
    }
    Ok(())
}

impl MetadataStore {
    pub(crate) fn lookup_native_namespace_replay(
        &self,
        ctx: &MutationContext,
        input: &NativeNamespaceInput,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        validate_context(ctx)?;
        let hash = input.hash(ctx)?;
        let _guard = self.native_guard()?;
        control::replay_or_check(&self.db.begin_write()?, ctx, hash)
    }

    pub(crate) fn create_native_catalog(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        input: &NativeNamespaceInput,
        profile: &EmbeddingProfile,
        policy: &NativePolicyInputs,
        clock: &dyn Clock,
        prepare: impl FnOnce(&NamespaceConfig) -> Result<(), StorageError>,
    ) -> Result<WriteReceipt, StorageError> {
        guard.check(self)?;
        validate_context(ctx)?;
        let hash = input.hash(ctx)?;
        let tx = self.db.begin_write()?;
        if let Some(receipt) = control::replay_or_check(&tx, ctx, hash)? {
            return Ok(receipt);
        }
        let state = control::read_control(&tx)?;
        state.committed_generation.next()?;
        let mut config = input.prepare(profile, clock.now_millis())?;
        // Resolve the frozen inherited inputs before reserving an ID or staging
        // files. A retained receipt above remains replayable under new inputs.
        policy.resolve(&config)?;
        let previous_max_id;
        {
            let mut namespaces = tx.open_table(NAMESPACE_TABLE)?;
            for row in namespaces.iter()? {
                let (_, value) = row?;
                if NamespaceConfig::decode(value.value())?
                    .name
                    .eq_ignore_ascii_case(&config.name)
                {
                    return Err(StorageError::DuplicateName(config.name.clone()));
                }
            }
            let mut counter = tx.open_table(NAMESPACE_COUNTER)?;
            previous_max_id = counter.get("max_id")?.map(|v| v.value()).unwrap_or(0);
            config.id =
                NamespaceId::new(previous_max_id.checked_add(1).ok_or_else(|| {
                    StorageError::InvalidControl("namespace ID exhausted".into())
                })?);
            counter.insert("max_id", config.id.get())?;
            namespaces.insert(config.id.get(), control::encode(&config)?.as_slice())?;
        }
        let transition = NativeNamespaceCreated {
            namespace_id: config.id,
            previous_max_id,
            previous_generation: state.committed_generation,
            name: config.name.clone(),
            created_at: config.created_at,
            profile: profile.clone(),
            policy: NamespacePolicyState::capture(&config)?,
        };
        let receipt = self.record_receipt(
            &tx,
            ctx,
            hash,
            MutationOutcome::NativeNamespaceCreated { transition },
        )?;
        prepare(&config)?;
        self.commit_catalog(tx, &receipt)?;
        Ok(receipt)
    }
}
