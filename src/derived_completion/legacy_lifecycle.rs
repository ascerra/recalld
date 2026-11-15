//! Prepared legacy lifecycle operations on the existing accepted owner/core.
use super::*;
use crate::model::MemoryId;
use crate::storage::control::{MutationContext, WriteReceipt};
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::time::Clock;

impl DerivedCompletion {
    pub(crate) async fn legacy_lifecycle_with_work(
        &self,
        ctx: MutationContext,
        id: MemoryId,
        quality: Option<u8>,
        asserted_namespace: Option<String>,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        let work = self.accept_work(parent)?;
        let result = async {
            let lease = publication_gate(&self.storage).await?.read_owned().await;
            let graph = self.graph.clone().read_owned().await;
            let storage = self.storage.clone();
            let child = work.child()?;
            let receipt = tokio::task::spawn_blocking(move || {
                // Cancellation cannot release ownership while the real worker writes.
                let _lease = lease;
                let _graph = graph;
                let mut storage = storage.write().map_err(|e| failed(e.to_string()))?;
                let result = match quality {
                    Some(quality) => storage.meta_store().record_legacy_feedback_clock_with_work(
                        &ctx,
                        id,
                        quality,
                        clock.as_ref(),
                        &policy,
                        asserted_namespace.as_deref(),
                        Some(&child),
                    ),
                    None => storage.tombstone_memory_scoped_with_work(
                        &ctx,
                        id,
                        None,
                        asserted_namespace.as_deref(),
                        Some(&child),
                    ),
                };
                storage.meta_store().finish_work(child);
                result
            })
            .await
            .map_err(|e| StorageError::WorkerOutcomeUnknown(e.to_string()))??;
            self.complete_with_work(Some(&work))
                .await
                .map_err(|source| StorageError::CommittedMutation {
                    receipt: Box::new(receipt.clone()),
                    source: Box::new(source),
                })?;
            Ok(receipt)
        }
        .await;
        self.finish_known_work(work);
        result
    }
}
