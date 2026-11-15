//! Native feedback on the existing owner, accepted work and publication boundary.
use super::*;
use crate::model::MemoryId;
use crate::storage::control::{MutationContext, MutationScope, WriteReceipt};
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::time::Clock;

impl DerivedCompletion {
    pub(crate) async fn feedback_native_with_work(
        &self,
        ctx: MutationContext,
        id: MemoryId,
        quality: u8,
        asserted_namespace: Option<String>,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        let work = self.accept_work(parent)?;
        let result = self
            .feedback_native_accepted(ctx, id, quality, asserted_namespace, policy, clock, &work)
            .await;
        self.finish_known_work(work);
        result
    }

    async fn feedback_native_accepted(
        &self,
        ctx: MutationContext,
        id: MemoryId,
        quality: u8,
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
            let storage = storage.write().map_err(|e| failed(e.to_string()))?;
            let result = (|| {
                if let Some(receipt) = storage.meta_store().lookup_native_feedback(
                    &ctx,
                    id,
                    quality,
                    asserted_namespace.as_deref(),
                    Some(&child),
                )? {
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
                        .capture_feedback(id, Some(namespace), quality, &policy)?;
                storage.meta_store().apply_native_feedback_with_work(
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
        .map_err(|e| StorageError::WorkerOutcomeUnknown(e.to_string()))??;
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
