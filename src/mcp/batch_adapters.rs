//! Actual ordered batch consumer. A transport outcome is never used as a
//! second journal: successful child outcomes live in their canonical commit.
use super::{McpStorageAdapter, StoreWriteError, bridge};
use crate::mcp::batches::{self, PreparedItem};
use crate::performance::{self, Hold, Timeline, Timer, Worker};
use crate::storage::StorageError;
use crate::storage::batch::*;
use crate::storage::control::{MutationOutcome, WriteReceipt};
use serde_json::json;

impl McpStorageAdapter {
    /// The runner has finished its known root before this point. Unknown roots
    /// remain tracked; failure observation never tries to drain them.
    pub(super) async fn native_batch_readiness(
        &self,
        result: Result<BatchOutcome, crate::serialization::native_batch::BatchFailure>,
    ) -> Result<
        crate::serialization::native_batch::NativeBatchOutcome,
        crate::serialization::native_batch::BatchFailure,
    > {
        use crate::serialization::{contract::Readiness, native_batch::NativeBatchOutcome};
        let completion = self.batch_completion();
        let deadline = || std::time::Instant::now() + std::time::Duration::from_secs(30);
        match result {
            Err(mut failure) => {
                // Historical oversize reservation evidence is not a committed
                // item receipt or a readiness claim. Do no completion for it.
                if failure.receipt.is_some() {
                    match completion
                        .observe_readiness(deadline())
                        .await
                        .and_then(Readiness::observed)
                    {
                        Ok(observed) => failure.readiness = Some(observed),
                        Err(_) => failure.warnings.push("READINESS_UNKNOWN".into()),
                    }
                }
                Err(failure)
            }
            Ok(mut outcome) => {
                if outcome.all_ready() {
                    #[cfg(test)]
                    match self
                        .fail_native_batch_after_children
                        .swap(0, std::sync::atomic::Ordering::SeqCst)
                    {
                        1 => self
                            .storage
                            .read()
                            .unwrap()
                            .meta_store()
                            .invalidate_derived_view(crate::storage::publication::DerivedView::Fts),
                        2 => self.storage.read().unwrap().meta_store().mark_unavailable(),
                        _ => {}
                    }
                    // Use actual current lineage/G, including a restored fork;
                    // retained historical receipt G is not current readiness.
                    let target = self
                        .storage
                        .read()
                        .map_err(|e| StorageError::RecoveryRequired(e.to_string()))
                        .and_then(|s| s.control_state().map(|c| c.generation_ref()));
                    if let Ok(target) = target {
                        if let Ok(ready) = completion.readiness_barrier(target, deadline()).await {
                            return Ok(NativeBatchOutcome {
                                outcome,
                                readiness: Some(ready.into()),
                                warnings: vec![],
                            });
                        }
                    }
                    mark_unready(&mut outcome);
                }
                let mut reply = NativeBatchOutcome::from(outcome);
                match completion
                    .observe_readiness(deadline())
                    .await
                    .and_then(Readiness::observed)
                {
                    Ok(observed) => reply.readiness = Some(observed),
                    Err(_) => reply.warnings.push("READINESS_UNKNOWN".into()),
                }
                Ok(reply)
            }
        }
    }

    pub(super) fn batch_completion(&self) -> crate::derived_completion::DerivedCompletion {
        crate::derived_completion::DerivedCompletion {
            storage: self.storage.clone(),
            cache: self.cache.clone(),
            graph: self.graph.clone(),
            vectors: self.vector_index.clone(),
            fts: self.fts_index.clone(),
            entities: self.entity_index.clone(),
        }
    }

    pub(crate) async fn run_batch(
        &self,
        input: BatchInput,
    ) -> Result<BatchOutcome, bridge::BridgeError> {
        self.run_batch_with_surface(input, None, None)
            .await
            .map_err(StoreWriteError::into_bridge)
    }

