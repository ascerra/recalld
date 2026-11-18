//! Original API1 store request/projection. Never an application2 envelope.
use super::{contract::*, native_single};
use crate::mcp::bridge::{StoreInput, StoredMemory};
use crate::storage::{StorageError, control::WriteReceipt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub input: StoreInput,
    pub profile_id: Option<String>,
    pub created_at: Option<i64>,
    /// REST has already selected the owned native format. A mismatched/default
    /// adapter must reject before writing through a legacy producer.
    pub require_native: bool,
}
impl Request {
    pub(crate) fn prepare_native(&self) -> Result<native_single::Parsed, InputError> {
        let invalid = |code, field: &str, message| InputError {
            code,
            field: format!("/{field}"),
            message,
        };
        let i = &self.input;
        crate::model::validation::validate_memory_input(crate::model::validation::MemoryInputRef {
            summary: &i.summary,
            full_text: i.full_text.as_deref(),
            tags: &i.tags,
            entities: &i.entities,
            topics: &i.topics,
            emotions: &i.emotions,
        })
        .map_err(|_| invalid("INVALID_INPUT", "", "invalid memory fields"))?;
        if i.initial_stability
            .is_some_and(|s| !s.is_finite() || s <= 0.)
        {
            return Err(invalid(
                "OUT_OF_RANGE",
                "initialStability",
                "initialStability must be finite and positive",
            ));
        }
        if let Some(id) = &self.profile_id {
            if !crate::embedding::profile::valid_profile_id(id) {
                return Err(invalid(
                    "INVALID_INPUT",
                    "embeddingProfileId",
                    "expected an ep1 profile ID",
                ));
            }
        }
        if self.input.embedding.is_none() && self.profile_id.is_some() {
            return Err(invalid(
                "INVALID_INPUT",
                "embeddingProfileId",
                "embeddingProfileId requires a supplied embedding",
            ));
        }
        let mut insert = crate::mcp::batches::native_fields(&self.input, &serde_json::Value::Null)
            .map_err(|_| invalid("INVALID_INPUT", "", "invalid native memory fields"))?;
        insert.memory.created_at = self.created_at;
        // The profile-ID resolver validates the full native input after binding
        // the exact canonical descriptor; do not fabricate one for this parser.
        if self
            .created_at
            .is_some_and(|t| chrono::DateTime::from_timestamp_millis(t).is_none())
        {
            return Err(invalid(
                "INVALID_INPUT",
                "createdAt",
                "createdAt must be representable epoch milliseconds",
            ));
        }
        if let Some(vector) = &self.input.embedding {
            if !(1..=8192).contains(&vector.len()) {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    "embedding",
                    "embedding dimension must be between 1 and 8192",
                ));
            }
            let mut check = vector.clone();
            crate::embedding::validation::normalize_vector(&mut check, vector.len()).map_err(
                |_| {
                    invalid(
                        "OUT_OF_RANGE",
                        "embedding",
                        "supplied vector must have finite nonzero norm",
                    )
                },
            )?;
        }
        Ok(native_single::Parsed {
            options: WriteOptions::default(),
            namespace_id: None,
            asserted_name: Some(self.input.namespace.clone()),
            store: self.input.clone(),
            profile_id: self.profile_id.clone(),
            input: crate::storage::native_store::NativeStoreInput {
                insert,
                check_duplicates: self.input.check_duplicates.unwrap_or(true),
            },
        })
    }
    pub(crate) fn validate_legacy(&self) -> Result<(), crate::mcp::bridge::BridgeError> {
        if self.require_native || self.profile_id.is_some() || self.created_at.is_some() {
            return Err(crate::mcp::bridge::BridgeError::InvalidInput(
                "canonical embedding profile selection requires the native store consumer".into(),
            ));
        }
        let i = &self.input;
        crate::model::validation::validate_legacy_memory_input(
            crate::model::validation::MemoryInputRef {
                summary: &i.summary,
                full_text: i.full_text.as_deref(),
                tags: &i.tags,
                entities: &i.entities,
                topics: &i.topics,
                emotions: &i.emotions,
            },
        )
        .map_err(|e| crate::mcp::bridge::BridgeError::InvalidInput(e.to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(
    tag = "route",
    content = "result",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum Reply {
    Legacy(Result<StoredMemory, String>),
    Owned(native_single::Reply),
    /// A malformed Remote wrapper cannot bind this received receipt to the
    /// current single operation. Preserve evidence without claiming commitment.
    Uncertain(WriteReceipt),
}
impl Reply {
    pub(crate) fn decode(value: serde_json::Value, request: &Request) -> Self {
        let retained = value
            .get("result")
            .and_then(|v| v.get("receipt"))
            .and_then(|v| serde_json::from_value::<WriteReceipt>(v.clone()).ok());
        match serde_json::from_value::<Self>(value) {
            Ok(Self::Uncertain(receipt)) => Self::Uncertain(receipt),
            Ok(Self::Owned(mut reply)) => {
                if reply.error.is_none()
                    && (!reply
                        .namespace
                        .as_ref()
                        .is_some_and(|n| n.eq_ignore_ascii_case(&request.input.namespace))
                        || !reply.readiness.as_ref().is_some_and(|r| r.ready)
                        || memory_response(request, &reply).is_err())
                {
                    let source = StorageError::RecoveryRequired(
                        "API1 store reply identity/content does not match the request".into(),
                    );
                    let source = match reply.receipt.clone() {
                        Some(receipt) => StorageError::CommittedMutation {
                            receipt: Box::new(receipt),
                            source: Box::new(source),
                        },
                        None => source,
                    };
                    reply.error = native_single::Reply::failure(source).error;
                    reply.data = None;
                }
                Self::Owned(reply)
            }
            Ok(Self::Legacy(result)) if !request.require_native && request.profile_id.is_none() => {
                Self::Legacy(result)
            }
            _ => match retained {
                Some(receipt) => Self::Uncertain(receipt),
                None => Self::Owned(native_single::Reply::uncertain()),
            },
        }
    }
    pub(crate) fn unavailable() -> Self {
        Self::Owned(native_single::Reply::failure(
            StorageError::RecoveryRequired("API1 store consumer unavailable".into()),
        ))
    }
    pub(crate) fn mcp(self) -> crate::mcp::protocol::ToolCallResult {
        use crate::mcp::protocol::ToolCallResult;
        match self {
            Self::Legacy(Ok(data)) => ToolCallResult::json(&data)
                .unwrap_or_else(|e| ToolCallResult::error(format!("Serialization error: {e}"))),
            Self::Legacy(Err(e)) => ToolCallResult::error(format!("Failed to store memory: {e}")),
            Self::Uncertain(receipt) => failure_mcp(uncertain_evidence(receipt).1),
            Self::Owned(reply) => {
                if reply.error.is_none()
                    && reply.namespace_id.is_some()
                    && reply.namespace.is_some()
                {
                    if let Some(data) = &reply.data {
                        if let Ok(result) = ToolCallResult::json(data) {
                            return result;
                        }
                    }
                }
                let (_, error) = failure(reply);
                failure_mcp(error)
            }
        }
    }
}

fn failure_mcp(error: Failure) -> crate::mcp::protocol::ToolCallResult {
    let text = serde_json::to_string(&error).expect("finite API1 failure");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut result = crate::mcp::protocol::ToolCallResult::text(text);
    result.structured_content = value.as_object().cloned();
    result.is_error = Some(true);
    result
}

#[derive(Serialize)]
struct ReceivedEvidence {
    receipt: WriteReceipt,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Failure {
    error: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    commit_state: CommitState,
    ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt: Option<WriteReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    received_evidence: Option<ReceivedEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readiness: Option<Readiness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation_id: Option<uuid::Uuid>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}
pub(crate) fn uncertain_evidence(receipt: WriteReceipt) -> (u16, Failure) {
    let (status, mut body) = failure(native_single::Reply::uncertain());
    body.received_evidence = Some(ReceivedEvidence { receipt });
    body.warnings.extend([
        "UNVERIFIED_RECEIVED_EVIDENCE".into(),
        "READINESS_UNKNOWN".into(),
    ]);
    (status, body)
}

pub(crate) fn failure(reply: native_single::Reply) -> (u16, Failure) {
    let (mut status, envelope) = reply.response();
    let error = envelope.error.unwrap_or(ExecutionError {
        category: ErrorCategory::Internal,
        code: "INVALID_RESPONSE".into(),
        message: "API1 store result unavailable".into(),
        field: None,
        retryable: false,
        commit_state: if envelope.receipt.is_some() {
            CommitState::Committed
        } else {
            CommitState::Unknown
        },
        operation_id: None,
    });
    if status == 200 {
        status = 500;
    }
    if error.code == "EMBEDDING_PROFILE_REQUIRED" {
        status = 422;
    }
    let message = if error.commit_state == CommitState::Unknown {
        "store outcome unavailable; reconcile before retrying".into()
    } else {
        error.message
    };
    (
        status,
        Failure {
            error: error.code,
            message,
            field: error.field,
            commit_state: error.commit_state,
            ready: false,
            operation_id: error
                .operation_id
                .or_else(|| envelope.receipt.as_ref().map(|r| r.operation_id)),
            receipt: envelope.receipt,
            received_evidence: None,
            readiness: envelope.readiness,
            warnings: envelope.warnings,
        },
    )
}

/// API1 creation payload from the exact immutable creation state. Content is
/// taken from the fresh request only after matching the retained commitments.
pub(crate) fn memory_response(
    request: &Request,
    reply: &native_single::Reply,
) -> Result<super::MemoryResponse, StorageError> {
    use sha2::{Digest, Sha256};
    let invalid = || {
        StorageError::RecoveryRequired(
            "native API1 creation projection does not match its receipt".into(),
        )
    };
    let receipt = reply.receipt.as_ref().ok_or_else(invalid)?;
    let crate::storage::control::MutationOutcome::NativeMemoryInsertedWithLinks { transition } =
        &receipt.outcome
    else {
        return Err(invalid());
    };
    let after = &transition.primary()?.after;
    let n = &after.lifecycle;
    let mut native = crate::mcp::batches::native_fields(&request.input, &serde_json::Value::Null)
        .map_err(|_| invalid())?;
    native.memory.created_at = request.created_at;
    let labels = native.labels();
    let hash = |b: &[u8]| -> [u8; 32] { Sha256::digest(b).into() };
    if after.content.summary_sha256 != hash(request.input.summary.as_bytes())
        || after.content.tags_sha256 != hash(&crate::storage::control::encode(&native.memory.tags)?)
        || after.content.labels_sha256 != hash(&crate::storage::control::encode(&labels)?)
        || after.content.label_provenance != labels.provenance
        || after.content.text_sha256 != request.input.full_text.as_ref().map(|s| hash(s.as_bytes()))
        || reply.namespace_id != Some(transition.namespace_id)
    {
        return Err(invalid());
    }
    let namespace = reply.namespace.clone().ok_or_else(invalid)?;
    let stored = reply.data.as_ref().ok_or_else(invalid)?;
    if stored.id != transition.memory_id.to_string() || stored.namespace != namespace {
        return Err(invalid());
    }
    Ok(super::MemoryResponse {
        id: transition.memory_id,
        namespace,
        created_at: n.created_at,
        last_accessed_at: n.last_accessed_at,
        summary: request.input.summary.clone(),
        full_text: request.input.full_text.clone(),
        tags: native.memory.tags,
        entities: labels.entities.clone(),
        topics: labels.topics.clone(),
        emotions: labels.emotions.clone(),
        labels: Some(labels),
        phase: n.phase,
        strength: n.strength.get(),
        decay_strength: n.decay_strength.get(),
        stability: n.stability.get(),
        difficulty: n.difficulty.get(),
        is_permastore: n.is_permastore,
        edge_count: after.edge_count,
        embedding: None,
        access_history: None,
        supersedes: stored.supersedes.clone(),
        near_duplicates: stored.near_duplicates.clone(),
    })
}
