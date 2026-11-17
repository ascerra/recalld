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
    completion: crate::derived_completion::DerivedCompletion,
    provider: Arc<dyn EmbeddingProvider>,
    policy: NativePolicyInputs,
    clock: Arc<dyn Clock>,
    #[cfg(test)]
    pub(crate) failure: Option<&'static str>,
}
impl NativeCatalogAdapter {
    /// Fresh, unkeyed API1 only. Keyed V2 replay retains its existing entry.
    pub(crate) async fn create_api1_bounded(
        &self,
        input: crate::storage::native_catalog::NativeNamespaceInput,
        surface: crate::serialization::catalog_api1::CatalogReplyAdmission,
    ) -> Result<WriteReceipt, CatalogError> {
        // The owning adapter has selected native format. Reject an impossible
        // local envelope before accepting work or sampling provider/clock/IDs.
        let admission = surface.admit_native()?;
        let root = self.completion.accept_work(None)?;
        let result = self
            .create_api1_accepted(input, surface, &admission, &root)
            .await;
        if !result
            .as_ref()
            .err()
            .is_some_and(crate::serialization::catalog_api1::outcome_unknown)
        {
            self.completion.finish_known_work(root);
        }
        result
    }

    async fn create_api1_accepted(
        &self,
        input: crate::storage::native_catalog::NativeNamespaceInput,
        surface: crate::serialization::catalog_api1::CatalogReplyAdmission,
        admission: &crate::storage::maintenance::ReceiptAdmission,
        root: &crate::storage::work::WorkTicket,
    ) -> Result<WriteReceipt, CatalogError> {
        let lease = crate::derived_completion::publication_gate(&self.storage)
            .await?
            .read_owned()
            .await;
        let child = root.child().map_err(StorageError::from)?;
        let this = self.clone();
        let worker_admission = admission.clone();
        let result = tokio::task::spawn_blocking(move || {
            let _lease = lease;
            #[cfg(test)]
            let _fault = tests::Fault::enter_api1(this.failure);
            let mut storage = this
                .storage
                .write()
                .map_err(|_| CatalogError::WorkerUnknown)?;
            #[cfg(test)]
            tests::api1_before_result(this.failure);
            let ctx = crate::serialization::contract::WriteOptions::default()
                .context(MutationScope::Catalog, ContractVersion::V1);
            let result = storage.create_native_namespace_with_provider_bounded(
                &ctx,
                &input,
                this.provider.as_ref(),
                &this.policy,
                this.clock.as_ref(),
                Some(&child),
                Some(&worker_admission),
                |receipt| surface.check_candidate(receipt),
            );
            // Record only actual returned evidence, before the worker's
            // fallible tail. The private transaction candidate is not authority.
            let result = result.and_then(|receipt| {
                worker_admission.record(&receipt).map_err(|source| {
                    StorageError::CommittedMutation {
                        receipt: Box::new(receipt.clone()),
                        source: Box::new(source),
                    }
                })?;
                Ok(receipt)
            });
            if let Err(error) = &result {
                worker_admission.fail(error);
            }
            #[cfg(test)]
            tests::api1_after_result(this.failure, &result, &worker_admission);
            storage.meta_store().finish_work(child);
            result.map_err(CatalogError::Storage)
        })
        .await
        .map_err(|_| CatalogError::Api1WorkerUnknown(Box::new(
            crate::serialization::catalog_api1::CatalogWorkerEvidence::capture(admission),
        )))?;
        let receipt = result?;
        // The actual worker has dropped storage/publication locks. This pass
        // admits only the receipt returned above, never foreign pending work.
        self.completion
            .complete_with_admission(Some(root), Some(admission))
            .await
            .map_err(|source| {
                CatalogError::Storage(StorageError::CommittedMutation {
                    receipt: Box::new(receipt.clone()),
                    source: Box::new(source),
                })
            })?;
        Ok(receipt)
    }

    pub(crate) fn new(
        completion: crate::derived_completion::DerivedCompletion,
        provider: Arc<dyn EmbeddingProvider>,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            storage: completion.storage.clone(),
            completion,
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
        let root = self.completion.accept_work(None)?;
        let result = self.create_accepted(request, &root).await;
        if !matches!(result, Err(CatalogError::WorkerUnknown)) {
            self.completion.finish_known_work(root);
        }
        result
    }

