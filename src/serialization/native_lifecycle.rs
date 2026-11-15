//! Strict prepared native lifecycle wire. Public contract selection remains gated.
use super::contract::*;
use crate::{
    model::{MemoryId, NamespaceId},
    storage::{
        StorageError,
        control::{MutationOutcome, WriteReceipt},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    Forget,
    Feedback,
}
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// Original application contract, independent of record/lifecycle schema.
    pub original_contract_version: u32,
    /// Entry capability, never inferred from application version or disk schema.
    /// Old private native-only aliases must reject legacy storage before writing.
    pub legacy_aware: bool,
    pub id: MemoryId,
    pub namespace: Option<String>,
    pub namespace_id: Option<NamespaceId>,
    pub quality: u8,
    pub options: WriteOptions,
}
fn invalid(field: &str, message: &'static str) -> InputError {
    InputError {
        code: "INVALID_TYPE",
        field: field.into(),
        message,
    }
}
pub(crate) fn parse(
    value: &Value,
    operation: Operation,
    mcp: bool,
    header_key: Option<&str>,
) -> Result<Request, InputError> {
    if mcp && ContractVersion::from_mcp(value)? != ContractVersion::V2 {
        return Err(invalid(
            "/contractVersion",
            "explicit contractVersion 2 required",
        ));
    }
    let mut request = parse_request(value, operation, mcp, header_key, 2)?;
    request.legacy_aware = true;
    Ok(request)
}

/// Private RPCs carry the original selector explicitly; no schema-derived default.
pub(crate) fn parse_private(value: &Value, operation: Operation) -> Result<Request, InputError> {
    if value.get("contractVersion").is_none() {
        return Err(invalid(
            "/contractVersion",
            "explicit contractVersion required",
        ));
    }
    let origin = ContractVersion::from_mcp(value)?.number();
    parse_request(value, operation, true, None, origin)
}

pub(crate) fn parse_lifecycle_private(
    value: &Value,
    operation: Operation,
) -> Result<Request, InputError> {
    let mut request = parse_private(value, operation)?;
    request.legacy_aware = true;
    Ok(request)
}

fn parse_request(
    value: &Value,
    operation: Operation,
    mcp: bool,
    header_key: Option<&str>,
    original_contract_version: u32,
) -> Result<Request, InputError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("", "expected an object"))?;
    let mut fields = vec![
        "id",
        "namespace",
        "namespaceId",
        "storeId",
        "idempotencyKey",
        "expectedGeneration",
    ];
    if operation == Operation::Feedback {
        fields.push("quality");
    }
    if mcp {
        fields.push("contractVersion");
    }
    reject_unknown_fields(object, &fields)?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .and_then(|v| Uuid::parse_str(v).ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(|| invalid("/id", "id must be a non-nil UUID string"))?;
    let namespace = object
        .get("namespace")
        .map(|v| {
            let name = v
                .as_str()
                .ok_or_else(|| invalid("/namespace", "namespace must be a string"))?;
            crate::model::validation::validate_namespace_name(name)
                .map_err(|_| invalid("/namespace", "invalid namespace name"))?;
            Ok(name.to_owned())
        })
        .transpose()?;
    let namespace_id = object
        .get("namespaceId")
        .map(|v| {
            v.as_u64()
                .filter(|v| (1..=u32::MAX as u64).contains(v))
                .map(|v| NamespaceId::new(v as u32))
                .ok_or_else(|| {
                    invalid(
                        "/namespaceId",
                        "namespaceId must be an integer in 1..4294967295",
                    )
                })
        })
        .transpose()?;
    let quality = object
        .get("quality")
        .map(|v| {
            v.as_u64()
                .filter(|v| (1..=4).contains(v))
                .map(|v| v as u8)
                .ok_or_else(|| invalid("/quality", "quality must be an integer in 1..4"))
        })
        .transpose()?
        .unwrap_or(3);
    Ok(Request {
        original_contract_version,
        legacy_aware: false,
        id: id.into(),
        namespace,
        namespace_id,
        quality,
        options: WriteOptions::parse(object, header_key)?,
    })
}
impl Request {
    pub(crate) fn rpc_method(&self, operation: Operation) -> &'static str {
        match (self.legacy_aware, operation) {
            (false, Operation::Forget) => "forget_native_v1",
            (false, Operation::Feedback) => "feedback_native_v1",
            (true, Operation::Forget) => "forget_lifecycle_v1",
            (true, Operation::Feedback) => "feedback_lifecycle_v1",
        }
    }
    pub(crate) fn wire(&self, operation: Operation) -> Value {
        let mut value = json!({"contractVersion":self.original_contract_version,"id":self.id});
        let object = value.as_object_mut().unwrap();
        if operation == Operation::Feedback {
            object.insert("quality".into(), json!(self.quality));
        }
        for (name, value) in [
            ("namespace", self.namespace.as_ref().map(|v| json!(v))),
            ("namespaceId", self.namespace_id.map(|v| json!(v))),
            ("storeId", self.options.store_id.map(|v| json!(v))),
            (
                "idempotencyKey",
                self.options.idempotency_key.as_ref().map(|v| json!(v)),
            ),
            (
                "expectedGeneration",
                self.options.expected_generation.map(|v| json!(v)),
            ),
        ] {
            if let Some(value) = value {
                object.insert(name.into(), value);
            }
        }
        value
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Data {
    pub id: MemoryId,
    pub lifecycle_version: crate::model::schema::LifecycleVersion,
    pub policy_revision: crate::storage::control::Generation,
    pub reinforced_at: Option<i64>,
    pub phase: crate::model::DecayPhase,
    pub strength: f32,
    pub stability: f32,
    pub is_permastore: bool,
    pub historical: bool,
}
/// Only historical fields actually present in legacy receipts. No current row
/// is presented as the numerical result of a past operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum LegacyData {
    Feedback {
        id: MemoryId,
        quality: u8,
        #[serde(rename = "appliedAt")]
        applied_at: i64,
        historical: bool,
    },
    Forget {
        id: MemoryId,
        status: crate::storage::tombstone::TombstoneStatus,
        historical: bool,
    },
}
impl LegacyData {
    fn from_receipt(receipt: &WriteReceipt, id: MemoryId) -> Option<Self> {
        match &receipt.outcome {
            MutationOutcome::LegacyFeedbackRecorded {
                memory_id,
                quality,
                applied_at,
            } if *memory_id == id => Some(Self::Feedback {
                id,
                quality: *quality,
                applied_at: *applied_at,
                historical: receipt.replayed,
            }),
            MutationOutcome::MemoryTombstoned { memory_id, status } if *memory_id == id => {
                Some(Self::Forget {
                    id,
                    status: *status,
                    historical: receipt.replayed,
                })
            }
            _ => None,
        }
    }
}

