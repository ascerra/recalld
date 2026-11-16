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
        crate::storage::inherited_policy::supported_inputs(&policy.global)?;
        crate::decay::lifecycle::resolve_policy(&config, &policy.global, policy.multiplier)
            .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
        let inherited_bits = config
            .decay_rate_multiplier
            .is_none()
            .then(|| crate::storage::inherited_policy::normalize(policy.multiplier))
            .transpose()?;
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
        }
        let operation_id = self
            .id_source()
            .uuid(crate::storage::ids::IdKind::Operation)?;
        config.schema.as_mut().unwrap().inherited_decay = inherited_bits.map(|multiplier_bits| {
            crate::storage::inherited_policy::InheritedDecayBinding {
                version: 1,
                multiplier_bits,
                adoption: control::GenerationRef {
                    store_id: state.store_id,
                    generation: state
                        .committed_generation
                        .next()
                        .expect("checked before allocation"),
                },
                operation_id,
            }
        });
        let vector_certification = crate::storage::vector_certification::VectorCertification {
            version: 1,
            origin: control::GenerationRef {
                store_id: state.store_id,
                generation: state.committed_generation.next()?,
            },
            operation_id,
            inventory_root: crate::storage::vector_certification::inventory_root(
                &config,
                std::iter::empty(),
            )?,
            vector_count: 0,
        };
        config.schema.as_mut().unwrap().vector_certification = Some(vector_certification.clone());
        config.schema.as_mut().unwrap().required_features =
            vec![crate::storage::vector_certification::REQUIRED_FEATURE.into()];
        tx.open_table(NAMESPACE_TABLE)?
            .insert(config.id.get(), control::encode(&config)?.as_slice())?;
        let transition = NativeNamespaceCreated {
            vector_certification: Some(vector_certification),
            namespace_id: config.id,
            previous_max_id,
            previous_generation: state.committed_generation,
            name: config.name.clone(),
            created_at: config.created_at,
            profile: profile.clone(),
            policy: NamespacePolicyState::capture(&config)?,
            inherited_decay: config.schema.as_ref().unwrap().inherited_decay.clone(),
        };
        let receipt = control::record_reserved_receipt(
            &tx,
            ctx,
            hash,
            MutationOutcome::NativeNamespaceCreated { transition },
            operation_id,
        )?;
        prepare(&config)?;
        self.commit_catalog(tx, &receipt)?;
        Ok(receipt)
    }
}
