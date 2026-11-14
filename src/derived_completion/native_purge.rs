//! Native purge uses the existing accepted task and completion owner.
use super::*;
use crate::storage::control::MutationContext;
use crate::storage::purge::{PurgeProgress, PurgeStage};

impl DerivedCompletion {
    pub(crate) async fn purge_native_accepted(
        &self,
        ctx: MutationContext,
        force: bool,
        parent: &WorkTicket,
    ) -> Result<PurgeProgress, StorageError> {
        if ctx.contract_version != 2 {
            return Err(StorageError::InvalidMutationContext(
                "native purge requires contract2".into(),
            ));
        }
        let work = self.accept_work(Some(parent))?;
        let result = self.native_purge_work(ctx, force, &work).await;
        self.finish_known_work(work);
        result
    }
    async fn native_purge_work(
        &self,
        ctx: MutationContext,
        force: bool,
        work: &WorkTicket,
    ) -> Result<PurgeProgress, StorageError> {
        let gate = publication_gate(&self.storage).await?;
        let lease = gate.read_owned().await;
        let graph = self.graph.clone().read_owned().await;
        let storage = self.storage.clone();
        let child = work.child()?;
        let result = tokio::task::spawn_blocking(move || {
            let _graph = graph;
            let mut storage = storage.write().map_err(|e| failed(e.to_string()))?;
            let result = storage.begin_namespace_purge_with_work(&ctx, force, Some(&child));
            storage.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|e| {
            failed(format!(
                "purge worker outcome unknown; reconcile original key: {e}"
            ))
        })?;
        drop(lease);
        let start = result?;
        let completion = self.complete_with_work(Some(work)).await;
        // Uncertain acknowledgements remain uncertain. A known child failure
        // already carries its exact receipt; never replace it with parent evidence.
        if matches!(
            &completion,
            Err(StorageError::CommittedMutation { .. } | StorageError::CommitOutcomeUnknown { .. })
        ) {
            return completion.map(|_| start);
        }
        let latest = self
            .storage
            .read()
            .map_err(|e| failed(e.to_string()))
            .and_then(|storage| {
                storage.lookup_namespace_purge(
                    start.store_id,
                    start.namespace_id,
                    start.start_receipt.operation_id,
                )
            })
            .and_then(|p| p.ok_or_else(|| failed("purge progress disappeared".into())));
        let latest = latest.map_err(|source| StorageError::CommittedMutation {
            receipt: Box::new(start.last_receipt.clone()),
            source: Box::new(source),
        })?;
        completion.map_err(|source| StorageError::CommittedMutation {
            receipt: Box::new(latest.last_receipt.clone()),
            source: Box::new(source),
        })?;
        if latest.stage != PurgeStage::Complete {
            return Err(StorageError::CommittedMutation {
                receipt: Box::new(latest.last_receipt),
                source: Box::new(failed("purge completion left active progress".into())),
            });
        }
        Ok(latest)
    }
}
