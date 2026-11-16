//! Awaited legacy consumers. Each namespace commits separately; errors retain
//! every completed receipt. No public CAS, keyed transport, or v2 policy here.
use std::sync::{Arc, RwLock};

use crate::cache::CacheManager;
use crate::model::{AccessKind, MemoryId, NamespaceId};
use crate::storage::control::{MutationContext, WriteReceipt};
use crate::storage::{RedbStorageEngine, StorageEngine, StorageError};

/// Internal result: receipts survive every admission check before a transport
/// releases the value. This is deliberately not a serialized success envelope.
pub(crate) struct Observed<T> {
    pub value: T,
    pub receipts: Vec<WriteReceipt>,
}

/// Rust-only recorder result. Receipts retain the existing full-publication path;
/// this type does not certify readiness or activate native exposure writes.
#[derive(Default)]
pub struct AccessObservation {
    pub receipts: Vec<WriteReceipt>,
}
impl AccessObservation {
    pub fn from_receipts(receipts: Vec<WriteReceipt>) -> Self {
        Self { receipts }
    }
}

impl<T> Observed<T> {
    pub fn unobserved(value: T) -> Self {
        Self {
            value,
            receipts: Vec::new(),
        }
    }

    /// Complete the actual owned views only after the payload leases are gone.
    /// A postcommit publication error retains every original receipt.
    pub(crate) async fn publish_derived(
        self,
        completion: Option<&crate::derived_completion::DerivedCompletion>,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Self, ObservationError> {
        if !self.receipts.is_empty() {
            if let Some(completion) = completion {
                completion
                    .complete_with_work(parent)
                    .await
                    .map_err(|e| ObservationError::not_ready(self.receipts.clone(), e))?;
            }
        }
        Ok(self)
    }

    pub fn finish<E>(self, admission: Result<(), E>) -> Result<T, E>
    where
        E: From<ObservationError> + std::fmt::Debug,
    {
        crate::performance::receipts(&self.receipts);
        tracing::trace!(target: "recalld::performance", kind = "final_admission",
            accepted = admission.is_ok(), receipt_count = self.receipts.len());
        match admission {
            Ok(()) => Ok(self.value),
            Err(error) if self.receipts.is_empty() => Err(error),
            Err(error) => {
                Err(ObservationError::not_ready(self.receipts, format!("{error:?}")).into())
            }
        }
    }
}

/// Payload-free failure, including preceding namespace commits. Never present
/// a failed multi-namespace observation as an ordinary safely retryable write.
#[derive(Debug, Clone, serde::Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("observation failed: {}", serde_json::to_string(self).unwrap_or_default())]
pub struct ObservationError {
    pub commit_state: &'static str,
    pub ready: bool,
    pub receipts: Vec<WriteReceipt>,
    pub unknown_operation_id: Option<uuid::Uuid>,
    pub message: String,
}

impl ObservationError {
    /// A completed exposure cannot become an ordinary retryable read failure.
    /// Retain the original numerical/ID receipts, never the selected payload.
    pub(crate) fn not_ready(receipts: Vec<WriteReceipt>, source: impl std::fmt::Display) -> Self {
        Self::failed(format!("INDEX_NOT_READY: {source}"), receipts)
    }

    pub(crate) fn worker_failed(error: impl std::fmt::Display) -> Self {
        Self {
            commit_state: "unknown",
            ready: false,
            receipts: vec![],
            unknown_operation_id: None,
            message: format!("observation worker failed; do not blindly retry: {error}"),
        }
    }

    fn failed(error: impl std::fmt::Display, receipts: Vec<WriteReceipt>) -> Self {
        Self {
            commit_state: if receipts.is_empty() {
                "notCommitted"
            } else {
                "committed"
            },
            ready: false,
            receipts,
            unknown_operation_id: None,
            message: error.to_string(),
        }
    }

    pub(crate) fn storage(error: StorageError, receipts: Vec<WriteReceipt>) -> Self {
        let unknown = match &error {
            StorageError::CommitOutcomeUnknown { operation_id, .. } => Some(*operation_id),
            _ => None,
        };
        let mut result = Self::failed(error, receipts);
        if unknown.is_some() {
            result.commit_state = "unknown";
            result.unknown_operation_id = unknown;
        }
        result
    }
}

/// Invalidation has no fallible normal path. A panic is a publication failure:
/// fail closed and retain the committed receipts. This runs on the blocking
/// worker under storage.write, so cancellation cannot drop publication after
/// commit. No graph lock is acquired here (graph -> storage remains valid).
pub(super) fn publish(
    storage: &RedbStorageEngine,
    receipts: &[WriteReceipt],
    invalidate: impl FnOnce(),
) -> Result<(), ObservationError> {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(invalidate)).is_err() {
        storage.mark_unavailable();
        return Err(ObservationError::failed(
            "INDEX_NOT_READY: cache publication failed; reopen required",
            receipts.to_vec(),
        ));
    }
    Ok(())
}

