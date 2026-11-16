//! API1 native purge projection: full known evidence, no inferred readiness.
use super::{McpNamespaceAdapter, bridge, map_mutation_error};
use crate::derived_completion::{DerivedCompletion, PurgeApi1Failure};
use crate::storage::StorageError;
use crate::storage::control::{MutationOutcome, WriteReceipt};
use crate::storage::purge::PurgeStage;
use serde_json::{Value, json};
use uuid::Uuid;

/// Prove a compact receipt fits inside the actual escaped daemon error frame.
/// A serialized JSON value has no literal control bytes: escaping it as part of
/// error.message adds at most one byte per byte (quotes/backslashes), hence 2*N.
/// The independently serialized envelope includes the longest fields, all
/// optional fields, max-width response ID, and 512 maximally escaped text bytes.
pub(crate) fn receipt_budget(frame_limit: usize) -> Result<usize, bridge::BridgeError> {
    use crate::daemon::protocol::{DaemonResponse, DaemonRpcError};
    let too_large =
        || bridge::BridgeError::TooLarge(StorageError::PurgeResponseTooLarge.to_string());
    let envelope = json!({
        "error": "PURGE_METRICS_UNAVAILABLE",
        "commitState": "committed",
        "commitEvidenceScope": "purgeComplete",
        "operationId": Uuid::from_u128(u128::MAX),
        "receipt": null,
        "message": "\0".repeat(512),
    });
    let frame = DaemonResponse::error(
        u64::MAX,
        DaemonRpcError {
            code: -32006,
            message: envelope.to_string(),
        },
    );
    let envelope_bytes = serde_json::to_vec(&frame)
        .map_err(|e| bridge::BridgeError::Internal(e.to_string()))?
        .len();
    let success = bridge::DeleteNamespaceResult {
        name: "\0".repeat(crate::model::constants::NAMESPACE_NAME_MAX_BYTES),
        id: u32::MAX,
        deleted: true,
        memories_deleted: u64::MAX,
        tombstones_purged: u64::MAX,
        edges_removed: u64::MAX,
        fts_rows_removed: u64::MAX,
        vector_file_removed: false,
    };
    let success = DaemonResponse::success(
        u64::MAX,
        serde_json::to_value(success).map_err(|e| bridge::BridgeError::Internal(e.to_string()))?,
    );
    if serde_json::to_vec(&success)
        .map_err(|e| bridge::BridgeError::Internal(e.to_string()))?
        .len()
        > frame_limit
    {
        return Err(too_large());
    }
    // Keeping the four placeholder null bytes also overcounts, rather than
    // depending on a subtraction or silently excluding an envelope field.
    frame_limit
        .checked_sub(envelope_bytes)
        .map(|n| n / 2)
        .filter(|n| *n > 0)
        .ok_or_else(too_large)
}

fn identity(receipt: &WriteReceipt) -> Option<(Uuid, crate::model::NamespaceId, Uuid)> {
    if receipt.receipt_version != 1
        || receipt.store_id.is_nil()
        || receipt.operation_id.is_nil()
        || receipt.committed_generation.get() == 0
    {
        return None;
    }
    match &receipt.outcome {
        MutationOutcome::NativeNamespacePurgeStarted { transition } => {
            transition.validate(receipt).ok()?;
            Some((
                receipt.store_id,
                transition.before_namespace.namespace_id,
                receipt.operation_id,
            ))
        }
        MutationOutcome::NativeNamespacePurgeAdvanced { transition } => {
            transition.validate(receipt).ok()?;
            Some((
                receipt.store_id,
                transition.namespace_id,
                transition.parent_operation_id,
            ))
        }
        _ => None,
    }
}
fn scope(receipt: &WriteReceipt) -> Option<&'static str> {
    identity(receipt)?;
    match &receipt.outcome {
        MutationOutcome::NativeNamespacePurgeStarted { .. } => Some("purgeStart"),
        MutationOutcome::NativeNamespacePurgeAdvanced { transition } => {
            Some(if transition.stage == PurgeStage::Complete {
                "purgeComplete"
            } else {
                "purgeStep"
            })
        }
        _ => None,
    }
}
fn unknown(source: &StorageError) -> Option<Option<Uuid>> {
    match source {
        StorageError::CommitOutcomeUnknown { operation_id, .. } => Some(Some(*operation_id)),
        StorageError::WorkerOutcomeUnknown(_) => Some(None),
        StorageError::CommittedMutation { source, .. } => unknown(source),
        _ => None,
    }
}
fn matched_receipt<'a>(
    source: &'a StorageError,
    known: Option<&WriteReceipt>,
) -> Option<&'a WriteReceipt> {
    if let StorageError::CommittedMutation { receipt, source } = source {
        let candidate = identity(receipt);
        if candidate.is_some()
            && known.map_or(
                matches!(
                    receipt.outcome,
                    MutationOutcome::NativeNamespacePurgeStarted { .. }
                ),
                |known| candidate == identity(known),
            )
        {
            return Some(receipt);
        }
        return matched_receipt(source, known);
    }
    None
}
fn diagnostic(source: &StorageError) -> String {
    // A wrapper's Display serializes entire receipts. Preserve that typed source
    // internally; the bounded diagnostic is only its underlying cause.
    let text = match source {
        StorageError::CommittedMutation { source, .. } => return diagnostic(source),
        other => other.to_string(),
    };
    bounded_message(&text)
}
fn bounded_message(text: &str) -> String {
    if text.len() <= 512 {
        return text.to_owned();
    }
    let mut end = 509;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

pub(crate) fn project(failure: PurgeApi1Failure) -> bridge::BridgeError {
    let (code, state, receipt, operation, message) = match &failure {
        PurgeApi1Failure::NamespaceNotFound(name) => {
            return bridge::BridgeError::NotFound(format!("namespace '{name}' not found"));
        }
        PurgeApi1Failure::MetricsUnavailable { receipt } => (
            "PURGE_METRICS_UNAVAILABLE",
            "committed",
            Some(receipt.as_ref()),
            None,
            bounded_message(&failure.to_string()),
        ),
        PurgeApi1Failure::WorkerUnknown { known_receipt } => (
            "COMMIT_OUTCOME_UNKNOWN",
            "unknown",
            known_receipt.as_deref(),
            Some(None),
            bounded_message(&failure.to_string()),
        ),
        PurgeApi1Failure::Storage {
            source,
            known_receipt,
        } => {
            if known_receipt.is_none() && matches!(source, StorageError::PurgeResponseTooLarge) {
                return bridge::BridgeError::TooLarge(source.to_string());
            }
            let receipt =
                matched_receipt(source, known_receipt.as_deref()).or(known_receipt.as_deref());
            if let Some(operation) = unknown(source) {
                (
                    "COMMIT_OUTCOME_UNKNOWN",
                    "unknown",
                    receipt,
                    Some(operation),
                    diagnostic(source),
                )
            } else if receipt.is_some() {
                (
                    "SERVICE_UNAVAILABLE",
                    "committed",
                    receipt,
                    None,
                    diagnostic(source),
                )
            } else {
                let PurgeApi1Failure::Storage { source, .. } = failure else {
                    unreachable!()
                };
                return map_mutation_error(source);
            }
        }
    };
    let mut payload = json!({"error": code, "commitState": state, "message": message});
    if let Some(receipt) = receipt {
        payload["receipt"] = json!(receipt);
        if let Some(scope) = scope(receipt) {
            payload["commitEvidenceScope"] = json!(scope);
        }
    }
    if let Some(operation) = operation {
        payload["operationId"] = json!(operation);
    }
    bridge::BridgeError::PartiallyApplied(payload.to_string())
}

fn same_fields(expected: &Value, received: &Value) -> bool {
    match (expected, received) {
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, value)| b.get(key).is_some_and(|other| same_fields(value, other)))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_fields(a, b))
        }
        // The typed receipt decoder validates the numeric fields. The default
        // JSON Value parser may round f64 text one ulp while preserving the
        // original f32 bits (e.g. initialStability=3.7145). Do not equate numeric
        // reserialization text with retained proof identity or discard evidence.
        (Value::Number(_), Value::Number(_)) => true,
        _ => expected == received,
    }
}

