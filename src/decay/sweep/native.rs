//! Native execution for the existing accepted startup/timer/manual sweep runner.
use super::*;
use crate::storage::StorageError;
use crate::storage::control::{MutationContext, MutationOutcome, MutationScope, WriteReceipt};
use crate::storage::native_lifecycle::NativePolicyInputs;
use crate::storage::native_maintenance::{NativeDecayResult, NativeDecaySkip};

impl DecaySweepRunner {
    pub(super) async fn execute_native_sweep_accepted(
        config: &SweepConfig,
        decay_config: &DecayConfig,
        storage: &Arc<std::sync::RwLock<RedbStorageEngine>>,
        multiplier: f64,
        as_of_millis: Option<i64>,
        work: &WorkTicket,
        completion: Option<&crate::derived_completion::DerivedCompletion>,
        original_contract_version: u32,
        admission: Option<&ReceiptAdmission>,
    ) -> SweepResult {
        let start = Instant::now();
        let mut result = SweepResult::default();
        let Some(completion) = completion else {
            result.publication_error =
                Some("native sweep requires derived completion handles".into());
            return result;
        };
        if !Arc::ptr_eq(&completion.storage, storage) {
            result.publication_error =
                Some("native sweep completion belongs to another owner".into());
            return result;
        }
        let now = as_of_millis
            .unwrap_or_else(|| crate::time::Clock::now_millis(&crate::time::SystemClock));
        let policy = NativePolicyInputs {
            global: decay_config.clone(),
            multiplier,
        };
        let engine = storage.clone();
        let inputs = policy.clone();
        // Capture every ID before applying any transition, so a Full row cannot
        // reappear in the Ghost phase later in this pass. Each intent reports its
        // actual namespace snapshot generation; the clock is common to the pass.
        let captured = tokio::task::spawn_blocking(move || {
            let engine = engine
                .read()
                .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
            #[cfg(test)]
            if engine
                .meta_store()
                .sweep_capture_fault
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                panic!("injected read-only sweep capture loss");
            }
            let mut intents = Vec::new();
            for ns in engine.list_namespaces()? {
                intents.extend(
                    engine
                        .meta_store()
                        .capture_native_decay_pass(ns.id, &inputs, now)?,
                );
            }
            Ok::<_, StorageError>(intents)
        })
        .await;
        let intents = match captured {
            Ok(Ok(intents)) => intents,
            Ok(Err(error)) => {
                if let Some(admission) = admission {
                    admission.fail(&error);
                }
                result.retain_publication_failure(&error);
                return result;
            }
            Err(error) => {
                if let Some(admission) = admission {
                    // This worker only captured read-only intents; no mutation
                    // was dispatched and no canonical outcome is ambiguous.
                    admission.fail(&StorageError::RecoveryRequired(error.to_string()));
                }
                result.publication_error = Some(error.to_string());
                return result;
            }
        };
        for (index, intent) in intents.into_iter().enumerate() {
            if admission.is_some_and(ReceiptAdmission::stopped) {
                break;
            }
            result.memories_scanned += 1;
            let id = intent.memory_id;
            let phase = intent.captured_phase;
            // Each producer supplies its explicit origin; attempts are unkeyed.
            // Skips do not bind retry identity and must never be promoted to
            // keyed no-op success.
            let ctx = MutationContext {
                store_id: Some(intent.capture_generation().store_id),
                scope: MutationScope::Namespace(intent.scope.namespace_id),
                idempotency_key: None,
                expected_generation: None,
                contract_version: original_contract_version,
            };
            match completion
                .apply_native_decay_accepted_bounded(ctx, intent, policy.clone(), work, admission)
                .await
            {
                Ok(NativeDecayResult::Committed(receipt)) => result.native_commit(&receipt),
                Ok(NativeDecayResult::Skipped(reason)) => match reason {
                    NativeDecaySkip::Unchanged => result.unchanged_skipped += 1,
                    NativeDecaySkip::Stale => result.stale_skipped += 1,
                    NativeDecaySkip::Exempt => result.exempt_skipped += 1,
                    NativeDecaySkip::MissingSummary => result.missing_summary_skipped += 1,
                },
                Err(error) => {
                    if let Some(admission) = admission {
                        admission.fail(&error);
                    }
                    if let StorageError::CommittedMutation { receipt, .. } = &error {
                        result.native_commit(receipt);
                    }
                    result.retain_publication_failure(&error);
                    result.errors.push(SweepRecordError {
                        memory_id: id,
                        phase,
                        error: error.to_string(),
                    });
                    // No fresh unkeyed writes after a partial/unknown failure.
                    // Existing completion/reopen is responsible for recovery.
                    break;
                }
            }
            if (index + 1) % config.yield_every_n == 0 {
                tokio::task::yield_now().await;
            }
        }
        // Physical relocation is a separate internal-contract1 commit after
        // successful lifecycle work. Never consume a fresh ID/generation after
        // a partial/unknown phase failure; its in-hand identity stays primary.
        if result.errors.is_empty()
            && result.publication_error.is_none()
            && (result.deletions > 0 || result.full_to_summary > 0)
        {
            match completion
                .compact_text_with_work_bounded(Some(work), admission)
                .await
            {
                Ok(compaction) => {
                    result.compaction_triggered = true;
                    result.compaction_receipt = Some(compaction.receipt);
                    result.compaction_cleanup_pending = compaction.cleanup_pending;
                }
                Err(error) => {
                    if let Some(admission) = admission {
                        admission.fail(&error);
                    }
                    result.retain_publication_failure(&error);
                }
            }
        }
        // Phase-index publication is owned by the canonical writer/completion;
        // there is no independent native phase-index persistence transaction.
        result.duration = start.elapsed();
        result
    }
}