    pub(crate) fn run_batch_typed(
        &self,
        input: BatchInput,
    ) -> impl std::future::Future<Output = Result<BatchOutcome, StoreWriteError>> + '_ {
        self.run_batch_with_surface(input, None, None)
    }

    pub(crate) fn run_batch_budgeted(
        &self,
        input: BatchInput,
        budget: Option<crate::mcp::batch_budget::BatchBudget>,
    ) -> impl std::future::Future<Output = Result<BatchOutcome, StoreWriteError>> + '_ {
        self.run_batch_with_surface(input, budget, None)
    }

    pub(crate) fn run_batch_with_surface(
        &self,
        input: BatchInput,
        budget: Option<crate::mcp::batch_budget::BatchBudget>,
        surface: Option<crate::serialization::batch_api1::Surface>,
    ) -> impl std::future::Future<Output = Result<BatchOutcome, StoreWriteError>> + '_ {
        let legacy_trace = input.mutation_version == 1;
        async move {
            // Construct the large body in its pinned slot instead of capturing
            // an already-constructed copy in the outer future.
            let mut run = std::pin::pin!(async move {
                performance::event!(legacy_trace;
                    kind = "batch_input",
                    operation = match input.kind {
                        BatchKind::Insert => "insert",
                        BatchKind::Forget => "forget",
                    },
                    supplied_items = input.items.len()
                );
                let completion = self.batch_completion();
                let work = completion.accept_work(None)?;
                let result = self.run_batch_accepted(input, &work, budget, surface).await;
                completion.finish_known_work(work);
                match &result {
                    Ok(outcome) => performance::event!(legacy_trace;
                        kind = "batch_outcome",
                        total = outcome.total,
                        succeeded = outcome.succeeded,
                        failed = outcome.failed,
                        unresolved = outcome.unresolved,
                        ready_items = outcome.items.iter().filter(|i| i.ready).count(),
                        retained_child_receipts =
                            outcome.items.iter().filter(|i| i.receipt.is_some()).count(),
                        retained_bookkeeping_receipts = outcome.bookkeeping_receipts.len(),
                        unknown_bookkeeping_operations = outcome.unknown_bookkeeping_operations.len()
                    ),
                    Err(_) => {
                        performance::event!(legacy_trace; kind = "batch_error", state = "returned_error")
                    }
                }
                result
            });
            if legacy_trace {
                performance::measure("mcp.batch", true, run.as_mut()).await
            } else {
                run.await
            }
        }
    }

    async fn run_batch_accepted(
        &self,
        input: BatchInput,
        work: &crate::storage::work::WorkTicket,
        budget: Option<crate::mcp::batch_budget::BatchBudget>,
        surface: Option<crate::serialization::batch_api1::Surface>,
    ) -> Result<BatchOutcome, StoreWriteError> {
        let legacy_trace = input.mutation_version == 1;
        let mut timeline = Timeline::new_if(legacy_trace);
        // Prepare the entire ordered caller intent before policy/default reads.
        let mut prepared = if let Some(surface) = surface {
            batches::prepare_api1_store(&input, surface)?
        } else if input.mutation_version == 3 {
            batches::prepare_native_store(&input)?
        } else {
            batches::prepare(&input)?
        };
        timeline.mark("batch.prepare");
        let budget_plans: Vec<_> = prepared.iter().map(|item| item.plan.clone()).collect();
        if budget.is_some() && surface.is_none() {
            for (index, (item, value)) in prepared.iter_mut().zip(&input.items).enumerate() {
                if let Some(error) = crate::serialization::native_batch::item_error(value, index) {
                    item.plan = BatchItemPlan::Invalid {
                        request_hash: item.plan.request_hash(),
                        error,
                    };
                }
            }
        }
        let plans: Vec<_> = prepared.iter().map(|item| item.plan.clone()).collect();
        let ctx = input.context();
        let kind = input.kind;
        let parent = input.batch_operation_id;
        let version = input.mutation_version;
        let legacy_semantic_write = version == 4
            && prepared
                .iter()
                .any(|item| matches!(item.plan, BatchItemPlan::Insert { .. }));
        let config = self.config.clone();
        let storage = self.storage.clone();
        let worker = work.child().map_err(StorageError::from)?;
        let trace_worker = Worker::new_if("batch.reservation.queue", legacy_trace);
        let reserved = tokio::task::spawn_blocking(move || {
            trace_worker.run(move || {
                let wait = Timer::new_if("batch.reservation.lock.wait", legacy_trace);
                let storage = storage
                    .read()
                    .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                wait.complete();
                let storage = Hold::new_if("batch.reservation.lock.hold", storage, legacy_trace);
                let result = (|| {
                    let store_id = storage.meta_store().control_state()?.store_id;
                    let timer = Timer::new_if("batch.reservation.call", legacy_trace);
                    let reservation = if version == 3 {
                        storage.meta_store().reserve_native_store_batch_with_work(
                            &ctx, &plans, parent, Some(&worker), || {
                                let settings = crate::graph::native_selection::freeze_settings(&config.graph)?;
                                if let Some(budget) = budget {
                                    budget.fresh(&plans, &settings)?;
                                }
                                Ok(settings)
                            },
                        )
                    } else {
                        storage.meta_store().reserve_batch_version_with_work(
                            &ctx, version, kind, &plans, parent, Some(&worker), || {
                                // Matching replay precedes fresh semantic admission;
                                // rejection cannot reserve parent or child IDs.
                                if legacy_semantic_write {
                                    return Err(StorageError::UnsupportedMutation(
                                        crate::storage::insertion::LEGACY_API2_SEMANTIC_UNSUPPORTED.into(),
                                    ));
                                }
                                Ok(None)
                            },
                        )
                    };
                    timer.outcome(reservation.is_ok());
                    match &reservation {
                        Ok(progress) => performance::event!(legacy_trace; kind = "batch_reservation", state = "committed", replayed = progress.reservation_receipt.replayed),
                        Err(StorageError::CommitOutcomeUnknown { .. }) => performance::event!(legacy_trace; kind = "batch_reservation", state = "unknown"),
                        Err(_) => performance::event!(legacy_trace; kind = "batch_reservation", state = "error"),
                    }
                    Ok::<_, StorageError>((store_id, reservation))
                })();
                storage.meta_store().finish_work(worker);
                result
            })
        })
        .await
        .map_err(|e| bridge::BridgeError::PartiallyApplied(format!("batch reservation worker outcome unknown; resubmit the same full request and key after recovery: {e}")))?
        .map_err(StoreWriteError::Canonical)?;
        timeline.mark("batch.reservation");
        let (store_id, reserved) = reserved;
        let mut progress = match reserved {
            Ok(progress) => progress,
            Err(StorageError::CommitOutcomeUnknown { operation_id, .. }) => {
                // Do not reinterpret unsafe same-handle readback as confirmation.
                return Ok(BatchOutcome {
                    result_version: 1,
                    batch_operation_id: operation_id,
                    store_id,
                    reservation_state: BatchReservationState::Unknown,
                    reservation_receipt: None,
                    bookkeeping_receipts: vec![],
                    unknown_bookkeeping_operations: vec![],
                    items: (0..prepared.len())
                        .map(|index| BatchItemOutcome {
                            index: index as u32,
                            status: BatchStatus::Unknown,
                            operation_id: None,
                            namespace_id: None,
                            receipt: None,
                            error: Some(BatchError::new("COMMIT_OUTCOME_UNKNOWN")),
                            ready: false,
                            retryable: false,
                            result: None,
                        })
                        .collect(),
                    total: prepared.len(),
                    succeeded: 0,
                    failed: 0,
                    unresolved: prepared.len(),
                });
            }
            Err(e) => return Err(e.into()),
        };
        if let Some(budget) = budget {
            budget.retained(&progress, &budget_plans)?;
        }
        let mut outcome = outcome_from_progress(&progress, &prepared, self.timezone)?;
        // A previous attempt may have committed a child but failed publication.
        // Replay never calls a provider or allocates another payload/identity.
        if self
            .batch_completion()
            .complete_with_work(Some(work))
            .await
            .is_err()
        {
            mark_unready(&mut outcome);
            return Ok(outcome);
        }
        timeline.mark("batch.initial_publication");
        for (index, prepared) in prepared.iter().enumerate() {
            if !matches!(progress.items[index].state, BatchItemState::NotAttempted {}) {
                continue;
            }
            let child = progress.child(index).ok_or_else(|| {
                bridge::BridgeError::Internal("pending batch item has no reservation".into())
            })?;
            let item_work = work.child().map_err(StorageError::from)?;
            performance::event!(legacy_trace; kind = "batch_child", phase = "attempt");
            let child_timer = Timer::new_if("batch.child", legacy_trace);
            let result = if input.mutation_version == 3 {
                self.run_linked_batch_child(prepared, &child, &item_work)
                    .await
            } else if input.mutation_version == 2 {
                self.run_native_batch_child(prepared, &child, &item_work)
                    .await
            } else if let Some(input) = &prepared.store {
                let mut input = input.clone();
                input.namespace = progress.items[index]
                    .defaults
                    .as_ref()
                    .unwrap()
                    .namespace_name
                    .clone();
                self.store_memory_reserved_with_work(input, Some(child.clone()), Some(&item_work))
                    .await
                    .map(|(stored, receipt)| {
                        (
                            receipt,
                            serde_json::to_value(stored).expect("stored memory serialization"),
                        )
                    })
            } else if let Some(id) = prepared.forget {
                self.batch_completion()
                    .forget_reserved_with_work(child.clone(), id, Some(&item_work))
                    .await
                    .map(|receipt| {
                        let result = forget_result(id, &receipt);
                        (receipt, result)
                    })
                    .map_err(StoreWriteError::Canonical)
            } else {
                return Err(bridge::BridgeError::Internal(
                    "pending item has no prepared input".into(),
                )
                .into());
            };
            self.batch_completion().finish_known_work(item_work);
            child_timer.outcome(result.is_ok());
            match result {
                Ok((receipt, result)) => {
                    performance::event!(legacy_trace;
                        kind = "batch_child",
                        phase = "return",
                        state = "committed",
                        replayed = receipt.replayed
                    );
                    outcome.items[index].status = BatchStatus::Ok;
                    outcome.items[index].receipt = Some(receipt);
                    outcome.items[index].result = Some(result);
                    outcome.items[index].retryable = false;
                }
                Err(StoreWriteError::Canonical(StorageError::CommittedMutation {
                    receipt,
                    ..
                })) => {
                    performance::event!(legacy_trace;
                        kind = "batch_child",
                        phase = "return",
                        state = "committed_unready",
                        replayed = receipt.replayed
                    );
                    // Keep the actual current child, even if older items also
                    // committed. It is a success with readiness=false.
                    outcome.items[index].status = BatchStatus::Ok;
                    outcome.items[index].result = if let Some(input) = &prepared.store {
                        Some(
                            serde_json::to_value(batches::stored_from_receipt(
                                input,
                                &receipt,
                                self.timezone,
                            )?)
                            .map_err(|e| bridge::BridgeError::Internal(e.to_string()))?,
                        )
                    } else {
                        prepared.forget.map(|id| forget_result(id, &receipt))
                    };
                    outcome.items[index].receipt = Some(*receipt);
                    outcome.items[index].error = Some(BatchError::new("INDEX_NOT_READY"));
                    outcome.items[index].retryable = false;
                    mark_unready(&mut outcome);
                    return Ok(outcome);
                }
                Err(StoreWriteError::Canonical(StorageError::CommitOutcomeUnknown {
                    operation_id,
                    ..
                })) => {
                    performance::event!(legacy_trace; kind = "batch_child", phase = "return", state = "unknown");
                    outcome.items[index].status = BatchStatus::Unknown;
                    outcome.items[index].operation_id = Some(operation_id);
                    outcome.items[index].error = Some(BatchError::new("COMMIT_OUTCOME_UNKNOWN"));
                    outcome.items[index].retryable = false;
                    mark_unready(&mut outcome);
                    return Ok(outcome);
                }
                Err(error) => {
                    if let Some(mut final_error) = final_error(&error) {
                        performance::event!(legacy_trace;
                            kind = "batch_child",
                            phase = "return",
                            state = "rejected"
                        );
                        // A pre-correction pending manifest can now fail input
                        // admission. Keep its indexed field diagnostic too.
                        if let BatchItemPlan::Invalid { error, .. } = &prepared.plan {
                            final_error = error.clone();
                        }
                        let storage = self.storage.clone();
                        let worker = work.child().map_err(StorageError::from)?;
                        let trace_worker = Worker::new_if("batch.rejection.queue", legacy_trace);
                        let finalized = tokio::task::spawn_blocking(move || {
                            trace_worker.run(move || {
                                let wait = Timer::new_if("batch.rejection.lock.wait", legacy_trace);
                                let storage = storage
                                    .read()
                                    .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                                wait.complete();
                                let storage = Hold::new_if(
                                    "batch.rejection.lock.hold",
                                    storage,
                                    legacy_trace,
                                );
                                let timer = Timer::new_if("batch.rejection.call", legacy_trace);
                                let result = storage.meta_store().finalize_batch_error_with_work(
                                    &child,
                                    final_error,
                                    Some(&worker),
                                );
                                timer.outcome(result.is_ok());
                                performance::event!(legacy_trace;
                                    kind = "batch_bookkeeping",
                                    state = match &result {
                                        Ok(_) => "returned_progress",
                                        Err(StorageError::CommitOutcomeUnknown { .. }) => "unknown",
                                        Err(_) => "error",
                                    }
                                );
                                storage.meta_store().finish_work(worker);
                                result
                            })
                        })
                        .await;
                        match finalized {
                            Ok(Ok(next)) => {
                                // A concurrent same-key attempt may already have
                                // committed this child. Its canonical result wins.
                                progress = next;
                                let mut current =
                                    item_outcome(&progress, index, prepared, self.timezone)?;
                                current.ready = false;
                                outcome.items[index] = current;
                                outcome.bookkeeping_receipts =
                                    progress.bookkeeping_receipts.clone();
                            }
                            Ok(Err(StorageError::CommitOutcomeUnknown {
                                operation_id, ..
                            })) => {
                                outcome.unknown_bookkeeping_operations.push(operation_id);
                                outcome.items[index].status = BatchStatus::Unknown;
                                outcome.items[index].error =
                                    Some(BatchError::new("BOOKKEEPING_OUTCOME_UNKNOWN"));
                                outcome.items[index].retryable = false;
                                mark_unready(&mut outcome);
                                return Ok(outcome);
                            }
                            _ => {
                                outcome.items[index].status = BatchStatus::Unknown;
                                outcome.items[index].error =
                                    Some(BatchError::new("RECONCILIATION_REQUIRED"));
                                outcome.items[index].retryable = false;
                                mark_unready(&mut outcome);
                                return Ok(outcome);
                            }
                        }
                    } else {
                        // Provider failure happened before canonical work; keep
                        // its reserved child pending. No provider detail is stored.
                        let unavailable = matches!(
                            error,
                            StoreWriteError::Bridge(bridge::BridgeError::Unavailable(_))
                                | StoreWriteError::Canonical(
                                    StorageError::ProviderVectorUnavailable(_)
                                )
                        );
                        performance::event!(legacy_trace;
                            kind = "batch_child",
                            phase = "return",
                            state = if unavailable {
                                "provider_unavailable"
                            } else {
                                "reconciliation_required"
                            }
                        );
                        outcome.items[index].status = if unavailable {
                            BatchStatus::NotAttempted
                        } else {
                            BatchStatus::Unknown
                        };
                        outcome.items[index].retryable = unavailable;
                        outcome.items[index].error = Some(BatchError::new(if unavailable {
                            "UNAVAILABLE"
                        } else {
                            "RECONCILIATION_REQUIRED"
                        }));
                        if unavailable {
                            outcome.items[index].error.as_mut().unwrap().field = self
                                .native_store_blocker(prepared)
                                .or_else(|| {
                                    (input.mutation_version == 4 && prepared.store.is_some())
                                        .then_some("embeddingProfileId")
                                })
                                .map(str::to_owned);
                        }
                        break;
                    }
                }
            }
        }
        timeline.mark("batch.children");
        if self
            .batch_completion()
            .complete_with_work(Some(work))
            .await
            .is_ok()
            && super::effects_stamp(&self.storage).await.is_ok()
        {
            // Another matching caller may have committed while this caller's
            // advisory/read stamp changed. Reconcile only through an admitted
            // healthy handle, never after an explicit unknown-commit return.
            let storage = self.storage.clone();
            let parent = outcome.batch_operation_id;
            let store_id = outcome.store_id;
            timeline.mark("batch.final_publication");
            let trace_worker = Worker::new_if("batch.progress.queue", legacy_trace);
            let current = tokio::task::spawn_blocking(move || {
                trace_worker.run(move || {
                    let wait = Timer::new_if("batch.progress.lock.wait", legacy_trace);
                    let storage = storage
                        .read()
                        .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                    wait.complete();
                    let storage = Hold::new_if("batch.progress.lock.hold", storage, legacy_trace);
                    let timer = Timer::new_if("batch.progress.call", legacy_trace);
                    let result = storage.meta_store().lookup_batch(store_id, parent);
                    timer.outcome(result.is_ok());
                    performance::event!(legacy_trace;
                        kind = "batch_progress",
                        state = match &result {
                            Ok(Some(_)) => "present",
                            Ok(None) => "missing",
                            Err(_) => "error",
                        }
                    );
                    result
                })
            })
            .await;
            timeline.mark("batch.final_progress");
            match current {
                Ok(Ok(Some(current))) => {
                    for (index, prepared) in prepared.iter().enumerate() {
                        if matches!(
                            outcome.items[index].status,
                            BatchStatus::Unknown | BatchStatus::NotAttempted
                        ) && !matches!(
                            current.items[index].state,
                            BatchItemState::NotAttempted {}
                        ) {
                            outcome.items[index] =
                                item_outcome(&current, index, prepared, self.timezone)?;
                        }
                    }
                    outcome.bookkeeping_receipts = current.bookkeeping_receipts;
                }
                _ => {
                    mark_unready(&mut outcome);
                    return Ok(outcome);
                }
            }
            for item in &mut outcome.items {
                item.ready = matches!(item.status, BatchStatus::Ok | BatchStatus::Error);
            }
        } else {
            mark_unready(&mut outcome);
        }
        outcome.recount();
        self.enrich_linked_batch(&mut outcome);
        Ok(outcome)
    }
}