/// Validate the known payload shape before adding structured MCP classification.
/// This validates returned evidence syntax, not independent server authenticity.
pub(crate) fn structured_failure(message: &str) -> Option<Value> {
    let payload: Value = serde_json::from_str(message).ok()?;
    let object = payload.as_object()?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "error" | "commitState" | "message" | "receipt" | "commitEvidenceScope" | "operationId"
        )
    }) {
        return None;
    }
    if payload.get("message")?.as_str()?.len() > 512 {
        return None;
    }
    let receipt = match payload.get("receipt") {
        Some(value) => {
            let receipt: WriteReceipt = serde_json::from_value(value.clone()).ok()?;
            if !same_fields(&serde_json::to_value(&receipt).ok()?, value)
                || payload.get("commitEvidenceScope")?.as_str()? != scope(&receipt)?
            {
                return None;
            }
            Some(receipt)
        }
        None => {
            if payload.get("commitEvidenceScope").is_some() {
                return None;
            }
            None
        }
    };
    match payload.get("error")?.as_str()? {
        "PURGE_METRICS_UNAVAILABLE" => {
            if payload["commitState"] != "committed"
                || scope(receipt.as_ref()?)? != "purgeComplete"
                || payload.get("operationId").is_some()
            {
                return None;
            }
        }
        "SERVICE_UNAVAILABLE" => {
            receipt.as_ref()?;
            if payload["commitState"] != "committed" || payload.get("operationId").is_some() {
                return None;
            }
        }
        "COMMIT_OUTCOME_UNKNOWN" => {
            if payload["commitState"] != "unknown" {
                return None;
            }
            let id = payload.get("operationId")?;
            if !id.is_null() && Uuid::parse_str(id.as_str()?).ok()?.is_nil() {
                return None;
            }
        }
        _ => return None,
    }
    Some(payload)
}

impl McpNamespaceAdapter {
    pub(super) async fn delete_native_namespace(
        &self,
        name: String,
        force: bool,
        frame_limit: Option<usize>,
    ) -> Result<bridge::DeleteNamespaceResult, bridge::BridgeError> {
        let receipt_limit = frame_limit.map(receipt_budget).transpose()?;
        let completion = DerivedCompletion {
            storage: self.storage.clone(),
            cache: self.cache.clone(),
            graph: self.graph.clone(),
            vectors: self.vector_index.clone(),
            fts: self.fts_index.clone(),
            entities: self.entity_index.clone(),
        };
        let result = completion
            .purge_native_api1_bounded(name, force, receipt_limit)
            .await
            .map_err(project)?;
        let p = result.progress;
        let live = p.removed.checked_sub(p.tombstones_removed).ok_or_else(|| {
            project(PurgeApi1Failure::Storage {
                source: StorageError::InvalidControl("purge count underflow".into()),
                known_receipt: Some(Box::new(p.last_receipt.clone())),
            })
        })?;
        Ok(bridge::DeleteNamespaceResult {
            name: p.name,
            id: p.namespace_id.get(),
            deleted: true,
            memories_deleted: live,
            tombstones_purged: p.tombstones_removed,
            edges_removed: p.edges_removed,
            fts_rows_removed: result.fts_rows_removed as u64,
            vector_file_removed: result.directory_removed,
        })
    }
}
