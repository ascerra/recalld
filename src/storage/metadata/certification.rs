//! The existing metadata publication lock owns this validation state. It grants
//! no work admission, derived readiness, or independent generation authority.
use super::*;
use crate::storage::control::GenerationRef;
use crate::storage::vector_certification::VectorCertification;
use std::collections::BTreeMap;

pub(super) struct OwnedVectorValidation {
    at: GenerationRef,
    schema: crate::storage::canonical_schema::CanonicalSchemaState,
    namespaces: BTreeMap<u32, VectorCertification>,
    pending_bindings: crate::storage::vector_certification::PendingBindings,
}
pub(super) struct VectorValidationCommit {
    before: GenerationRef,
    after: GenerationRef,
    created: Option<(u32, VectorCertification)>,
}
impl OwnedVectorValidation {
    fn require_insert(
        &self,
        tx: &redb::WriteTransaction,
        namespace_id: NamespaceId,
        content: &crate::storage::native_records::ContentProof,
        execution: Option<&crate::storage::vector_certification::VectorExecution>,
    ) -> Result<(), StorageError> {
        let table = tx.open_table(NAMESPACE_TABLE)?;
        let value = table
            .get(namespace_id.get())?
            .ok_or(StorageError::NamespaceNotFound(namespace_id.get()))?;
        let namespace = NamespaceConfig::decode(value.value())?;
        if namespace.id != namespace_id {
            return Err(StorageError::InvalidControl(
                "insert namespace identity mismatch".into(),
            ));
        }
        self.require_namespace(self.at, &namespace)?;
        content.validate_namespace_profile(&namespace)?;
        if matches!(
            namespace.schema.as_ref().unwrap().embedding_binding,
            crate::model::schema::EmbeddingBinding::UnknownLegacy
        ) {
            // The validated creation transition carries evidence for this row,
            // never a namespace certification or an assertion about old vectors.
            if content.embedding_origin.is_none() || execution.is_some() {
                return Err(StorageError::RecoveryRequired(
                    "unknown namespace insertion lacks per-record creation evidence".into(),
                ));
            }
        } else if execution.map(|e| &e.certification) != self.namespaces.get(&namespace_id.get()) {
            return Err(StorageError::RecoveryRequired(
                "fresh vector lacks owned certification/execution proof".into(),
            ));
        }
        Ok(())
    }
    pub(super) fn require_at(&self, at: GenerationRef) -> Result<(), StorageError> {
        if self.at != at {
            return Err(StorageError::RecoveryRequired(
                "owned vector validation does not match canonical generation".into(),
            ));
        }
        Ok(())
    }
    pub(super) fn require_namespace(
        &self,
        at: GenerationRef,
        namespace: &NamespaceConfig,
    ) -> Result<(), StorageError> {
        self.require_at(at)?;
        if namespace.schema.as_ref().is_some_and(|schema| {
            matches!(
                schema.embedding_binding,
                crate::model::schema::EmbeddingBinding::UnknownLegacy
            )
        }) {
            namespace
                .schema
                .as_ref()
                .unwrap()
                .validate(namespace)
                .map_err(StorageError::InvalidControl)?;
            // Structural validation and canonical generation remain mandatory;
            // no certification is fabricated for compatibility comparisons.
            if self.namespaces.contains_key(&namespace.id.get()) {
                return Err(StorageError::RecoveryRequired(
                    "unknown namespace has certified inventory".into(),
                ));
            }
            return Ok(());
        }
        let binding = namespace
            .schema
            .as_ref()
            .and_then(|s| s.vector_certification.as_ref());
        let owned = self.namespaces.get(&namespace.id.get());
        if binding.is_none() && owned.is_none() {
            return Err(StorageError::VectorCertificationRequired {
                namespace_id: namespace.id.get(),
            });
        }
        if owned != binding {
            return Err(StorageError::RecoveryRequired(
                "owned vector certification differs from canonical binding".into(),
            ));
        }
        Ok(())
    }
}
impl MetadataStore {
    pub(super) fn initialize_vector_validation(&self) -> Result<(), StorageError> {
        // Constructor preflight already validated canonical history and payloads
        // under stable engine ownership. Capture its exact live canonical G.
        let read = self.db.begin_read()?;
        let state = control::validate_existing(&read)?
            .ok_or_else(|| StorageError::InvalidControl("native control missing".into()))?;
        let (summary, pending_bindings) =
            crate::storage::vector_certification::validate_current_with_pending(&read, &state)?;
        let schema = crate::storage::canonical_schema::read_state(&read, &state)?
            .ok_or_else(|| StorageError::InvalidControl("native schema missing".into()))?;
        *self
            .vector_validation
            .write()
            .map_err(|_| StorageError::RecoveryRequired("vector validation poisoned".into()))? =
            Some(OwnedVectorValidation {
                at: state.generation_ref(),
                schema,
                namespaces: summary.selected,
                pending_bindings,
            });
        Ok(())
    }

