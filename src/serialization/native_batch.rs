//! Actual native insertion batch wire. Public activation remains separately gated.
use super::contract::*;
use crate::{
    mcp::bridge::BridgeError,
    storage::{
        StorageError,
        batch::*,
        control::{Generation, WriteReceipt},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

fn input(code: &'static str, field: &str, message: &'static str) -> InputError {
    InputError {
        code,
        field: field.into(),
        message,
    }
}
pub(crate) fn parse(
    value: &Value,
    mcp: bool,
    header_key: Option<&str>,
    default_namespace: &str,
) -> Result<BatchInput, InputError> {
    let object = value
        .as_object()
        .ok_or_else(|| input("INVALID_TYPE", "", "expected an object"))?;
    let mut allowed = vec![
        "memories",
        "storeId",
        "idempotencyKey",
        "expectedGeneration",
        "batchOperationId",
    ];
    if mcp {
        allowed.push("contractVersion");
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(input(
                "INVALID_COMBINATION",
                "/contractVersion",
                "explicit contractVersion 2 required",
            ));
        }
    }
    reject_unknown_fields(object, &allowed)?;
    let options = WriteOptions::parse(object, header_key)?;
    let items = object
        .get("memories")
        .and_then(Value::as_array)
        .ok_or_else(|| input("INVALID_TYPE", "/memories", "memories must be an array"))?;
    if items.is_empty() || items.len() > crate::model::constants::MAX_BATCH_MEMORIES {
        return Err(input(
            "OUT_OF_RANGE",
            "/memories",
            "batch must contain 1–100 items",
        ));
    }
    let batch_operation_id = object
        .get("batchOperationId")
        .map(|v| {
            v.as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
                .filter(|id| !id.is_nil())
                .ok_or_else(|| {
                    input(
                        "INVALID_TYPE",
                        "/batchOperationId",
                        "batchOperationId must be a non-nil UUID",
                    )
                })
        })
        .transpose()?;
    if batch_operation_id.is_some()
        && (options.store_id.is_none() || options.idempotency_key.is_some())
    {
        return Err(input(
            "INVALID_COMBINATION",
            "/batchOperationId",
            "operation resume requires storeId and no idempotencyKey",
        ));
    }
    Ok(BatchInput {
        request_context: None,
        result_version: 1,
        mutation_version: 3,
        kind: BatchKind::Insert,
        items: items.clone(),
        default_namespace: default_namespace.into(),
        store_id: options.store_id,
        idempotency_key: options.idempotency_key,
        expected_generation: options.expected_generation,
        batch_operation_id,
    })
}

/// Public generated-vector fields only. Supplied-profile-ID and fixture controls
/// require their own replay-aware owner consumer; no descriptor substitution.
pub(crate) fn item_error(value: &Value, index: usize) -> Option<BatchError> {
    let object = match value.as_object() {
        Some(value) => value,
        None => {
            return Some(BatchError {
                code: "INVALID_INPUT".into(),
                field: Some(format!("memories[{index}]")),
            });
        }
    };
    if object.get("embedding").is_some_and(|v| !v.is_array()) {
        return Some(BatchError {
            code: "INVALID_INPUT".into(),
            field: Some(format!("memories[{index}].embedding")),
        });
    }
    if reject_unknown_fields(
        object,
        &[
            "summary",
            "fullText",
            "tags",
            "entities",
            "topics",
            "emotions",
            "namespace",
            "parentId",
            "supersedes",
            "checkDuplicates",
            "embedding",
            "embeddingProfileId",
        ],
    )
    .is_err()
    {
        // Persist only the existing closed diagnostic vocabulary, never a
        // caller-controlled field name or an unbounded diagnostic string.
        return Some(BatchError {
            code: "INVALID_INPUT".into(),
            field: Some(format!("memories[{index}]")),
        });
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchFailure {
    pub error: ExecutionError,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<WriteReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation_receipt: Option<WriteReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Readiness>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}
impl BatchFailure {
    fn new(category: ErrorCategory, code: &str, field: Option<String>, message: String) -> Self {
        let field = field.filter(|s| s.len() <= 128);
        Self {
            error: ExecutionError {
                category,
                code: code.into(),
                field,
                message,
                retryable: false,
                commit_state: CommitState::NotCommitted,
                operation_id: None,
            },
            receipt: None,
            reservation_receipt: None,
            readiness: None,
            warnings: vec![],
        }
    }
    pub(crate) fn unavailable() -> Self {
        Self::new(
            ErrorCategory::Unavailable,
            "STORAGE_UNAVAILABLE",
            None,
            "native batch consumer unavailable".into(),
        )
    }
    pub(crate) fn uncertain() -> Self {
        let mut failure = Self::unavailable();
        failure.error.code = "COMMIT_OUTCOME_UNKNOWN".into();
        failure.error.commit_state = CommitState::Unknown;
        failure.error.message =
            "native batch reply unavailable; reconcile the original request and key after recovery"
                .into();
        failure
    }
    pub(crate) fn from_storage(error: StorageError) -> Self {
        match error {
            error @ StorageError::VectorCertificationRequired { .. } => Self::new(
                ErrorCategory::Unavailable,
                "UNAVAILABLE",
                Some("/namespace".into()),
                error.to_string(),
            ),
            StorageError::BatchResponseTooLarge {
                reservation_receipt,
            } => {
                let mut out = Self::new(ErrorCategory::InvalidInput, "RESPONSE_TOO_LARGE", None,
                    if reservation_receipt.is_some() {
                        "historical batch cannot fit this transport; no new work performed, original item receipts remain retained; do not retry with a new key"
                    } else { "batch worst-case response exceeds transport budget; reduce the batch before reservation" }.into());
                if let Some(receipt) = reservation_receipt {
                    out.error.commit_state = CommitState::Committed;
                    out.error.operation_id = Some(receipt.operation_id);
                    out.reservation_receipt = Some(*receipt);
                }
                out
            }
            StorageError::CommittedMutation { receipt, .. } => {
                let mut out = Self::unavailable();
                out.error.code = "INDEX_NOT_READY".into();
                out.error.commit_state = CommitState::Committed;
                out.error.operation_id = Some(receipt.operation_id);
                out.receipt = Some(*receipt);
                out
            }
            StorageError::CommitOutcomeUnknown { operation_id, .. } => {
                let mut out = Self::uncertain();
                out.error.operation_id = Some(operation_id);
                out
            }
            error @ (StorageError::GenerationConflict { .. }
            | StorageError::IdempotencyConflict
            | StorageError::StoreLineageMismatch) => {
                let (code, field) = match error {
                    StorageError::GenerationConflict { .. } => {
                        ("GENERATION_CONFLICT", "/expectedGeneration")
                    }
                    StorageError::IdempotencyConflict => {
                        ("IDEMPOTENCY_CONFLICT", "/idempotencyKey")
                    }
                    _ => ("STORE_LINEAGE_MISMATCH", "/storeId"),
                };
                Self::new(
                    ErrorCategory::Conflict,
                    code,
                    Some(field.into()),
                    error.to_string(),
                )
            }
            error
            @ (StorageError::InvalidMutationContext(_) | StorageError::InvalidMutation(_)) => {
                Self::new(
                    ErrorCategory::InvalidInput,
                    "INVALID_INPUT",
                    None,
                    error.to_string(),
                )
            }
            _ => Self::unavailable(),
        }
    }
    pub(crate) fn from_bridge(error: BridgeError) -> Self {
        match error {
            BridgeError::InvalidInput(message) => {
                Self::new(ErrorCategory::InvalidInput, "INVALID_INPUT", None, message)
            }
            BridgeError::Unavailable(_) => Self::unavailable(),
            BridgeError::PartiallyApplied(_) => Self::uncertain(),
            _ => Self::unavailable(),
        }
    }
}
impl From<InputError> for BatchFailure {
    fn from(error: InputError) -> Self {
        Self::new(
            ErrorCategory::InvalidInput,
            error.code,
            Some(error.field),
            error.message.into(),
        )
    }
}

/// Dedicated internal RPC reply preserves typed failure and complete receipts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBatchOutcome {
    #[serde(flatten)]
    pub outcome: BatchOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Readiness>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}
impl From<BatchOutcome> for NativeBatchOutcome {
    fn from(outcome: BatchOutcome) -> Self {
        Self {
            outcome,
            readiness: None,
            warnings: vec![],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeBatchReply {
    Outcome(NativeBatchOutcome),
    Failure(BatchFailure),
}
impl NativeBatchReply {
    pub(crate) fn result(self) -> Result<NativeBatchOutcome, BatchFailure> {
        match self {
            Self::Outcome(v) => Ok(v),
            Self::Failure(e) => Err(e),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemScope {
    pub namespace_id: crate::model::NamespaceId,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchWireItem {
    pub index: u32,
    pub status: BatchStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<BatchItemScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<WriteReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
    pub ready: bool,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
impl From<BatchItemOutcome> for BatchWireItem {
    fn from(item: BatchItemOutcome) -> Self {
        let error = item.error.map(|error| ExecutionError {
            category: match error.code.as_str() {
                "NAMESPACE_NOT_FOUND" | "NOT_FOUND" => ErrorCategory::NotFound,
                "POLICY_CHANGED" | "NAMESPACE_MISMATCH" | "EDGE_CONFLICT" | "CONFLICT" => {
                    ErrorCategory::Conflict
                }
                "INVALID_INPUT" => ErrorCategory::InvalidInput,
                _ => ErrorCategory::Unavailable,
            },
            code: if error.code == "CONFLICT"
                && error.field.as_deref() == Some("embeddingProfileId")
            {
                "EMBEDDING_PROFILE_MISMATCH".into()
            } else {
                error.code
            },
            message: "inspect this indexed outcome; successful items remain committed".into(),
            // The stored diagnostic vocabulary is not a JSON Pointer. The
            // public field identifies this item without changing retained bytes.
            field: Some(format!("/memories/{}", item.index)),
            retryable: item.retryable,
            commit_state: if item.receipt.is_some() {
                CommitState::Committed
            } else if item.status == BatchStatus::Unknown {
                CommitState::Unknown
            } else {
                CommitState::NotCommitted
            },
            operation_id: item.operation_id,
        });
        Self {
            index: item.index,
            status: item.status,
            operation_id: item.operation_id,
            scope: item
                .namespace_id
                .map(|namespace_id| BatchItemScope { namespace_id }),
            receipt: item.receipt,
            error,
            ready: item.ready,
            retryable: item.retryable,
            data: item.result,
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchData {
    pub batch_operation_id: Uuid,
    pub store_id: Uuid,
    pub reservation_state: BatchReservationState,
    pub reservation_receipt: Option<WriteReceipt>,
    pub bookkeeping_receipts: Vec<WriteReceipt>,
    pub unknown_bookkeeping_operations: Vec<Uuid>,
    pub outcomes: Vec<BatchWireItem>,
    pub total: Generation,
    pub succeeded: Generation,
    pub failed: Generation,
    pub unresolved: Generation,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEnvelope {
    #[serde(flatten)]
    pub envelope: Envelope<BatchData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_operation_id: Option<Uuid>,
    /// Only historical oversize failures: reservation evidence, not an atomic
    /// batch receipt, item outcome list, or readiness assertion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservation_receipt: Option<WriteReceipt>,
}
/// Compact v2 text keeps nested receipt hashes within the declared response
/// budget. Decode that exact serialization for structured content so f32 wire
/// values agree; the v1 pretty-text serializer is unchanged.
pub(crate) fn mcp_result(
    status: u16,
    envelope: &BatchEnvelope,
) -> crate::mcp::protocol::ToolCallResult {
    let text = serde_json::to_string(envelope).expect("finite typed batch envelope");
    let Value::Object(object) = serde_json::from_str(&text).expect("batch JSON") else {
        unreachable!()
    };
    let mut out = crate::mcp::protocol::ToolCallResult::text(text);
    out.structured_content = Some(object);
    if status != 200 {
        out.is_error = Some(true);
    }
    out
}
pub(crate) fn response(result: Result<NativeBatchOutcome, BatchFailure>) -> (u16, BatchEnvelope) {
    let scope = Scope::Catalog {
        kind: CatalogKind::Catalog,
    };
    match result {
        Ok(NativeBatchOutcome {
            outcome: out,
            readiness,
            warnings,
        }) => {
            let states = out
                .items
                .iter()
                .map(|i| match i.status {
                    BatchStatus::Ok => ItemState::Ok { ready: i.ready },
                    BatchStatus::Error => ItemState::Error,
                    BatchStatus::Unknown => ItemState::Unknown,
                    BatchStatus::NotAttempted => ItemState::NotAttempted,
                })
                .collect::<Vec<_>>();
            let Ok((counts, status, http)) = BatchCounts::classify(&states) else {
                return response(Err(BatchFailure::uncertain()));
            };
            if counts.total != out.total
                || counts.succeeded != out.succeeded
                || counts.failed != out.failed
                || counts.unresolved != out.unresolved
                || out
                    .items
                    .iter()
                    .enumerate()
                    .any(|(n, i)| i.index as usize != n)
            {
                return response(Err(BatchFailure::uncertain()));
            }
            let counter =
                |n: usize| -> Generation { n.to_string().try_into().expect("bounded batch count") };
            let data = BatchData {
                batch_operation_id: out.batch_operation_id,
                store_id: out.store_id,
                reservation_state: out.reservation_state,
                reservation_receipt: out.reservation_receipt,
                bookkeeping_receipts: out.bookkeeping_receipts,
                unknown_bookkeeping_operations: out.unknown_bookkeeping_operations,
                outcomes: out.items.into_iter().map(BatchWireItem::from).collect(),
                total: counter(counts.total),
                succeeded: counter(counts.succeeded),
                failed: counter(counts.failed),
                unresolved: counter(counts.unresolved),
            };
            let mut envelope = Envelope::success(scope, data);
            envelope.readiness = readiness;
            envelope.warnings = warnings;
            envelope.status = status;
            if http != 200 {
                envelope.error = Some(ExecutionError {
                    category: if http == 207 {
                        ErrorCategory::Partial
                    } else if http == 422 {
                        ErrorCategory::InvalidInput
                    } else {
                        ErrorCategory::Unavailable
                    },
                    code: if http == 207 {
                        "BATCH_PARTIALLY_APPLIED"
                    } else if http == 422 {
                        "BATCH_ALL_FAILED"
                    } else {
                        "BATCH_UNRESOLVED"
                    }
                    .into(),
                    message: "inspect every indexed outcome; batches are not atomic".into(),
                    field: None,
                    retryable: false,
                    commit_state: CommitState::Committed,
                    operation_id: None,
                });
                // Reservation uncertainty is not an assertion of commitment.
                if matches!(
                    envelope.data.as_ref().unwrap().reservation_state,
                    BatchReservationState::Unknown
                ) {
                    envelope.error.as_mut().unwrap().commit_state = CommitState::Unknown;
                }
            }
            (
                http,
                BatchEnvelope {
                    envelope,
                    batch_operation_id: None,
                    reservation_receipt: None,
                },
            )
        }
        Err(failure) => {
            let status = match failure.error.category {
                ErrorCategory::InvalidInput => 400,
                ErrorCategory::NotFound => 404,
                ErrorCategory::Conflict => 409,
                _ => 503,
            };
            let parent = failure.reservation_receipt.as_ref().map(|r| r.operation_id);
            let mut envelope = Envelope::failure(Some(scope), failure.error);
            envelope.receipt = failure.receipt;
            envelope.readiness = failure.readiness;
            envelope.warnings = failure.warnings;
            (
                status,
                BatchEnvelope {
                    envelope,
                    batch_operation_id: parent,
                    reservation_receipt: failure.reservation_receipt,
                },
            )
        }
    }
}

pub(crate) fn input_schema() -> Value {
    use crate::model::constants::*;
    let mut item = json!({"type":"object","additionalProperties":false,"required":["summary"],"properties":{
        "summary":{"type":"string","minLength":1,"maxLength":SUMMARY_MAX_BYTES},
        "fullText":{"type":["string","null"],"maxLength":FULL_TEXT_MAX_BYTES},
        "namespace":{"type":["string","null"],"minLength":1,"maxLength":NAMESPACE_NAME_MAX_BYTES},
        "parentId":{"type":["string","null"],"format":"uuid"},"supersedes":{"type":["string","null"],"format":"uuid"},
        "embedding":{"type":"array","minItems":1,"maxItems":8192,"items":{"type":"number"},"description":"Finite nonzero vector matching the canonical Bound profile. Original caller bits remain retry identity."},
        "embeddingProfileId":{"type":"string","pattern":"^ep1:[0-9a-f]{64}$","minLength":68,"maxLength":68},
        "checkDuplicates":{"type":["boolean","null"],"default":true,"description":"Omitted/null enables advisory duplicate checks."}}});
    item["dependentRequired"] =
        json!({"embedding":["embeddingProfileId"],"embeddingProfileId":["embedding"]});
    for (name, count, bytes) in [
        ("tags", MAX_TAGS, TAG_MAX_BYTES),
        ("entities", MAX_ENTITIES, ENTITY_MAX_BYTES),
        ("topics", MAX_TOPICS, TOPIC_MAX_BYTES),
        ("emotions", MAX_EMOTIONS, EMOTION_MAX_BYTES),
    ] {
        item["properties"][name] = json!({"type":["array","null"],"maxItems":count,"items":{"type":"string","maxLength":bytes},"description":"Null is omission-compatible; UTF-8 byte limits enforced."});
    }
    json!({"type":"object","additionalProperties":false,"required":["contractVersion","memories"],"properties":{
        "contractVersion":{"const":2,"type":"integer"},"memories":{"type":"array","minItems":1,"maxItems":MAX_BATCH_MEMORIES,"items":item},
        "storeId":{"type":"string","format":"uuid"},"idempotencyKey":{"type":"string","minLength":1,"maxLength":128},"expectedGeneration":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"},"batchOperationId":{"type":"string","format":"uuid"}}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_wire_strict_selectors_keys_schema_and_legacy_gate() {
        let store = Uuid::from_u128(1);
        let value = json!({"contractVersion":2,"storeId":store,"memories":[{"summary":"valid"}],"idempotencyKey":"same","expectedGeneration":"9007199254740993"});
        let input = parse(&value, true, Some("same"), "chosen").unwrap();
        assert_eq!(input.default_namespace, "chosen");
        assert_eq!(input.expected_generation.unwrap().get(), 9007199254740993);
        assert_eq!(
            input.context().expected_generation,
            input.expected_generation
        );
        assert!(parse(&value, true, Some("other"), "chosen").is_err());
        for key in [json!(null), json!(""), json!("é"), json!("x".repeat(129))] {
            let mut bad = value.clone();
            bad["idempotencyKey"] = key;
            assert!(parse(&bad, true, None, "chosen").is_err());
        }
        for generation in [
            json!(null),
            json!(1),
            json!("01"),
            json!("-1"),
            json!("9223372036854775808"),
        ] {
            let mut bad = value.clone();
            bad["expectedGeneration"] = generation;
            assert!(parse(&bad, true, None, "chosen").is_err());
        }
        for version in [json!(null), json!("2"), json!(3), json!(1)] {
            let mut bad = value.clone();
            bad["contractVersion"] = version;
            assert!(parse(&bad, true, None, "chosen").is_err());
        }
        let mut bad = value.clone();
        bad["unknown"] = json!(true);
        assert!(parse(&bad, true, None, "chosen").is_err());
        for memories in [json!([]), json!(null), json!({})] {
            let mut bad = value.clone();
            bad["memories"] = memories;
            assert!(parse(&bad, true, None, "chosen").is_err());
        }
        let schema = input_schema();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["memories"]["items"]["additionalProperties"],
            false
        );
        assert_eq!(schema["properties"]["contractVersion"]["const"], 2);
        assert!(ContractVersion::V2.require_supported().is_err());
        assert!(ContractVersion::V1.require_supported().is_ok());
        assert!(item_error(&json!({"summary":"valid"}), 0).is_none());
        let invalid = item_error(&json!({"summary":"valid","embeddingProfile":{}}), 99).unwrap();
        assert_eq!(invalid.code, "INVALID_INPUT");
        assert_eq!(invalid.field.as_deref(), Some("memories[99]"));
    }
}