pub(super) fn mark_unready(outcome: &mut BatchOutcome) {
    for item in &mut outcome.items {
        item.ready = false;
        if item.status == BatchStatus::Ok {
            item.retryable = false;
            item.error = Some(BatchError::new("INDEX_NOT_READY"));
        }
    }
    outcome.recount();
}

fn outcome_from_progress(
    progress: &BatchProgress,
    prepared: &[PreparedItem],
    timezone: chrono_tz::Tz,
) -> Result<BatchOutcome, bridge::BridgeError> {
    let mut outcome = BatchOutcome {
        result_version: 1,
        batch_operation_id: progress.reservation_receipt.operation_id,
        store_id: progress.reservation_receipt.store_id,
        reservation_state: BatchReservationState::Committed,
        reservation_receipt: Some(progress.reservation_receipt.clone()),
        bookkeeping_receipts: progress.bookkeeping_receipts.clone(),
        unknown_bookkeeping_operations: vec![],
        items: prepared
            .iter()
            .enumerate()
            .map(|(index, item)| item_outcome(progress, index, item, timezone))
            .collect::<Result<_, _>>()?,
        total: 0,
        succeeded: 0,
        failed: 0,
        unresolved: 0,
    };
    outcome.recount();
    Ok(outcome)
}

fn item_outcome(
    progress: &BatchProgress,
    index: usize,
    prepared: &PreparedItem,
    timezone: chrono_tz::Tz,
) -> Result<BatchItemOutcome, bridge::BridgeError> {
    let item = &progress.items[index];
    let mut outcome = BatchItemOutcome {
        index: item.index,
        status: BatchStatus::NotAttempted,
        operation_id: Some(item.operation_id),
        namespace_id: item.defaults.as_ref().map(|d| d.namespace_id),
        receipt: None,
        error: None,
        ready: false,
        retryable: true,
        result: None,
    };
    match &item.state {
        BatchItemState::NotAttempted {} => {}
        BatchItemState::Error { error } => {
            outcome.status = BatchStatus::Error;
            outcome.error = Some(error.clone());
            outcome.retryable = false;
        }
        BatchItemState::Ok { receipt } => {
            let mut receipt = receipt.clone();
            receipt.replayed = true;
            outcome.status = BatchStatus::Ok;
            outcome.retryable = false;
            outcome.result = if let Some(input) = &prepared.store {
                let mut input = input.clone();
                input.namespace = item.defaults.as_ref().unwrap().namespace_name.clone();
                Some(
                    serde_json::to_value(batches::stored_from_receipt(&input, &receipt, timezone)?)
                        .map_err(|e| bridge::BridgeError::Internal(e.to_string()))?,
                )
            } else {
                prepared.forget.map(|id| forget_result(id, &receipt))
            };
            outcome.receipt = Some(receipt);
        }
    }
    Ok(outcome)
}

