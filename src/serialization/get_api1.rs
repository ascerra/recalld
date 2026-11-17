//! Ordinary API1 point projection. The captured owner, not the API version,
//! chooses legacy or native semantics; public success remains MemoryRecord.
use super::contract::{CommitState, ErrorCategory, Readiness, Scope};
use super::get::{GetReply, GetRequest};
use crate::mcp::bridge::{BridgeError, MemoryRecord};
use crate::mcp::protocol::ToolCallResult;
use crate::model::MemoryId;
use crate::storage::control::{GenerationRef, WriteReceipt};
use serde::Serialize;

// Source-derived singleton receipt/completion algebra and actual emitter proof:
// docs/implementation/api1-point-get-runtime-handoff.md. Not an arbitrary error
// string bound or an arbitrary outer MCP request-ID allowance.
pub(crate) const COMPLETION_RESERVE: usize = 80 * 1024;
pub(crate) const MAX_FRAGMENTS: usize = 128;

#[derive(Debug, Clone)]
pub struct Request {
    pub id: MemoryId,
    pub namespace: Option<String>,
    pub record_access: bool,
}
impl Request {
    pub(crate) fn point(&self) -> GetRequest {
        GetRequest {
            id: self.id,
            namespace: self.namespace.clone(),
            record_access: self.record_access,
            original_contract_version: 1,
            #[cfg(test)]
            before_barrier: None,
            #[cfg(test)]
            before_observation: None,
            #[cfg(test)]
            copy_pause: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Native,
    Legacy,
}
impl Mode {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Legacy => "legacy",
        }
    }
}

#[derive(Debug)]
pub enum Reply {
    /// Only an actual owner or a fully validated private result enters here.
    #[doc(hidden)]
    Point {
        reply: Box<GetReply>,
        timezone: chrono_tz::Tz,
        native: bool,
    },
    /// A valid old-peer capability absence selects this lane before sending.
    #[doc(hidden)]
    Compatibility(Result<Option<MemoryRecord>, BridgeError>),
    /// Only the failed private decoder supplies original admitted raw fragments.
    #[doc(hidden)]
    Unverified {
        reply: Box<GetReply>,
        receipts_json: Vec<String>,
    },
}
impl Reply {
    pub(crate) fn point(reply: GetReply, timezone: chrono_tz::Tz, mode: Mode) -> Self {
        Self::Point {
            reply: Box::new(reply),
            timezone,
            native: mode == Mode::Native,
        }
    }
    pub(crate) fn unavailable(access: bool, sent: bool) -> Self {
        Self::Unverified {
            reply: Box::new(GetReply::remote_failure(access, sent, vec![])),
            receipts_json: vec![],
        }
    }
    pub(crate) fn mcp(self, id_spelling: &str, scoped: bool) -> ToolCallResult {
        match self {
            Self::Compatibility(result) => match result {
                Ok(Some(record)) => ToolCallResult::json(&record)
                    .unwrap_or_else(|_| ToolCallResult::error("Memory serialization failed")),
                Ok(None) => ToolCallResult::error(format!("Memory not found: {id_spelling}")),
                Err(error) => ToolCallResult::error(format!("Failed to get memory: {error}")),
            },
            Self::Point {
                reply, timezone, ..
            } => {
                if reply.http_status == 404 && reply.observation_receipts.is_empty() {
                    return ToolCallResult::error(if scoped {
                        format!("Failed to get memory: not found: {id_spelling}")
                    } else {
                        format!("Memory not found: {id_spelling}")
                    });
                }
                project(&reply, timezone, &[])
            }
            Self::Unverified {
                reply,
                receipts_json,
            } => project(&reply, chrono_tz::UTC, &receipts_json),
        }
    }
}

