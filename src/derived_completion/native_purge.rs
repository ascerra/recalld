//! Native purge uses the existing accepted task and completion owner.
use super::*;
use crate::storage::control::{MutationContext, WriteReceipt};
use crate::storage::purge::{PurgeCleanupObservation, PurgeProgress, PurgeStage};

#[derive(Debug, thiserror::Error)]
pub(crate) enum PurgeApi1Failure {
    #[error("{source}")]
    Storage {
        source: StorageError,
        known_receipt: Option<Box<WriteReceipt>>,
    },
    #[error("purge worker outcome unknown; reconcile before retry")]
    WorkerUnknown {
        known_receipt: Option<Box<WriteReceipt>>,
    },
    #[error(
        "Purge completed canonically; this invocation did not observe physical cleanup metrics. Do not retry deletion by name to recover metrics."
    )]
    MetricsUnavailable { receipt: Box<WriteReceipt> },
    #[error("namespace '{0}' not found")]
    NamespaceNotFound(String),
}
impl From<StorageError> for PurgeApi1Failure {
    fn from(source: StorageError) -> Self {
        Self::Storage {
            source,
            known_receipt: None,
        }
    }
}
impl PurgeApi1Failure {
    fn ambiguous(&self) -> bool {
        fn unknown(source: &StorageError) -> bool {
            match source {
                StorageError::CommitOutcomeUnknown { .. }
                | StorageError::WorkerOutcomeUnknown(_) => true,
                StorageError::CommittedMutation { source, .. } => unknown(source),
                _ => false,
            }
        }
        match self {
            Self::WorkerUnknown { .. } => true,
            Self::Storage { source, .. } => unknown(source),
            _ => false,
        }
    }
    fn into_storage(self) -> StorageError {
        match self {
            Self::Storage {
                source,
                known_receipt,
            } => match source {
                source @ (StorageError::CommittedMutation { .. }
                | StorageError::CommitOutcomeUnknown { .. }
                | StorageError::WorkerOutcomeUnknown(_)) => source,
                source => match known_receipt {
                    Some(receipt) => StorageError::CommittedMutation {
                        receipt,
                        source: Box::new(source),
                    },
                    None => source,
                },
            },
            Self::WorkerUnknown { .. } => {
                StorageError::WorkerOutcomeUnknown("purge worker did not return its outcome".into())
            }
            Self::MetricsUnavailable { receipt } => StorageError::CommittedMutation {
                receipt,
                source: Box::new(failed("purge physical observations unavailable".into())),
            },
            Self::NamespaceNotFound(name) => {
                StorageError::InvalidMutation(format!("namespace '{name}' not found"))
            }
        }
    }
}

pub(crate) struct PurgeApi1Success {
    pub progress: PurgeProgress,
    pub directory_removed: bool,
    pub fts_rows_removed: usize,
}

enum PurgeSelector {
    Context(MutationContext),
    Name(String),
}

impl DerivedCompletion {
    pub(crate) async fn purge_native_accepted(
        &self,
        ctx: MutationContext,
        force: bool,
        parent: &WorkTicket,
    ) -> Result<PurgeProgress, StorageError> {
        ctx.validate()?;
        let work = self.accept_work(Some(parent))?;
        let result = self
            .native_purge_work(PurgeSelector::Context(ctx), force, false, &work, None)
            .await;
        if !result.as_ref().is_err_and(PurgeApi1Failure::ambiguous) {
            self.finish_known_work(work);
        }
        result
            .map(|(progress, _)| progress)
            .map_err(PurgeApi1Failure::into_storage)
    }

    #[cfg(test)]
    pub(crate) async fn purge_native_api1(
        &self,
        name: String,
        force: bool,
    ) -> Result<PurgeApi1Success, PurgeApi1Failure> {
        self.purge_native_api1_bounded(name, force, None).await
    }

