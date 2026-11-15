//! One prepared lifecycle consumer for direct MCP, private RPC and REST adapters.
use crate::{
    derived_completion::DerivedCompletion,
    serialization::{
        contract::{DerivedReadiness, Readiness},
        native_lifecycle::{Operation, Reply, Request},
    },
    storage::{
        StorageEngine, StorageError,
        control::{GenerationRef, MutationContext, MutationScope},
        native_lifecycle::NativePolicyInputs,
    },
    time::Clock,
};
use std::sync::Arc;

pub(crate) async fn execute(
    owner: &DerivedCompletion,
    request: Request,
    operation: Operation,
    policy: &NativePolicyInputs,
    clock: Arc<dyn Clock>,
) -> Reply {
    execute_with_projection(owner, request, operation, policy, clock, None).await
}

pub(crate) async fn execute_with_projection(
    owner: &DerivedCompletion,
    request: Request,
    operation: Operation,
    policy: &NativePolicyInputs,
    clock: Arc<dyn Clock>,
    legacy_projection: Option<&mut Option<crate::decay::lifecycle::FeedbackPlan>>,
) -> Reply {
    let work = match owner.accept_work(None) {
        Ok(work) => work,
        Err(e) => return Reply::storage(e),
    };
    // Scope resolution belongs to this accepted root, before any enqueue/await.
    // Never use connection defaults or a current name lookup for a by-ID retry.
    let storage = owner.storage.clone();
    let child = match work.child() {
        Ok(child) => child,
        Err(e) => {
            owner.finish_known_work(work);
            return Reply::storage(e.into());
        }
    };
    let requested = request.clone();
    let resolved = tokio::task::spawn_blocking(move || {
        let storage = storage
            .read()
            .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
        let meta = storage.meta_store();
        let result = (|| {
            if !requested.legacy_aware && !meta.uses_native_lifecycle() {
                return Err(StorageError::RecoveryRequired(
                    "native-only lifecycle entry requires native storage".into(),
                ));
            }
            let _guard = meta.lifecycle_guard_with_work(Some(&child))?;
            let record = storage.get_record(requested.id)?;
            let canonical = if let Some(record) = record {
                Some(crate::model::NamespaceId::new(record.namespace_id))
            } else {
                // ponytail: rare deleted-row lookup scans retained payload-free ledger.
                // A UUID index can replace this only if measured lookup cost warrants it.
                let mut namespaces = std::collections::BTreeSet::new();
                for entry in meta.removal_ledger_export()?.entries {
                    if entry.memory_uuid == requested.id.into_inner()
                        && entry.terminal_kind.is_some()
                    {
                        namespaces.insert(entry.namespace_id);
                    }
                }
                if let Some(asserted) = requested.namespace_id {
                    if !namespaces.is_empty() && !namespaces.contains(&asserted.get()) {
                        return Err(StorageError::MutationNamespaceMismatch);
                    }
                    Some(asserted)
                } else {
                    if namespaces.len() > 1 {
                        return Err(StorageError::MutationNamespaceMismatch);
                    }
                    namespaces
                        .first()
                        .copied()
                        .map(crate::model::NamespaceId::new)
                }
            };
            if let (Some(actual), Some(asserted)) = (canonical, requested.namespace_id) {
                if actual != asserted {
                    return Err(StorageError::MutationNamespaceMismatch);
                }
            }
            let namespace = canonical
                .or(requested.namespace_id)
                .ok_or(StorageError::NotFound(requested.id.into_inner()))?;
            Ok((namespace, meta.uses_native_lifecycle()))
        })();
        meta.finish_work(child);
        result
    })
    .await;
    let (scope, native) = match resolved {
        Ok(Ok(scope)) => scope,
        other => {
            owner.finish_known_work(work);
            return Reply::storage(match other {
                Ok(Err(e)) => e,
                Err(e) => StorageError::RecoveryRequired(e.to_string()),
                _ => unreachable!(),
            });
        }
    };
    let context = MutationContext {
        store_id: request.options.store_id,
        scope: MutationScope::Namespace(scope),
        idempotency_key: request.options.idempotency_key.clone(),
        expected_generation: request.options.expected_generation,
        contract_version: request.original_contract_version,
    };
    let result = if !native {
        owner
            .legacy_lifecycle_projected_with_work(
                context,
                request.id,
                (operation == Operation::Feedback).then_some(request.quality),
                request.namespace,
                policy.clone(),
                clock,
                Some(&work),
                legacy_projection.is_some(),
            )
            .await
    } else {
        match operation {
            Operation::Forget => owner
                .forget_native_with_work(
                    context,
                    request.id,
                    request.namespace,
                    policy.clone(),
                    clock,
                    Some(&work),
                )
                .await
                .map(|receipt| (receipt, None)),
            Operation::Feedback => owner
                .feedback_native_with_work(
                    context,
                    request.id,
                    request.quality,
                    request.namespace,
                    policy.clone(),
                    clock,
                    Some(&work),
                )
                .await
                .map(|receipt| (receipt, None)),
        }
    };
    let receipt = match result {
        Ok((receipt, view)) => {
            if let Some(output) = legacy_projection {
                *output = view;
            }
            receipt
        }
        Err(e) => {
            owner.finish_known_work(work);
            return Reply::storage(e);
        }
    };
    // Name proof follows known receipt lookup: a damaged retired binding must
    // not erase the original committed outcome. Never infer a reused name.
    let storage = owner.storage.clone();
    let child = match work.child() {
        Ok(child) => child,
        Err(source) => {
            owner.finish_known_work(work);
            return Reply::storage(StorageError::CommittedMutation {
                receipt: Box::new(receipt),
                source: Box::new(source.into()),
            });
        }
    };
    let binding = tokio::task::spawn_blocking(move || {
        let storage = storage
            .read()
            .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
        let meta = storage.meta_store();
        let result = (|| {
            let guard = meta.lifecycle_guard_with_work(Some(&child))?;
            meta.lifecycle_namespace_name(&guard, scope)?
                .map(|name| (scope, name))
                .ok_or(StorageError::NamespaceNotFound(scope.get()))
        })();
        meta.finish_work(child);
        result
    })
    .await;
    owner.finish_known_work(work);
    let scope = match binding {
        Ok(Ok(scope)) => scope,
        other => {
            return Reply::storage(StorageError::CommittedMutation {
                receipt: Box::new(receipt),
                source: Box::new(match other {
                    Ok(Err(e)) => e,
                    Err(e) => StorageError::RecoveryRequired(e.to_string()),
                    _ => unreachable!(),
                }),
            });
        }
    };
    // Drain only after the producer root finishes. This is a current registry
    // report, never the historical receipt generation relabeled as current.
    let target = GenerationRef {
        store_id: receipt.store_id,
        generation: receipt.committed_generation,
    };
    let report = owner
        .readiness_barrier(
            target,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await;
    match report {
        Ok(report) => {
            let mut reply = Reply::committed(scope, receipt, request.id);
            reply.readiness = Some(Readiness {
                store_id: report.store_id,
                committed_generation: report.committed_generation,
                ready: report.ready,
                pending_work: report.pending_work,
                derived: report
                    .derived
                    .into_iter()
                    .map(|d| DerivedReadiness {
                        name: d.name.into(),
                        applied_generation: d.applied_generation,
                        ready: d.ready,
                    })
                    .collect(),
            });
            reply
        }
        Err(source) => {
            let mut reply = Reply::storage(StorageError::CommittedMutation {
                receipt: Box::new(receipt),
                source: Box::new(source),
            });
            reply.scope = Some(scope);
            reply
        }
    }
}

pub(crate) async fn dispatch(
    storage: &dyn super::bridge::StorageEngine,
    operation: Operation,
    arguments: serde_json::Value,
) -> Reply {
    dispatch_parsed(
        storage,
        operation,
        crate::serialization::native_lifecycle::parse(&arguments, operation, true, None),
    )
    .await
}

pub(crate) async fn dispatch_private(
    storage: &dyn super::bridge::StorageEngine,
    operation: Operation,
    arguments: serde_json::Value,
) -> Reply {
    dispatch_parsed(
        storage,
        operation,
        crate::serialization::native_lifecycle::parse_private(&arguments, operation),
    )
    .await
}

async fn dispatch_parsed(
    storage: &dyn super::bridge::StorageEngine,
    operation: Operation,
    parsed: Result<Request, crate::serialization::contract::InputError>,
) -> Reply {
    match parsed {
        Ok(request) => match operation {
            Operation::Forget => storage.forget_native(request).await,
            Operation::Feedback => storage.feedback_native(request).await,
        },
        Err(error) => Reply::input(error),
    }
}

/// Explicit new capability; old native-only aliases keep their old rejection.
pub(crate) async fn dispatch_lifecycle_private(
    storage: &dyn super::bridge::StorageEngine,
    operation: Operation,
    arguments: serde_json::Value,
) -> Reply {
    dispatch_parsed(
        storage,
        operation,
        crate::serialization::native_lifecycle::parse_lifecycle_private(&arguments, operation),
    )
    .await
}