pub async fn record_exposures(
    storage: Arc<RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    accesses: Vec<(MemoryId, AccessKind)>,
    sampled_at: i64,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    record_exposures_scoped(storage, cache, accesses, sampled_at, None).await
}

pub async fn record_exposures_scoped(
    storage: Arc<RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    accesses: Vec<(MemoryId, AccessKind)>,
    sampled_at: i64,
    scope: Option<crate::storage::control::ExposureScope>,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    record_exposures_scoped_with_work(storage, cache, accesses, sampled_at, scope, None).await
}

pub(crate) async fn record_exposures_scoped_with_work(
    storage: Arc<RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    accesses: Vec<(MemoryId, AccessKind)>,
    sampled_at: i64,
    scope: Option<crate::storage::control::ExposureScope>,
    parent: Option<&crate::storage::work::WorkTicket>,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    record_exposures_scoped_with_origin_work(storage, cache, accesses, sampled_at, scope, parent, 1)
        .await
}

pub(crate) async fn record_exposures_scoped_with_origin_work(
    storage: Arc<RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    accesses: Vec<(MemoryId, AccessKind)>,
    sampled_at: i64,
    scope: Option<crate::storage::control::ExposureScope>,
    parent: Option<&crate::storage::work::WorkTicket>,
    original_contract_version: u32,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    record_exposures_with_scopes(
        storage,
        cache,
        accesses,
        sampled_at,
        crate::decay::similarity_scope::ExposureScopes::Existing(scope),
        parent,
        original_contract_version,
    )
    .await
}

pub(crate) async fn record_exposures_with_scopes(
    storage: Arc<RwLock<RedbStorageEngine>>,
    cache: Arc<CacheManager>,
    accesses: Vec<(MemoryId, AccessKind)>,
    sampled_at: i64,
    scope: crate::decay::similarity_scope::ExposureScopes,
    parent: Option<&crate::storage::work::WorkTicket>,
    original_contract_version: u32,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    if accesses.is_empty() {
        return Ok(Vec::new());
    }
    let work = crate::storage::work::accept_work(&storage, parent)
        .map_err(|e| ObservationError::storage(e, vec![]))?;
    let child = work
        .child()
        .map_err(|e| ObservationError::failed(e, vec![]))?;
    let worker_storage = storage.clone();
    let worker = crate::performance::Worker::new("exposure.worker_queue");
    let result = tokio::task::spawn_blocking(move || worker.run(|| {
        let wait = crate::performance::Timer::new("storage.write.wait");
        let storage = worker_storage
            .write()
            .map_err(|e| ObservationError::failed(e, vec![]))?;
        wait.complete();
        let storage = crate::performance::Hold::new("storage.write.hold", storage);
        let result = (|| {
            let transaction = crate::performance::Timer::new("exposure.canonical_commit");
            let result = record_locked_with_scopes(
                &storage,
                &accesses,
                sampled_at,
                &scope,
                Some(&child),
                original_contract_version,
            );
            transaction.outcome(result.is_ok());
            let receipts = match &result {
                Ok(r) => r,
                Err(e) => &e.receipts,
            };
            crate::performance::receipts(receipts);
            let publication = crate::performance::Timer::new("exposure.cache_publication");
            let ids: Vec<_> = accesses.iter().map(|(id, _)| *id).collect();
            // Publish even preceding successful batches when a later batch fails.
            if let Err(mut publication) = publish(&storage, receipts, || {
                tokio::runtime::Handle::current().block_on(cache.batch_invalidate(&ids));
            }) {
                if let Err(prior) = &result
                    && prior.commit_state == "unknown"
                {
                    publication.commit_state = "unknown";
                    publication.unknown_operation_id = prior.unknown_operation_id;
                }
                return Err(publication);
            }
            publication.complete();
            tracing::trace!(target: "recalld::performance", kind = "exposure", requested = accesses.len(), receipt_count = receipts.len(), success = result.is_ok());
            #[cfg(test)]
            if result.as_ref().is_ok_and(|r| !r.is_empty()) {
                storage.meta_store().after_observation_probe();
            }
            result
        })();
        storage.meta_store().finish_work(child);
        result
    }))
    .await
    .map_err(ObservationError::worker_failed)?;
    crate::storage::work::finish_work(&storage, work);
    result
}

