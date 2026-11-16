//! Standalone graph writes by the existing owner. This is not insertion autolink.
use super::*;
use crate::graph::native_producer::NativeLinkSelection;
use crate::storage::control::{MutationContext, MutationScope};
use crate::storage::metadata::effects::EffectResult;
use crate::storage::native_graph::NativeGraphInput;
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs};
use crate::time::Clock;

pub(crate) enum GraphRequest {
    Explicit(NativeGraphInput),
    Automatic(NativeLinkSelection),
}
impl DerivedCompletion {
    pub(crate) async fn native_graph(
        &self,
        ctx: MutationContext,
        request: GraphRequest,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<EffectResult>, StorageError> {
        ctx.validate()?;
        if matches!(request, GraphRequest::Automatic(_)) && ctx.idempotency_key.is_some() {
            return Err(StorageError::InvalidMutationContext(
                "automatic graph selection is unkeyed; supply frozen edges for keyed replay".into(),
            ));
        }
        let work = self.accept_work(parent)?;
        let result = self
            .native_graph_accepted(ctx, request, policy, clock, &work)
            .await;
        self.finish_known_work(work);
        result
    }
    async fn native_graph_accepted(
        &self,
        ctx: MutationContext,
        request: GraphRequest,
        policy: NativePolicyInputs,
        clock: Arc<dyn Clock>,
        work: &WorkTicket,
    ) -> Result<Option<EffectResult>, StorageError> {
        let lease = publication_gate(&self.storage).await?.read_owned().await;
        let graph = self.graph.clone().write_owned().await;
        let storage = self.storage.clone();
        let child = work.child()?;
        let result = tokio::task::spawn_blocking(move || {
            // Cancellation of the async waiter cannot release worker ownership.
            let _lease = lease;
            let _graph = graph;
            let mut engine = storage.write().map_err(|e| failed(e.to_string()))?;
            let result = (|| {
                let MutationScope::Namespace(namespace_id) = ctx.scope else {
                    return Err(StorageError::MutationNamespaceMismatch);
                };
                let (input, selected) = match request {
                    GraphRequest::Explicit(input) => {
                        if let Some(receipt) =
                            engine
                                .meta_store()
                                .lookup_native_graph(&ctx, &input, Some(&child))?
                        {
                            return crate::storage::metadata::native_graph::replay_result(receipt)
                                .map(Some);
                        }
                        (input, None)
                    }
                    GraphRequest::Automatic(selection) => {
                        // Existing native context/lineage/CAS admission before selection.
                        engine.meta_store().lookup_native_graph(
                            &ctx,
                            &NativeGraphInput { edges: vec![] },
                            Some(&child),
                        )?;
                        let (input, at) = crate::graph::native_producer::select(
                            &engine,
                            namespace_id,
                            &selection,
                        )?;
                        (input, Some(at))
                    }
                };
                let ns = engine
                    .get_namespace(namespace_id)?
                    .ok_or(StorageError::NamespaceNotFound(namespace_id.get()))?;
                let mut intent = engine.meta_store().capture_native_graph(
                    LifecycleScope {
                        namespace_id,
                        namespace_name: ns.name,
                        source: None,
                    },
                    &input,
                    &policy,
                    Some(&child),
                )?;
                if selected.is_some_and(|at| at != intent.capture_generation()) {
                    return Err(StorageError::LifecycleConflict(
                        "native graph changed after selection".into(),
                    ));
                }
                intent.selection_generation = selected;
                engine.apply_native_graph(&ctx, &intent, &policy, clock.as_ref(), Some(&child))
            })();
            engine.meta_store().finish_work(child);
            result
        })
        .await
        .map_err(|e| {
            failed(format!(
                "native graph worker outcome unknown; reconcile before retry: {e}"
            ))
        })??;
        // Even an unkeyed no-change result requires the actual current views.
        self.complete_with_work(Some(work))
            .await
            .map_err(|source| match &result {
                Some(committed) => StorageError::CommittedMutation {
                    receipt: Box::new(committed.receipt.clone()),
                    source: Box::new(source),
                },
                None => source,
            })?;
        Ok(result)
    }
}
