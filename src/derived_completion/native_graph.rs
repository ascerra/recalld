//! Receipt-preserving native graph publication by the actual owner. This is
//! standalone graph work; it cannot make insert-then-autolink crash durable.
use super::*;
use crate::storage::control::{MutationContext, MutationScope};
use crate::storage::metadata::effects::EffectResult;
use crate::storage::native_graph::NativeGraphInput;
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs};
use crate::time::Clock;

impl DerivedCompletion {
    pub(crate) async fn native_graph(
        &self,
        ctx: MutationContext,
        input: NativeGraphInput,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<EffectResult>, StorageError> {
        // Admission precedes waiting for either publication or graph ownership.
        let work = self.accept_work(parent)?;
        let result = self
            .native_graph_accepted(ctx, input, policy, clock, &work)
            .await;
        self.finish_known_work(work);
        result
    }

    async fn native_graph_accepted(
        &self,
        ctx: MutationContext,
        input: NativeGraphInput,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        work: &WorkTicket,
    ) -> Result<Option<EffectResult>, StorageError> {
        let gate = publication_gate(&self.storage).await?;
        let lease = gate.read_owned().await;
        let graph = self.graph.clone().write_owned().await;
        let storage = self.storage.clone();
        let child = work.child()?;
        let result = tokio::task::spawn_blocking(move || {
            let mut engine = storage.write().map_err(|e| failed(e.to_string()))?;
            let result = (|| {
                // No current namespace/policy/clock evaluation before replay.
                if let Some(receipt) =
                    engine
                        .meta_store()
                        .lookup_native_graph(&ctx, &input, Some(&child))?
                {
                    return crate::storage::metadata::native_graph::replay_result(receipt)
                        .map(Some);
                }
                let MutationScope::Namespace(namespace_id) = ctx.scope else {
                    return Err(StorageError::MutationNamespaceMismatch);
                };
                let namespace = engine
                    .get_namespace(namespace_id)?
                    .ok_or(StorageError::NamespaceNotFound(namespace_id.get()))?;
                let intent = engine.meta_store().capture_native_graph(
                    LifecycleScope {
                        namespace_id,
                        namespace_name: namespace.name,
                        source: None,
                    },
                    &input,
                    &policy,
                    Some(&child),
                )?;
                engine.apply_native_graph(&ctx, &intent, &policy, clock.as_ref(), Some(&child))
            })();
            drop(graph);
            engine.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|e| failed(format!("native graph worker outcome unknown: {e}")))?;
        drop(lease);
        let result = result?;
        if let Some(committed) = &result {
            self.complete_with_work(Some(work))
                .await
                .map_err(|source| StorageError::CommittedMutation {
                    receipt: Box::new(committed.receipt.clone()),
                    source: Box::new(source),
                })?;
        }
        Ok(result)
    }
}