    /// Full validation belongs to owned reopen. An exact-G reader checks the
    /// immutable schema singleton in its own transaction, without history scans.
    pub(super) fn owned_schema_at(
        &self,
        read: &redb::ReadTransaction,
        at: GenerationRef,
    ) -> Result<crate::storage::canonical_schema::CanonicalSchemaState, StorageError> {
        let owned = self
            .vector_validation
            .read()
            .map_err(|_| StorageError::RecoveryRequired("vector validation poisoned".into()))?;
        let owned = owned.as_ref().ok_or_else(|| {
            StorageError::RecoveryRequired("owned vector validation missing".into())
        })?;
        owned.require_at(at)?;
        let table = read.open_table(crate::storage::canonical_schema::SCHEMA)?;
        let value = table
            .get("state")?
            .ok_or_else(|| StorageError::InvalidControl("native schema missing".into()))?;
        let schema: crate::storage::canonical_schema::CanonicalSchemaState =
            control::decode(value.value())?;
        if table.len()? != 1 || schema != owned.schema {
            return Err(StorageError::RecoveryRequired(
                "owned native schema changed".into(),
            ));
        }
        Ok(schema)
    }

    pub(super) fn require_vector_validation_at(
        &self,
        at: GenerationRef,
    ) -> Result<(), StorageError> {
        self.vector_validation
            .read()
            .map_err(|_| StorageError::RecoveryRequired("vector validation poisoned".into()))?
            .as_ref()
            .ok_or_else(|| {
                StorageError::RecoveryRequired("owned vector validation missing".into())
            })?
            .require_at(at)
    }
    pub(super) fn require_certified_namespace(
        &self,
        at: GenerationRef,
        namespace: &NamespaceConfig,
    ) -> Result<(), StorageError> {
        self.vector_validation
            .read()
            .map_err(|_| StorageError::RecoveryRequired("vector validation poisoned".into()))?
            .as_ref()
            .ok_or_else(|| {
                StorageError::RecoveryRequired("owned vector validation missing".into())
            })?
            .require_namespace(at, namespace)
    }

    pub(super) fn permits_pending_certification(
        &self,
        at: GenerationRef,
        namespace: &NamespaceConfig,
        before_hash: [u8; 32],
        current_hash: [u8; 32],
        parent: GenerationRef,
    ) -> Result<bool, StorageError> {
        let owned = self
            .vector_validation
            .read()
            .map_err(|_| StorageError::RecoveryRequired("vector validation poisoned".into()))?;
        let owned = owned.as_ref().ok_or_else(|| {
            StorageError::RecoveryRequired("owned vector validation missing".into())
        })?;
        owned.require_at(at)?;
        if parent.store_id != at.store_id {
            return Err(StorageError::StoreLineageMismatch);
        }
        Ok(owned
            .pending_bindings
            .permits(namespace, before_hash, current_hash, parent))
    }

    pub(super) fn prepare_vector_validation_commit(
        &self,
        tx: &redb::WriteTransaction,
        receipt: &WriteReceipt,
    ) -> Result<Option<VectorValidationCommit>, StorageError> {
        if !self.native_lifecycle_only {
            return Ok(None);
        }
        let owned = self
            .vector_validation
            .read()
            .map_err(|_| StorageError::RecoveryRequired("vector validation poisoned".into()))?;
        let owned = owned.as_ref().ok_or_else(|| {
            StorageError::RecoveryRequired("owned vector validation missing".into())
        })?;
        let state = control::read_control(tx)?;
        if state.store_id != receipt.store_id
            || state.committed_generation != receipt.committed_generation
            || owned.at.store_id != state.store_id
            || owned.at.generation.next()? != state.committed_generation
        {
            return Err(StorageError::RecoveryRequired(
                "unparticipating canonical writer or stale vector validation".into(),
            ));
        }
        let mut created = None;
        match &receipt.outcome {
            MutationOutcome::NativeTextGenerationSelected { transition } => {
                // The selection writer already compares the complete logical
                // record digest before/after relocation and the copied text.
                // Carry certification only for that exact owned generation and
                // the typed descriptors selected in this same transaction.
                transition.validate(receipt)?;
                if transition.source != owned.at {
                    return Err(StorageError::RecoveryRequired(
                        "text relocation differs from owned vector generation".into(),
                    ));
                }
                let catalog = tx.open_table(text_generation::CATALOG)?;
                let active: uuid::Uuid = control::decode(
                    catalog
                        .get("active")?
                        .ok_or(StorageError::InvalidTextRef)?
                        .value(),
                )?;
                for descriptor in [&transition.before, &transition.after] {
                    let retained = catalog
                        .get(descriptor.id.to_string().as_str())?
                        .ok_or(StorageError::InvalidTextRef)?;
                    if control::decode::<text_generation::TextGeneration>(retained.value())?
                        != *descriptor
                    {
                        return Err(StorageError::InvalidTextRef);
                    }
                }
                if active != transition.after.id {
                    return Err(StorageError::InvalidTextRef);
                }
            }
            MutationOutcome::NativeNamespaceCreated { transition } => {
                transition.validate(receipt)?;
                let binding = transition.vector_certification.as_ref().ok_or_else(|| {
                    StorageError::InvalidControl(
                        "fresh namespace lacks empty-vector certification".into(),
                    )
                })?;
                if owned
                    .namespaces
                    .contains_key(&transition.namespace_id.get())
                {
                    return Err(StorageError::InvalidControl(
                        "certified namespace identity reused".into(),
                    ));
                }
                created = Some((transition.namespace_id.get(), binding.clone()));
            }
            MutationOutcome::NativeMemoryInserted { transition } => {
                transition.validate(receipt, true)?;
                owned.require_insert(
                    tx,
                    transition.namespace_id,
                    &transition.primary()?.after.content,
                    transition.vector_execution.as_ref(),
                )?;
            }
            MutationOutcome::NativeMemoryInsertedWithLinks { transition } => {
                transition.validate(receipt)?;
                owned.require_insert(
                    tx,
                    transition.namespace_id,
                    &transition.primary()?.after.content,
                    transition.vector_execution.as_ref(),
                )?;
            }
            // These existing typed writers preserve vectors or remove exact
            // owned references. Their transaction/ledger validators remain in
            // force; carrying this validation position is not a readiness ack.
            MutationOutcome::NativeNamespacePurgeStarted { .. }
            | MutationOutcome::NativeNamespacePurgeAdvanced { .. }
            | MutationOutcome::NativeNamespacePolicyUpdated { .. }
            | MutationOutcome::NativeNamespacePolicyUnchanged { .. }
            | MutationOutcome::InheritedDecayAdopted { .. }
            | MutationOutcome::NativePhaseMaterialized { .. }
            | MutationOutcome::NativeMemoryHardDeleted { .. }
            | MutationOutcome::NativeMemoryTombstoned { .. }
            | MutationOutcome::NativeGraphChanged { .. }
            | MutationOutcome::NativeGraphUnchanged { .. }
            | MutationOutcome::LifecycleFeedbackApplied { .. }
            | MutationOutcome::LifecycleExposureRecorded { .. }
            | MutationOutcome::BatchReserved { .. }
            | MutationOutcome::BatchItemRejected { .. } => {}
            _ => {
                return Err(StorageError::RecoveryRequired(
                    "native writer has no vector-certification transition consumer".into(),
                ));
            }
        }
        Ok(Some(VectorValidationCommit {
            before: owned.at,
            after: state.generation_ref(),
            created,
        }))
    }

