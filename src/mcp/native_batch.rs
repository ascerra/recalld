//! Native child execution in the existing ordered batch consumer. The accepted
//! root precedes reservation/provider work; no lock spans the provider await.
use super::{McpStorageAdapter, batch_adapters::forget_result, bridge};
use crate::mcp::batches;
use crate::storage::{
    StorageError,
    batch::{BatchChild, BatchItemPlan},
    control::WriteReceipt,
    native_lifecycle::{LifecycleScope, NativePolicyInputs},
    work::WorkTicket,
};

impl McpStorageAdapter {
    pub(super) fn native_store_blocker(
        &self,
        prepared: &batches::PreparedItem,
    ) -> Option<&'static str> {
        if matches!(prepared.plan, BatchItemPlan::NativeInsert { .. }) {
            crate::storage::native_records::native_store_execution_blocker(
                &self.config,
                prepared
                    .store
                    .as_ref()
                    .and_then(|s| s.check_duplicates)
                    .unwrap_or(true),
            )
        } else {
            None
        }
    }

    pub(super) async fn run_native_batch_child(
        &self,
        prepared: &batches::PreparedItem,
        child: &BatchChild,
        work: &WorkTicket,
    ) -> Result<(WriteReceipt, serde_json::Value), super::StoreWriteError> {
        let policy = NativePolicyInputs {
            global: crate::decay::DecayConfig::default(),
            multiplier: self.config.decay.decay_rate_multiplier,
        };
        // Reconcile before current namespace/capture/provider. A completed item
        // is immutable even after deletion or policy/provider reconfiguration.
        let replay = {
            let engine = self
                .storage
                .read()
                .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
            match &prepared.plan {
                BatchItemPlan::NativeInsert { input, .. } => engine
                    .meta_store()
                    .lookup_native_insert_child(child, input, Some(work))?,
                BatchItemPlan::Forget { id, .. } => engine
                    .meta_store()
                    .lookup_native_forget_child(child, *id, Some(work))?,
                _ => {
                    return Err(
                        StorageError::InvalidMutation("invalid native child plan".into()).into(),
                    );
                }
            }
        };
        let receipt = if let Some(receipt) = replay {
            receipt
        } else {
            if let Some(field) = self.native_store_blocker(prepared) {
                return Err(bridge::BridgeError::Unavailable(format!(
                    "native store {field} requires a participating auxiliary writer"
                ))
                .into());
            }
            let insert = match &prepared.plan {
                BatchItemPlan::NativeInsert { input, .. } => {
                    let intent = {
                        let engine = self
                            .storage
                            .read()
                            .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                        engine.meta_store().capture_native_insert(
                            LifecycleScope {
                                namespace_id: child.namespace(),
                                namespace_name: child.namespace_name().to_owned(),
                                source: None,
                            },
                            input,
                            &policy,
                        )?
                    };
                    let generated = if input.memory.embedding.is_some() {
                        crate::embedding::runtime::GeneratedEmbedding {
                            values: vec![],
                            profile: None,
                        }
                    } else {
                        crate::embedding::runtime::generate_native(
                            self.embedding.as_ref(),
                            &intent.namespace,
                            &input.document_v1(),
                        )
                        .await
                        .map_err(|e| StorageError::ProviderVectorUnavailable(e.to_string()))?
                    };
                    Some((intent, generated))
                }
                _ => None,
            };
            let forget = if let BatchItemPlan::Forget { id, .. } = &prepared.plan {
                let engine = self
                    .storage
                    .read()
                    .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                Some(engine.meta_store().capture_native_tombstone(
                    *id,
                    Some(child.namespace()),
                    &policy,
                )?)
            } else {
                None
            };
            let gate = crate::derived_completion::publication_gate(&self.storage).await?;
            let lease = gate.read_owned().await;
            let storage = self.storage.clone();
            let clock = self.clock.clone();
            let reservation = child.clone();
            let ticket = work.child().map_err(StorageError::from)?;
            let result = tokio::task::spawn_blocking(move || {
                let mut engine = storage
                    .write()
                    .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                let result = if let Some((intent, generated)) = insert {
                    engine.insert_native_record_reserved(
                        &reservation.context(),
                        &intent,
                        &generated,
                        &policy,
                        clock.as_ref(),
                        Some(&ticket),
                        Some(&reservation),
                    )
                } else if let Some(intent) = forget {
                    engine.tombstone_native_record_reserved(
                        &reservation.context(),
                        &intent,
                        &policy,
                        clock.as_ref(),
                        Some(&ticket),
                        Some(&reservation),
                    )
                } else {
                    Err(StorageError::InvalidMutation(
                        "missing native operation".into(),
                    ))
                };
                engine.meta_store().finish_work(ticket);
                result
            })
            .await
            .map_err(|e| {
                bridge::BridgeError::PartiallyApplied(format!(
                    "native child worker outcome unknown: {e}"
                ))
            })?;
            drop(lease);
            result?
        };
        self.batch_completion()
            .complete_with_work(Some(work))
            .await
            .map_err(|source| StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            })?;
        let result = if let Some(input) = &prepared.store {
            serde_json::to_value(batches::stored_from_receipt(
                input,
                &receipt,
                self.timezone,
            )?)
            .map_err(|e| bridge::BridgeError::Internal(e.to_string()))?
        } else {
            forget_result(
                prepared.forget.ok_or_else(|| {
                    bridge::BridgeError::Internal("native forget input missing".into())
                })?,
                &receipt,
            )
        };
        Ok((receipt, result))
    }
}
