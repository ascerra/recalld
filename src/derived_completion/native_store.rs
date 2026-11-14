//! Namespace-keyed single insertion. Selection and links use the existing
//! same-insert proof; no batch reservation or second work registry is created.
use super::*;
use crate::config::GraphConfig;
use crate::embedding::{EmbeddingProvider, runtime::GeneratedEmbedding};
use crate::storage::control::{MutationContext, MutationScope, WriteReceipt};
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs};
use crate::storage::native_store::NativeStoreInput;
use crate::time::Clock;

impl DerivedCompletion {
    pub(crate) async fn store_native_profile_with_work(
        &self,
        ctx: MutationContext,
        input: NativeStoreInput,
        supplied_profile_id: Option<String>,
        provider: Arc<dyn EmbeddingProvider>,
        settings: GraphConfig,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        use crate::storage::metadata::NativeStoreProfileResolution;
        let work = self.accept_work(parent)?;
        let result = async {
            let resolved = self
                .storage
                .read()
                .map_err(|e| failed(e.to_string()))?
                .meta_store()
                .resolve_native_store_profile(
                    &ctx,
                    &input,
                    supplied_profile_id.as_deref(),
                    Some(&work),
                )?;
            match resolved {
                NativeStoreProfileResolution::Replay(receipt) => {
                    self.complete_with_work(Some(&work))
                        .await
                        .map_err(|source| StorageError::CommittedMutation {
                            receipt: Box::new(receipt.clone()),
                            source: Box::new(source),
                        })?;
                    Ok(receipt)
                }
                NativeStoreProfileResolution::Fresh(input) => {
                    self.store_native_accepted(ctx, input, provider, settings, policy, clock, &work)
                        .await
                }
            }
        }
        .await;
        self.finish_known_work(work);
        result
    }

    pub(crate) async fn store_native_with_work(
        &self,
        ctx: MutationContext,
        input: NativeStoreInput,
        provider: Arc<dyn EmbeddingProvider>,
        settings: GraphConfig,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        parent: Option<&WorkTicket>,
    ) -> Result<WriteReceipt, StorageError> {
        let work = self.accept_work(parent)?;
        let result = self
            .store_native_accepted(ctx, input, provider, settings, policy, clock, &work)
            .await;
        self.finish_known_work(work);
        result
    }

    async fn store_native_accepted(
        &self,
        ctx: MutationContext,
        input: NativeStoreInput,
        provider: Arc<dyn EmbeddingProvider>,
        settings: GraphConfig,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        work: &WorkTicket,
    ) -> Result<WriteReceipt, StorageError> {
        // Historical defaults/settings are loaded by lookup before evaluating
        // current config, provider output, a clock, or allocating payloads.
        let replay = self
            .storage
            .read()
            .map_err(|e| failed(e.to_string()))?
            .meta_store()
            .lookup_native_store(&ctx, &input, Some(work))?;
        let receipt = if let Some(receipt) = replay {
            receipt
        } else {
            let MutationScope::Namespace(namespace_id) = ctx.scope else {
                return Err(StorageError::MutationNamespaceMismatch);
            };
            let intent = {
                let engine = self.storage.read().map_err(|e| failed(e.to_string()))?;
                let namespace = engine
                    .get_namespace(namespace_id)?
                    .ok_or(StorageError::NamespaceNotFound(namespace_id.get()))?;
                engine.meta_store().capture_native_store(
                    LifecycleScope {
                        namespace_id,
                        namespace_name: namespace.name,
                        source: None,
                    },
                    &input,
                    &crate::graph::native_selection::freeze_settings(&settings)?,
                    &policy,
                )?
            };
            let generated = if input.insert.memory.embedding.is_some() {
                GeneratedEmbedding {
                    values: vec![],
                    profile: None,
                }
            } else {
                crate::embedding::runtime::generate_native(
                    provider.as_ref(),
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
                let engine = self.storage.read().map_err(|e| failed(e.to_string()))?;
                let selected = crate::graph::native_selection::capture(
                    &engine,
                    intent.insert_intent(),
                    &generated,
                )?;
                engine.meta_store().prepare_native_store_links(
                    &intent,
                    &selected.frozen_plan(&intent)?,
                    &policy,
                )?
            };
            let lease = publication_gate(&self.storage).await?.read_owned().await;
            let graph = self.graph.clone().write_owned().await;
            let storage = self.storage.clone();
            let child = work.child()?;
            tokio::task::spawn_blocking(move || {
                // Keep both leases in the worker through commit even if its
                // awaiting task is cancelled. Never await graph under storage.
                let _lease = lease;
                let _graph = graph;
                let mut engine = storage.write().map_err(|e| failed(e.to_string()))?;
                let result = engine.store_native_prepared(
                    &ctx,
                    &prepared,
                    &generated,
                    &policy,
                    clock.as_ref(),
                    Some(&child),
                );
                engine.meta_store().finish_work(child);
                result
            })
            .await
            .map_err(|e| {
                failed(format!(
                    "native single worker outcome unknown; reconcile before retry: {e}"
                ))
            })??
        };
        self.complete_with_work(Some(work))
            .await
            .map_err(|source| StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            })?;
        Ok(receipt)
    }
}