    pub(crate) async fn purge_native_api1_bounded(
        &self,
        name: String,
        force: bool,
        receipt_limit: Option<usize>,
    ) -> Result<PurgeApi1Success, PurgeApi1Failure> {
        let work = self.accept_work(None)?;
        let result = self
            .native_purge_work(PurgeSelector::Name(name), force, true, &work, receipt_limit)
            .await;
        if !result.as_ref().is_err_and(PurgeApi1Failure::ambiguous) {
            self.finish_known_work(work);
        }
        let (progress, observed) = result?;
        let (Some(directory_removed), Some(fts_rows_removed)) =
            (observed.directory_removed, observed.fts_rows_removed)
        else {
            return Err(PurgeApi1Failure::MetricsUnavailable {
                receipt: Box::new(progress.last_receipt),
            });
        };
        Ok(PurgeApi1Success {
            progress,
            directory_removed,
            fts_rows_removed,
        })
    }

    async fn native_purge_work(
        &self,
        selector: PurgeSelector,
        force: bool,
        observe: bool,
        work: &WorkTicket,
        receipt_limit: Option<usize>,
    ) -> Result<(PurgeProgress, PurgePhysicalObservation), PurgeApi1Failure> {
        let gate = publication_gate(&self.storage).await?;
        let lease = gate.read_owned().await;
        let graph = self.graph.clone().read_owned().await;
        let storage = self.storage.clone();
        let child = work.child().map_err(StorageError::from)?;
        let start = tokio::task::spawn_blocking(move || {
            // Cancellation cannot release the lease before this write finishes.
            let _publication = lease;
            let _graph = graph;
            #[cfg(test)]
            if storage
                .read()
                .unwrap()
                .meta_store()
                .native_purge_worker_fault
                .load(std::sync::atomic::Ordering::SeqCst)
                == 1
            {
                panic!("injected native purge begin worker loss");
            }
            let mut storage = storage.write().map_err(|e| failed(e.to_string()))?;
            let result = (|| {
                storage.meta_store().require_native_purge_owner()?;
                let ctx = match selector {
                    PurgeSelector::Context(ctx) => ctx,
                    PurgeSelector::Name(name) => {
                        let namespace = storage
                            .get_namespace_by_name(&name)?
                            .ok_or(PurgeApi1Failure::NamespaceNotFound(name))?;
                        // API contract and physical schema are independent.
                        MutationContext::unkeyed(namespace.id)
                    }
                };
                storage
                    .begin_namespace_purge_admitted(&ctx, force, Some(&child), receipt_limit)
                    .map_err(PurgeApi1Failure::from)
            })();
            storage.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|_| PurgeApi1Failure::WorkerUnknown {
            known_receipt: None,
        })??;
        #[cfg(test)]
        {
            let pause = self
                .storage
                .read()
                .unwrap()
                .meta_store()
                .native_purge_pause
                .lock()
                .unwrap()
                .take();
            if let Some((started, resume)) = pause {
                let _ = started.send(start.clone());
                let _ = resume.await;
            }
        }
        let known_start = || Some(Box::new(start.last_receipt.clone()));
        let completion = self
            .complete_observing(
                Some(work),
                false,
                observe.then(|| PurgeCleanupObservation::for_progress(&start)),
            )
            .await;
        let observed = match completion {
            Ok(observed) => observed,
            Err(CompletionFailure::WorkerUnknown) => {
                return Err(PurgeApi1Failure::WorkerUnknown {
                    known_receipt: known_start(),
                });
            }
            // Preserve typed matched-child/foreign/unknown evidence. Do not read
            // a potentially poisoned owner merely to upgrade the known receipt.
            Err(CompletionFailure::Storage(source)) => {
                return Err(PurgeApi1Failure::Storage {
                    source,
                    known_receipt: known_start(),
                });
            }
        };
        let latest = self
            .storage
            .read()
            .map_err(|e| failed(e.to_string()))
            .and_then(|storage| {
                storage.lookup_namespace_purge(
                    start.store_id,
                    start.namespace_id,
                    start.start_receipt.operation_id,
                )
            })
            .and_then(|p| p.ok_or_else(|| failed("purge progress disappeared".into())))
            .map_err(|source| PurgeApi1Failure::Storage {
                source,
                known_receipt: known_start(),
            })?;
        if latest.stage != PurgeStage::Complete {
            return Err(PurgeApi1Failure::Storage {
                source: failed("purge completion left active progress".into()),
                known_receipt: Some(Box::new(latest.last_receipt)),
            });
        }
        Ok((latest, observed))
    }
}
