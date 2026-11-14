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
        let tracker = self
            .storage
            .read()
            .map_err(|e| failed(e.to_string()))?
            .meta_store()
            .work_tracker();
        if tracker.store_id() != target.store_id {
            return Err(failed("barrier target belongs to another lineage".into()));
        }
        // Never hold outer storage/derived locks while accepted workers drain.
        // A timeout drops the eventual freeze lease even if blocking work exits
        // later; this controller does not fabricate a worker completion.
        let operation = async {
            let admission = tokio::task::spawn_blocking(move || tracker.close_and_drain(deadline))
                .await
                .map_err(|e| failed(e.to_string()))??;
            let gate = publication_gate(&self.storage).await?;
            let _publication = gate.write_owned().await;
            let _graph = self.graph.clone().read_owned().await;
            let _vectors = self.vectors.clone().read_owned().await;
            let _fts = self.fts.clone().lock_owned().await;
            let _entities = self.entities.clone().read_owned().await;
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                let storage = storage.read().map_err(|e| failed(e.to_string()))?;
                let metadata = storage.meta_store();
                let frozen = metadata.freeze_current_metadata(admission)?;
                let state = metadata.frozen_publication_state(&frozen)?;
                if state.canonical.store_id != target.store_id
                    || state.canonical.generation < target.generation
                {
                    return Err(failed(
                        "barrier target is not committed in this lineage".into(),
                    ));
                }
                state.ready_report()
            })
            .await
            .map_err(|e| failed(e.to_string()))?
        };
        tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), operation)
            .await
            .map_err(|_| failed("barrier deadline expired".into()))?
    }
}
