//! A consumer of the existing sweep planner, work tracker and completion owner.
use super::*;
use crate::decay::sweep::{DecaySweepRunner, PhaseTransition, SweepConfig, SweepResult};
use crate::storage::work::{accept_work, finish_work};
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct SweepState {
    pub(super) invalidated: bool,
    report: Option<Value>,
}

fn failure(code: &str, message: impl ToString, report: Option<&Value>) -> Value {
    json!({"code":code,"message":message.to_string(),"retryable":false,"sweep":report})
}

impl OwnedEvaluator {
    pub(crate) async fn sweep(&self, request_id: u64, deadline: Instant) -> Result<Value, Value> {
        if self.native_prefix && !self.native_sweep {
            return Err(failure(
                "UNSUPPORTED_CONTROL",
                "native prefix sweep is unavailable",
                None,
            ));
        }
        self.controls
            .sweep_for_schema(
                request_id,
                self.core.config().decay.decay_rate_multiplier,
                deadline,
                if self.native_sweep { 2 } else { 1 },
            )
            .await
    }
    pub(crate) fn sweep_status(&self) -> Value {
        self.controls.sweep_status()
    }
}

impl FrozenControls {
    fn sweep_status(&self) -> Value {
        let state = self.sweep_state.lock().unwrap_or_else(|e| e.into_inner());
        json!({"episodeInvalidated":state.invalidated,"lastSweep":state.report})
    }

    #[cfg(test)]
    async fn sweep(
        &self,
        request_id: u64,
        multiplier: f64,
        deadline: Instant,
    ) -> Result<Value, Value> {
        self.sweep_for_schema(request_id, multiplier, deadline, 1)
            .await
    }

    async fn sweep_for_schema(
        &self,
        request_id: u64,
        multiplier: f64,
        deadline: Instant,
        required_schema: u32,
    ) -> Result<Value, Value> {
        let permit = self.busy.clone().try_lock_owned().map_err(|_| {
            failure(
                "CONTROL_UNAVAILABLE",
                "prior control worker has not joined",
                None,
            )
        })?;
        {
            let state = self.sweep_state.lock().unwrap_or_else(|e| e.into_inner());
            if state.invalidated {
                return Err(failure(
                    "SWEEP_EPISODE_INVALID",
                    "failed sweep requires a fresh episode owner",
                    state.report.as_ref(),
                ));
            }
        }
        if Instant::now() >= deadline {
            return Err(failure(
                "SWEEP_REJECTED",
                "deadline expired before admission",
                None,
            ));
        }
        let owner = self.owner.clone();
        {
            let engine = owner
                .storage
                .read()
                .map_err(|e| failure("SWEEP_REJECTED", e, None))?;
            if engine
                .control_state()
                .map_err(|e| failure("SWEEP_REJECTED", e, None))?
                .control_schema_version
                != required_schema
            {
                return Err(failure(
                    "UNSUPPORTED_CONTROL",
                    "sweep track canonical schema mismatch",
                    None,
                ));
            }
        }
        let work =
            accept_work(&owner.storage, None).map_err(|e| failure("SWEEP_REJECTED", e, None))?;
        // Accepted work and this controller permit protect this one clock sample.
        let now = self.clock.now_millis();
        let initial = json!({"controlRequestId":request_id,"sampledAtMillis":now,"phase":"running",
            "executionCompleted":false,"readiness":null,"result":null});
        self.sweep_state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .report = Some(initial.clone());
        let state = self.sweep_state.clone();
        let coordinator = tokio::spawn(async move {
            let _permit = permit;
            // Catch worker panic without releasing the controller permit early.
            // A panic leaves registered unfinished work unresolved, never ready.
            let worker = tokio::spawn(async move {
                let result = DecaySweepRunner::execute_sweep_with_origin_work(
                    &SweepConfig::default(),
                    &crate::decay::DecayConfig::default(),
                    &crate::graph::ActivationConfig::default(),
                    &owner.storage,
                    &owner.graph,
                    &owner.cache,
                    multiplier,
                    Some(now),
                    Some(&work),
                    Some(&owner),
                    1, // Explicit manual application1; schema does not choose origin.
                )
                .await;
                if result.uncertain_operation.is_none() {
                    finish_work(&owner.storage, work);
                }
                // Full known receipt and uncertain operation remain in this
                // report even if any subsequent canonical/readiness read fails.
                let failed = !result.errors.is_empty()
                    || result.publication_error.is_some()
                    || result.failed_commit.is_some()
                    || result.uncertain_operation.is_some();
                let mut report = initial;
                report["result"] = describe(&result);
                if required_schema == 2 {
                    report["result"]["missingSummarySkipped"] =
                        json!(result.missing_summary_skipped);
                }
                report["phase"] = json!("failed");
                if failed {
                    return Err(failure(
                        "SWEEP_FAILED",
                        "sweep or publication did not complete",
                        Some(&report),
                    ));
                }
                let target = owner
                    .storage
                    .read()
                    .map_err(|e| failure("SWEEP_FAILED", e, Some(&report)))?
                    .control_state()
                    .map_err(|e| failure("SWEEP_FAILED", e, Some(&report)))?
                    .generation_ref();
                // The underlying real barrier, without an inner timeout that
                // could abandon a blocking capture before the permit is freed.
                let ready = owner
                    .with_frozen_core(Some(target), deadline, |_, _, ready| Ok(ready))
                    .await
                    .map_err(|e| failure("SWEEP_FAILED", e, Some(&report)))?;
                if Instant::now() > deadline {
                    return Err(failure(
                        "SWEEP_FAILED",
                        "sweep finished after deadline",
                        Some(&report),
                    ));
                }
                report["readiness"] =
                    serde_json::to_value(ready).expect("readiness is serializable");
                report["phase"] = json!("completed");
                report["executionCompleted"] = json!(true);
                Ok(report)
            });
            let outcome = worker.await.unwrap_or_else(|e| {
                Err(failure(
                    "SWEEP_FAILED",
                    format!("worker outcome unknown: {e}"),
                    None,
                ))
            });
            let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
            match &outcome {
                Ok(report) => state.report = Some(report.clone()),
                Err(error) => {
                    state.invalidated = true;
                    let mut report = error
                        .get("sweep")
                        .filter(|v| !v.is_null())
                        .cloned()
                        .unwrap_or_else(|| state.report.clone().unwrap());
                    report["phase"] = json!("failed");
                    report["error"] =
                        json!({"code":error["code"],"message":error["message"],"retryable":false});
                    state.report = Some(report);
                }
            }
            outcome
        });
        match tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), coordinator).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(error)) => {
                let mut state = self.sweep_state.lock().unwrap_or_else(|e| e.into_inner());
                state.invalidated = true;
                Err(failure(
                    "SWEEP_FAILED",
                    format!("coordinator outcome unknown: {error}"),
                    state.report.as_ref(),
                ))
            }
            Err(_) => {
                let mut state = self.sweep_state.lock().unwrap_or_else(|e| e.into_inner());
                state.invalidated = true;
                Err(failure(
                    "SWEEP_TIMEOUT",
                    "episode invalid; worker may still be joining; inspect sweepStatus",
                    state.report.as_ref(),
                ))
            }
        }
    }
}

