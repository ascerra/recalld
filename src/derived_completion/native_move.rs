use super::*;
use crate::embedding::EmbeddingProvider;
use crate::storage::control::WriteReceipt;
use crate::storage::native_move::{MoveProgress, NativeMoveInput, SharedMoveProgress};

impl DerivedCompletion {
    pub async fn move_native(
        &self,
        input: NativeMoveInput,
        provider: Arc<dyn EmbeddingProvider>,
    ) -> Result<WriteReceipt, StorageError> {
        self.move_native_with_work(input, provider, None).await
    }

    pub async fn move_native_with_work(
        &self,
        input: NativeMoveInput,
        provider: Arc<dyn EmbeddingProvider>,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        input.validate()?;
        let work = self.accept_work(parent)?;
        let result = async {
            // Original caller identity precedes current names, records and
            // provider configuration, including when the source was purged.
            let replay = self.storage.read().map_err(|e| failed(e.to_string()))?
                .meta_store().lookup_native_move(&input, Some(&work))?;
            let receipt = if let Some(receipt) = replay { receipt } else {
                let capture = self.storage.read().map_err(|e| failed(e.to_string()))?
                    .capture_native_move(input, Some(&work))?;
                let mut generated = Vec::with_capacity(capture.documents.len());
                for document in &capture.documents {
                    generated.push(crate::embedding::runtime::generate_native(
                        provider.as_ref(), &capture.destination, &document.text,
                    ).await.map_err(|e| StorageError::ProviderVectorUnavailable(e.to_string()))?);
                }
                let lease = publication_gate(&self.storage).await?.read_owned().await;
                let graph = self.graph.clone().write_owned().await;
                let storage = self.storage.clone();
                let child = work.child()?;
                let progress: SharedMoveProgress = Arc::new(std::sync::Mutex::new(MoveProgress::Preparing));
                let worker_progress = progress.clone();
                let joined = tokio::task::spawn_blocking(move || {
                    let _lease = lease;
                    let _graph = graph;
                    let mut engine = storage.write().map_err(|e| failed(e.to_string()))?;
                    let result = engine.move_native_prepared(&capture, &generated, &worker_progress, Some(&child));
                    if !matches!(result, Err(StorageError::CommitOutcomeUnknown { .. })) {
                        engine.meta_store().finish_work(child);
                    }
                    result
                }).await;
                match joined {
                    Ok(result) => result?,
                    Err(error) => {
                        let source = failed(format!("move worker failed: {error}"));
                        return Err(match &*progress.lock().unwrap_or_else(|e| e.into_inner()) {
                            MoveProgress::Preparing => source,
                            MoveProgress::Staging(operation_id) => StorageError::CommitOutcomeUnknown {
                                operation_id: *operation_id,
                                source: redb::CommitError::Storage(redb::StorageError::Io(std::io::Error::other("move worker outcome unknown; reconcile original keyed request"))),
                            },
                            MoveProgress::Committed(receipt) => StorageError::CommittedMutation {
                                receipt: Box::new(receipt.clone()), source: Box::new(source),
                            },
                        });
                    }
                }
            };
            self.complete_with_work(Some(&work)).await.map_err(|source| StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()), source: Box::new(source),
            })?;
            Ok(receipt)
        }.await;
        if !matches!(result, Err(StorageError::CommitOutcomeUnknown { .. })) {
            self.finish_known_work(work);
        }
        result
    }
}
