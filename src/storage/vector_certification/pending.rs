//! Private compact evidence for original pending children across one explicit
//! certification-only namespace change. Never a serialized authority.
use super::*;
use redb::ReadableTable;
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct PendingBindings(Box<[Binding]>);
struct Binding {
    namespace: u32,
    before_hash: [u8; 32],
    after_hash: [u8; 32],
    certification: VectorCertification,
    origins: Box<[GenerationRef]>,
}

pub(crate) fn validate_pending_bindings(
    read: &redb::ReadTransaction,
    state: &control::ControlState,
    bindings: &PendingBindings,
    batches: &[crate::storage::batch::BatchProgress],
) -> Result<(), StorageError> {
    let namespaces = read.open_table(crate::storage::metadata::NAMESPACE_TABLE)?;
    for batch in batches.iter().filter(|batch| batch.unresolved() != 0) {
        if !matches!(batch.version, 2 | 3) || batch.reservation_receipt.store_id != state.store_id {
            return Err(StorageError::StoreLineageMismatch);
        }
        for item in &batch.items {
            if !matches!(
                item.state,
                crate::storage::batch::BatchItemState::NotAttempted {}
            ) {
                continue;
            }
            let defaults = item
                .defaults
                .as_ref()
                .ok_or_else(|| invalid("pending child defaults missing"))?;
            let row = namespaces
                .get(defaults.namespace_id.get())?
                .ok_or(StorageError::NamespaceNotFound(defaults.namespace_id.get()))?;
            let namespace = NamespaceConfig::decode(row.value())?;
            let current_hash = crate::storage::metadata::batch::policy_hash(&namespace)?;
            if current_hash != defaults.namespace_policy_hash
                && !bindings.permits(
                    &namespace,
                    defaults.namespace_policy_hash,
                    current_hash,
                    GenerationRef {
                        store_id: batch.reservation_receipt.store_id,
                        generation: batch.reservation_receipt.committed_generation,
                    },
                )
            {
                return Err(invalid(
                    "pending child policy is neither original nor certification-only",
                ));
            }
        }
    }
    Ok(())
}
impl PendingBindings {
    /// Called only after the shared strict history validator has completed.
    /// Charge both construction/boxed-array overlap and allocation overhead
    /// before allocating. No duplicated NamespaceConfig or record arrays remain.
    pub(super) fn from_validated(
        cutovers: &BTreeMap<u32, VectorsReembedded>,
    ) -> Result<Self, StorageError> {
        let mut charged = (cutovers.len() as u64)
            .checked_mul(2 * std::mem::size_of::<Binding>() as u64)
            .and_then(|n| n.checked_add(128))
            .ok_or_else(|| invalid("pending certification evidence size overflow"))?;
        let mut transient = 0_u64;
        for cutover in cutovers.values() {
            // Namespace reconstruction and JSON hashing are temporary. Charge
            // their simultaneous strings/escaped JSON capacity too, before any
            // clone. Extensions/features are closed by the cutover validator.
            let ns = &cutover.before_namespace;
            let EmbeddingBinding::Bound {
                profile_id,
                descriptor,
                ..
            } = &ns.schema.embedding_binding
            else {
                return Err(invalid("pending cutover profile missing"));
            };
            let mut strings = 0_u64;
            for value in [
                &ns.name,
                profile_id,
                &descriptor.model_id,
                descriptor.model_revision.as_deref().unwrap_or(""),
                &descriptor.document_preprocessing.version,
                &descriptor.document_preprocessing.prefix,
                &descriptor.query_preprocessing.version,
                &descriptor.query_preprocessing.prefix,
            ] {
                strings = strings
                    .checked_add(value.len() as u64)
                    .ok_or_else(|| invalid("pending namespace size overflow"))?;
            }
            transient = transient.max(
                strings
                    .checked_mul(16)
                    .and_then(|n| n.checked_add(16 * 1024))
                    .ok_or_else(|| invalid("pending namespace capacity overflow"))?,
            );
            charged = charged
                .checked_add(
                    (cutover.source_origins.len() as u64)
                        .checked_mul(2 * std::mem::size_of::<GenerationRef>() as u64)
                        .and_then(|n| n.checked_add(128))
                        .ok_or_else(|| invalid("pending certification origins size overflow"))?,
                )
                .ok_or_else(|| invalid("pending certification evidence size overflow"))?;
        }
        if charged
            .checked_add(transient)
            .is_none_or(|n| n > MAX_PROOF_BYTES)
        {
            return Err(invalid(
                "compact pending certification evidence exceeds 16 MiB heap budget",
            ));
        }
        let mut bindings = Vec::with_capacity(cutovers.len());
        for (namespace, cutover) in cutovers {
            let mut expected = cutover.before_namespace.namespace()?;
            let before_hash = crate::storage::metadata::batch::policy_hash(&expected)?;
            let schema = expected
                .schema
                .as_mut()
                .ok_or_else(|| invalid("cutover native namespace missing"))?;
            if schema.vector_certification.is_some() || !schema.required_features.is_empty() {
                return Err(invalid(
                    "cutover before namespace already has required features",
                ));
            }
            schema.vector_certification = Some(cutover.certification.clone());
            schema.required_features.push(REQUIRED_FEATURE.into());
            let after_hash = crate::storage::metadata::batch::policy_hash(&expected)?;
            bindings.push(Binding {
                namespace: *namespace,
                before_hash,
                after_hash,
                certification: cutover.certification.clone(),
                origins: cutover.source_origins.clone().into_boxed_slice(),
            });
        }
        Ok(Self(bindings.into_boxed_slice()))
    }
    pub(crate) fn permits(
        &self,
        namespace: &NamespaceConfig,
        before_hash: [u8; 32],
        current_hash: [u8; 32],
        parent: GenerationRef,
    ) -> bool {
        let Ok(index) = self
            .0
            .binary_search_by_key(&namespace.id.get(), |b| b.namespace)
        else {
            return false;
        };
        let binding = &self.0[index];
        if binding.before_hash != before_hash
            || binding.after_hash != current_hash
            || namespace
                .schema
                .as_ref()
                .and_then(|s| s.vector_certification.as_ref())
                != Some(&binding.certification)
        {
            return false;
        }
        binding
            .origins
            .binary_search_by_key(&parent.store_id, |o| o.store_id)
            .ok()
            .is_some_and(|i| parent.generation <= binding.origins[i].generation)
    }
}
