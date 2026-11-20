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
                match engine
                    .meta_store()
                    .capture_native_decay_pass(ns.id, &inputs, now)
                {
                    Ok(found) => intents.extend(found),
                    // One namespace's policy must not stop the pass for the others.
                    Err(StorageError::InvalidMutation(reason)) => tracing::warn!(
                        namespace = %ns.name,
                        %reason,
                        "decay sweep skipped namespace: its decay policy cannot be resolved"
                    ),
                    Err(error) => return Err(error),
                }
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
        // Startup and timer passes publish after every `write_batch_size`
        // commits and after their last one, rather than after every commit:
        // each publication rebuilds every record and edge. The batch also
        // bounds the effects awaiting publication, which every commit reads.
        // The first commit is published on its own, so a pass whose derived
        // views cannot be published still stops after one write. Bounded
        // passes report a receipt per commit and keep publishing each one
        // before recording it.
        let defer_publication = admission.is_none();
        let mut deferred = Deferred::default();
        for (index, intent) in intents.into_iter().enumerate() {
            if deferred.count > 0
                && (!deferred.published || deferred.count >= config.write_batch_size)
                && !deferred.publish(completion, work, &mut result).await
            {
                break;
            }
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
            let applied = if defer_publication {
                completion
                    .apply_native_decay_in_pass(ctx, intent, policy.clone(), work)
                    .await
            } else {
                completion
                    .apply_native_decay_accepted_bounded(
                        ctx,
                        intent,
                        policy.clone(),
                        work,
                        admission,
                    )
                    .await
            };
            match applied {
                Ok(NativeDecayResult::Committed(receipt)) => {
                    if defer_publication {
                        deferred.add(id, phase, &receipt);
                    }
                    result.native_commit(&receipt)
                }
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
        // Publish the remaining commits before reporting or compacting.
        if deferred.count > 0 {
            deferred.publish(completion, work, &mut result).await;
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

/// Commits of a native startup or timer pass that await derived publication.
#[derive(Default)]
struct Deferred {
    /// The first of them. A failed publication is reported against it, as a
    /// per-commit publication failure would have been.
    first: Option<(MemoryId, DecayPhase, WriteReceipt)>,
    count: usize,
    published: bool,
}

impl Deferred {
    fn add(&mut self, id: MemoryId, phase: DecayPhase, receipt: &WriteReceipt) {
        self.first
            .get_or_insert_with(|| (id, phase, receipt.clone()));
        self.count += 1;
    }

    /// Publish every deferred commit. On failure, records it against the
    /// first of them and returns false: the pass makes no further writes.
    async fn publish(
        &mut self,
        completion: &crate::derived_completion::DerivedCompletion,
        work: &WorkTicket,
        result: &mut SweepResult,
    ) -> bool {
        self.published = true;
        self.count = 0;
        let first = self.first.take();
        let Err(source) = completion.complete_with_admission(Some(work), None).await else {
            return true;
        };
        let error = match first {
            Some((memory_id, phase, receipt)) => {
                let error = StorageError::CommittedMutation {
                    receipt: Box::new(receipt),
                    source: Box::new(source),
                };
                result.errors.push(SweepRecordError {
                    memory_id,
                    phase,
                    error: error.to_string(),
                });
                error
            }
            None => source,
        };
        result.retain_publication_failure(&error);
        false
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
