//! API1 projections over existing lifecycle owners; historical receipts stay unchanged.
use super::{
    contract::*,
    native_lifecycle::{self as native, Operation, Request},
};
use crate::{
    mcp::bridge::ReinforceResult,
    model::{DecayPhase, MemoryId},
    storage::{StorageError, control::MutationOutcome},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Value {
    Feedback { result: ReinforceResult },
    Forget { deleted: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Reply {
    pub value: Option<Value>,
    /// None only for the explicitly selected old adapter/peer compatibility lane.
    pub lifecycle: Option<native::Reply>,
}

pub(crate) fn request(id: MemoryId, quality: u8, namespace: Option<&str>) -> Request {
    Request {
        original_contract_version: 1,
        legacy_aware: true,
        id,
        quality,
        namespace: namespace.map(str::to_owned),
        namespace_id: None,
        options: WriteOptions::default(),
    }
}
pub(crate) fn parse(value: &Json, operation: Operation) -> Result<Request, InputError> {
    let invalid = |field: &str, message: &'static str| InputError {
        code: "INVALID_INPUT",
        field: field.into(),
        message,
    };
    let object = value
        .as_object()
        .ok_or_else(|| invalid("", "expected an object"))?;
    let fields: &[&str] = if operation == Operation::Feedback {
        &["contractVersion", "id", "quality", "namespace"]
    } else {
        &["contractVersion", "id", "namespace"]
    };
    reject_unknown_fields(object, fields)?;
    if value.get("contractVersion").and_then(Json::as_u64) != Some(1) {
        return Err(invalid(
            "/contractVersion",
            "API1 lifecycle requires explicit original1",
        ));
    }
    // API1 historically accepts every syntactically valid UUID, including nil
    // (which resolves as missing). Do not tighten it through the private v2 parser.
    let id = crate::mcp::args::require_uuid(value, "id")
        .map_err(|_| invalid("/id", "id must be a UUID string"))?;
    let quality = crate::mcp::args::parse_quality(value)
        .map_err(|_| invalid("/quality", "quality must be an integer in 1..4"))?;
    let namespace = crate::mcp::args::parse_namespace_assertion(value)
        .map_err(|_| invalid("/namespace", "invalid namespace assertion"))?;
    Ok(request(id, quality, namespace))
}

// The current feedback/forget outcomes contain one bounded numeric/hash record,
// no payload or peer expansion. Bound diagnostic strings as well; preserve the
// full receipt and identity. This is not a bound for insertion/hard-delete/batches.
fn bound_error_text(reply: &mut native::Reply) {
    if let Some(error) = reply.error.as_mut() {
        error.message =
            crate::model::validation::truncate_on_char_boundary(&error.message, 1024).to_owned();
    }
}

impl Reply {
    pub(crate) fn compatibility(value: Value) -> Self {
        Self {
            value: Some(value),
            lifecycle: None,
        }
    }
    pub(crate) fn failure(mut reply: native::Reply) -> Self {
        bound_error_text(&mut reply);
        Self {
            value: None,
            lifecycle: Some(reply),
        }
    }
    pub(crate) fn legacy_feedback(
        result: Result<crate::api::ReinforceResult, Box<dyn std::error::Error + Send + Sync>>,
        id: MemoryId,
    ) -> Self {
        match result {
            Ok(p) => Self::compatibility(Value::Feedback {
                result: ReinforceResult {
                    id: id.to_string(),
                    strength: p.strength,
                    stability: p.stability,
                    phase: format!("{:?}", p.phase),
                    is_permastore: p.is_permastore,
                },
            }),
            Err(error) => {
                use crate::decay::lifecycle::LifecycleError;
                let (category, code) = match error.downcast_ref::<LifecycleError>() {
                    Some(LifecycleError::Observation(e)) => {
                        if let Some(receipt) = e.receipts.first() {
                            return Self::failure(native::Reply::storage(
                                StorageError::CommittedMutation {
                                    receipt: Box::new(receipt.clone()),
                                    source: Box::new(StorageError::RecoveryRequired(
                                        e.message.clone(),
                                    )),
                                },
                            ));
                        }
                        return Self::failure(native::Reply::failure(ExecutionError {
                            category: ErrorCategory::Unavailable,
                            code: "UNAVAILABLE".into(),
                            message: e.message.clone(),
                            field: None,
                            retryable: false,
                            commit_state: if e.commit_state == "unknown" {
                                CommitState::Unknown
                            } else {
                                CommitState::NotCommitted
                            },
                            operation_id: e.unknown_operation_id,
                        }));
                    }
                    Some(LifecycleError::NamespaceMismatch(_)) => {
                        (ErrorCategory::Conflict, "NAMESPACE_MISMATCH")
                    }
                    Some(LifecycleError::ContentUnavailable) => {
                        (ErrorCategory::Conflict, "CONTENT_UNAVAILABLE")
                    }
                    Some(LifecycleError::NotFound) => (ErrorCategory::NotFound, "NOT_FOUND"),
                    Some(LifecycleError::InvalidInput(_)) => {
                        (ErrorCategory::InvalidInput, "INVALID_INPUT")
                    }
                    _ => {
                        return Self::bridge_error(crate::mcp::bridge::BridgeError::Internal(
                            error.to_string(),
                        ));
                    }
                };
                Self::failure(native::Reply::failure(ExecutionError {
                    category,
                    code: code.into(),
                    message: error.to_string(),
                    field: None,
                    retryable: false,
                    commit_state: CommitState::NotCommitted,
                    operation_id: None,
                }))
            }
        }
    }
    pub(crate) fn bridge_error(error: crate::mcp::bridge::BridgeError) -> Self {
        use crate::mcp::bridge::BridgeError::*;
        let (category, code, commit_state) = match &error {
            InvalidInput(_) => (
                ErrorCategory::InvalidInput,
                "INVALID_INPUT",
                CommitState::NotCommitted,
            ),
            NotFound(_) => (
                ErrorCategory::NotFound,
                "NOT_FOUND",
                CommitState::NotCommitted,
            ),
            NamespaceMismatch(_) => (
                ErrorCategory::Conflict,
                "NAMESPACE_MISMATCH",
                CommitState::NotCommitted,
            ),
            _ => (
                ErrorCategory::Unavailable,
                "UNAVAILABLE",
                CommitState::Unknown,
            ),
        };
        Self::failure(native::Reply::failure(ExecutionError {
            category,
            code: code.into(),
            message: error.to_string(),
            field: None,
            retryable: false,
            commit_state,
            operation_id: None,
        }))
    }
    pub(crate) fn project(
        request: &Request,
        operation: Operation,
        mut lifecycle: native::Reply,
        fresh: Option<crate::decay::lifecycle::FeedbackPlan>,
    ) -> Self {
        bound_error_text(&mut lifecycle);
        let value = if lifecycle.error.is_some() {
            (operation == Operation::Forget
                && request.namespace.is_none()
                && lifecycle.receipt.is_none()
                && lifecycle.error.as_ref().is_some_and(|e| {
                    e.code == "NOT_FOUND" && e.commit_state == CommitState::NotCommitted
                }))
            .then_some(Value::Forget { deleted: false })
        } else {
            match operation {
                Operation::Feedback => fresh
                    .map(|p| Value::Feedback {
                        result: ReinforceResult {
                            id: request.id.to_string(),
                            strength: p.strength,
                            stability: p.stability,
                            phase: format!("{:?}", p.phase),
                            is_permastore: p.is_permastore,
                        },
                    })
                    .or_else(|| {
                        // Native numerical values come from the typed canonical
                        // outcome, never from a peer's independently supplied view.
                        lifecycle
                            .receipt
                            .as_ref()
                            .and_then(|receipt| {
                                let scope = lifecycle.scope.clone()?;
                                native::Reply::committed(scope, receipt.clone(), request.id).data
                            })
                            .map(|d| Value::Feedback {
                                result: ReinforceResult {
                                    id: d.id.to_string(),
                                    strength: d.strength,
                                    stability: d.stability,
                                    phase: format!("{:?}", d.phase),
                                    is_permastore: d.is_permastore,
                                },
                            })
                    }),
                Operation::Forget => lifecycle.receipt.as_ref().and_then(|r| match &r.outcome {
                    MutationOutcome::MemoryTombstoned { status, .. } => Some(Value::Forget {
                        deleted: *status == crate::storage::tombstone::TombstoneStatus::Tombstoned,
                    }),
                    MutationOutcome::NativeMemoryTombstoned { transition } => transition
                        .records
                        .iter()
                        .find(|r| r.memory_id == request.id)
                        .and_then(|r| r.before.as_ref())
                        .map(|before| Value::Forget {
                            deleted: before.lifecycle.phase != DecayPhase::Tombstone,
                        }),
                    _ => None,
                }),
            }
        };
        Self {
            value,
            lifecycle: Some(lifecycle),
        }
    }
    fn malformed(mut lifecycle: native::Reply) -> Self {
        if let Some(receipt) = lifecycle.receipt.take() {
            lifecycle = native::Reply::storage(StorageError::CommittedMutation {
                receipt: Box::new(receipt),
                source: Box::new(StorageError::InvalidMutation(
                    "invalid API1 projection".into(),
                )),
            });
        } else if lifecycle.error.is_none() {
            lifecycle = native::Reply::storage(StorageError::WorkerOutcomeUnknown(
                "invalid API1 response".into(),
            ));
        }
        if let Some(e) = lifecycle.error.as_mut() {
            e.code = "INVALID_RESPONSE".into();
        }
        Self::failure(lifecycle)
    }
    pub(crate) fn decode(value: Json, request: &Request, operation: Operation) -> Self {
        let lifecycle = native::Reply::decode_response(
            value.get("lifecycle").cloned().unwrap_or(Json::Null),
            request,
            operation,
        );
        let Ok(mut reply) = serde_json::from_value::<Self>(value) else {
            return Self::malformed(lifecycle);
        };
        // The new RPC must carry canonical evidence. Only a preselected old lane
        // may construct a compatibility result without it.
        if reply.lifecycle.is_none() {
            return Self::malformed(lifecycle);
        }
        let valid = match (&reply.value, &lifecycle.error) {
            (None, Some(_)) => true,
            (Some(Value::Forget { deleted: false }), Some(e)) => {
                operation == Operation::Forget
                    && request.namespace.is_none()
                    && lifecycle.receipt.is_none()
                    && e.code == "NOT_FOUND"
                    && e.commit_state == CommitState::NotCommitted
            }
            (Some(Value::Feedback { result }), None) if operation == Operation::Feedback => {
                if lifecycle.data.is_some() {
                    let expected = Self::project(request, operation, lifecycle.clone(), None);
                    serde_json::to_value(&reply.value).ok()
                        == serde_json::to_value(&expected.value).ok()
                } else {
                    lifecycle.receipt.as_ref().is_some_and(|r| {
                        !r.replayed
                            && matches!(r.outcome, MutationOutcome::LegacyFeedbackRecorded { .. })
                    }) && result.id == request.id.to_string()
                        && result.strength == 1.
                        && result.stability.is_finite()
                        && result.stability > 0.
                        && matches!(result.phase.as_str(), "Full" | "Summary")
                }
            }
            (Some(Value::Forget { .. }), None) if operation == Operation::Forget => {
                let expected = Self::project(request, operation, lifecycle.clone(), None);
                serde_json::to_value(&reply.value).ok()
                    == serde_json::to_value(&expected.value).ok()
            }
            _ => false,
        };
        if !valid {
            return Self::malformed(lifecycle);
        }
        reply.lifecycle = Some(lifecycle);
        reply
    }
    pub(crate) fn mcp(self, id_spelling: &str) -> crate::mcp::protocol::ToolCallResult {
        use crate::mcp::protocol::ToolCallResult;
        let value = match self.value {
            Some(Value::Feedback { result }) => serde_json::to_value(result).unwrap(),
            Some(Value::Forget { deleted }) => json!({"id":id_spelling,"deleted":deleted}),
            None => {
                let mut result = ToolCallResult::json(&self.lifecycle).unwrap();
                result.is_error = Some(true);
                return result;
            }
        };
        ToolCallResult::json(&value).unwrap()
    }
    pub(crate) fn http_error(self, id: &str) -> crate::api::AppError {
        use crate::api::AppError;
        let Some(mut execution) = self.lifecycle else {
            return AppError::Internal {
                source: Box::new(StorageError::RecoveryRequired("missing API1 result".into())),
            };
        };
        let error = execution.error.take().unwrap_or(ExecutionError {
            category: ErrorCategory::Unavailable,
            code: "INVALID_RESPONSE".into(),
            message: "missing API1 projection".into(),
            field: None,
            retryable: false,
            commit_state: CommitState::Unknown,
            operation_id: None,
        });
        if let Some(receipt) = execution.receipt {
            return AppError::CommittedMutation {
                receipt,
                message: error.message,
            };
        }
        if error.commit_state == CommitState::Unknown {
            return AppError::Observation(crate::decay::observation::ObservationError {
                cause: None,
                commit_state: "unknown",
                ready: false,
                receipts: vec![],
                unknown_operation_id: error.operation_id,
                message: error.message,
            });
        }
        match error.category {
            ErrorCategory::InvalidInput => AppError::BadRequest {
                message: error.message,
                field: error.field,
            },
            ErrorCategory::NotFound => AppError::NotFound {
                resource: "memory",
                id: id.into(),
            },
            ErrorCategory::Conflict if error.code == "NAMESPACE_MISMATCH" => {
                AppError::NamespaceMismatch {
                    message: error.message,
                }
            }
            ErrorCategory::Conflict => AppError::Conflict {
                message: error.message,
            },
            _ => AppError::from(StorageError::RecoveryRequired(error.message)),
        }
    }
}

pub(crate) async fn execute(
    owner: &crate::derived_completion::DerivedCompletion,
    r: Request,
    op: Operation,
    configured: Option<&crate::storage::native_lifecycle::NativePolicyInputs>,
    legacy_policy: crate::storage::native_lifecycle::NativePolicyInputs,
    clock: std::sync::Arc<dyn crate::time::Clock>,
) -> Reply {
    if let Err(error) = parse(&r.wire(op), op) {
        return Reply::failure(native::Reply::input(error));
    }
    let native = match owner.storage.read() {
        Ok(s) => s.meta_store().uses_native_lifecycle(),
        Err(e) => {
            return Reply::failure(native::Reply::storage(StorageError::RecoveryRequired(
                e.to_string(),
            )));
        }
    };
    if native && configured.is_none() {
        return Reply::failure(native::Reply::unavailable(
            "native lifecycle policy not configured",
        ));
    }
    let mut view = None;
    let result = crate::mcp::native_lifecycle_adapter::execute_with_projection(
        owner,
        r.clone(),
        op,
        configured.unwrap_or(&legacy_policy),
        clock,
        Some(&mut view),
    )
    .await;
    Reply::project(&r, op, result, view)
}

#[cfg(test)]
#[path = "lifecycle_api1_tests.rs"]
mod tests;
