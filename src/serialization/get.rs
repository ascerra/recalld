//! Prepared by-ID wire contract. Public version admission remains closed.
use super::contract::*;
use crate::model::MemoryId;
use crate::search::{MemoryResponse, SearchError};
use crate::storage::control::{GenerationRef, WriteReceipt};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct GetRequest {
    #[cfg(test)]
    pub(crate) before_barrier: Option<fn(&crate::derived_completion::DerivedCompletion)>,
    #[cfg(test)]
    pub(crate) before_observation: Option<(
        std::sync::Arc<tokio::sync::Notify>,
        std::sync::Arc<tokio::sync::Notify>,
    )>,
    pub id: MemoryId,
    pub namespace: Option<String>,
    pub record_access: bool,
    pub original_contract_version: u32,
}

impl GetRequest {
    pub(crate) fn validate(&self) -> Result<(), InputError> {
        if !matches!(self.original_contract_version, 1 | 2) {
            return Err(input(
                "UNSUPPORTED_CONTRACT_VERSION",
                "/contractVersion",
                "unsupported get origin",
            ));
        }
        if self.id.into_inner().is_nil() {
            return Err(input("OUT_OF_RANGE", "/id", "id must be a non-nil UUID"));
        }
        if let Some(name) = &self.namespace {
            crate::model::validation::validate_namespace_name(name)
                .map_err(|_| input("OUT_OF_RANGE", "/namespace", "invalid namespace assertion"))?;
        }
        Ok(())
    }
    pub(crate) fn from_mcp(value: &Value) -> Result<Self, InputError> {
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(input(
                "INVALID_COMBINATION",
                "/contractVersion",
                "prepared get requires contractVersion 2",
            ));
        }
        Self::parse(value, None)
    }

    pub(crate) fn from_http(value: &Value, id: MemoryId) -> Result<Self, InputError> {
        Self::parse(value, Some(id))
    }

    fn parse(value: &Value, path: Option<MemoryId>) -> Result<Self, InputError> {
        let fields = value
            .as_object()
            .ok_or_else(|| input("INVALID_TYPE", "", "get arguments must be an object"))?;
        reject_unknown_fields(
            fields,
            if path.is_some() {
                &["namespace", "recordAccess"]
            } else {
                &["contractVersion", "id", "namespace", "recordAccess"]
            },
        )?;
        let id = match path {
            Some(id) => id,
            None => {
                let text = fields
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| input("INVALID_TYPE", "/id", "id must be a UUID string"))?;
                let id = uuid::Uuid::parse_str(text)
                    .map_err(|_| input("INVALID_TYPE", "/id", "id must be a UUID"))?;
                MemoryId::from_uuid(id)
            }
        };
        if id.into_inner().is_nil() {
            return Err(input("OUT_OF_RANGE", "/id", "id must be a non-nil UUID"));
        }
        let namespace = match fields.get("namespace") {
            None | Some(Value::Null) => None,
            Some(Value::String(name)) => {
                crate::model::validation::validate_namespace_name(name).map_err(|_| {
                    input("OUT_OF_RANGE", "/namespace", "invalid namespace assertion")
                })?;
                Some(name.clone())
            }
            _ => {
                return Err(input(
                    "INVALID_TYPE",
                    "/namespace",
                    "namespace must be a string or null",
                ));
            }
        };
        let record_access = match fields.get("recordAccess") {
            None => true,
            Some(Value::Bool(value)) => *value,
            _ => {
                return Err(input(
                    "INVALID_TYPE",
                    "/recordAccess",
                    "recordAccess must be a boolean",
                ));
            }
        };
        Ok(Self {
            #[cfg(test)]
            before_barrier: None,
            #[cfg(test)]
            before_observation: None,
            id,
            namespace,
            record_access,
            original_contract_version: 2,
        })
    }
}