    async fn create_accepted(
        &self,
        request: CreateNamespaceRequest,
        root: &crate::storage::work::WorkTicket,
    ) -> Result<WriteReceipt, CatalogError> {
        let gate = crate::derived_completion::publication_gate(&self.storage).await?;
        let lease = gate.read_owned().await;
        let child = root.child().map_err(StorageError::from)?;
        let this = self.clone();
        let result = tokio::task::spawn_blocking(move || {
            // Keep the lease with the worker even if its async caller cancels.
            let _lease = lease;
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
                Some(&child),
            );
            // Poisoned/uncertain work remains unresolved. No async cancellation
            // or failed join can clear the root from the request task.
            storage.meta_store().finish_work(child);
            result.map_err(CatalogError::Storage)
        })
        .await
        .map_err(|_| CatalogError::WorkerUnknown)?;
        // Worker released its read lease; completion may take it exclusively.
        let receipt = result?;
        self.complete_receipt(&receipt, root).await?;
        Ok(receipt)
    }

    pub(crate) async fn update_policy(
        &self,
        request: crate::serialization::policy::PolicyRequest,
    ) -> Result<PolicyOutcome, CatalogError> {
        let root = self.completion.accept_work(None)?;
        let result = self.policy_accepted(request, &root).await;
        if !matches!(result, Err(CatalogError::WorkerUnknown)) {
            self.completion.finish_known_work(root);
        }
        result
    }

    async fn policy_accepted(
        &self,
        request: crate::serialization::policy::PolicyRequest,
        root: &crate::storage::work::WorkTicket,
    ) -> Result<PolicyOutcome, CatalogError> {
        let gate = crate::derived_completion::publication_gate(&self.storage).await?;
        let lease = gate.read_owned().await;
        let child = root.child().map_err(StorageError::from)?;
        let this = self.clone();
        let result = tokio::task::spawn_blocking(move || {
            let _lease = lease;
            use crate::storage::StorageEngine;
            #[cfg(test)]
            let _fault = tests::Fault::enter(this.failure);
            let mut engine = this
                .storage
                .write()
                .map_err(|_| CatalogError::WorkerUnknown)?;
            let result = (|| {
                let ctx = request.write.context(
                    MutationScope::Namespace(request.namespace_id),
                    ContractVersion::V2,
                );
                let replacement = request.replacement.policy();
                // Durable stable ID + original policy, before fresh name/policy
                // lookup or CAS. No provider or clock is involved in this path.
                let receipt = if let Some(receipt) =
                    engine
                        .meta_store()
                        .lookup_native_policy(&ctx, &replacement, Some(&child))?
                {
                    Some(receipt)
                } else {
                    let namespace = engine
                        .get_namespace(request.namespace_id)?
                        .ok_or(StorageError::NamespaceNotFound(request.namespace_id.get()))?;
                    let scope = crate::storage::native_lifecycle::LifecycleScope {
                        namespace_id: request.namespace_id,
                        namespace_name: namespace.name,
                        source: None,
                    };
                    let intent = engine.meta_store().capture_native_policy(
                        scope,
                        &replacement,
                        &this.policy,
                        Some(&child),
                    )?;
                    engine.apply_native_policy(&ctx, &intent, &this.policy, Some(&child))?
                };
                let namespace = engine
                    .get_namespace(request.namespace_id)
                    .and_then(|value| {
                        value.ok_or(StorageError::NamespaceNotFound(request.namespace_id.get()))
                    })
                    .map_err(|source| match &receipt {
                        Some(receipt) => StorageError::CommittedMutation {
                            receipt: Box::new(receipt.clone()),
                            source: Box::new(source),
                        },
                        None => source,
                    })?;
                let policy = match receipt.as_ref().map(|r| &r.outcome) {
                    Some(MutationOutcome::NativeNamespacePolicyUpdated { transition }) => {
                        crate::serialization::policy::PolicyValues::retained(&transition.after)
                    }
                    Some(MutationOutcome::NativeNamespacePolicyUnchanged { transition }) => {
                        crate::serialization::policy::PolicyValues::retained(&transition.policy)
                    }
                    None => request.replacement,
                    _ => {
                        return Err(StorageError::CommittedMutation {
                            receipt: Box::new(
                                receipt.clone().expect("unexpected outcome has a receipt"),
                            ),
                            source: Box::new(StorageError::InvalidControl(
                                "unexpected native policy receipt".into(),
                            )),
                        });
                    }
                };
                Ok::<_, StorageError>(PolicyOutcome {
                    namespace_id: request.namespace_id,
                    namespace: namespace.name,
                    policy,
                    receipt,
                })
            })();
            engine.meta_store().finish_work(child);
            result.map_err(CatalogError::Storage)
        })
        .await
        .map_err(|_| CatalogError::WorkerUnknown)?;
        let outcome = result?;
        if let Some(receipt) = &outcome.receipt {
            self.complete_receipt(receipt, root).await?;
        }
        Ok(outcome)
    }

    async fn complete_receipt(
        &self,
        receipt: &WriteReceipt,
        root: &crate::storage::work::WorkTicket,
    ) -> Result<(), CatalogError> {
        self.completion
            .complete_with_work(Some(root))
            .await
            .map_err(|source| {
                CatalogError::Storage(StorageError::CommittedMutation {
                    receipt: Box::new(receipt.clone()),
                    source: Box::new(source),
                })
            })
    }
}

