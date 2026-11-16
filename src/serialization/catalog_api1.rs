//! Local API1 catalog evidence admission. This is not a daemon negotiation.
use crate::{
    mcp::protocol::{Era, JsonRpcId, JsonRpcResponse, ResultEnvelope, ToolCallResult},
    model::NamespaceConfig,
    native_catalog_adapter::CatalogError,
    storage::{StorageError, control::{MutationOutcome, WriteReceipt}, maintenance::ReceiptAdmission},
};
use serde_json::{Value, json};
use std::io::Write;
use uuid::Uuid;

const LOCAL_LIMIT: usize = 8_388_608;

/// Public only because local adapter traits carry it. Callers cannot deserialize
/// or construct authority; the actual emitter constructs the descriptor.
#[derive(Clone)]
pub struct CatalogReplyAdmission {
    surface: Surface,
    limit: usize,
}

#[derive(Clone)]
enum Surface {
    Mcp { id: JsonRpcId, era: Era, timezone: chrono_tz::Tz },
    Http,
}

impl CatalogReplyAdmission {
    pub(crate) fn mcp(id: JsonRpcId, era: Era) -> Self {
        Self { surface: Surface::Mcp { id, era, timezone: chrono_tz::UTC }, limit: LOCAL_LIMIT }
    }

    pub(crate) fn http() -> Self {
        Self { surface: Surface::Http, limit: LOCAL_LIMIT }
    }

    pub(crate) fn with_timezone(mut self, timezone: chrono_tz::Tz) -> Self {
        if let Surface::Mcp { timezone: selected, .. } = &mut self.surface {
            *selected = timezone;
        }
        self
    }

    /// Invoke only after selecting the constructor-validated native owner.
    /// Descriptor construction itself must not impose a new legacy limit.
    pub(crate) fn admit_native(&self) -> Result<ReceiptAdmission, StorageError> {
        self.check_failure(None)?;
        ReceiptAdmission::new(self.limit)
    }

    pub(crate) fn check_candidate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        self.check_failure(Some(receipt))?;
        let ns = namespace(receipt)?;
        let value = match &self.surface {
            Surface::Mcp { timezone, .. } => serde_json::to_value(info(ns, *timezone)),
            Surface::Http => return check_encoded(&super::ApiResponse {
                data: http_namespace(ns),
                took_us: Some(u64::MAX),
            }, self.limit),
        }.map_err(|_| StorageError::ReceiptBudgetExceeded)?;
        self.check_value(&value, false)
    }

    fn check_failure(&self, receipt: Option<&WriteReceipt>) -> Result<(), StorageError> {
        // Use the exact Value -> pretty tool text/structured content -> outer
        // envelope path. Typed f32 byte lengths are not a proxy for this path.
        for (code, state, ready) in [
            ("SERVICE_UNAVAILABLE", "committed", None),
            ("INDEX_NOT_READY", "committed", Some(false)),
            ("SERVICE_UNAVAILABLE", "unknown", None),
            ("RESPONSE_TOO_LARGE", "notCommitted", None),
        ] {
            let value = evidence(code, state, Some(Uuid::from_u128(u128::MAX)),
                receipt, ready, &"\0".repeat(512));
            self.check_value(&value, true)?;
        }
        Ok(())
    }

    fn check_value(&self, value: &Value, failure: bool) -> Result<(), StorageError> {
        let (value, limit) = self.wire_value(value, failure)?;
        check_encoded(&value, limit)
    }

    fn wire_value(&self, value: &Value, failure: bool) -> Result<(Value, usize), StorageError> {
        Ok(match &self.surface {
            Surface::Http => (value.clone(), self.limit),
            Surface::Mcp { id, era, .. } => {
                // BridgeError::PartiallyApplied carries compact JSON text.
                // The tool handler parses that text before making its pretty
                // tool body. Reproduce this parse: f64 decimal decoding need
                // not retain the same representation on every serde build.
                let parsed;
                let value = if failure {
                    parsed = serde_json::from_str::<Value>(&value.to_string())
                        .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                    &parsed
                } else { value };
                let mut tool = ToolCallResult::json(value)
                    .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                if failure { tool.is_error = Some(true); }
                if *era == Era::Legacy { tool.structured_content = None; }
                let tool = serde_json::to_value(tool)
                    .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                let env = serde_json::to_value(ResultEnvelope::complete(tool))
                    .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                let response = serde_json::to_value(JsonRpcResponse::success(id.clone(), env))
                    .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                (response, self.limit.checked_sub(1).ok_or(StorageError::ReceiptBudgetExceeded)?)
            }
        })
    }

    #[cfg(test)]
    pub(crate) fn encoded_for_test(&self, value: &Value, failure: bool) -> Vec<u8> {
        let (value, _) = self.wire_value(value, failure).unwrap();
        let mut bytes = serde_json::to_vec(&value).unwrap();
        if matches!(self.surface, Surface::Mcp { .. }) { bytes.push(b'\n'); }
        bytes
    }
}

