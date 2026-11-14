//! Native forget on the existing owner, accepted work and publication boundary.
use super::*;
use crate::model::MemoryId;
use crate::storage::control::{MutationContext, MutationScope, WriteReceipt};
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::time::Clock;

impl DerivedCompletion {
    pub(crate) async fn forget_native_with_work(
        &self,
        ctx: MutationContext,
        id: MemoryId,
        asserted_namespace: Option<String>,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        let work = self.accept_work(parent)?;
        let result = self
            .forget_native_accepted(ctx, id, asserted_namespace, policy, clock, &work)
            .await;
        self.finish_known_work(work);
        result
    }

    async fn forget_native_accepted(
        &self,
        ctx: MutationContext,
        id: MemoryId,
        asserted_namespace: Option<String>,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        work: &WorkTicket,
    ) -> Result<WriteReceipt, StorageError> {
        let lease = publication_gate(&self.storage).await?.read_owned().await;
        let graph = self.graph.clone().read_owned().await;
        let storage = self.storage.clone();
        let child = work.child()?;
        let receipt = tokio::task::spawn_blocking(move || {
            // The worker retains both leases even if its async waiter is aborted.
            let _lease = lease;
            let _graph = graph;
            let mut storage = storage.write().map_err(|e| failed(e.to_string()))?;
            let result = (|| {
                if let Some(receipt) =
                    storage
                        .meta_store()
                        .lookup_native_tombstone_by_id(&ctx, id, Some(&child))?
                {
                    // A name is an assertion, never a replacement for stable scope.
                    // Retired names need a validated purge-history consumer. Until
                    // then, fail closed with the known receipt instead of rebinding.
                    if let Some(name) = asserted_namespace.as_deref() {
                        let assertion = (|| {
                            let MutationScope::Namespace(namespace) = ctx.scope else {
                                return Err(StorageError::MutationNamespaceMismatch);
                            };
                            if storage
                                .get_namespace(namespace)?
                                .is_some_and(|ns| ns.name == name)
                            {
                                Ok(())
                            } else {
                                Err(StorageError::NamespaceMismatch {
                                    id: id.into_inner(),
                                    namespace: name.to_owned(),
                                })
                            }
                        })();
                        assertion.map_err(|source| StorageError::CommittedMutation {
                            receipt: Box::new(receipt.clone()),
                            source: Box::new(source),
                        })?;
                    }
                    return Ok(receipt);
                }
                if let Some(name) = asserted_namespace.as_deref() {
                    storage.assert_memory_namespace(id, name)?;
                }
                let MutationScope::Namespace(namespace) = ctx.scope else {
                    return Err(StorageError::MutationNamespaceMismatch);
                };
                let intent =
                    storage
                        .meta_store()
                        .capture_native_tombstone(id, Some(namespace), &policy)?;
                storage.tombstone_native_record(
                    &ctx,
                    &intent,
                    &policy,
                    clock.as_ref(),
                    Some(&child),
                )
            })();
            storage.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|e| {
            failed(format!(
                "native forget worker outcome unknown; reconcile before retry: {e}"
            ))
        })??;
        // Never upgrade the worker's read lease in place. Publication uses the
        // same owner and registry; every known receipt survives completion failure.
        self.complete_with_work(Some(work))
            .await
            .map_err(|source| StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            })?;
        Ok(receipt)
    }
}
