//! Prepared by-ID wire contract. Public version admission remains closed.
use super::contract::*;
use crate::model::MemoryId;
use crate::search::{MemoryResponse, SearchError};
use crate::storage::control::{GenerationRef, WriteReceipt};
use serde::Serialize;
use serde_json::Value;

// One observed UUID: two receipt aliases, each <=4096 compact JSON bytes;
// fixed completion/error/readiness fields <=8192. MCP contains structured JSON
// plus its escaped compact text (together <=3x); u64 daemon framing <=1024.
// 3 * (2 * 4096 + 8192) + 1024 = 50176 < 65536. The derivation and typed
// maximal-state checks are in get_tests.rs and RESPONSE-BOUND.md. This is not
// a bound for arbitrary outer MCP IDs or a future receivedEvidence container.
pub(crate) const POINT_EVIDENCE_RESERVE: usize = 64 * 1024;
pub(crate) const POINT_FRAME_RESERVE: usize = 1024;

#[derive(Debug, Clone)]
pub struct GetRequest {
    #[cfg(test)]
    pub(crate) before_barrier: Option<fn(&crate::derived_completion::DerivedCompletion)>,
    #[cfg(test)]
    pub(crate) before_observation: Option<(
        std::sync::Arc<tokio::sync::Notify>,
        std::sync::Arc<tokio::sync::Notify>,
    )>,
    #[cfg(test)]
    pub(crate) copy_pause: Option<std::sync::Arc<PointCopyPause>>,
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
            #[cfg(test)]
            copy_pause: None,
            id,
            namespace,
            record_access,
            original_contract_version: 2,
        })
    }
}

/// Owned test latch; bounded fallback prevents a cancelled test from stranding
/// a blocking worker. It is never a serving admission or authority mechanism.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct PointCopyPause {
    pub entered: tokio::sync::Notify,
    released: std::sync::Mutex<bool>,
    wake: std::sync::Condvar,
    pub timed_out: std::sync::atomic::AtomicBool,
}
#[cfg(test)]
impl PointCopyPause {
    pub(crate) fn pause(&self) -> Result<(), crate::storage::StorageError> {
        self.entered.notify_one();
        let (released, _) = self
            .wake
            .wait_timeout_while(
                self.released.lock().unwrap(),
                std::time::Duration::from_secs(5),
                |v| !*v,
            )
            .unwrap();
        if !*released {
            self.timed_out
                .store(true, std::sync::atomic::Ordering::SeqCst);
            return Err(crate::storage::StorageError::RecoveryRequired(
                "test point-copy latch expired".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
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

    pub(crate) fn input(mut error: InputError) -> Self {
        // A caller-controlled unknown key may expand when JSON-pointer escaped.
        // Reject the whole request with a bounded diagnostic, before any work;
        // never truncate a committed receipt or a selected payload to make room.
        if error.field.len() > 256 {
            error = input(
                error.code,
                "",
                "get argument field exceeds diagnostic capacity",
            );
        }
        Self::from_failure(super::query_response::QueryReply::input(error))
    }

    pub(crate) fn failure(error: SearchError) -> Self {
        let precommit_conflict = matches!(&error, SearchError::Observation(observation)
            if observation.cause == Some(crate::decay::observation::ObservationCause::LifecycleConflict)
                && observation.commit_state == "notCommitted"
                && observation.receipts.is_empty()
                && observation.unknown_operation_id.is_none());
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
        let mut reply = Self::from_failure(super::query_response::QueryReply::failure(error));
        if precommit_conflict {
            let error = reply.envelope.error.as_mut().expect("typed failure");
            error.category = ErrorCategory::Conflict;
            error.code = "LIFECYCLE_CONFLICT".into();
            reply.http_status = 409;
        }
        reply
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
        let reserve = if will_observe {
            POINT_EVIDENCE_RESERVE
        } else {
            POINT_FRAME_RESERVE
        };
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

#[cfg(test)]
#[path = "get_tests.rs"]
mod tests;
