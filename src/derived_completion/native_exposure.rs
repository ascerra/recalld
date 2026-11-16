//! Accepted exposure commits for the existing query observer. Success is
//! published by QueryEngine; partial errors publish their preceding commits here.
use super::*;
use crate::decay::observation::{ObservationError, exposure_groups};
use crate::model::{AccessKind, MemoryId};
use crate::storage::control::{ExposureScope, MutationContext, WriteReceipt};
use crate::storage::native_lifecycle::{ExposureIntent, LifecycleScope, NativePolicyInputs};

impl DerivedCompletion {
    pub(crate) async fn record_native_exposures_with_work(
        &self,
        accesses: Vec<(MemoryId, AccessKind)>,
        scope: Option<ExposureScope>,
        sampled_at: i64,
        policy: NativePolicyInputs,
        parent: Option<&WorkTicket>,
        original_contract_version: u32,
    ) -> Result<Vec<WriteReceipt>, ObservationError> {
        self.record_native_exposures_with_scopes(
            accesses,
            crate::decay::similarity_scope::ExposureScopes::Existing(scope),
            sampled_at,
            policy,
            parent,
            original_contract_version,
        )
        .await
    }

    pub(crate) async fn record_native_exposures_with_scopes(
        &self,
        accesses: Vec<(MemoryId, AccessKind)>,
        scope: crate::decay::similarity_scope::ExposureScopes,
        sampled_at: i64,
        policy: NativePolicyInputs,
        parent: Option<&WorkTicket>,
        original_contract_version: u32,
    ) -> Result<Vec<WriteReceipt>, ObservationError> {
        if accesses.is_empty() {
            return Ok(Vec::new());
        }
        let work = self
            .accept_work(parent)
            .map_err(|e| ObservationError::storage(e, vec![]))?;
        let result = self
            .native_exposures_accepted(
                accesses,
                scope,
                sampled_at,
                policy,
                &work,
                original_contract_version,
                None,
            )
            .await;
        self.finish_known_work(work);
        result
    }

    /// Consume the original point-capture witness; never recapture policy or
    /// record tokens at the later observation boundary.
    pub(crate) async fn record_native_point_exposure_with_work(
        &self,
        intent: ExposureIntent,
        sampled_at: i64,
        policy: NativePolicyInputs,
        parent: Option<&WorkTicket>,
        original_contract_version: u32,
    ) -> Result<Vec<WriteReceipt>, ObservationError> {
        if !matches!(original_contract_version, 1 | 2)
            || intent.original.len() != 1
            || intent.original[0].1 != AccessKind::DirectRetrieval
            || intent.scope.source != Some(intent.original[0].0)
        {
            return Err(ObservationError::storage(
                StorageError::InvalidMutation("invalid captured point exposure".into()),
                vec![],
            ));
        }
        let scope = ExposureScope {
            namespace: intent.scope.namespace_name.clone(),
            source: intent.scope.source,
        };
        let work = self.accept_work(parent)
            .map_err(|e| ObservationError::storage(e, vec![]))?;
        let result = self.native_exposures_accepted(
            intent.original.clone(),
            crate::decay::similarity_scope::ExposureScopes::Existing(Some(scope)),
            sampled_at,
            policy,
            &work,
            original_contract_version,
            Some(intent),
        ).await;
        self.finish_known_work(work);
        result
    }

    async fn native_exposures_accepted(
        &self,
        accesses: Vec<(MemoryId, AccessKind)>,
        scope: crate::decay::similarity_scope::ExposureScopes,
        sampled_at: i64,
        policy: NativePolicyInputs,
        work: &WorkTicket,
        original_contract_version: u32,
        mut captured: Option<ExposureIntent>,
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
                let scopes = scope
                    .validate(&storage, &groups)
                    .map_err(|e| ObservationError::storage(e, vec![]))?;
                // This is the already-sampled query time, not another time source.
                let clock = crate::time::ManualClock::new(sampled_at);
                let mut receipts = Vec::new();
                let captured_point = captured.is_some();
                for (namespace, batch) in groups {
                    let scope = scopes.get(&namespace).expect("validated group").as_ref();
                    let namespace = NamespaceId::new(namespace);
                    let result = (|| {
                        // Instance-local regression: fail a later group before
                        // its commit, after a preceding real namespace commit.
                        #[cfg(test)]
                        if storage
                            .meta_store()
                            .observation_fault
                            .compare_exchange(
                                5,
                                0,
                                std::sync::atomic::Ordering::SeqCst,
                                std::sync::atomic::Ordering::SeqCst,
                            )
                            .is_ok()
                        {
                            return Err(StorageError::InvalidMutation(
                                "injected later exposure group failure".into(),
                            ));
                        }
                        let intent = if let Some(intent) = captured.take() {
                            if intent.scope.namespace_id != namespace || intent.original != batch {
                                return Err(StorageError::InvalidMutation(
                                    "captured point exposure scope changed".into(),
                                ));
                            }
                            intent
                        } else {
                            if captured_point {
                                return Err(StorageError::InvalidMutation(
                                    "captured point exposure cannot recapture".into(),
                                ));
                            }
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
                            storage.meta_store().capture_exposure(scope, &batch, &policy)?
                        };
                        let mut ctx = MutationContext::unkeyed(namespace);
                        ctx.contract_version = original_contract_version;
                        storage.meta_store().record_native_exposure_with_work(
                            &ctx,
                            &intent,
                            &policy,
                            &clock,
                            Some(&child),
                        )
                    })();
                    match result {
                        Ok(Some(receipt)) => {
                            receipts.push(receipt);
                            #[cfg(test)]
                            let _ = storage.meta_store().observation_fault.compare_exchange(
                                4,
                                5,
                                std::sync::atomic::Ordering::SeqCst,
                                std::sync::atomic::Ordering::SeqCst,
                            );
                        }
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
