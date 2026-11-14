//! Same-insert native execution by the existing accepted batch runner. Frozen
//! canonical selection and the reserved outcome commit together; no rediscovery.
use super::{McpStorageAdapter, StoreWriteError, bridge};
use crate::mcp::batches;
use crate::storage::{
    StorageError,
    batch::{BatchChild, BatchItemPlan, BatchOutcome},
    control::{MutationOutcome, WriteReceipt},
    native_lifecycle::{LifecycleScope, NativePolicyInputs},
    work::WorkTicket,
};

impl McpStorageAdapter {
    pub(super) async fn run_linked_batch_child(
        &self,
        item: &batches::PreparedItem,
        child: &BatchChild,
        work: &WorkTicket,
    ) -> Result<(WriteReceipt, serde_json::Value), StoreWriteError> {
        let BatchItemPlan::NativeStore { input, .. } = &item.plan else {
            return Err(StorageError::InvalidMutation("wrong linked batch plan".into()).into());
        };
        let replay = self
            .storage
            .read()
            .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?
            .meta_store()
            .lookup_native_store_child(child, input, Some(work))?;
        let receipt = if let Some(receipt) = replay {
            receipt
        } else {
            let policy = NativePolicyInputs {
                global: crate::decay::DecayConfig::default(),
                multiplier: self.config.decay.decay_rate_multiplier,
            };
            let settings = child.native_store_settings()?;
            let intent = self
                .storage
                .read()
                .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?
                .meta_store()
                .capture_native_store(
                    LifecycleScope {
                        namespace_id: child.namespace(),
                        namespace_name: child.namespace_name().into(),
                        source: None,
                    },
                    input,
                    settings,
                    &policy,
                )?;
            let generated = if input.insert.memory.embedding.is_some() {
                crate::embedding::runtime::GeneratedEmbedding {
                    values: vec![],
                    profile: None,
                }
            } else {
                crate::embedding::runtime::generate_native(
                    self.embedding.as_ref(),
                    &intent.insert_intent().namespace,
                    input
                        .insert
                        .memory
                        .full_text
                        .as_deref()
                        .unwrap_or(&input.insert.memory.summary),
                )
                .await
                .map_err(|e| StorageError::ProviderVectorUnavailable(e.to_string()))?
            };
            let prepared = {
                let engine = self
                    .storage
                    .read()
                    .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                let candidates = crate::graph::native_selection::capture(
                    &engine,
                    intent.insert_intent(),
                    &generated,
                )?;
                engine.meta_store().prepare_native_store_links(
                    &intent,
                    &candidates.frozen_plan(&intent)?,
                    &policy,
                )?
            };
            let gate = crate::derived_completion::publication_gate(&self.storage).await?;
            let lease = gate.read_owned().await;
            let graph = self.graph.clone().write_owned().await;
            let storage = self.storage.clone();
            let clock = self.clock.clone();
            let child = child.clone();
            let ticket = work.child().map_err(StorageError::from)?;
            let result = tokio::task::spawn_blocking(move || {
                let mut engine = storage
                    .write()
                    .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
                let result = engine.store_native_prepared_reserved(
                    &child.context(),
                    &prepared,
                    &generated,
                    &policy,
                    clock.as_ref(),
                    Some(&ticket),
                    Some(&child),
                );
                drop(graph);
                engine.meta_store().finish_work(ticket);
                result
            })
            .await
            .map_err(|e| {
                bridge::BridgeError::PartiallyApplied(format!(
                    "linked native child outcome unknown: {e}"
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
        let original = item.store.as_ref().ok_or_else(|| {
            bridge::BridgeError::Internal("linked store response input missing".into())
        })?;
        let result = serde_json::to_value(batches::stored_from_receipt(
            original,
            &receipt,
            self.timezone,
        )?)
        .map_err(|e| bridge::BridgeError::Internal(e.to_string()))?;
        Ok((receipt, result))
    }

    /// Post-readiness enrichment only. The durable proof keeps IDs/scores, while
    /// current visible payload and its generation belong solely to this reply.
    pub(super) fn enrich_linked_batch(&self, outcome: &mut BatchOutcome) {
        for item in &mut outcome.items {
            if !item.ready {
                continue;
            }
            let Some(receipt) = &item.receipt else {
                continue;
            };
            let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome
            else {
                continue;
            };
            let Some(result) = item
                .result
                .as_mut()
                .and_then(serde_json::Value::as_object_mut)
            else {
                continue;
            };
            let candidates = transition.advisory_matches();
            let read = match self.storage.read() {
                Ok(engine) => crate::mcp::native_advisory::hydrate(
                    &engine,
                    transition.namespace_id,
                    candidates.as_deref(),
                ),
                Err(_) => crate::mcp::native_advisory::NativeAdvisoryRead {
                    report: None,
                    read_generation: None,
                    warnings: vec!["ADVISORY_UNAVAILABLE"],
                },
            };
            if let Some(report) = read.report {
                result.insert(
                    "nearDuplicates".into(),
                    serde_json::to_value(report).expect("native advisory serializes finite scores"),
                );
            }
            result.insert("advisory".into(),serde_json::json!({"requested":transition.check_duplicates,"readGeneration":read.read_generation,"warnings":read.warnings}));
        }
    }
}