    pub(super) fn publish_vector_validation_commit(
        &self,
        pending: Option<VectorValidationCommit>,
        receipt: &WriteReceipt,
    ) -> Result<(), StorageError> {
        let Some(pending) = pending else {
            return Ok(());
        };
        let result = (|| {
            // Instance-local test fault at the real postcommit publication boundary.
            // The production wrapper below retains the already durable receipt.
            #[cfg(test)]
            if self.vector_publication_failure_nth.fetch_update(
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
                |remaining| remaining.checked_sub(1),
            ) == Ok(1)
            {
                return Err(StorageError::RecoveryRequired(
                    "injected vector validation publication failure".into(),
                ));
            }
            let mut owned = self
                .vector_validation
                .write()
                .map_err(|_| StorageError::RecoveryRequired("vector validation poisoned".into()))?;
            let owned = owned.as_mut().ok_or_else(|| {
                StorageError::RecoveryRequired("owned vector validation missing".into())
            })?;
            owned.require_at(pending.before)?;
            if let Some((namespace, certification)) = pending.created {
                owned.namespaces.insert(namespace, certification);
            }
            owned.at = pending.after;
            Ok(())
        })();
        result.map_err(|source| {
            self.mark_unavailable();
            StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_certification_absence_is_distinct_from_stale_or_inconsistent_owner() {
        let root = tempfile::tempdir().unwrap();
        let (path, request, _) = crate::storage::vector_certification::tests::fixture(root.path());
        let engine = crate::storage::RedbStorageEngine::open_native(&path).unwrap();
        let store = engine.meta_store();
        let at = store.control_state().unwrap().generation_ref();
        let namespace = store.get_namespace(request.namespace_id).unwrap().unwrap();
        assert!(matches!(
            store.require_certified_namespace(at, &namespace),
            Err(StorageError::VectorCertificationRequired { .. })
        ));
        let mut stale = at;
        stale.generation = at.generation.next().unwrap();
        assert!(matches!(
            store.require_certified_namespace(stale, &namespace),
            Err(StorageError::RecoveryRequired(_))
        ));
        let mut owner = store.vector_validation.write().unwrap();
        owner.as_mut().unwrap().namespaces.insert(
            namespace.id.get(),
            VectorCertification {
                version: 1,
                origin: at,
                operation_id: uuid::Uuid::new_v4(),
                inventory_root: [0; 32],
                vector_count: 0,
            },
        );
        drop(owner);
        assert!(matches!(
            store.require_certified_namespace(at, &namespace),
            Err(StorageError::RecoveryRequired(_))
        ));
        *store.vector_validation.write().unwrap() = None;
        assert!(matches!(
            store.require_certified_namespace(at, &namespace),
            Err(StorageError::RecoveryRequired(_))
        ));
    }
}