#[cfg(test)]
fn record_locked_scoped(
    storage: &RedbStorageEngine,
    accesses: &[(MemoryId, AccessKind)],
    sampled_at: i64,
    scope: Option<&crate::storage::control::ExposureScope>,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    record_locked_scoped_with_work(storage, accesses, sampled_at, scope, None, 1)
}

fn record_locked_scoped_with_work(
    storage: &RedbStorageEngine,
    accesses: &[(MemoryId, AccessKind)],
    sampled_at: i64,
    scope: Option<&crate::storage::control::ExposureScope>,
    parent: Option<&crate::storage::work::WorkTicket>,
    original_contract_version: u32,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    record_locked_with_scopes(
        storage,
        accesses,
        sampled_at,
        &crate::decay::similarity_scope::ExposureScopes::Existing(scope.cloned()),
        parent,
        original_contract_version,
    )
}

fn record_locked_with_scopes(
    storage: &RedbStorageEngine,
    accesses: &[(MemoryId, AccessKind)],
    sampled_at: i64,
    scope: &crate::decay::similarity_scope::ExposureScopes,
    parent: Option<&crate::storage::work::WorkTicket>,
    original_contract_version: u32,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    let groups = exposure_groups(storage, accesses)?;
    let scopes = scope
        .validate(storage, &groups)
        .map_err(|e| ObservationError::storage(e, vec![]))?;
    let mut receipts = Vec::new();
    for (namespace, batch) in groups {
        let scope = scopes.get(&namespace).expect("validated group").as_ref();
        let mut ctx = MutationContext::unkeyed(NamespaceId::new(namespace));
        ctx.contract_version = original_contract_version;
        match storage
            .meta_store()
            .record_exposure_batch_scoped_with_work(&ctx, &batch, sampled_at, scope, parent)
        {
            Ok(Some(receipt)) => receipts.push(receipt),
            Ok(None) => {}
            Err(error) => return Err(ObservationError::storage(error, receipts)),
        }
    }
    Ok(receipts)
}

/// Shared canonical grouping for both legacy and native observation writers.
/// Validate all kinds/IDs first; direct dominates associative for duplicate UUIDs.
pub(crate) fn exposure_groups(
    storage: &RedbStorageEngine,
    accesses: &[(MemoryId, AccessKind)],
) -> Result<std::collections::BTreeMap<u32, Vec<(MemoryId, AccessKind)>>, ObservationError> {
    // First occurrence order within each namespace; namespace order ascending.
    // Direct exposure dominates an additional associative occurrence.
    let mut distinct: Vec<(MemoryId, AccessKind)> = Vec::new();
    let mut positions = std::collections::HashMap::<MemoryId, usize>::new();
    for &(id, kind) in accesses {
        if !matches!(
            kind,
            AccessKind::DirectRetrieval | AccessKind::AssociativeRetrieval
        ) {
            return Err(ObservationError::failed("invalid exposure kind", vec![]));
        }
        if let Some(&index) = positions.get(&id) {
            if kind == AccessKind::DirectRetrieval {
                distinct[index].1 = kind;
            }
        } else {
            positions.insert(id, distinct.len());
            distinct.push((id, kind));
        }
    }
    let mut groups = std::collections::BTreeMap::<u32, Vec<_>>::new();
    for (id, kind) in distinct {
        let record = storage
            .get_record(id)
            .map_err(|e| ObservationError::storage(e, vec![]))?
            .ok_or_else(|| ObservationError::failed(format!("memory {id} not found"), vec![]))?;
        groups
            .entry(record.namespace_id)
            .or_default()
            .push((id, kind));
    }
    Ok(groups)
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;

#[cfg(test)]
fn record_locked(
    storage: &RedbStorageEngine,
    accesses: &[(MemoryId, AccessKind)],
    sampled_at: i64,
) -> Result<Vec<WriteReceipt>, ObservationError> {
    record_locked_scoped(storage, accesses, sampled_at, None)
}
