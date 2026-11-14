//! Shared concrete native catalog adapter. Owns no database or registry: both
//! transports use their existing engine with explicitly frozen dependencies.
use crate::{
    embedding::EmbeddingProvider,
    serialization::{
        catalog::CreateNamespaceRequest,
        contract::{
            CatalogKind, CommitState, ContractVersion, Envelope, ErrorCategory, ExecutionError,
            InputError, Scope,
        },
    },
    storage::{
        RedbStorageEngine, StorageError,
        control::{MutationOutcome, MutationScope, WriteReceipt},
        native_lifecycle::NativePolicyInputs,
    },
    time::Clock,
};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct NativeCatalogAdapter {
    storage: Arc<RwLock<RedbStorageEngine>>,
    provider: Arc<dyn EmbeddingProvider>,
    policy: NativePolicyInputs,
    clock: Arc<dyn Clock>,
    #[cfg(test)]
    pub(crate) failure: Option<&'static str>,
}
impl NativeCatalogAdapter {
    pub(crate) fn new(
        storage: Arc<RwLock<RedbStorageEngine>>,
        provider: Arc<dyn EmbeddingProvider>,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            storage,
            provider,
            policy,
            clock,
            #[cfg(test)]
            failure: None,
        }
    }

    pub(crate) async fn create(
        &self,
        request: CreateNamespaceRequest,
    ) -> Result<WriteReceipt, CatalogError> {
        // Accept before scheduling; the same root authorizes both replay lookup
        // and final publication even if a concurrent freeze closes new roots.
        let root = self
            .storage
            .read()
            .map_err(|_| CatalogError::Unavailable)?
            .meta_store()
            .work_tracker()
            .begin()
            .map_err(StorageError::from)?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            #[cfg(test)]
            let _fault = tests::Fault::enter(this.failure);
            let mut storage = this
                .storage
                .write()
                .map_err(|_| CatalogError::WorkerUnknown)?;
            let ctx = request
                .write
                .context(MutationScope::Catalog, ContractVersion::V2);
            let result = storage.create_native_namespace_with_provider(
                &ctx,
                &request.input,
                this.provider.as_ref(),
                &this.policy,
                this.clock.as_ref(),
                Some(&root),
            );
            // Poisoned/uncertain work remains unresolved. No async cancellation
            // or failed join can clear the root from the request task.
            storage.meta_store().finish_work(root);
            result.map_err(CatalogError::Storage)
        })
        .await
        .map_err(|_| CatalogError::WorkerUnknown)?
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("{0}")]
    Input(#[from] InputError),
    #[error("{0}")]
    Storage(#[from] StorageError),
    #[error("native catalog adapter is unavailable")]
    Unavailable,
    #[error("catalog worker outcome is unknown; reconcile before retry")]
    WorkerUnknown,
}

pub type CatalogEnvelope = Envelope<serde_json::Value>;

/// Serialize the exact retained outcome. Never re-read a newer namespace row
/// or claim whole-store readiness from a catalog receipt.
pub(crate) fn response(result: Result<WriteReceipt, CatalogError>) -> (u16, CatalogEnvelope) {
    match result {
        Ok(receipt) => {
            let data = match &receipt.outcome {
                MutationOutcome::NativeNamespaceCreated { transition } => serde_json::json!({
                    "namespaceId":transition.namespace_id,"name":transition.name,
                    "embeddingDim":transition.profile.dimension,
                    "embeddingProfileId":transition.profile.profile_id().expect("validated creation proof")
                }),
                _ => {
                    return failure(CatalogError::Storage(StorageError::CommittedMutation {
                        receipt: Box::new(receipt),
                        source: Box::new(StorageError::InvalidControl(
                            "unexpected catalog outcome".into(),
                        )),
                    }));
                }
            };
            let mut out = Envelope::success(
                Scope::Catalog {
                    kind: CatalogKind::Catalog,
                },
                data,
            );
            out.receipt = Some(receipt);
            out.warnings.push("Canonical catalog receipt does not attest whole-store derived readiness; public contract2 remains disabled".into());
            (200, out)
        }
        Err(error) => failure(error),
    }
}

fn failure(error: CatalogError) -> (u16, CatalogEnvelope) {
    use ErrorCategory::*;
    let (status, category, code, field, state, operation_id, retryable) = match &error {
        CatalogError::Input(e) => (
            400,
            InvalidInput,
            e.code,
            Some(e.field.clone()),
            CommitState::NotCommitted,
            None,
            false,
        ),
        CatalogError::Storage(e) => match e {
            StorageError::StoreLineageMismatch => (
                409,
                Conflict,
                "STORE_LINEAGE_MISMATCH",
                Some("/storeId".into()),
                CommitState::NotCommitted,
                None,
                false,
            ),
            StorageError::GenerationConflict { .. } => (
                409,
                Conflict,
                "GENERATION_CONFLICT",
                Some("/expectedGeneration".into()),
                CommitState::NotCommitted,
                None,
                false,
            ),
            StorageError::IdempotencyConflict => (
                409,
                Conflict,
                "IDEMPOTENCY_CONFLICT",
                Some("/idempotencyKey".into()),
                CommitState::NotCommitted,
                None,
                false,
            ),
            StorageError::EmbeddingProfileMismatch { field } => (
                409,
                Conflict,
                "EMBEDDING_PROFILE_MISMATCH",
                Some(format!("/{field}")),
                CommitState::NotCommitted,
                None,
                false,
            ),
            StorageError::ProviderConfigurationUnavailable => (
                503,
                Unavailable,
                "EMBEDDING_UNAVAILABLE",
                None,
                CommitState::NotCommitted,
                None,
                true,
            ),
            StorageError::DuplicateName(_) => (
                409,
                Conflict,
                "NAMESPACE_EXISTS",
                Some("/name".into()),
                CommitState::NotCommitted,
                None,
                false,
            ),
            StorageError::InvalidMutation(_)
            | StorageError::InvalidMutationContext(_)
            | StorageError::InvalidNamespaceName { .. } => (
                400,
                InvalidInput,
                "INVALID_COMBINATION",
                None,
                CommitState::NotCommitted,
                None,
                false,
            ),
            StorageError::CommittedMutation { receipt, .. } => (
                503,
                Unavailable,
                "INDEX_NOT_READY",
                None,
                CommitState::Committed,
                Some(receipt.operation_id),
                false,
            ),
            StorageError::CommittedEffectsPending { operation_id, .. } => (
                503,
                Unavailable,
                "INDEX_NOT_READY",
                None,
                CommitState::Committed,
                Some(*operation_id),
                false,
            ),
            StorageError::CommitOutcomeUnknown { operation_id, .. } => (
                503,
                Unavailable,
                "STORAGE_UNAVAILABLE",
                None,
                CommitState::Unknown,
                Some(*operation_id),
                false,
            ),
            _ => (
                503,
                Unavailable,
                "STORAGE_UNAVAILABLE",
                None,
                CommitState::NotCommitted,
                None,
                true,
            ),
        },
        CatalogError::Unavailable => (
            503,
            Unavailable,
            "STORAGE_UNAVAILABLE",
            None,
            CommitState::NotCommitted,
            None,
            true,
        ),
        CatalogError::WorkerUnknown => (
            503,
            Unavailable,
            "STORAGE_UNAVAILABLE",
            None,
            CommitState::Unknown,
            None,
            false,
        ),
    };
    let message = error.to_string();
    let receipt = match error {
        CatalogError::Storage(StorageError::CommittedMutation { receipt, .. }) => Some(*receipt),
        _ => None,
    };
    let mut out = Envelope::failure(
        Some(Scope::Catalog {
            kind: CatalogKind::Catalog,
        }),
        ExecutionError {
            category,
            code: code.into(),
            message,
            field,
            retryable,
            commit_state: state,
            operation_id,
        },
    );
    out.receipt = receipt;
    (status, out)
}

#[cfg(test)]
#[path = "native_catalog_adapter_tests.rs"]
mod tests;