fn input(code: &'static str, field: &str, message: &'static str) -> InputError {
    InputError {
        code,
        field: field.into(),
        message,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetData {
    pub evaluated_at: i64,
    pub record_access: bool,
    pub memory: MemoryResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetReply {
    #[serde(flatten)]
    pub envelope: Envelope<GetData>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub observation_receipts: Vec<WriteReceipt>,
    #[serde(skip)]
    pub http_status: u16,
}

impl GetReply {
    pub(crate) fn success(scope: Scope, at: GenerationRef, data: GetData) -> Self {
        let mut envelope = Envelope::success(scope, data);
        envelope.read_generation = Some(at);
        Self {
            envelope,
            observation_receipts: Vec::new(),
            http_status: 200,
        }
    }

    pub(crate) fn input(error: InputError) -> Self {
        Self::from_failure(super::query_response::QueryReply::input(error))
    }

    pub(crate) fn failure(error: SearchError) -> Self {
        let error = match error {
            SearchError::CanonicalRead(error)
                if matches!(*error, crate::storage::StorageError::NotFound(_)) =>
            {
                if let crate::storage::StorageError::NotFound(id) = *error {
                    SearchError::MemoryNotFound(MemoryId::from_uuid(id))
                } else {
                    unreachable!()
                }
            }
            error => error,
        };
        Self::from_failure(super::query_response::QueryReply::failure(error))
    }

    fn from_failure(mut failure: super::query_response::QueryReply) -> Self {
        let mut error = failure.envelope.error.take().expect("typed failure");
        if error.commit_state == CommitState::Unknown {
            error.code = "COMMIT_OUTCOME_UNKNOWN".into();
        }
        if error.message == "query could not complete" {
            error.message = "get could not complete".into();
        }
        let mut envelope = Envelope::failure(failure.envelope.scope, error);
        envelope.receipt = failure.envelope.receipt;
        envelope.read_generation = failure.envelope.read_generation;
        envelope.readiness = failure.envelope.readiness;
        envelope.warnings = failure.envelope.warnings;
        Self {
            envelope,
            observation_receipts: failure.observation_receipts,
            http_status: failure.http_status,
        }
    }

    pub(crate) fn mcp(&self) -> crate::mcp::protocol::ToolCallResult {
        let value = serde_json::to_value(self).expect("serializable get DTO");
        let mut result = crate::mcp::protocol::ToolCallResult::text(value.to_string());
        result.structured_content = value.as_object().cloned();
        result.is_error = Some(self.http_status >= 400);
        result
    }

    pub(crate) fn fits(&self, limit: usize, will_observe: bool) -> bool {
        // One point exposure has one payload-free record transition, two bounded
        // numerical lifecycle states, two SHA arrays and at most one old event.
        // Reserve covers both receipt aliases in both MCP representations plus
        // four-view readiness. The actual escaping of the payload is counted.
        // The RPC caller must supply its negotiated private frame limit, not
        // merely the larger outer daemon ceiling. Arbitrary outer MCP IDs are
        // not certified by this internal reply check.
        let reserve = if will_observe { 64 * 1024 } else { 1024 };
        serde_json::to_vec(&self.mcp())
            .ok()
            .and_then(|bytes| bytes.len().checked_add(reserve))
            .is_some_and(|size| size <= limit.min(crate::daemon::protocol::MAX_MESSAGE_SIZE))
    }

    pub(crate) fn oversize(mut self) -> Self {
        self.envelope.data = None;
        self.envelope.status = Status::Error;
        self.http_status = 400;
        self.envelope.error = Some(ExecutionError {
            category: ErrorCategory::InvalidInput,
            code: "RESPONSE_TOO_LARGE".into(),
            message: "whole get response exceeds transport capacity; no payload was truncated"
                .into(),
            field: None,
            retryable: false,
            commit_state: if self.observation_receipts.is_empty() {
                CommitState::NotCommitted
            } else {
                CommitState::Committed
            },
            operation_id: None,
        });
        self
    }
}