pub(crate) fn memory(m: &crate::search::MemoryResponse, timezone: chrono_tz::Tz) -> MemoryRecord {
    MemoryRecord {
        labels: m.labels.clone(),
        id: m.id.to_string(),
        namespace: m.namespace.clone(),
        summary: m.summary.clone().unwrap_or_default(),
        full_text: m.full_text.clone(),
        tags: m.tags.iter().map(ToString::to_string).collect(),
        phase: format!("{:?}", m.phase),
        strength: m.strength,
        stability: m.stability,
        created_at: crate::time::format_timestamp(m.created_at, timezone),
        last_accessed_at: crate::time::format_timestamp(m.last_accessed_at, timezone),
        is_permastore: m.is_permastore,
        edge_count: m.edge_count,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawEvidence<'a> {
    receipts_json: &'a [String],
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Failure<'a> {
    error: &'a str,
    category: &'a ErrorCategory,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'a str>,
    retryable: bool,
    commit_state: &'a CommitState,
    ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'a Scope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt: Option<&'a WriteReceipt>,
    observation_receipts: &'a [WriteReceipt],
    #[serde(skip_serializing_if = "Option::is_none")]
    read_generation: Option<&'a GenerationRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readiness: Option<&'a Readiness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation_id: Option<uuid::Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    received_evidence: Option<RawEvidence<'a>>,
    warnings: &'a [String],
}

pub(crate) fn project(reply: &GetReply, timezone: chrono_tz::Tz, raw: &[String]) -> ToolCallResult {
    if let Some(error) = &reply.envelope.error {
        let failure = Failure {
            error: &error.code,
            category: &error.category,
            message: &error.message,
            field: error.field.as_deref(),
            retryable: error.retryable,
            commit_state: &error.commit_state,
            ready: false,
            scope: reply.envelope.scope.as_ref(),
            receipt: reply.envelope.receipt.as_ref(),
            observation_receipts: &reply.observation_receipts,
            read_generation: reply.envelope.read_generation.as_ref(),
            readiness: reply.envelope.readiness.as_ref(),
            operation_id: error.operation_id.or_else(|| {
                (error.commit_state == CommitState::Committed)
                    .then(|| reply.envelope.receipt.as_ref().map(|r| r.operation_id))
                    .flatten()
            }),
            received_evidence: (!raw.is_empty()).then_some(RawEvidence { receipts_json: raw }),
            warnings: &reply.envelope.warnings,
        };
        // Direct typed text preserves known receipt bits; opaque unverified raw
        // strings preserve the admitted peer bytes through both public forms.
        let mut result =
            ToolCallResult::text(serde_json::to_string(&failure).expect("typed failure"));
        result.structured_content = serde_json::to_value(&failure)
            .expect("typed failure")
            .as_object()
            .cloned();
        result.is_error = Some(true);
        result
    } else {
        ToolCallResult::json(&memory(
            &reply.envelope.data.as_ref().expect("point payload").memory,
            timezone,
        ))
        .expect("finite point payload")
    }
}

pub(crate) fn tagged(reply: &GetReply, timezone: chrono_tz::Tz, mode: Mode) -> serde_json::Value {
    serde_json::json!({"wireVersion":1,"kind":mode.name(),"httpStatus":reply.http_status,
        "timezone":timezone.name(),"replyJson":serde_json::to_string(reply).expect("typed point")})
}

pub(crate) fn fits(
    reply: &GetReply,
    timezone: chrono_tz::Tz,
    mode: Mode,
    limit: usize,
    observe: bool,
) -> bool {
    use crate::daemon::protocol::{DaemonResponse, MAX_MESSAGE_SIZE};
    let reserve = if observe { COMPLETION_RESERVE } else { 1024 };
    let Some(inner) = limit.checked_sub(reserve) else {
        return false;
    };
    let raw = serde_json::to_vec(reply).expect("typed point");
    let tagged = DaemonResponse::success(u64::MAX, tagged(reply, timezone, mode));
    let public = DaemonResponse::success(
        u64::MAX,
        serde_json::to_value(project(reply, timezone, &[])).expect("point projection"),
    );
    crate::daemon::prepared_query::shape::check_typed(&raw, inner).is_ok()
        && serde_json::to_vec(&tagged)
            .expect("point frame")
            .len()
            .checked_add(reserve)
            .is_some_and(|n| n <= limit)
        && serde_json::to_vec(&public)
            .expect("point projection frame")
            .len()
            .checked_add(reserve)
            .is_some_and(|n| n <= MAX_MESSAGE_SIZE)
}