fn describe(result: &SweepResult) -> Value {
    let transitions: Vec<Value> = result.transitions.iter().map(|transition| match transition {
        PhaseTransition::Summarized { id, old_phase, new_phase, retrievability }
        | PhaseTransition::Ghosted { id, old_phase, new_phase, retrievability } =>
            json!({"id":id,"fromPhase":old_phase,"toPhase":new_phase,"retrievability":retrievability}),
        PhaseTransition::Deleted { id, old_phase, retrievability } =>
            json!({"id":id,"fromPhase":old_phase,"toPhase":"deleted","retrievability":retrievability}),
    }).collect();
    let errors: Vec<Value> = result
        .errors
        .iter()
        .map(|error| json!({"id":error.memory_id,"phase":error.phase,"message":error.error}))
        .collect();
    json!({"memoriesScanned":result.memories_scanned,"permastoreSkipped":result.permastore_skipped,
        "decayDisabledSkipped":result.decay_disabled_skipped,"fullToSummary":result.full_to_summary,
        "summaryToGhost":result.summary_to_ghost,"deletions":result.deletions,
        "savedByConnectionBonus":result.saved_by_connection_bonus,"compactionTriggered":result.compaction_triggered,
        "staleSkipped":result.stale_skipped,"unchangedSkipped":result.unchanged_skipped,"exemptSkipped":result.exempt_skipped,
        "publicationError":result.publication_error,"knownCommittedReceipt":result.failed_commit.as_ref().map(|v| &v.0),
        "knownCommitError":result.failed_commit.as_ref().map(|v| &v.1),"uncertainOperation":result.uncertain_operation,
        "transitions":transitions,"errors":errors})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DecayPhase, NamespaceId};
    use crate::storage::StorageEngine;
    use crate::test_support::{Fixture, MemorySpec};
    use std::time::Duration;

    async fn fixture() -> (Fixture, Arc<FrozenControls>, crate::model::MemoryId) {
        let f = Fixture::new();
        let id = f
            .insert_memory_with(MemorySpec::new(NamespaceId::new(1)))
            .await;
        let owner = DerivedCompletion {
            storage: f.storage.clone(),
            cache: f.cache.clone(),
            graph: f.graph.clone(),
            vectors: f.vector_index.clone(),
            fts: f.fts_index.clone(),
            entities: f.entity_index.clone(),
        };
        owner.complete().await.unwrap();
        // Fixture records are timestamp zero. Deliberately well beyond all
        // thresholds; reverse phase ordering must still allow only one step.
        let controls = FrozenControls::new(owner, Arc::new(ManualClock::new(36_500 * 86_400_000)));
        (f, Arc::new(controls), id)
    }
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_sweep_track_rejects_legacy_owner_before_work_or_write() {
        let (f, controls, _) = fixture().await;
        let before = f.storage.read().unwrap().control_state().unwrap();
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let status = tracker.status();
        let error = controls
            .sweep_for_schema(1, 1.0, deadline(), 2)
            .await
            .unwrap_err();
        assert_eq!(error["code"], "UNSUPPORTED_CONTROL");
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
        assert_eq!(tracker.status().pending, status.pending);
        assert_eq!(tracker.status().admission_open, status.admission_open);
        assert_eq!(controls.sweep_status()["lastSweep"], Value::Null);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn owned_sweep_uses_real_phases_single_time_and_four_view_barrier() {
        let (f, c, id) = fixture().await;
        let before = f
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
        let noop = c.sweep(1, 0.0, deadline()).await.unwrap();
        assert_eq!(noop["result"]["decayDisabledSkipped"], 1);
        assert_eq!(
            noop["readiness"]["committedGeneration"],
            before.generation.get().to_string()
        );
        for (request, phase, count) in [
            (2, DecayPhase::Summary, "fullToSummary"),
            (3, DecayPhase::Ghost, "summaryToGhost"),
        ] {
            let report = c.sweep(request, 1.0, deadline()).await.unwrap();
            assert_eq!(report["sampledAtMillis"], 36_500_i64 * 86_400_000);
            assert_eq!(report["result"][count], 1);
            assert!(report["readiness"]["ready"].as_bool().unwrap());
            assert_eq!(report["readiness"]["pendingWork"], 0);
            assert!(
                report["readiness"]["derived"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|v| v["ready"] == true)
            );
            assert_eq!(
                f.storage
                    .read()
                    .unwrap()
                    .get_record(id)
                    .unwrap()
                    .unwrap()
                    .phase,
                phase
            );
            assert_eq!(c.sweep_status()["lastSweep"], report);
        }
        let report = c.sweep(4, 1.0, deadline()).await.unwrap();
        assert_eq!(report["result"]["deletions"], 1);
        assert!(f.storage.read().unwrap().get_record(id).unwrap().is_none());
        assert!(!c.sweep_status()["episodeInvalidated"].as_bool().unwrap());
        c.advance(1, deadline()).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn owned_sweep_timeout_keeps_clock_and_capture_blocked_and_late_status_readable() {
        let (f, c, _) = fixture().await;
        let graph = f.graph.write().await;
        let task = {
            let c = c.clone();
            tokio::spawn(async move {
                c.sweep(11, 1.0, Instant::now() + Duration::from_millis(150))
                    .await
            })
        };
        let error = task.await.unwrap().unwrap_err();
        assert_eq!(error["code"], "SWEEP_TIMEOUT");
        assert_eq!(c.sweep_status()["lastSweep"]["controlRequestId"], 11);
        assert!(c.busy.try_lock().is_err());
        assert!(c.advance(100, deadline()).await.is_err());
        assert!(c.capture(deadline(), |_, _, _| Ok(())).await.is_err());
        assert!(c.sweep(12, 1.0, deadline()).await.is_err());
        assert_eq!(c.clock.now_millis(), 36_500_i64 * 86_400_000);
        drop(graph);
        let joined = tokio::time::timeout(Duration::from_secs(10), c.busy.lock())
            .await
            .unwrap();
        drop(joined);
        assert_eq!(c.sweep_status()["lastSweep"]["phase"], "failed");
        assert!(c.sweep_status()["lastSweep"]["result"].is_object());
        // Joining a late mutation does not authorize capture/reset of a failed
        // episode, or silently make its actions eligible for scoring.
        assert!(c.capture(deadline(), |_, _, _| Ok(())).await.is_err());
        assert!(c.advance(100, deadline()).await.is_err());
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .status()
                .pending,
            0
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn owned_sweep_caller_cancellation_retains_worker_then_publishes_status() {
        let (f, c, _) = fixture().await;
        let graph = f.graph.write().await;
        let task = {
            let c = c.clone();
            tokio::spawn(async move { c.sweep(21, 1.0, deadline()).await })
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            while c.sweep_status()["lastSweep"].is_null() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(c.busy.try_lock().is_err());
        assert!(c.advance(1, deadline()).await.is_err());
        drop(graph);
        let joined = tokio::time::timeout(Duration::from_secs(10), c.busy.lock())
            .await
            .unwrap();
        drop(joined);
        assert_eq!(c.sweep_status()["lastSweep"]["phase"], "completed");
        assert_eq!(c.sweep_status()["lastSweep"]["readiness"]["ready"], true);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .status()
                .pending,
            0
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn owned_sweep_known_commit_receipt_survives_real_effect_failure() {
        let (f, c, id) = fixture().await;
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .fail_effect_application
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let error = c.sweep(31, 1.0, deadline()).await.unwrap_err();
        let receipt = &error["sweep"]["result"]["knownCommittedReceipt"];
        assert!(receipt.is_object(), "{error}");
        assert_eq!(receipt["outcome"]["memoryId"], json!(id));
        assert_eq!(
            c.sweep_status()["lastSweep"]["result"]["knownCommittedReceipt"],
            *receipt
        );
        assert_eq!(error["retryable"], false);
        assert_eq!(error["sweep"]["readiness"], Value::Null);
        assert!(c.sweep(32, 1.0, deadline()).await.is_err());
        assert!(c.capture(deadline(), |_, _, _| Ok(())).await.is_err());
    }
}