/// Private RPC payload keeps typed receipts/errors; adapters never parse diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Reply {
    pub scope: Option<(NamespaceId, String)>,
    pub data: Option<Data>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_data: Option<LegacyData>,
    pub receipt: Option<WriteReceipt>,
    pub error: Option<ExecutionError>,
    pub readiness: Option<Readiness>,
}
impl Reply {
    pub(crate) fn input(error: InputError) -> Self {
        Self::failure(ExecutionError {
            category: ErrorCategory::InvalidInput,
            code: error.code.into(),
            message: error.message.into(),
            field: Some(error.field),
            retryable: false,
            commit_state: CommitState::NotCommitted,
            operation_id: None,
        })
    }
    pub(crate) fn failure(error: ExecutionError) -> Self {
        Self {
            scope: None,
            data: None,
            legacy_data: None,
            receipt: None,
            error: Some(error),
            readiness: None,
        }
    }
    pub(crate) fn unavailable(message: &str) -> Self {
        Self::failure(ExecutionError {
            category: ErrorCategory::Unavailable,
            code: "UNAVAILABLE".into(),
            message: message.into(),
            field: None,
            retryable: false,
            commit_state: CommitState::NotCommitted,
            operation_id: None,
        })
    }
    pub(crate) fn storage(error: StorageError) -> Self {
        use StorageError::*;
        if let CommittedMutation { receipt, source } = error {
            let mut reply = Self::storage(*source);
            let error = reply.error.as_mut().unwrap();
            error.commit_state = CommitState::Committed;
            error.operation_id = Some(receipt.operation_id);
            error.retryable = false;
            reply.receipt = Some(*receipt);
            return reply;
        }
        let (category, code, state, operation_id) = match &error {
            StoreLineageMismatch => (
                ErrorCategory::Conflict,
                "STORE_LINEAGE_MISMATCH",
                CommitState::NotCommitted,
                None,
            ),
            GenerationConflict { .. } => (
                ErrorCategory::Conflict,
                "GENERATION_CONFLICT",
                CommitState::NotCommitted,
                None,
            ),
            IdempotencyConflict => (
                ErrorCategory::Conflict,
                "IDEMPOTENCY_CONFLICT",
                CommitState::NotCommitted,
                None,
            ),
            NamespaceMismatch { .. } | MutationNamespaceMismatch => (
                ErrorCategory::Conflict,
                "NAMESPACE_MISMATCH",
                CommitState::NotCommitted,
                None,
            ),
            LifecycleContentUnavailable => (
                ErrorCategory::Conflict,
                "CONTENT_UNAVAILABLE",
                CommitState::NotCommitted,
                None,
            ),
            LifecycleConflict(_) => (
                ErrorCategory::Conflict,
                "LIFECYCLE_CONFLICT",
                CommitState::NotCommitted,
                None,
            ),
            NotFound(_) | NamespaceNotFound(_) => (
                ErrorCategory::NotFound,
                "NOT_FOUND",
                CommitState::NotCommitted,
                None,
            ),
            InvalidMutationContext(_) | InvalidMutation(_) => (
                ErrorCategory::InvalidInput,
                "INVALID_INPUT",
                CommitState::NotCommitted,
                None,
            ),
            WorkerOutcomeUnknown(_) => (
                ErrorCategory::Unavailable,
                "COMMIT_OUTCOME_UNKNOWN",
                CommitState::Unknown,
                None,
            ),
            CommitOutcomeUnknown { operation_id, .. } => (
                ErrorCategory::Unavailable,
                "COMMIT_OUTCOME_UNKNOWN",
                CommitState::Unknown,
                Some(*operation_id),
            ),
            CommittedEffectsPending { operation_id, .. } => (
                ErrorCategory::Unavailable,
                "INDEX_NOT_READY",
                CommitState::Committed,
                Some(*operation_id),
            ),
            _ => (
                ErrorCategory::Unavailable,
                "UNAVAILABLE",
                CommitState::NotCommitted,
                None,
            ),
        };
        Self::failure(ExecutionError {
            category,
            code: code.into(),
            message: error.to_string(),
            field: None,
            retryable: false,
            commit_state: state,
            operation_id,
        })
    }
    pub(crate) fn committed(
        scope: (NamespaceId, String),
        receipt: WriteReceipt,
        id: MemoryId,
    ) -> Self {
        if let Some(legacy_data) = LegacyData::from_receipt(&receipt, id) {
            return Self {
                scope: Some(scope),
                data: None,
                legacy_data: Some(legacy_data),
                receipt: Some(receipt),
                error: None,
                readiness: None,
            };
        }
        let after = match &receipt.outcome {
            MutationOutcome::LifecycleFeedbackApplied { transition } => transition
                .records
                .iter()
                .find(|r| r.memory_id == id)
                .map(|r| &r.after),
            MutationOutcome::NativeMemoryTombstoned { transition } => transition
                .records
                .iter()
                .find(|r| r.memory_id == id)
                .map(|r| &r.after.lifecycle),
            _ => None,
        };
        let Some(after) = after else {
            let mut reply = Self::unavailable("unexpected native lifecycle receipt");
            reply.error.as_mut().unwrap().commit_state = CommitState::Committed;
            reply.error.as_mut().unwrap().operation_id = Some(receipt.operation_id);
            reply.receipt = Some(receipt);
            return reply;
        };
        let data = Data {
            id,
            lifecycle_version: after.lifecycle.lifecycle_version,
            policy_revision: after.lifecycle.policy_revision,
            reinforced_at: after.lifecycle.reinforced_at,
            phase: after.phase,
            strength: after.strength.get(),
            stability: after.stability.get(),
            is_permastore: after.is_permastore,
            historical: receipt.replayed,
        };
        Self {
            scope: Some(scope),
            data: Some(data),
            legacy_data: None,
            receipt: Some(receipt),
            error: None,
            readiness: None,
        }
    }
    pub(crate) fn valid_response(&self, request: &Request, operation: Operation) -> bool {
        if let Some(error) = &self.error {
            return self.data.is_none()
                && self.legacy_data.is_none()
                && self.readiness.is_none()
                && (error.commit_state == CommitState::Committed) == self.receipt.is_some()
                && self.receipt.as_ref().is_none_or(|r| {
                    error.commit_state == CommitState::Committed
                        && error.operation_id == Some(r.operation_id)
                        && Self::receipt_matches(r, request, operation)
                });
        }
        let (Some(scope), Some(receipt), Some(ready)) =
            (&self.scope, &self.receipt, &self.readiness)
        else {
            return false;
        };
        if scope.0.is_unset()
            || !Self::receipt_matches(receipt, request, operation)
            || request.namespace_id.is_some_and(|id| id != scope.0)
            || request
                .namespace
                .as_ref()
                .is_some_and(|name| !name.eq_ignore_ascii_case(&scope.1))
            || request
                .options
                .store_id
                .is_some_and(|id| id != receipt.store_id)
            || !ready.ready
            || ready.store_id != receipt.store_id
            || ready.committed_generation < receipt.committed_generation
        {
            return false;
        }
        if let Some(legacy) = &self.legacy_data {
            let operation_matches = match (legacy, operation) {
                (LegacyData::Feedback { quality, .. }, Operation::Feedback) => {
                    *quality == request.quality
                }
                (LegacyData::Forget { .. }, Operation::Forget) => true,
                _ => false,
            };
            return request.legacy_aware
                && self.data.is_none()
                && LegacyData::from_receipt(receipt, request.id).as_ref() == Some(legacy)
                && operation_matches;
        }
        let Some(data) = &self.data else {
            return false;
        };
        if data.id != request.id {
            return false;
        }
        match (&receipt.outcome, operation) {
            (MutationOutcome::LifecycleFeedbackApplied { transition }, Operation::Feedback) => {
                transition.namespace_id == scope.0
                    && transition.records.len() == 1
                    && transition.records[0].memory_id == request.id
            }
            (MutationOutcome::NativeMemoryTombstoned { transition }, Operation::Forget) => {
                transition.namespace_id == scope.0 && transition.memory_id == request.id
            }
            _ => false,
        }
    }
    fn receipt_matches(r: &WriteReceipt, request: &Request, operation: Operation) -> bool {
        !r.store_id.is_nil()
            && !r.operation_id.is_nil()
            && r.receipt_version == 1
            && request.options.store_id.is_none_or(|id| id == r.store_id)
            && match (&r.outcome, operation) {
                (MutationOutcome::MemoryTombstoned { memory_id, .. }, Operation::Forget) => {
                    request.legacy_aware && *memory_id == request.id
                }
                (
                    MutationOutcome::LegacyFeedbackRecorded {
                        memory_id, quality, ..
                    },
                    Operation::Feedback,
                ) => {
                    request.legacy_aware && *memory_id == request.id && *quality == request.quality
                }
                (MutationOutcome::NativeMemoryTombstoned { transition }, Operation::Forget) => {
                    transition.memory_id == request.id
                        && request
                            .namespace_id
                            .is_none_or(|id| id == transition.namespace_id)
                }
                (MutationOutcome::LifecycleFeedbackApplied { transition }, Operation::Feedback) => {
                    transition.records.len() == 1
                        && transition.records[0].memory_id == request.id
                        && transition
                            .feedback
                            .as_ref()
                            .is_some_and(|f| f.quality == request.quality)
                        && request
                            .namespace_id
                            .is_none_or(|id| id == transition.namespace_id)
                }
                _ => false,
            }
    }
    pub(crate) fn decode_response(value: Value, request: &Request, operation: Operation) -> Self {
        let receipt: Option<WriteReceipt> = value
            .get("receipt")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok());
        match serde_json::from_value::<Self>(value) {
            Ok(reply) if reply.valid_response(request, operation) => reply,
            _ => {
                let mut reply =
                    Self::unavailable("lifecycle response invalid; reconcile original intent");
                let error = reply.error.as_mut().unwrap();
                error.code = "INVALID_RESPONSE".into();
                error.commit_state = CommitState::Unknown;
                if let Some(receipt) =
                    receipt.filter(|r| Self::receipt_matches(r, request, operation))
                {
                    error.commit_state = CommitState::Committed;
                    error.operation_id = Some(receipt.operation_id);
                    reply.receipt = Some(receipt);
                }
                reply
            }
        }
    }
    pub(crate) fn status_code(&self) -> u16 {
        match self.error.as_ref().map(|e| e.category) {
            None => 200,
            Some(ErrorCategory::InvalidInput) => 400,
            Some(ErrorCategory::NotFound) => 404,
            Some(ErrorCategory::Conflict) => 409,
            Some(ErrorCategory::Unavailable) => 503,
            Some(ErrorCategory::Partial) => 207,
            Some(ErrorCategory::Internal) => 500,
        }
    }
    pub(crate) fn envelope(self) -> Envelope<Value> {
        let scope = self
            .scope
            .map(|(namespace_id, namespace)| Scope::Namespace {
                namespace_id,
                namespace,
            });
        let mut envelope = if let Some(error) = self.error {
            Envelope::failure(scope, error)
        } else {
            // A successful native operation always has a verified scope and outcome.
            Envelope::success(
                scope.expect("native scope"),
                match (self.data, self.legacy_data) {
                    (Some(data), None) => serde_json::to_value(data).expect("native outcome"),
                    (None, Some(data)) => serde_json::to_value(data).expect("legacy outcome"),
                    _ => unreachable!("lifecycle success requires exactly one validated outcome"),
                },
            )
        };
        envelope.receipt = self.receipt;
        envelope.readiness = self.readiness;
        envelope
    }
    pub(crate) fn mcp(self) -> crate::mcp::protocol::ToolCallResult {
        let failed = self.error.is_some();
        let mut result = crate::mcp::protocol::ToolCallResult::json(&self.envelope())
            .unwrap_or_else(|_| crate::mcp::protocol::ToolCallResult::error("Serialization error"));
        if failed {
            result.is_error = Some(true);
        }
        result
    }
    pub(crate) fn http(self) -> axum::response::Response {
        use axum::response::IntoResponse;
        (
            axum::http::StatusCode::from_u16(self.status_code()).unwrap(),
            axum::Json(self.envelope()),
        )
            .into_response()
    }
}
