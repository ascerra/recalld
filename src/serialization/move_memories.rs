//! Compact API1 move results. Internal history proofs never cross this boundary.
use crate::{
    mcp::protocol::{Era, JsonRpcId, JsonRpcResponse, ResultEnvelope, ToolCallResult},
    model::MemoryId,
    storage::{
        StorageError,
        control::{MutationOutcome, WriteReceipt},
        native_move::NativeMoveInput,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub store_id: Uuid,
    pub operation_id: Uuid,
    pub ids: Vec<MemoryId>,
    pub source_namespace: String,
    pub destination_namespace: String,
    pub source_namespace_id: u32,
    pub destination_namespace_id: u32,
    pub destination_profile_id: String,
    pub committed_generation: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Failure {
    pub code: String,
    pub commit_state: String,
    pub operation_id: Option<Uuid>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub receipt: Option<Receipt>,
    pub error: Option<Failure>,
}

impl Receipt {
    fn from_receipt(receipt: &WriteReceipt) -> Result<Self, StorageError> {
        let MutationOutcome::NativeMemoriesMoved { transition } = &receipt.outcome else {
            return Err(StorageError::InvalidControl(
                "unexpected move receipt".into(),
            ));
        };
        transition.validate(receipt)?;
        Ok(Self {
            store_id: receipt.store_id,
            operation_id: receipt.operation_id,
            ids: transition.ids.clone(),
            source_namespace: transition.source_namespace.clone(),
            destination_namespace: transition.destination_namespace.clone(),
            source_namespace_id: transition.source_namespace_id.get(),
            destination_namespace_id: transition.destination_namespace_id.get(),
            destination_profile_id: transition.destination_profile_id.clone(),
            committed_generation: String::from(receipt.committed_generation),
            replayed: receipt.replayed,
        })
    }
}

impl Reply {
    pub(crate) fn error(
        code: &str,
        state: &str,
        operation_id: Option<Uuid>,
        message: &str,
    ) -> Self {
        Self {
            receipt: None,
            error: Some(Failure {
                code: code.into(),
                commit_state: state.into(),
                operation_id,
                message: crate::model::validation::truncate_on_char_boundary(message, 512).into(),
            }),
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self::error(
            "SERVICE_UNAVAILABLE",
            "notCommitted",
            None,
            "move_memories requires a native move coordinator; no mutation was attempted.",
        )
    }

    pub(crate) fn from_result(result: Result<WriteReceipt, StorageError>) -> Self {
        match result {
            Ok(receipt) => match Receipt::from_receipt(&receipt) {
                Ok(receipt) => Self {
                    receipt: Some(receipt),
                    error: None,
                },
                Err(_) => Self::error(
                    "INTERNAL_ERROR",
                    "unknown",
                    Some(receipt.operation_id),
                    "Move receipt validation failed; retain the original request and reconcile using the same key.",
                ),
            },
            Err(error) => Self::from_storage(&error),
        }
    }

    fn from_storage(error: &StorageError) -> Self {
        let mut source = error;
        let mut receipt = None;
        let mut operation = None;
        let mut committed = false;
        loop {
            match source {
                StorageError::CommittedMutation {
                    receipt: known,
                    source: cause,
                } => {
                    committed = true;
                    operation.get_or_insert(known.operation_id);
                    if receipt.is_none() {
                        match Receipt::from_receipt(known) {
                            Ok(known) => receipt = Some(known),
                            Err(_) => {
                                return Self::error(
                                    "INTERNAL_ERROR",
                                    "unknown",
                                    operation,
                                    "Move receipt validation failed; reconcile the original request.",
                                );
                            }
                        }
                    }
                    source = cause;
                }
                StorageError::CommittedEffectsPending {
                    operation_id,
                    source: cause,
                } => {
                    committed = true;
                    operation.get_or_insert(*operation_id);
                    source = cause;
                }
                _ => break,
            }
        }
        let (code, state) = match source {
            StorageError::CommitOutcomeUnknown { operation_id, .. } => {
                operation = Some(*operation_id);
                ("SERVICE_UNAVAILABLE", "unknown")
            }
            StorageError::WorkerOutcomeUnknown(_) => ("SERVICE_UNAVAILABLE", "unknown"),
            _ if committed => ("SERVICE_UNAVAILABLE", "committed"),
            StorageError::RecoveryRequired(_) => ("SERVICE_UNAVAILABLE", "unknown"),
            StorageError::ReceiptBudgetExceeded => ("RESPONSE_TOO_LARGE", "notCommitted"),
            StorageError::IdempotencyConflict => ("IDEMPOTENCY_CONFLICT", "notCommitted"),
            StorageError::StoreLineageMismatch => ("STORE_LINEAGE_MISMATCH", "notCommitted"),
            StorageError::InvalidMutation(_) | StorageError::InvalidMutationContext(_) => {
                ("INVALID_INPUT", "notCommitted")
            }
            _ => ("SERVICE_UNAVAILABLE", "notCommitted"),
        };
        // Format only the leaf: wrapper Display implementations include the full proof.
        let mut reply = Self::error(code, state, operation, &source.to_string());
        reply.receipt = receipt;
        reply
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.receipt.is_none() && self.error.is_none() {
            return Err("move reply has neither result nor error".into());
        }
        if let Some(receipt) = &self.receipt {
            for name in [&receipt.source_namespace, &receipt.destination_namespace] {
                crate::model::validation::validate_namespace_name(name)
                    .map_err(|e| e.to_string())?;
            }
            let _: crate::storage::control::Generation = receipt
                .committed_generation
                .clone()
                .try_into()
                .map_err(|e: StorageError| e.to_string())?;
            if receipt.store_id.is_nil()
                || receipt.operation_id.is_nil()
                || receipt.source_namespace_id == 0
                || receipt.destination_namespace_id == 0
                || receipt.source_namespace_id == receipt.destination_namespace_id
                || !crate::embedding::profile::valid_profile_id(&receipt.destination_profile_id)
                || !(1..=100).contains(&receipt.ids.len())
                || receipt.ids.iter().any(|id| *id == MemoryId::nil())
                || receipt
                    .ids
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != receipt.ids.len()
            {
                return Err("invalid compact move receipt".into());
            }
        }
        if let Some(error) = &self.error {
            if !matches!(
                error.commit_state.as_str(),
                "notCommitted" | "committed" | "unknown"
            ) || error.code.len() > 32
                || !error
                    .code
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b == b'_')
                || error.message.len() > 512
                || error.operation_id.is_some_and(|id| id.is_nil())
            {
                return Err("invalid compact move failure".into());
            }
        }
        Ok(())
    }

    pub(crate) fn tool(&self) -> ToolCallResult {
        let mut tool = ToolCallResult::json(self).expect("compact move reply is JSON");
        if self.error.is_some() {
            tool.is_error = Some(true);
        }
        tool
    }
}

pub(crate) fn parse(mut arguments: Value) -> Result<NativeMoveInput, String> {
    crate::mcp::args::require_object(&arguments).map_err(|e| e.to_string())?;
    crate::serialization::contract::ContractVersion::from_mcp(&arguments)
        .and_then(|v| v.require_supported())
        .map_err(|e| e.to_string())?;
    arguments.as_object_mut().unwrap().remove("contractVersion");
    let input: NativeMoveInput = serde_json::from_value(arguments).map_err(|e| e.to_string())?;
    input.validate().map_err(|e| e.to_string())?;
    Ok(input)
}

/// Response format only, never mutation authority or a caller-selected budget.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Caller {
    id: JsonRpcId,
    modern: bool,
}

#[derive(Clone)]
pub struct Admission {
    caller: Caller,
    daemon_limit: usize,
}

impl Admission {
    pub(crate) fn mcp(id: JsonRpcId, era: Era) -> Self {
        Self {
            caller: Caller {
                id,
                modern: era == Era::Modern,
            },
            daemon_limit: LIMIT,
        }
    }
    pub(crate) fn caller(&self) -> Caller {
        self.caller.clone()
    }
    pub(crate) fn daemon(caller: Caller, limit: usize) -> Self {
        Self {
            caller,
            daemon_limit: limit.min(LIMIT),
        }
    }

    pub(crate) fn check(&self) -> Result<(), StorageError> {
        // Superset of every compact result: 100 UUIDs, two maximum-length
        // validated names, maximal generation/identity, and a maximally escaped
        // bounded error message. No internal receipt or request key is emitted.
        let reply = Reply {
            receipt: Some(Receipt {
                store_id: Uuid::from_u128(u128::MAX),
                operation_id: Uuid::from_u128(u128::MAX),
                ids: vec![MemoryId::from_uuid(Uuid::from_u128(u128::MAX)); 100],
                source_namespace: "x".repeat(64),
                destination_namespace: "x".repeat(64),
                source_namespace_id: u32::MAX,
                destination_namespace_id: u32::MAX,
                destination_profile_id: format!("ep1:{}", "f".repeat(64)),
                committed_generation: u64::MAX.to_string(),
                replayed: false,
            }),
            error: Some(Failure {
                code: "X".repeat(32),
                commit_state: "notCommitted".into(),
                operation_id: Some(Uuid::from_u128(u128::MAX)),
                message: "\0".repeat(512),
            }),
        };
        let daemon = crate::daemon::protocol::DaemonResponse::success(u64::MAX, json!(&reply));
        if serde_json::to_vec(&daemon)
            .map_err(|_| StorageError::ReceiptBudgetExceeded)?
            .len()
            > self.daemon_limit
        {
            return Err(StorageError::ReceiptBudgetExceeded);
        }
        let mut tool = reply.tool();
        // Reserve the actual recovery text as well as both structured/text copies.
        let mut recovery = Reply::error(
            "SERVICE_UNAVAILABLE",
            "unknown",
            None,
            "worker outcome unknown",
        )
        .tool();
        crate::mcp::recovery_guidance::append_recovery_guidance(
            "move_memories",
            &json!({"hasRetryIdentity":true}),
            &mut recovery,
        );
        tool.content.extend(recovery.content.into_iter().skip(1));
        if !self.caller.modern {
            tool.structured_content = None;
        }
        let response = JsonRpcResponse::success(
            self.caller.id.clone(),
            serde_json::to_value(ResultEnvelope::complete(
                serde_json::to_value(tool).unwrap(),
            ))
            .unwrap(),
        );
        if serde_json::to_vec(&response)
            .map_err(|_| StorageError::ReceiptBudgetExceeded)?
            .len()
            >= LIMIT
        {
            return Err(StorageError::ReceiptBudgetExceeded);
        }
        Ok(())
    }
}

pub(crate) fn output_schema() -> Value {
    let uuid = json!({"type":"string","format":"uuid"});
    json!({"type":"object","additionalProperties":false,"required":["receipt","error"],"properties":{
        "receipt":{"anyOf":[{"type":"null"},{"type":"object","additionalProperties":false,
            "required":["storeId","operationId","ids","sourceNamespace","destinationNamespace","sourceNamespaceId","destinationNamespaceId","destinationProfileId","committedGeneration","replayed"],
            "properties":{"storeId":uuid,"operationId":uuid,"ids":{"type":"array","minItems":1,"maxItems":100,"items":uuid},
                "sourceNamespace":{"type":"string","maxLength":64},"destinationNamespace":{"type":"string","maxLength":64},
                "sourceNamespaceId":{"type":"integer","minimum":1,"maximum":u32::MAX},
                "destinationNamespaceId":{"type":"integer","minimum":1,"maximum":u32::MAX},
                "destinationProfileId":{"type":"string","pattern":"^ep1:[0-9a-f]{64}$"},
                "committedGeneration":{"type":"string","pattern":"^[0-9]+$"},"replayed":{"type":"boolean"}}}]},
        "error":{"anyOf":[{"type":"null"},{"type":"object","additionalProperties":false,
            "required":["code","commitState","operationId","message"],"properties":{
                "code":{"type":"string"},"commitState":{"enum":["notCommitted","committed","unknown"]},
                "operationId":{"anyOf":[uuid,{"type":"null"}]},"message":{"type":"string"}}}]}
    }})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Value {
        json!({"storeId":Uuid::from_u128(1),"idempotencyKey":"stable-key",
            "sourceNamespace":"source","destinationNamespace":"destination",
            "ids":[Uuid::from_u128(2)],"embeddingMode":"regenerate"})
    }

    #[test]
    fn move_input_requires_exact_explicit_identity_and_supported_mode() {
        parse(input()).unwrap();
        for field in [
            "storeId",
            "idempotencyKey",
            "sourceNamespace",
            "destinationNamespace",
            "ids",
            "embeddingMode",
        ] {
            let mut value = input();
            value.as_object_mut().unwrap().remove(field);
            assert!(parse(value).is_err(), "{field}");
        }
        for (field, bad) in [
            ("storeId", json!(Uuid::nil())),
            ("idempotencyKey", json!("")),
            ("idempotencyKey", json!("x".repeat(129))),
            ("destinationNamespace", json!("SOURCE")),
            ("sourceNamespace", json!("../outside")),
            ("embeddingMode", json!("reuse")),
            ("ids", json!([])),
            ("ids", json!([Uuid::from_u128(2), Uuid::from_u128(2)])),
            ("ids", json!(vec![Uuid::from_u128(2); 101])),
            ("contractVersion", json!(2)),
            ("namespace", json!("ignored-default")),
        ] {
            let mut value = input();
            value[field] = bad;
            assert!(parse(value).is_err(), "{field}");
        }
    }

    #[test]
    fn move_admission_checks_original_id_both_eras_and_daemon_frame_before_work() {
        for era in [Era::Legacy, Era::Modern] {
            let admission = Admission::mcp(JsonRpcId::String("id\n\"\\λ".into()), era);
            admission.check().unwrap();
            let caller: Caller = serde_json::from_value(json!(admission.caller())).unwrap();
            Admission::daemon(caller, crate::daemon::protocol::LEGACY_MAX_MESSAGE_SIZE)
                .check()
                .unwrap();
            assert!(Admission::daemon(admission.caller(), 1024).check().is_err());
            assert!(
                Admission::mcp(JsonRpcId::String("x".repeat(LIMIT)), era)
                    .check()
                    .is_err()
            );
            // The daemon frame is admitted exactly, not with a guessed receipt-only size.
            let (mut low, mut high) = (0, LIMIT);
            while low < high {
                let mid = low + (high - low) / 2;
                if Admission::daemon(admission.caller(), mid).check().is_ok() {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            Admission::daemon(admission.caller(), low).check().unwrap();
            assert!(
                Admission::daemon(admission.caller(), low - 1)
                    .check()
                    .is_err()
            );
        }
    }

    #[test]
    fn move_failure_keeps_operation_and_unknown_precedence_without_internal_proof() {
        let id = Uuid::from_u128(7);
        let committed = Reply::from_result(Err(StorageError::CommittedEffectsPending {
            operation_id: id,
            source: Box::new(StorageError::RecoveryRequired("publication failed".into())),
        }));
        assert_eq!(committed.error.as_ref().unwrap().commit_state, "committed");
        assert_eq!(committed.error.as_ref().unwrap().operation_id, Some(id));
        let unknown = Reply::from_result(Err(StorageError::CommittedEffectsPending {
            operation_id: id,
            source: Box::new(StorageError::WorkerOutcomeUnknown("worker lost".into())),
        }));
        assert_eq!(unknown.error.as_ref().unwrap().commit_state, "unknown");
        assert_eq!(unknown.error.as_ref().unwrap().operation_id, Some(id));
        unknown.validate().unwrap();
        let long = Reply::error("INVALID_INPUT", "notCommitted", None, &"λ".repeat(1000));
        assert_eq!(long.error.as_ref().unwrap().message.len(), 512);
        assert!(json!(unknown).get("transition").is_none());
    }
}
