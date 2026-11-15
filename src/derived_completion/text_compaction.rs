//! The existing maintenance caller waits for actual four-view publication.
use super::*;

impl DerivedCompletion {
    pub(crate) async fn compact_text_with_work(
        &self,
        parent: Option<&WorkTicket>,
    ) -> Result<crate::storage::CompactionResult, StorageError> {
        let work = self.accept_work(parent)?;
        let result = self.compact_text_accepted(&work).await;
        self.finish_known_work(work);
        result
    }

    async fn compact_text_accepted(
        &self,
        work: &WorkTicket,
    ) -> Result<crate::storage::CompactionResult, StorageError> {
        let lease = publication_gate(&self.storage).await?.read_owned().await;
        let graph = self.graph.clone().read_owned().await;
        let storage = self.storage.clone();
        let child = work.child()?;
        let result = tokio::task::spawn_blocking(move || {
            // Cancellation of the async waiter cannot release a queued worker's
            // publication lease. Lock order remains graph -> storage.
            let _lease = lease;
            let _graph = graph;
            let mut storage = storage.write().map_err(|e| failed(e.to_string()))?;
            let result = storage.compact_text_log_with_work(Some(&child));
            storage.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|e| failed(format!("text compaction worker outcome unknown: {e}")))??;
        self.complete_with_work(Some(work))
            .await
            .map_err(|source| StorageError::CommittedMutation {
                receipt: Box::new(result.receipt.clone()),
                source: Box::new(source),
            })?;
        Ok(result)
    }
}
