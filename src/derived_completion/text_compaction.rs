//! The existing maintenance caller waits for actual four-view publication.
use super::*;

impl DerivedCompletion {
    pub(crate) async fn compact_text_with_work(
        &self,
        parent: Option<&WorkTicket>,
    ) -> Result<crate::storage::CompactionResult, StorageError> {
        self.compact_text_with_work_bounded(parent, None).await
    }

    pub(crate) async fn compact_text_with_work_bounded(
        &self,
        parent: Option<&WorkTicket>,
        admission: Option<&crate::storage::maintenance::ReceiptAdmission>,
    ) -> Result<crate::storage::CompactionResult, StorageError> {
        let work = self.accept_work(parent)?;
        let result = self
            .compact_text_accepted_bounded(
                &work,
                admission,
                #[cfg(test)]
                None,
            )
            .await;
        if let (Some(admission), Err(error)) = (admission, &result) {
            admission.fail(error);
        }
        if !admission.is_some_and(crate::storage::maintenance::ReceiptAdmission::unknown) {
            self.finish_known_work(work);
        }
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
        self.compact_text_accepted_bounded(
            work,
            None,
            #[cfg(test)]
            worker_pause,
        )
        .await
    }

    async fn compact_text_accepted_bounded(
        &self,
        work: &WorkTicket,
        admission: Option<&crate::storage::maintenance::ReceiptAdmission>,
        #[cfg(test)] worker_pause: Option<(
            tokio::sync::oneshot::Sender<()>,
            std::sync::mpsc::Receiver<()>,
        )>,
    ) -> Result<crate::storage::CompactionResult, StorageError> {
        // A sweep may have just committed phase/deletion effects. Finish them
        // before starting a new physical mutation, so their known failure keeps
        // its original receipt and does not create an extra unreported commit.
        // Reject an already unavailable/purging owner before completion could
        // resume unrelated purge work. This read guard ends before the await.
        {
            let storage = self.storage.read().map_err(|e| failed(e.to_string()))?;
            storage.ensure_available()?;
        }
        self.complete_with_admission(Some(work), admission).await?;
        let lease = publication_gate(&self.storage).await?.read_owned().await;
        let graph = self.graph.clone().read_owned().await;
        let storage = self.storage.clone();
        let child = work.child()?;
        let worker_admission = admission.cloned();
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
            let result =
                storage.compact_text_log_with_work_bounded(Some(&child), worker_admission.as_ref());
            storage.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|e| {
            if admission.is_some() {
                StorageError::WorkerOutcomeUnknown(e.to_string())
            } else {
                failed(format!("text compaction worker outcome unknown: {e}"))
            }
        })??;
        if let Some(admission) = admission {
            admission.record(&result.receipt).map_err(|source| {
                StorageError::CommittedMutation {
                    receipt: Box::new(result.receipt.clone()),
                    source: Box::new(source),
                }
            })?;
        }
        self.complete_with_admission(Some(work), admission)
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
    async fn bounded_compaction_entry_refuses_peer_after_completion_before_worker() {
        let f = crate::test_support::Fixture::new();
        let id = f.insert_memory().await;
        let c = f.completion();
        c.complete().await.unwrap();
        let admission = crate::storage::maintenance::ReceiptAdmission::new(8192).unwrap();
        let observed = admission.clone();
        let (entered, waiting) = tokio::sync::oneshot::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            let work = c.accept_work(None).unwrap();
            let result = c
                .compact_text_accepted_bounded(&work, Some(&admission), Some((entered, resume)))
                .await;
            if let Err(error) = &result {
                admission.fail(error);
            }
            c.finish_known_work(work);
            result
        });
        tokio::time::timeout(std::time::Duration::from_secs(10), waiting)
            .await
            .unwrap()
            .unwrap();
        let (foreign, at, selection, pending) = {
            let engine = f.storage.write().unwrap();
            let record = engine.get_record(id).unwrap().unwrap();
            let ns = engine
                .get_namespace(crate::model::NamespaceId::new(record.namespace_id))
                .unwrap()
                .unwrap();
            let result = engine
                .apply_legacy_decay_with_receipt(
                    &crate::storage::control::MutationContext::unkeyed(ns.id),
                    id,
                    &record,
                    &ns,
                    record.phase,
                    0.5,
                    0.5,
                )
                .unwrap();
            let crate::storage::maintenance::DecayMutationResult::Committed(receipt) = result
            else {
                panic!("real peer commit");
            };
            (
                receipt,
                engine.control_state().unwrap().generation_ref(),
                engine.meta_store().text_selection().unwrap(),
                serde_json::to_vec(&engine.meta_store().pending_effects().unwrap()).unwrap(),
            )
        };
        release.send(()).unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            result,
            Err(StorageError::MaintenanceDependencyUnavailable)
        ));
        let (_, known, _) = observed.snapshot().unwrap();
        assert!(known.is_empty());
        let engine = f.storage.read().unwrap();
        assert_eq!(engine.control_state().unwrap().generation_ref(), at);
        assert_eq!(engine.meta_store().text_selection().unwrap(), selection);
        assert_eq!(
            serde_json::to_vec(&engine.meta_store().pending_effects().unwrap()).unwrap(),
            pending
        );
        assert_eq!(
            engine.meta_store().pending_effects().unwrap()[0].receipt,
            foreign
        );
        assert!(
            !engine
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
    }

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
