//! Guarded barrier consumer of the owned tracker and the single view registry.
//! Producer coverage is a shipping prerequisite, not inferred from zero tickets.
use super::*;
use crate::storage::{Readiness, control::GenerationRef};
use std::time::Instant;

impl DerivedCompletion {
    pub(crate) async fn readiness_barrier(
        &self,
        target: GenerationRef,
        deadline: Instant,
    ) -> Result<Readiness, StorageError> {
        let capture = self.with_frozen_core(Some(target), deadline, |_, _, ready| Ok(ready));
        tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), capture)
            .await
            .map_err(|_| failed("barrier deadline expired".into()))?
    }

    /// Retain actual admission, publication, derived and storage leases through
    /// a synchronous capture. This is not itself a snapshot or cache/RNG proof.
    /// The controller is not registered as work: it must drain accepted roots.
    /// Cancellation cannot release leases already transferred to the worker.
    pub(crate) async fn with_frozen_core<T: Send + 'static>(
        &self,
        target: Option<GenerationRef>,
        deadline: Instant,
        capture: impl FnOnce(
            &RedbStorageEngine,
            &crate::storage::metadata::MetadataFreeze<'_>,
            Readiness,
        ) -> Result<T, StorageError>
        + Send
        + 'static,
    ) -> Result<T, StorageError> {
        let tracker = self
            .storage
            .read()
            .map_err(|e| failed(e.to_string()))?
            .meta_store()
            .work_tracker();
        if target.is_some_and(|target| tracker.store_id() != target.store_id) {
            return Err(failed("barrier target belongs to another lineage".into()));
        }
        // No storage/derived locks while accepted workers drain. When this
        // await is cancelled, the blocking closer owns its eventual lease.
        let admission = tokio::task::spawn_blocking(move || tracker.close_and_drain(deadline))
            .await
            .map_err(|e| failed(e.to_string()))??;
        let acquire = async {
            let gate = publication_gate(&self.storage).await?;
            let publication = gate.write_owned().await;
            let graph = self.graph.clone().read_owned().await;
            let vectors = self.vectors.clone().read_owned().await;
            let fts = self.fts.clone().lock_owned().await;
            let entities = self.entities.clone().read_owned().await;
            Ok::<_, StorageError>((publication, graph, vectors, fts, entities))
        };
        let leases = tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), acquire)
            .await
            .map_err(|_| failed("freeze deadline expired acquiring views".into()))??;
        let storage = self.storage.clone();
        // All leases move into this worker, not just the metadata admission.
        // No hard timeout over blocking OS I/O is promised. The controller must
        // retain its busy permit while joining even after its handler times out.
        tokio::task::spawn_blocking(move || {
            let _leases = leases;
            let storage = storage.read().map_err(|e| failed(e.to_string()))?;
            let metadata = storage.meta_store();
            let frozen = metadata.freeze_current_metadata(admission)?;
            let result = (|| {
                let state = metadata.frozen_publication_state(&frozen)?;
                if target.is_some_and(|target| {
                    state.canonical.store_id != target.store_id
                        || state.canonical.generation < target.generation
                }) {
                    return Err(failed(
                        "barrier target is not committed in this lineage".into(),
                    ));
                }
                let ready = state.ready_report()?;
                let result = capture(&storage, &frozen, ready)?;
                // Never return a successful staged capture after its deadline.
                // Capture callbacks stage data only; manifest publication belongs
                // after this checked result, not inside a potentially blocking copy.
                metadata.frozen_publication_state(&frozen)?.ready_report()?;
                Ok(result)
            })();
            let admission = frozen.release_publication();
            drop(storage);
            drop(_leases);
            drop(admission);
            result
        })
        .await
        .map_err(|e| failed(e.to_string()))?
    }
}
