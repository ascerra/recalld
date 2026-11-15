//! The existing maintenance caller waits for actual four-view publication.
use super::*;

impl DerivedCompletion {
    pub(crate) async fn compact_text_with_work(
        &self,
        parent: Option<&WorkTicket>,
    ) -> Result<crate::storage::CompactionResult, StorageError> {
        let work = self.accept_work(parent)?;
        let result = self
            .compact_text_accepted(
                &work,
                #[cfg(test)]
                None,
            )
            .await;
        self.finish_known_work(work);
        result
    }

    async fn compact_text_accepted(
        &self,
        work: &WorkTicket,
        #[cfg(test)] worker_pause: Option<(
            tokio::sync::oneshot::Sender<()>,
            std::sync::mpsc::Receiver<()>,
        )>,
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
            #[cfg(test)]
            if let Some((entered, release)) = worker_pause {
                entered.send(()).unwrap();
                release
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn compaction_cancelled_waiter_cannot_release_worker_publication_lease() {
        let f = crate::test_support::Fixture::new();
        f.insert_memory().await;
        let c = DerivedCompletion {
            storage: f.storage.clone(),
            cache: f.cache.clone(),
            graph: f.graph.clone(),
            vectors: f.vector_index.clone(),
            fts: f.fts_index.clone(),
            entities: f.entity_index.clone(),
        };
        c.complete().await.unwrap();
        let before = f
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
        let gate = publication_gate(&f.storage).await.unwrap();
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let (entered, waiting) = tokio::sync::oneshot::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            let work = c.accept_work(None).unwrap();
            let result = c
                .compact_text_accepted(&work, Some((entered, resume)))
                .await;
            c.finish_known_work(work);
            result
        });
        waiting.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(gate.clone().try_write_owned().is_err());
        assert!(tracker.status().pending > 0);
        release.send(()).unwrap();
        let lease = tokio::time::timeout(std::time::Duration::from_secs(10), gate.write_owned())
            .await
            .unwrap();
        let after = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap();
        assert_eq!(
            after.canonical.generation,
            before.generation.next().unwrap()
        );
        assert!(!after.matches_canonical());
        assert!(tracker.status().unresolved > 0);
        drop(lease);
    }
}
