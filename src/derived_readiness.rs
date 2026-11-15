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

impl DerivedCompletion {
    pub(crate) async fn observe_readiness(
        &self,
        deadline: Instant,
    ) -> Result<crate::storage::ReadinessObservation, StorageError> {
        let storage = self.storage.clone();
        let operation = async {
            let gate = publication_gate(&storage).await?;
            let publication = gate.read_owned().await;
            tokio::task::spawn_blocking(move || {
                // The actual lease stays with the worker after caller timeout.
                let _publication = publication;
                let engine = storage.read().map_err(|e| failed(e.to_string()))?;
                engine.meta_store().observe_readiness(deadline)
            })
            .await
            .map_err(|e| failed(e.to_string()))?
        };
        tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), operation)
            .await
            .map_err(|_| failed("readiness observation deadline expired".into()))?
    }
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use crate::test_support::{DIM, Fixture};
    use std::time::Duration;

    fn owner(fx: &Fixture) -> DerivedCompletion {
        DerivedCompletion {
            storage: fx.storage.clone(),
            cache: fx.cache.clone(),
            graph: fx.graph.clone(),
            vectors: fx.vector_index.clone(),
            fts: fx.fts_index.clone(),
            entities: fx.entity_index.clone(),
        }
    }
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn actual_committed_failure_observes_unknown_views_receipt_and_callers_live_root() {
        let fx = Fixture::new();
        let publisher = owner(&fx);
        let empty = publisher.observe_readiness(deadline()).await.unwrap();
        assert!(
            empty
                .derived
                .iter()
                .all(|v| v.applied.is_none() && !v.matches_canonical)
        );
        let live = fx.insert_memory().await;
        let dead = fx.insert_memory().await;
        publisher.complete().await.unwrap();
        let ready = publisher.observe_readiness(deadline()).await.unwrap();
        assert!(ready.derived.iter().all(|v| v.matches_canonical));
        let root = publisher.accept_work(None).unwrap();
        fx.storage.write().unwrap().delete_memory(dead).unwrap();
        *fx.vector_index.write().await = FlatVectorIndex::new(DIM + 1);
        let error = publisher.complete_with_work(Some(&root)).await.unwrap_err();
        let StorageError::CommittedMutation { receipt, .. } = error else {
            panic!("lost committed receipt: {error}")
        };
        let observation = publisher.observe_readiness(deadline()).await.unwrap();
        let mut envelope = crate::serialization::contract::Envelope::<serde_json::Value>::failure(
            None,
            crate::serialization::contract::ExecutionError {
                category: crate::serialization::contract::ErrorCategory::Unavailable,
                code: "INDEX_NOT_READY".into(),
                field: None,
                message: "publication failed after commit".into(),
                retryable: false,
                commit_state: crate::serialization::contract::CommitState::Committed,
                operation_id: Some(receipt.operation_id),
            },
        );
        envelope.receipt = Some((*receipt).clone());
        envelope.observe_failure(Ok(observation.clone()));
        let wire = serde_json::to_value(&envelope).unwrap();
        assert_eq!(wire["readiness"]["pendingWork"], 1);
        assert_eq!(wire["readiness"]["pendingWorkScope"], "acceptedTickets");
        assert_eq!(wire["readiness"]["observedWork"]["pendingEffects"], 1);
        assert_eq!(wire["readiness"]["ready"], false);
        assert!(
            wire["readiness"]["derived"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["name"] == "fts" && v["appliedGeneration"].is_null())
        );
        assert_eq!(envelope.receipt.as_ref(), Some(receipt.as_ref()));
        assert_eq!(observation.canonical.store_id, receipt.store_id);
        assert_eq!(
            observation.canonical.generation,
            receipt.committed_generation
        );
        assert_eq!(
            observation.accepted_work, 1,
            "observation must not drain/finish caller"
        );
        assert_eq!(observation.unresolved_work, 0);
        assert_eq!(observation.pending_effects, 1);
        assert_eq!(observation.pending_purges, 0);
        assert_eq!(observation.pending_batch_items, 0);
        assert!(
            observation
                .derived
                .iter()
                .any(|v| v.name == "fts" && v.applied.is_none())
        );
        assert!(observation.derived.iter().all(|v| !v.matches_canonical));
        assert!(
            observation
                .derived
                .iter()
                .filter_map(|v| v.applied)
                .all(|g| g == ready.canonical)
        );
        assert_eq!(
            fx.storage
                .read()
                .unwrap()
                .meta_store()
                .pending_effects()
                .unwrap()[0]
                .receipt,
            *receipt
        );
        assert!(fx.record(live).is_some());
        // Durable effects remain after an explicitly finished known root.
        // pendingWork=0 here is a ticket count, never a claim of no durable work.
        publisher.finish_known_work(root);
        let no_root = publisher.observe_readiness(deadline()).await.unwrap();
        assert_eq!(no_root.accepted_work, 0);
        assert_eq!(no_root.pending_effects, 1);
        envelope.observe_failure(Ok(no_root));
        let wire = serde_json::to_value(&envelope).unwrap();
        assert_eq!(wire["readiness"]["pendingWork"], 0);
        assert_eq!(wire["readiness"]["pendingWorkScope"], "acceptedTickets");
        assert_eq!(wire["readiness"]["observedWork"]["pendingEffects"], 1);
        assert_eq!(wire["readiness"]["ready"], false);
        envelope.observe_failure(publisher.observe_readiness(Instant::now()).await);
        assert!(envelope.readiness.is_none());
        assert_eq!(envelope.warnings, ["READINESS_UNKNOWN"]);
        assert_eq!(envelope.receipt.as_ref(), Some(receipt.as_ref()));
        let root = publisher.accept_work(None).unwrap();
        *fx.vector_index.write().await = FlatVectorIndex::new(DIM);
        publisher.complete_with_work(Some(&root)).await.unwrap();
        let repaired = publisher.observe_readiness(deadline()).await.unwrap();
        assert_eq!(repaired.canonical, observation.canonical);
        assert!(
            repaired
                .derived
                .iter()
                .all(|v| v.applied == Some(repaired.canonical) && v.matches_canonical)
        );
        assert_eq!(repaired.pending_effects, 0);
        assert_eq!(repaired.accepted_work, 1);
        assert!(
            !crate::serialization::contract::Readiness::observed(repaired.clone())
                .unwrap()
                .ready
        );
        publisher.finish_known_work(root);
        assert_eq!(
            publisher
                .observe_readiness(deadline())
                .await
                .unwrap()
                .accepted_work,
            0
        );
        let barrier = publisher
            .readiness_barrier(repaired.canonical, deadline())
            .await
            .unwrap();
        let wire =
            serde_json::to_value(crate::serialization::contract::Readiness::from(barrier)).unwrap();
        assert_eq!(wire["ready"], true);
        assert_eq!(wire["pendingWork"], 0);
        assert!(wire.get("pendingWorkScope").is_none());
        assert!(wire.get("observedWork").is_none());
        assert!(
            wire["derived"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v["appliedGeneration"].is_string())
        );
    }

    #[tokio::test]
    async fn unknown_handle_and_expired_deadline_never_invent_a_readiness_object() {
        let fx = Fixture::new();
        let publisher = owner(&fx);
        assert!(publisher.observe_readiness(Instant::now()).await.is_err());
        fx.storage.read().unwrap().meta_store().mark_unavailable();
        assert!(publisher.observe_readiness(deadline()).await.is_err());
    }
}
