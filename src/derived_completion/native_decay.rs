//! Accepted maintenance consumer. The producer owns root/pass registration.
use super::*;
use crate::storage::control::MutationContext;
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::storage::native_maintenance::{NativeDecayIntent, NativeDecayResult};

impl DerivedCompletion {
    /// Consume the producer's existing ticket, retaining the original snapshot
    /// clock through canonical commit and awaited publication. No new root.
    pub(crate) async fn apply_native_decay_accepted(
        &self,
        ctx: MutationContext,
        intent: NativeDecayIntent,
        policy: NativePolicyInputs,
        parent: &WorkTicket,
    ) -> Result<NativeDecayResult, StorageError> {
        let work = self.accept_work(Some(parent))?;
        let result = self.native_decay_work(ctx, intent, policy, &work).await;
        self.finish_known_work(work);
        result
    }

    async fn native_decay_work(
        &self,
        ctx: MutationContext,
        intent: NativeDecayIntent,
        policy: NativePolicyInputs,
        work: &WorkTicket,
    ) -> Result<NativeDecayResult, StorageError> {
        let gate = publication_gate(&self.storage).await?;
        let lease = gate.read_owned().await;
        // Graph -> storage is the shared writer order. The engine recomputes
        // from canonical edges/rows, so a stale cached graph is never authority.
        let graph = self.graph.clone().read_owned().await;
        let storage = self.storage.clone();
        let child = work.child()?;
        let result = tokio::task::spawn_blocking(move || {
            // Cancellation of the waiter must not release a running worker's lease.
            let _lease = lease;
            let _graph = graph;
            let mut storage = storage.write().map_err(|e| failed(e.to_string()))?;
            let result = storage.apply_native_decay(&ctx, &intent, &policy, Some(&child));
            storage.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|e| failed(e.to_string()))?;
        let result = result?;
        if let NativeDecayResult::Committed(receipt) = &result {
            self.complete_with_work(Some(work))
                .await
                .map_err(|source| StorageError::CommittedMutation {
                    receipt: Box::new(receipt.clone()),
                    source: Box::new(source),
                })?;
        }
        Ok(result)
    }
}