fn check_encoded(value: &impl serde::Serialize, limit: usize) -> Result<(), StorageError> {
    struct Count { remaining: usize }
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.remaining = self.remaining.checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::other("catalog response limit"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    serde_json::to_writer(Count { remaining: limit }, value)
        .map_err(|_| StorageError::ReceiptBudgetExceeded)
}

pub(crate) fn namespace(receipt: &WriteReceipt) -> Result<NamespaceConfig, StorageError> {
    if let MutationOutcome::NativeNamespaceCreated { transition } = &receipt.outcome {
        transition.validate(receipt)?;
        transition.namespace()
    } else {
        Err(StorageError::InvalidControl("unexpected native catalog receipt".into()))
    }
}

pub(crate) fn committed_namespace(receipt: WriteReceipt) -> Result<NamespaceConfig, CatalogError> {
    namespace(&receipt).map_err(|source| CatalogError::Storage(StorageError::CommittedMutation {
        receipt: Box::new(receipt), source: Box::new(source),
    }))
}

pub(crate) fn input(input: crate::mcp::bridge::CreateNamespaceInput) -> crate::storage::native_catalog::NativeNamespaceInput {
    crate::storage::native_catalog::NativeNamespaceInput {
        name: input.name, embedding_dim: input.embedding_dim.map(u32::from), embedding_profile_id: None,
        initial_stability: input.initial_stability, desired_retention: input.desired_retention,
        decay_rate_multiplier: input.decay_rate_multiplier,
    }
}

pub(crate) fn info(ns: NamespaceConfig, timezone: chrono_tz::Tz) -> crate::mcp::bridge::NamespaceInfo {
    crate::mcp::bridge::NamespaceInfo {
        id: ns.id.get(), name: ns.name, embedding_dim: ns.embedding_dim as u16,
        memory_count: 0, created_at: crate::time::format_timestamp(ns.created_at, timezone),
    }
}

pub(crate) fn http_namespace(ns: NamespaceConfig) -> super::NamespaceResponse {
    super::NamespaceResponse {
        id: ns.id.get(), name: ns.name, embedding_dim: ns.embedding_dim,
        initial_stability: ns.initial_stability, default_difficulty: ns.default_difficulty,
        permastore_threshold: ns.permastore_threshold, desired_retention: ns.desired_retention,
        created_at: ns.created_at, memory_count: 0,
    }
}

fn evidence(code: &str, state: &str, operation: Option<Uuid>, receipt: Option<&WriteReceipt>,
    ready: Option<bool>, message: &str) -> Value {
    let mut value = json!({"error": code, "commitState": state, "operationId": operation,
        "message": message});
    if let Some(receipt) = receipt { value["receipt"] = json!(receipt); }
    if let Some(ready) = ready { value["ready"] = json!(ready); }
    value
}

/// Unknown takes precedence over separately retained known evidence. Generic
/// committed failures do not establish any derived readiness observation.
pub(crate) fn failure(error: &CatalogError) -> (u16, Value) {
    let (mut receipt, mut source) = match error {
        CatalogError::Storage(source) => (None, Some(source)),
        _ => (None, None),
    };
    loop {
        match source {
            Some(StorageError::CommittedMutation { receipt: known, source: cause }) => {
                if receipt.is_none() { receipt = Some(known.as_ref()); }
                source = Some(cause.as_ref());
            }
            Some(StorageError::CommittedEffectsPending { source: cause, .. }) => source = Some(cause.as_ref()),
            _ => break,
        }
    }
    let unknown = match source {
        Some(StorageError::CommitOutcomeUnknown { operation_id, .. }) => Some(Some(*operation_id)),
        Some(StorageError::WorkerOutcomeUnknown(_)) => Some(None),
        _ if matches!(error, CatalogError::WorkerUnknown) => Some(None),
        _ => None,
    };
    let (status, code, state, operation, ready) = if let Some(operation) = unknown {
        (503, "SERVICE_UNAVAILABLE", "unknown", operation, None)
    } else if let Some(receipt) = receipt {
        (503, "SERVICE_UNAVAILABLE", "committed", Some(receipt.operation_id), None)
    } else if matches!(source, Some(StorageError::ReceiptBudgetExceeded)) {
        (413, "RESPONSE_TOO_LARGE", "notCommitted", None, None)
    } else if matches!(source, Some(StorageError::DuplicateName(_))) {
        (409, "NAMESPACE_EXISTS", "notCommitted", None, None)
    } else if matches!(source, Some(StorageError::InvalidMutation(_) | StorageError::InvalidMutationContext(_)
        | StorageError::InvalidNamespaceName { .. } | StorageError::NamespaceEmbeddingProfileMismatch { .. }))
        || matches!(error, CatalogError::Input(_)) {
        (400, "INVALID_INPUT", "notCommitted", None, None)
    } else {
        (503, "SERVICE_UNAVAILABLE", "notCommitted", None, None)
    };
    let message = source.map(ToString::to_string).unwrap_or_else(|| error.to_string());
    let message = crate::model::validation::truncate_on_char_boundary(&message, 512);
    (status, evidence(code, state, operation, receipt, ready, message))
}

pub(crate) fn bridge_failure(error: CatalogError) -> crate::mcp::bridge::BridgeError {
    crate::mcp::bridge::BridgeError::PartiallyApplied(failure(&error).1.to_string())
}

pub(crate) fn outcome_unknown(error: &CatalogError) -> bool {
    fn storage(error: &StorageError) -> bool {
        match error {
            StorageError::CommitOutcomeUnknown { .. } | StorageError::WorkerOutcomeUnknown(_) => true,
            StorageError::CommittedMutation { source, .. } | StorageError::CommittedEffectsPending { source, .. } => storage(source),
            _ => false,
        }
    }
    match error {
        CatalogError::WorkerUnknown => true,
        CatalogError::Storage(source) => storage(source),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_catalog_adapter::{NativeCatalogAdapter, tests::{fixture, FixedClock}};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_catalog_api1_full_encoding_limits_and_unknown_over_known() {
        let (f, _, provider) = fixture();
        let adapter = NativeCatalogAdapter::new(f.completion(), provider,
            Default::default(), std::sync::Arc::new(FixedClock));
        let receipt = adapter.create_api1_bounded(input(crate::mcp::bridge::CreateNamespaceInput {
            name: "encoded".into(), embedding_dim: None, initial_stability: Some(3.7145),
            desired_retention: Some(0.9), decay_rate_multiplier: None,
        }), CatalogReplyAdmission::http()).await.unwrap();
        let error = CatalogError::Storage(StorageError::CommittedMutation {
            receipt: Box::new(receipt.clone()),
            source: Box::new(StorageError::WorkerOutcomeUnknown("worker lost".into())),
        });
        assert!(outcome_unknown(&error));
        let (_, value) = failure(&error);
        assert_eq!(value["commitState"], "unknown");
        assert_eq!(value["operationId"], Value::Null);
        assert_eq!(value["receipt"], json!(receipt));
        assert!(value.get("ready").is_none());
        let value = evidence("SERVICE_UNAVAILABLE", "unknown", Some(Uuid::from_u128(u128::MAX)),
            Some(&receipt), None, &"\0".repeat(512));
        for era in [Era::Legacy, Era::Modern] {
            let id = JsonRpcId::String("\0\"\\é".repeat(64));
            let parsed: Value = serde_json::from_str(&value.to_string()).unwrap();
            let mut tool = ToolCallResult::json(&parsed).unwrap();
            tool.is_error = Some(true);
            if era == Era::Legacy { tool.structured_content = None; }
            let response = JsonRpcResponse::success(id.clone(), serde_json::to_value(
                ResultEnvelope::complete(serde_json::to_value(tool).unwrap())).unwrap());
            let size = serde_json::to_vec(&response).unwrap().len() + 1;
            let mut admission = CatalogReplyAdmission::mcp(id, era);
            admission.limit = size;
            admission.check_value(&value, true).unwrap();
            admission.limit -= 1;
            assert!(matches!(admission.check_value(&value, true), Err(StorageError::ReceiptBudgetExceeded)));
        }
        let size = serde_json::to_vec(&value).unwrap().len();
        let mut admission = CatalogReplyAdmission::http();
        admission.limit = size;
        admission.check_value(&value, true).unwrap();
        admission.limit -= 1;
        assert!(matches!(admission.check_value(&value, true), Err(StorageError::ReceiptBudgetExceeded)));
    }
}
