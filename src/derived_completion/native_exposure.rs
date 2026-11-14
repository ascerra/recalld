//! Accepted exposure commits for the existing query observer. Success is
//! published by QueryEngine; partial errors publish their preceding commits here.
use super::*;
use crate::decay::observation::{ObservationError, exposure_groups};
use crate::model::{AccessKind, MemoryId};
use crate::storage::control::{ExposureScope, MutationContext, WriteReceipt};
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs};

impl DerivedCompletion {
    pub(crate) async fn record_native_exposures_with_work(
        &self,
        accesses: Vec<(MemoryId, AccessKind)>,
        scope: Option<ExposureScope>,
        sampled_at: i64,
        policy: NativePolicyInputs,
        parent: Option<&WorkTicket>,
    ) -> Result<Vec<WriteReceipt>, ObservationError> {
        if accesses.is_empty() {
            return Ok(Vec::new());
        }
        let work = self
            .accept_work(parent)
            .map_err(|e| ObservationError::storage(e, vec![]))?;
        let result = self
            .native_exposures_accepted(accesses, scope, sampled_at, policy, &work)
            .await;
        self.finish_known_work(work);
        result
    }

    async fn native_exposures_accepted(
        &self,
        accesses: Vec<(MemoryId, AccessKind)>,
        scope: Option<ExposureScope>,
        sampled_at: i64,
        policy: NativePolicyInputs,
        work: &WorkTicket,
    ) -> Result<Vec<WriteReceipt>, ObservationError> {
        let gate = publication_gate(&self.storage)
            .await
            .map_err(|e| ObservationError::storage(e, vec![]))?;
        let lease = gate.read_owned().await;
        let graph = self.graph.clone().read_owned().await;
        let storage = self.storage.clone();
        let child = work
            .child()
            .map_err(|e| ObservationError::storage(e.into(), vec![]))?;
        let result = tokio::task::spawn_blocking(move || {
            // Cancellation cannot release the lease while a canonical worker runs.
            // Lock order remains publication -> graph -> storage.
            let _lease = lease;
            let _graph = graph;
            let storage = storage.write().map_err(ObservationError::worker_failed)?;
            let result = (|| {
                let groups = exposure_groups(&storage, &accesses)?;
                // This is the already-sampled query time, not another time source.
                let clock = crate::time::ManualClock::new(sampled_at);
                let mut receipts = Vec::new();
                for (namespace, batch) in groups {
                    let namespace = NamespaceId::new(namespace);
                    let result = (|| {
                        let config = storage
                            .get_namespace(namespace)?
                            .ok_or(StorageError::NamespaceNotFound(namespace.get()))?;
                        let scope = LifecycleScope {
                            namespace_id: namespace,
                            namespace_name: scope
                                .as_ref()
                                .map_or(config.name, |s| s.namespace.clone()),
                            source: scope.as_ref().and_then(|s| s.source),
                        };
                        // This fresh capture's G is truthful. It is NOT the earlier
                        // selection/readGeneration; no whole-query CAS is claimed.
                        let intent = storage
                            .meta_store()
                            .capture_exposure(scope, &batch, &policy)?;
                        let mut ctx = MutationContext::unkeyed(namespace);
                        ctx.contract_version = 2;
                        storage.meta_store().record_native_exposure_with_work(
                            &ctx,
                            &intent,
                            &policy,
                            &clock,
                            Some(&child),
                        )
                    })();
                    match result {
                        Ok(Some(receipt)) => receipts.push(receipt),
                        Ok(None) => {}
                        Err(error) => return Err(ObservationError::storage(error, receipts)),
                    }
                }
                Ok(receipts)
            })();
            storage.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(ObservationError::worker_failed)?;
        if let Err(mut error) = result {
            // The success caller performs full publication. An early return must
            // still await completion of any preceding namespace commits. Preserve
            // uncertainty and every receipt even if the poisoned owner cannot heal.
            if !error.receipts.is_empty() {
                if let Err(publication) = self.complete_with_work(Some(work)).await {
                    error
                        .message
                        .push_str(&format!("; INDEX_NOT_READY: {publication}"));
                }
            }
            return Err(error);
        }
        result
    }
}