#[derive(Debug)]
pub struct PolicyOutcome {
    pub namespace_id: crate::model::NamespaceId,
    pub namespace: String,
    pub policy: crate::serialization::policy::PolicyValues,
    pub receipt: Option<WriteReceipt>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyData {
    pub namespace_id: crate::model::NamespaceId,
    pub policy: crate::serialization::policy::PolicyValues,
    pub changed: bool,
}
pub(crate) fn policy_response(
    result: Result<PolicyOutcome, CatalogError>,
    namespace_id: Option<crate::model::NamespaceId>,
) -> (u16, Envelope<PolicyData>) {
    match result {
        Ok(outcome) => {
            let changed = matches!(
                outcome.receipt.as_ref().map(|r| &r.outcome),
                Some(MutationOutcome::NativeNamespacePolicyUpdated { .. })
            );
            let mut envelope = Envelope::success(
                Scope::Namespace {
                    namespace_id: outcome.namespace_id,
                    namespace: outcome.namespace,
                },
                PolicyData {
                    namespace_id: outcome.namespace_id,
                    policy: outcome.policy,
                    changed,
                },
            );
            envelope.receipt = outcome.receipt;
            (200, envelope)
        }
        Err(error) => {
            let (status, old) = failure(error);
            let mut envelope = Envelope::failure(None, old.error.expect("error envelope"));
            // Retired canonical names require the future validated purge witness;
            // never relabel historical receipts using a newly reused name.
            envelope.receipt = old.receipt;
            if let Some(id) = namespace_id {
                envelope
                    .warnings
                    .push(format!("Requested immutable namespaceId {}", id.get()));
            }
            (status, envelope)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("catalog worker outcome unknown; reconcile retained evidence")]
    Api1WorkerUnknown(#[source] Box<crate::serialization::catalog_api1::CatalogWorkerEvidence>),
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
            StorageError::NamespaceEmbeddingProfileMismatch { field } => (
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
            StorageError::NamespaceNotFound(_) => (
                404,
                NotFound,
                "NAMESPACE_NOT_FOUND",
                Some("/namespaceId".into()),
                CommitState::NotCommitted,
                None,
                false,
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
        CatalogError::Api1WorkerUnknown(evidence) => (
            503, Unavailable, "STORAGE_UNAVAILABLE", None, CommitState::Unknown,
            match evidence.checked() { Ok((operation, _)) => operation, Err(_) => None },
            false,
        ),
    };
    let message = error.to_string();
    let receipt = match error {
        CatalogError::Storage(StorageError::CommittedMutation { receipt, .. }) => Some(*receipt),
        CatalogError::Api1WorkerUnknown(evidence) => match evidence.checked() {
            Ok((_, receipt)) => receipt.cloned(),
            Err(_) => None, // Full invalid snapshot remains in the error message above.
        },
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
pub(crate) mod tests;