pub(super) fn forget_result(
    id: crate::model::MemoryId,
    receipt: &WriteReceipt,
) -> serde_json::Value {
    let deleted = match &receipt.outcome {
        MutationOutcome::MemoryTombstoned { status, .. } => {
            *status == crate::storage::tombstone::TombstoneStatus::Tombstoned
        }
        MutationOutcome::NativeMemoryTombstoned { transition } => {
            transition.primary().is_ok_and(|p| {
                p.before
                    .as_ref()
                    .is_some_and(|b| b.lifecycle.phase != crate::model::DecayPhase::Tombstone)
            })
        }
        _ => false,
    };
    json!({"id":id.to_string(),"deleted":deleted})
}

fn final_error(error: &StoreWriteError) -> Option<BatchError> {
    let code = match error {
        StoreWriteError::Bridge(bridge::BridgeError::InvalidInput(_))
        | StoreWriteError::Canonical(StorageError::InvalidMutation(_))
        | StoreWriteError::Canonical(StorageError::CallerVectorInvalid(_)) => "INVALID_INPUT",
        StoreWriteError::Bridge(bridge::BridgeError::NotFound(_))
        | StoreWriteError::Canonical(StorageError::NotFound(_)) => "NOT_FOUND",
        StoreWriteError::Canonical(StorageError::NamespaceNotFound(_)) => "NAMESPACE_NOT_FOUND",
        StoreWriteError::Canonical(StorageError::MutationNamespaceMismatch) => "NAMESPACE_MISMATCH",
        StoreWriteError::Canonical(StorageError::EdgeConflict(_)) => "EDGE_CONFLICT",
        StoreWriteError::Canonical(StorageError::IdempotencyConflict) => "CONFLICT",
        _ => return None,
    };
    Some(BatchError::new(code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::bridge::StorageEngine as _;
    use crate::storage::StorageEngine as _;
    use crate::test_support::Fixture;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn request(f: &Fixture, key: &str, items: Vec<serde_json::Value>) -> BatchInput {
        BatchInput {
            request_context: None,
            expected_generation: None,
            mutation_version: 1,
            result_version: 1,
            kind: BatchKind::Insert,
            items,
            default_namespace: "default".into(),
            store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
            idempotency_key: Some(key.into()),
            batch_operation_id: None,
        }
    }
    fn item() -> serde_json::Value {
        json!({"summary":"batch fixture","embedding":[1.,0.,0.,0.],"checkDuplicates":false})
    }
    fn generation(f: &Fixture) -> u64 {
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation
            .get()
    }

    fn tracker(f: &Fixture) -> crate::storage::work::WorkTracker {
        f.storage.read().unwrap().meta_store().work_tracker()
    }
    fn deadline() -> std::time::Instant {
        std::time::Instant::now() + std::time::Duration::from_secs(10)
    }

    struct PausedProvider {
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        calls: Arc<AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for PausedProvider {
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.entered.notify_one();
            self.release.notified().await;
            Ok(vec![1., 0., 0., 0.])
        }
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> &str {
            "synthetic-work-gate"
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn single_provider_root_finishes_its_descendants_while_closing() {
        let f = Fixture::new();
        let mut adapter = f.storage_adapter();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        adapter.embedding = Arc::new(PausedProvider {
            entered: entered.clone(),
            release: release.clone(),
            calls: calls.clone(),
        });
        let input = crate::mcp::args::parse_store_input(
            &json!({"summary":"single accepted before provider","checkDuplicates":false}),
            "default",
        )
        .unwrap();
        let task = tokio::spawn(async move { adapter.store_memory(input).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
            .await
            .unwrap();
        let tracker = tracker(&f);
        assert!(tracker.status().pending > 0);
        let closing = tracker.clone();
        let drain = tokio::task::spawn_blocking(move || closing.close_and_drain(deadline()));
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while tracker.status().admission_open {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        release.notify_one();
        let result = task.await.unwrap().unwrap();
        let freeze = drain.await.unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            f.storage
                .read()
                .unwrap()
                .get_record(crate::model::MemoryId::from(
                    uuid::Uuid::parse_str(&result.id).unwrap()
                ))
                .unwrap()
                .is_some()
        );
        assert!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .pending_effects()
                .unwrap()
                .is_empty()
        );
        let before = generation(&f);
        let input = crate::mcp::args::parse_store_input(
            &json!({"summary":"new single rejected"}),
            "default",
        )
        .unwrap();
        assert!(
            f.storage_adapter()
                .store_memory(input)
                .await
                .unwrap_err()
                .to_string()
                .contains("admission is closed")
        );
        assert_eq!(generation(&f), before);
        drop(freeze);
    }

    #[tokio::test]
    async fn batch_work_close_before_entry_rejects_before_reservation_or_provider() {
        let f = Fixture::new();
        let mut adapter = f.storage_adapter();
        let calls = Arc::new(AtomicUsize::new(0));
        adapter.embedding = Arc::new(PausedProvider {
            entered: Arc::new(tokio::sync::Notify::new()),
            release: Arc::new(tokio::sync::Notify::new()),
            calls: calls.clone(),
        });
        let input = request(&f, "closed", vec![json!({"summary":"never staged"})]);
        let before = generation(&f);
        let tracker = tracker(&f);
        let lease = tracker.close_and_drain(deadline()).unwrap();
        let error = adapter.mutate_batch(input).await.unwrap_err();
        assert!(error.to_string().contains("admission is closed"), "{error}");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(generation(&f), before);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .pending_batch_items()
                .unwrap(),
            0
        );
        let store = f.storage.read().unwrap();
        let frozen = store.meta_store().freeze_metadata(lease).unwrap();
        assert_eq!(
            frozen.generation(),
            store.control_state().unwrap().generation_ref()
        );
    }

    #[tokio::test]
    async fn batch_work_accepted_provider_and_graph_descendants_finish_during_close() {
        let f = Fixture::new();
        let mut adapter = f.storage_adapter();
        Arc::make_mut(&mut adapter.config).graph.autolink_enabled = true;
        Arc::make_mut(&mut adapter.config).graph.temporal_window_ms = 0;
        assert!(
            adapter
                .mutate_batch(request(&f, "seed", vec![item()]))
                .await
                .unwrap()
                .all_ready()
        );
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        adapter.embedding = Arc::new(PausedProvider {
            entered: entered.clone(),
            release: release.clone(),
            calls: Arc::new(AtomicUsize::new(0)),
        });
        let input = request(
            &f,
            "closing",
            vec![
                json!({"summary":"second generated", "checkDuplicates":false}),
                json!({"summary":"runtime invalid vector","embedding":[0.,0.,0.,0.],"checkDuplicates":false}),
            ],
        );
        let task = tokio::spawn(async move { adapter.mutate_batch(input).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
            .await
            .unwrap();
        let tracker = tracker(&f);
        assert!(tracker.status().pending > 0);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .pending_batch_items()
                .unwrap(),
            2
        );
        let closing = tracker.clone();
        let drain = tokio::task::spawn_blocking(move || closing.close_and_drain(deadline()));
        tracker.wait_until_closed();
        assert!(!drain.is_finished());
        release.notify_one();
        let outcome = task.await.unwrap().unwrap();
        assert_eq!(
            (outcome.succeeded, outcome.failed, outcome.unresolved),
            (1, 1, 0),
            "{outcome:?}"
        );
        assert!(outcome.items[0].ready);
        assert_eq!(
            outcome.bookkeeping_receipts.len(),
            1,
            "runtime rejection must finalize as an accepted descendant"
        );
        let lease = drain.await.unwrap().unwrap();
        assert_eq!(tracker.status().pending, 0);
        let edges = f.persisted_edges();
        assert_eq!(
            edges.len(),
            1,
            "optional graph publication must not silently fail when closing"
        );
        for id in [edges[0].source, edges[0].target] {
            let outgoing = edges.iter().filter(|edge| edge.source == id).count() as u16;
            assert_eq!(f.edge_count(id), outgoing);
            // A safe cache miss is allowed. Bound cache hits must agree with
            // canonical counts even if an older insertion refill arrived late.
            if let Some(cached) = f.cache.get(id).await {
                assert_eq!(cached.edge_count, outgoing);
            }
            assert!(f.graph.read().await.has_typed_edge_between_ids(
                &edges[0].source,
                &edges[0].target,
                edges[0].edge_type
            ));
        }
        let store = f.storage.read().unwrap();
        assert!(store.meta_store().pending_effects().unwrap().is_empty());
        let frozen = store.meta_store().freeze_metadata(lease).unwrap();
        assert_eq!(
            frozen.generation(),
            store.control_state().unwrap().generation_ref()
        );
    }

    #[tokio::test]
    async fn batch_work_cancelled_provider_stays_unresolved_and_reopen_keeps_payload_pending() {
        let f = Fixture::new();
        let mut adapter = f.storage_adapter();
        let entered = Arc::new(tokio::sync::Notify::new());
        adapter.embedding = Arc::new(PausedProvider {
            entered: entered.clone(),
            release: Arc::new(tokio::sync::Notify::new()),
            calls: Arc::new(AtomicUsize::new(0)),
        });
        let input = request(
            &f,
            "cancelled",
            vec![json!({"summary":"caller payload required","checkDuplicates":false})],
        );
        let task = tokio::spawn(async move { adapter.mutate_batch(input).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
            .await
            .unwrap();
        let tracker = tracker(&f);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(tracker.status().unresolved > 0);
        assert!(tracker.close_and_drain(std::time::Instant::now()).is_err());
        let dir = f.close();
        let engine = crate::storage::RedbStorageEngine::open(dir.path()).unwrap();
        let meta = engine.meta_store();
        assert_eq!(meta.work_tracker().status().pending, 0);
        assert_eq!(meta.pending_batch_items().unwrap(), 1);
        let lease = meta.work_tracker().close_and_drain(deadline()).unwrap();
        let error = meta.freeze_metadata(lease).err().unwrap();
        assert!(error.to_string().contains("matching caller resubmission"));
    }

    #[tokio::test]
    async fn batch_work_terminal_errors_freeze_and_lifetime_scan_checks_each_manifest() {
        let f = Fixture::new();
        let adapter = f.storage_adapter();
        for index in 0..3 {
            let result = adapter
                .mutate_batch(request(
                    &f,
                    &format!("invalid-{index}"),
                    vec![json!({"summary":false})],
                ))
                .await
                .unwrap();
            assert_eq!(result.failed, 1);
            assert_eq!(result.unresolved, 0);
        }
        let store = f.storage.read().unwrap();
        let meta = store.meta_store();
        assert_eq!(meta.pending_batch_items().unwrap(), 0);
        let mut checks = 0;
        let stopped = meta.pending_batch_items_checked(|| {
            checks += 1;
            if checks == 3 {
                return Err(crate::storage::work::WorkError::Deadline {
                    pending: 0,
                    unresolved: 0,
                }
                .into());
            }
            Ok(())
        });
        assert!(stopped.is_err());
        assert_eq!(
            checks, 3,
            "deadline callback must run between lifetime rows"
        );
        let before = meta.control_state().unwrap().generation_ref();
        let frozen = meta
            .freeze_metadata(meta.work_tracker().close_and_drain(deadline()).unwrap())
            .unwrap();
        assert_eq!(frozen.generation(), before);
    }

    #[tokio::test]
    async fn batch_work_reserved_forget_finishes_with_accepted_parent_during_close() {
        let f = Fixture::new();
        let adapter = f.storage_adapter();
        let inserted = adapter
            .mutate_batch(request(&f, "forget-seed", vec![item()]))
            .await
            .unwrap();
        let id =
            crate::storage::insertion::inserted_id(inserted.items[0].receipt.as_ref().unwrap())
                .unwrap();
        let mut input = request(&f, "forget-closing", vec![json!(id.to_string())]);
        input.kind = BatchKind::Forget;
        let completion = adapter.batch_completion();
        let work = completion.accept_work(None).unwrap();
        let tracker = tracker(&f);
        let closing = tracker.clone();
        let drain = tokio::task::spawn_blocking(move || closing.close_and_drain(deadline()));
        tracker.wait_until_closed();
        let result = adapter
            .run_batch_accepted(input, &work, None, None)
            .await
            .unwrap();
        assert!(result.all_ready(), "{result:?}");
        assert!(
            !drain.is_finished(),
            "root remains accepted until final consumer return"
        );
        completion.finish_known_work(work);
        let lease = drain.await.unwrap().unwrap();
        let store = f.storage.read().unwrap();
        assert_eq!(
            store.get_record(id).unwrap().unwrap().phase,
            crate::model::DecayPhase::Tombstone
        );
        assert!(store.meta_store().freeze_metadata(lease).is_ok());
    }

    #[tokio::test]
    async fn batch_labels_historical_receipt_replays_but_old_pending_input_cannot_bypass_admission()
    {
        struct NeverProvider;
        #[async_trait::async_trait]
        impl crate::embedding::EmbeddingProvider for NeverProvider {
            async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
                panic!("historical replay or invalid pending input invoked provider")
            }
            fn dimensions(&self) -> usize {
                4
            }
            fn model_name(&self) -> &str {
                "synthetic-historical-labels"
            }
        }
        for committed in [false, true] {
            let f = Fixture::new();
            let mut adapter = f.storage_adapter();
            adapter.embedding = Arc::new(NeverProvider);
            let input = request(
                &f,
                "historical-label",
                vec![
                    json!({"summary":"old caller identity","entities":["José"],"checkDuplicates":false}),
                ],
            );
            let prepared = batches::prepare(&input).unwrap();
            assert!(matches!(prepared[0].plan, BatchItemPlan::Invalid { .. }));
            // Exact old accepted representation: UTF-8 entity was dropped by
            // parse_tags_lossy. The caller hash still binds original raw labels.
            let old_record = crate::storage::insertion::InsertMemory {
                id: None,
                summary: "old caller identity".into(),
                full_text: None,
                tags: vec![],
                initial_stability: None,
                created_at: None,
                embedding: None,
                parent_id: None,
                supersedes: None,
            };
            let plan = BatchItemPlan::Insert {
                request_hash: prepared[0].plan.request_hash(),
                namespace: "default".into(),
                input: old_record.clone(),
            };
            let progress = f
                .storage
                .read()
                .unwrap()
                .meta_store()
                .reserve_batch(&input.context(), BatchKind::Insert, &[plan])
                .unwrap();
            let child = progress.child(0).unwrap();
            let old_receipt = if committed {
                Some(
                    f.storage
                        .write()
                        .unwrap()
                        .insert_memory_reserved(
                            &child.context(),
                            &old_record,
                            &[1., 0., 0., 0.],
                            123,
                            Some(&child),
                        )
                        .unwrap(),
                )
            } else {
                None
            };
            let before = generation(&f);
            let result = adapter.mutate_batch(input.clone()).await.unwrap();
            assert_eq!(
                result.batch_operation_id,
                progress.reservation_receipt.operation_id
            );
            if let Some(receipt) = old_receipt {
                assert!(result.all_ready());
                assert_eq!(
                    result.items[0].receipt.as_ref().unwrap().operation_id,
                    receipt.operation_id
                );
                assert!(result.items[0].receipt.as_ref().unwrap().replayed);
                assert!(result.items[0].result.is_some());
                assert_eq!(generation(&f), before);
            } else {
                assert_eq!(result.items[0].status, BatchStatus::Error);
                assert_eq!(
                    result.items[0].error.as_ref().unwrap().code,
                    "INVALID_INPUT"
                );
                assert!(f.storage.read().unwrap().scan_all().unwrap().is_empty());
                assert_eq!(
                    result.items[0].error.as_ref().unwrap().field.as_deref(),
                    Some("entities")
                );
                assert_eq!(result.bookkeeping_receipts.len(), 1);
                assert_eq!(generation(&f), before + 1);
            }
            let replay_before = generation(&f);
            let replay = adapter.mutate_batch(input).await.unwrap();
            assert_eq!(replay.items[0].status, result.items[0].status);
            assert_eq!(generation(&f), replay_before);
        }
    }

    #[tokio::test]
    async fn actual_batch_concurrent_identical_requests_share_children_and_mutations() {
        let f = Fixture::new();
        let adapter = f.storage_adapter();
        let input = request(&f, "concurrent", vec![item()]);
        let before = generation(&f);
        let (one, two) = tokio::join!(
            adapter.mutate_batch(input.clone()),
            adapter.mutate_batch(input)
        );
        let one = one.unwrap();
        let two = two.unwrap();
        assert!(one.all_ready());
        assert!(two.all_ready());
        assert_eq!(one.batch_operation_id, two.batch_operation_id);
        assert_eq!(
            one.items[0].receipt.as_ref().unwrap().operation_id,
            two.items[0].receipt.as_ref().unwrap().operation_id
        );
        assert_eq!(generation(&f), before + 2);
        assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn supplied_batch_semantic_fields_validate_before_provider_or_payload_staging() {
        struct NeverProvider;
        #[async_trait::async_trait]
        impl crate::embedding::EmbeddingProvider for NeverProvider {
            async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
                panic!("provider must not run for supplied or invalid semantic inputs")
            }
            fn dimensions(&self) -> usize {
                4
            }
            fn model_name(&self) -> &str {
                "never"
            }
        }
        let f = Fixture::new();
        let mut adapter = f.storage_adapter();
        adapter.embedding = Arc::new(NeverProvider);
        let invalid = vec![
            json!({"summary":"s","embedding":"wrong"}),
            json!({"summary":"s","initialStability":false}),
            json!({"summary":"s","initialStability":0}),
            json!({"summary":"s","initialStability":-0.0}),
            json!({"summary":"s","embedding":[0.,0.,0.,0.]}),
            json!({"summary":"s","id":uuid::Uuid::new_v4()}),
        ];
        let before = generation(&f);
        let outcome = adapter
            .mutate_batch(request(&f, "invalid semantic", invalid))
            .await
            .unwrap();
        assert_eq!(
            (outcome.succeeded, outcome.failed, outcome.unresolved),
            (0, 6, 0)
        );
        assert!(f.storage.read().unwrap().scan_all().unwrap().is_empty());
        assert_eq!(generation(&f), before + 2); // reservation plus zero-vector final rejection
        let supplied = request(
            &f,
            "supplied",
            vec![
                json!({"summary":"s","embedding":[2.,0.,0.,0.],"initialStability":7.25,"checkDuplicates":false}),
            ],
        );
        let success = adapter.mutate_batch(supplied.clone()).await.unwrap();
        assert!(success.all_ready());
        assert!(
            matches!(success.items[0].receipt.as_ref().unwrap().outcome,MutationOutcome::MemoryInserted{initial_stability_bits,..} if initial_stability_bits==7.25f32.to_bits())
        );
        let before = generation(&f);
        let mut changed = supplied;
        changed.items[0]
            .as_object_mut()
            .unwrap()
            .remove("initialStability");
        assert!(adapter.mutate_batch(changed).await.is_err());
        assert_eq!(generation(&f), before);
    }

    #[tokio::test]
    async fn actual_batch_mixed_results_and_changed_default_or_original_bits_conflict() {
        let f = Fixture::new();
        f.create_namespace("other");
        let mut adapter = f.storage_adapter();
        let mut config = crate::config::RecalldConfig::default();
        config.graph.autolink_enabled = false;
        adapter.config = Arc::new(config);
        let input = request(
            &f,
            "mixed",
            vec![
                json!(false),
                item(),
                json!({"summary":"invalid zero","embedding":[0.,0.,0.,0.]}),
                item(),
            ],
        );
        let before = generation(&f);
        let outcome = adapter.mutate_batch(input.clone()).await.unwrap();
        assert_eq!(
            (
                outcome.total,
                outcome.succeeded,
                outcome.failed,
                outcome.unresolved
            ),
            (4, 2, 2, 0)
        );
        assert_eq!(outcome.bookkeeping_receipts.len(), 1);
        assert_eq!(generation(&f), before + 4); // parent, 2 children, 1 final rejection
        assert!(outcome.items.iter().all(|i| i.ready));
        assert!(!outcome.all_ready());
        assert_ne!(
            outcome.items[1].receipt.as_ref().unwrap().operation_id,
            outcome.items[3].receipt.as_ref().unwrap().operation_id
        );
        let replay = adapter.mutate_batch(input.clone()).await.unwrap();
        assert_eq!(generation(&f), before + 4);
        assert!(replay.reservation_receipt.unwrap().replayed);
        assert_eq!(
            replay.items[1].receipt.as_ref().unwrap().operation_id,
            outcome.items[1].receipt.as_ref().unwrap().operation_id
        );
        for changed in [
            "namespace",
            "scaled",
            "signedzero",
            "order",
            "stability",
            "unknown",
        ] {
            let mut retry = input.clone();
            match changed {
                "namespace" => retry.default_namespace = "other".into(),
                "scaled" => retry.items[1]["embedding"] = json!([2., 0., 0., 0.]),
                "signedzero" => retry.items[1]["embedding"] = json!([1., -0.0, 0., 0.]),
                "stability" => {
                    retry.items[1]["initialStability"] = json!(
                        f.storage
                            .read()
                            .unwrap()
                            .get_namespace(crate::model::NamespaceId::new(1))
                            .unwrap()
                            .unwrap()
                            .initial_stability
                    )
                }
                "unknown" => retry.items[1]["callerExtension"] = json!({"future":"intent"}),
                _ => retry.items.swap(0, 1),
            }
            assert!(
                adapter
                    .mutate_batch(retry)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("IDEMPOTENCY_CONFLICT")
            );
            assert_eq!(generation(&f), before + 4);
        }
    }

    #[tokio::test]
    async fn committed_unready_child_keeps_current_receipt_and_replays_after_online_repair() {
        let f = Fixture::new();
        let adapter = f.storage_adapter();
        *f.vector_index.write().await =
            crate::search::FlatVectorIndex::new(crate::test_support::DIM + 1);
        let input = request(&f, "publication", vec![item(), item()]);
        let outcome = adapter.mutate_batch(input.clone()).await.unwrap();
        assert_eq!(
            (outcome.succeeded, outcome.failed, outcome.unresolved),
            (1, 0, 1)
        );
        let current = outcome.items[0].receipt.as_ref().unwrap();
        assert_eq!(Some(current.operation_id), outcome.items[0].operation_id);
        assert_ne!(current.operation_id, outcome.batch_operation_id);
        assert!(!outcome.items[0].ready);
        assert!(!outcome.items[0].retryable);
        let pending = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap();
        assert_eq!(pending[0].receipt.operation_id, current.operation_id);
        *f.vector_index.write().await =
            crate::search::FlatVectorIndex::new(crate::test_support::DIM);
        let replay = adapter.mutate_batch(input).await.unwrap();
        assert!(replay.all_ready());
        assert_eq!(replay.succeeded, 2);
        assert!(replay.items[0].receipt.as_ref().unwrap().replayed);
        assert_eq!(
            replay.items[0].receipt.as_ref().unwrap().operation_id,
            current.operation_id
        );
        assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn actual_adapter_distinguishes_unknown_reservation_child_and_error_bookkeeping() {
        for stage in [1, 2, 3] {
            let f = Fixture::new();
            let adapter = f.storage_adapter();
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .fail_batch_commit
                .store(stage, Ordering::SeqCst);
            let mut value = item();
            if stage == 2 {
                value["embedding"] = json!([0., 0., 0., 0.]);
            }
            let input = request(&f, "unknown", vec![value, item()]);
            let outcome = adapter.mutate_batch(input.clone()).await.unwrap();
            assert_eq!(outcome.items[0].status, BatchStatus::Unknown);
            assert!(!outcome.items[0].ready);
            assert!(!outcome.items[0].retryable);
            let parent = outcome.batch_operation_id;
            let store_id = outcome.store_id;
            if stage == 1 {
                assert_eq!(outcome.reservation_state, BatchReservationState::Unknown);
                assert!(outcome.reservation_receipt.is_none());
                assert!(outcome.items[0].operation_id.is_none());
            } else {
                assert_eq!(outcome.reservation_state, BatchReservationState::Committed);
                assert_ne!(outcome.items[0].operation_id, Some(parent));
                assert!(outcome.items[0].receipt.is_none());
                assert_eq!(
                    outcome.unknown_bookkeeping_operations.len(),
                    usize::from(stage == 2)
                );
            }
            let tracker = tracker(&f);
            assert!(tracker.status().unresolved > 0);
            assert!(tracker.close_and_drain(std::time::Instant::now()).is_err());
            drop(adapter);
            let dir = f.close();
            let engine = crate::storage::RedbStorageEngine::open(dir.path()).unwrap();
            let progress = engine
                .meta_store()
                .lookup_batch(store_id, parent)
                .unwrap()
                .unwrap();
            assert!(engine.meta_store().pending_batch_items().unwrap() > 0);
            let tracker = engine.meta_store().work_tracker();
            let lease = tracker.close_and_drain(deadline()).unwrap();
            assert!(engine.meta_store().freeze_metadata(lease).is_err());
            if stage == 3 {
                assert!(
                    matches!(&progress.items[0].state,BatchItemState::Ok{receipt} if Some(receipt.operation_id)==outcome.items[0].operation_id)
                );
            }
            if stage == 2 {
                assert_eq!(
                    progress.bookkeeping_receipts[0].operation_id,
                    outcome.unknown_bookkeeping_operations[0]
                );
                assert!(matches!(
                    progress.items[0].state,
                    BatchItemState::Error { .. }
                ));
            }
            if stage == 1 {
                assert!(
                    progress
                        .items
                        .iter()
                        .all(|i| matches!(i.state, BatchItemState::NotAttempted {}))
                );
            }
        }
    }

    #[tokio::test]
    async fn provider_failure_stays_pending_and_committed_replay_skips_provider() {
        struct Provider {
            calls: Arc<AtomicUsize>,
        }
        #[async_trait::async_trait]
        impl crate::embedding::EmbeddingProvider for Provider {
            async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
                Ok(if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    vec![0.; 4]
                } else {
                    vec![1., 0., 0., 0.]
                })
            }
            fn dimensions(&self) -> usize {
                4
            }
            fn model_name(&self) -> &str {
                "synthetic-batch"
            }
        }
        let f = Fixture::new();
        let mut adapter = f.storage_adapter();
        let calls = Arc::new(AtomicUsize::new(0));
        adapter.embedding = Arc::new(Provider {
            calls: calls.clone(),
        });
        let input = request(
            &f,
            "provider",
            vec![json!({"summary":"generated","checkDuplicates":false})],
        );
        let before = generation(&f);
        let first = adapter.mutate_batch(input.clone()).await.unwrap();
        assert_eq!(first.items[0].status, BatchStatus::NotAttempted);
        assert_eq!(generation(&f), before + 1);
        assert!(f.storage.read().unwrap().scan_all().unwrap().is_empty());
        assert!(first.items[0].retryable);
        let tracker = tracker(&f);
        assert_eq!(tracker.status().pending, 0);
        let lease = tracker.close_and_drain(deadline()).unwrap();
        assert!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .freeze_metadata(lease)
                .is_err()
        );
        assert!(tracker.status().admission_open);
        let complete = adapter.mutate_batch(input.clone()).await.unwrap();
        assert!(complete.all_ready());
        let replay = adapter.mutate_batch(input).await.unwrap();
        assert!(replay.all_ready());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(generation(&f), before + 2);
    }

    #[tokio::test]
    async fn actual_mcp_retains_every_index_and_sets_error_on_committed_not_ready() {
        let f = Fixture::new();
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.storage = Arc::new(f.storage_adapter());
        *f.vector_index.write().await =
            crate::search::FlatVectorIndex::new(crate::test_support::DIM + 1);
        let result = crate::mcp::tools::dispatch_tool(
            &bridge,
            "store_memories",
            json!({"memories":[false,item(),item()]}),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
        let response: serde_json::Value = serde_json::from_str(text).unwrap();
        let outcome: BatchOutcome = serde_json::from_value(response["batch"].clone()).unwrap();
        assert_eq!(
            (
                outcome.total,
                outcome.succeeded,
                outcome.failed,
                outcome.unresolved
            ),
            (3, 1, 1, 1)
        );
        assert_eq!(
            outcome.items[1].receipt.as_ref().map(|r| r.operation_id),
            outcome.items[1].operation_id
        );
        assert_ne!(
            outcome.items[1].operation_id,
            Some(outcome.batch_operation_id)
        );
        assert_eq!(response["results"].as_array().unwrap().len(), 3);
        assert_eq!(response["stored"], 1);
        assert_eq!(response["results"][1]["ready"], false);
    }
}