impl SweepResult {
    /// Counts/events describe known canonical commits, even if publication fails.
    fn native_commit(&mut self, receipt: &WriteReceipt) {
        match &receipt.outcome {
            MutationOutcome::NativePhaseMaterialized { transition: t } => {
                let before = &t
                    .change
                    .before
                    .as_ref()
                    .expect("validated native phase")
                    .lifecycle;
                let after = &t.change.after.lifecycle;
                let event = match (before.phase, after.phase) {
                    (DecayPhase::Full, DecayPhase::Summary) => {
                        self.full_to_summary += 1;
                        Some(PhaseTransition::Summarized {
                            id: t.memory_id,
                            old_phase: before.phase,
                            new_phase: after.phase,
                            retrievability: after.decay_strength.get(),
                        })
                    }
                    (DecayPhase::Summary, DecayPhase::Ghost) => {
                        self.summary_to_ghost += 1;
                        Some(PhaseTransition::Ghosted {
                            id: t.memory_id,
                            old_phase: before.phase,
                            new_phase: after.phase,
                            retrievability: after.decay_strength.get(),
                        })
                    }
                    _ => None,
                };
                if let Some(event) = event {
                    self.transitions.push(event);
                }
                let threshold = match before.phase {
                    DecayPhase::Full => Some(t.policy.thresholds[0].get()),
                    DecayPhase::Summary => Some(t.policy.thresholds[1].get()),
                    _ => None,
                };
                if threshold
                    .is_some_and(|at| after.strength.get() <= at && after.decay_strength.get() > at)
                {
                    self.saved_by_connection_bonus += 1;
                }
            }
            MutationOutcome::NativeMemoryHardDeleted { transition: t } => {
                self.deletions += 1;
                let before = &t.deletion.before.lifecycle;
                let projection = crate::decay::lifecycle::project(
                    before.stability.get(),
                    before.lifecycle.decay_anchor_at,
                    &t.policy.policy(),
                    t.sampled_at.max(before.lifecycle.lifecycle_updated_at),
                    t.policy.bonus.get(),
                )
                .expect("validated native deletion projection");
                self.transitions.push(PhaseTransition::Deleted {
                    id: t.memory_id,
                    old_phase: before.phase,
                    retrievability: projection.decay_strength,
                });
            }
            _ => unreachable!("native decay returned another operation"),
        }
    }
}
