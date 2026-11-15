//! API1 projection over the existing native compound producer. No persisted codec.
use super::{
    contract::*,
    native_batch::{self, BatchFailure, NativeBatchOutcome},
};
use crate::{mcp::bridge::BridgeError, storage::batch::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Surface {
    Mcp,
    Rest,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub input: BatchInput,
    pub surface: Surface,
}
impl Request {
    pub(crate) fn validate(&self) -> Result<(), BridgeError> {
        let r = &self.input;
        if r.kind != BatchKind::Insert
            || r.mutation_version != 1
            || r.result_version != 1
            || r.request_context.is_some()
            || r.expected_generation.is_some()
            || r.items.is_empty()
            || r.items.len() > crate::model::constants::MAX_BATCH_MEMORIES
        {
            return Err(BridgeError::InvalidInput(
                "invalid public API1 insertion batch selectors or size".into(),
            ));
        }
        if self.surface == Surface::Rest
            && (r.store_id.is_some()
                || r.idempotency_key.is_some()
                || r.batch_operation_id.is_some())
        {
            return Err(BridgeError::InvalidInput(
                "REST API1 batch does not select retry keys".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn legacy(&self) -> Result<(), BridgeError> {
        self.validate()?;
        if self.surface != Surface::Mcp
            || self
                .input
                .items
                .iter()
                .any(|v| v.get("embeddingProfileId").is_some())
        {
            return Err(BridgeError::InvalidInput("UNSUPPORTED_CONSTRAINT: native batch capability required for explicit profile/REST selection".into()));
        }
        Ok(())
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
    Legacy(Result<BatchOutcome, String>),
    Owned(Result<NativeBatchOutcome, BatchFailure>),
    // An incomplete remote projection cannot erase already decoded evidence.
    Uncertain(Option<NativeBatchOutcome>),
}
impl Reply {
    /// Validate mutable display fields before daemon or public serialization.
    /// Canonical receipts and operation identities are never shortened.
    pub(crate) fn owned(result: Result<NativeBatchOutcome, BatchFailure>) -> Self {
        match result {
            Ok(mut out) if !display_bounds(&out) => {
                for item in &mut out.outcome.items {
                    item.result = None;
                    if item.error.as_ref().is_some_and(|e| !diagnostic_bounds(e)) {
                        item.error = Some(BatchError::new("INVALID_BATCH_DISPLAY"));
                    }
                    item.ready = false;
                }
                out.readiness = None;
                out.warnings = vec!["BATCH_DISPLAY_OMITTED".into(), "READINESS_UNKNOWN".into()];
                Self::Uncertain(Some(out))
            }
            Err(error) => Self::Owned(Err(bounded_failure(error))),
            other => Self::Owned(other),
        }
    }
    pub(crate) fn invalid(e: BridgeError) -> Self {
        Self::Owned(Err(BatchFailure::from_bridge(e)))
    }
    pub(crate) fn decode(value: Value, request: &Request) -> Self {
        // Decode evidence independently of optional live readiness. A malformed
        // live wrapper must not erase an already typed canonical outcome.
        let retained = value
            .get("result")
            .and_then(|v| v.get("Ok"))
            .and_then(|v| serde_json::from_value::<BatchOutcome>(v.clone()).ok())
            .map(NativeBatchOutcome::from);
        let failed = value.get("result").and_then(|v| v.get("Err"));
        let receipt = failed.and_then(|v| v.get("receipt")).and_then(|v| {
            serde_json::from_value::<crate::storage::control::WriteReceipt>(v.clone()).ok()
        });
        let reservation = failed
            .and_then(|v| v.get("reservationReceipt"))
            .and_then(|v| {
                serde_json::from_value::<crate::storage::control::WriteReceipt>(v.clone()).ok()
            });
        match serde_json::from_value::<Self>(value) {
            Ok(Self::Owned(Ok(out))) if coherent(&out, request.input.items.len()) => {
                Self::owned(Ok(out))
            }
            Ok(Self::Owned(Ok(out))) | Ok(Self::Uncertain(Some(out))) => Self::Uncertain(Some(out)),
            Ok(Self::Owned(Err(error))) => Self::owned(Err(error)),
            Ok(Self::Legacy(v)) if request.legacy().is_ok() => Self::Legacy(v),
            _ => {
                if let Some(out) = retained {
                    Self::Uncertain(Some(out))
                } else if receipt.is_some() || reservation.is_some() {
                    let mut failure = BatchFailure::uncertain();
                    failure.receipt = receipt;
                    failure.reservation_receipt = reservation;
                    failure.error.commit_state = CommitState::Committed;
                    failure.warnings.push("READINESS_UNKNOWN".into());
                    Self::Owned(Err(failure))
                } else {
                    Self::Uncertain(None)
                }
            }
        }
    }
    pub(crate) fn mcp(self) -> crate::mcp::protocol::ToolCallResult {
        use crate::mcp::protocol::ToolCallResult;
        if let Self::Legacy(result) = self {
            return match result {
                Ok(out) => crate::mcp::tools::durable_batch_result(out, BatchKind::Insert, &[]),
                Err(e) => ToolCallResult::error(e),
            };
        }
        let (status, value) = self.response(Surface::Mcp);
        let text = serde_json::to_string(&value).expect("finite typed batch response");
        let mut out = ToolCallResult::text(text);
        out.structured_content = value.as_object().cloned();
        if status != 200 {
            out.is_error = Some(true);
        }
        out
    }
    pub(crate) fn response(self, surface: Surface) -> (u16, Value) {
        match self {
            Self::Owned(Ok(out)) => {
                if !display_bounds(&out) {
                    return Self::owned(Ok(out)).response(surface);
                }
                if !coherent(&out, out.outcome.total) {
                    return Self::Uncertain(Some(out)).response(surface);
                }
                let states: Vec<_> = out
                    .outcome
                    .items
                    .iter()
                    .map(|i| match i.status {
                        BatchStatus::Ok => ItemState::Ok { ready: i.ready },
                        BatchStatus::Error => ItemState::Error,
                        BatchStatus::Unknown => ItemState::Unknown,
                        BatchStatus::NotAttempted => ItemState::NotAttempted,
                    })
                    .collect();
                let Ok((_, _, mut status)) = BatchCounts::classify(&states) else {
                    return Self::Uncertain(Some(out)).response(surface);
                };
                if status == 200 && !out.readiness.as_ref().is_some_and(|r| r.ready) {
                    status = 207;
                }
                let o = &out.outcome;
                let mut results = Vec::with_capacity(o.items.len());
                let mut created = Vec::new();
                let mut failed = Vec::new();
                for i in &o.items {
                    let mut result = i.result.clone().unwrap_or_else(|| json!({}));
                    let Some(obj) = result.as_object_mut() else {
                        return Self::Uncertain(Some(out)).response(surface);
                    };
                    obj.insert("index".into(), json!(i.index));
                    obj.insert("status".into(), json!(i.status));
                    obj.insert("ready".into(), json!(i.ready));
                    if let Some(e) = &i.error {
                        obj.insert("error".into(), json!(e.code));
                        obj.insert("errorDetail".into(), json!(e));
                    }
                    if i.status == BatchStatus::Ok {
                        let mut c = json!({"id":result["id"],"namespace":result["namespace"]});
                        for key in ["supersedes", "nearDuplicates"] {
                            if let Some(v) = result.get(key) {
                                c[key] = v.clone();
                            }
                        }
                        created.push(c);
                    } else if i.status == BatchStatus::Error {
                        failed.push(json!({"index":i.index,"error":i.error.as_ref().map(|e|e.code.as_str()).unwrap_or("INVALID_INPUT"),"field":i.error.as_ref().and_then(|e|e.field.as_ref())}));
                    }
                    results.push(result);
                }
                // Full immutable receipts occur once, in batch. Compatibility
                // results duplicate bounded advisory data, never receipt bodies.
                let mut value = if surface == Surface::Mcp {
                    json!({"results":results,"total":o.total,"stored":o.succeeded,"errors":o.failed})
                } else {
                    let mut data = json!({"created":created,"total":o.succeeded});
                    if !failed.is_empty() {
                        data["failed"] = json!(failed);
                    }
                    json!({"data":data})
                };
                value["batch"] = canonical(o);
                if let Some(r) = &out.readiness {
                    value["readiness"] = canonical(r);
                }
                if !out.warnings.is_empty() {
                    value["warnings"] = json!(out.warnings);
                }
                if status != 200 {
                    value["error"] = json!({"code":"BATCH_NOT_READY_OR_PARTIAL","commitState":if o.reservation_state==BatchReservationState::Unknown {"unknown"}else{"committed"},"message":"inspect every indexed outcome; committed children remain committed","retryable":false});
                }
                (
                    if surface == Surface::Rest && status == 200 {
                        201
                    } else {
                        status
                    },
                    value,
                )
            }
            Self::Owned(Err(error)) => {
                let (status, full) = native_batch::response(Err(bounded_failure(error)));
                let mut value = canonical(&full);
                let obj = value.as_object_mut().unwrap();
                // API1 failure retains typed evidence without advertising API2.
                for key in ["contractVersion", "scope", "status"] {
                    obj.remove(key);
                }
                (status, value)
            }
            Self::Uncertain(out) => {
                let mut value = json!({"error":{"code":"COMMIT_OUTCOME_UNKNOWN","commitState":"unknown","retryable":false,"message":"batch reply incomplete; reconcile original request and identity before retry"}});
                if let Some(mut out) = out {
                    for i in &mut out.outcome.items {
                        i.ready = false;
                    }
                    if out.outcome.reservation_receipt.is_some()
                        || out.outcome.items.iter().any(|i| i.receipt.is_some())
                    {
                        value["error"]["commitState"] = json!("committed");
                    }
                    value["batch"] = canonical(&out.outcome);
                    value["warnings"] = if out.warnings.is_empty() {
                        json!(["READINESS_UNKNOWN"])
                    } else {
                        json!(out.warnings)
                    };
                }
                (503, value)
            }
            Self::Legacy(_) => Self::Uncertain(None).response(surface),
        }
    }
}
fn bounded_failure(mut failure: BatchFailure) -> BatchFailure {
    let mut omitted = false;
    if failure.error.message.len() > 512 {
        failure.error.message =
            "batch error detail exceeds display limit; inspect typed code and retained evidence"
                .into();
        omitted = true;
    }
    if failure.error.code.len() > 64
        || !failure
            .error
            .code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b == b'_')
    {
        failure.error.code = "INVALID_BATCH_ERROR".into();
        omitted = true;
    }
    if failure.error.field.as_ref().is_some_and(|s| s.len() > 128) {
        failure.error.field = None;
        omitted = true;
    }
    if failure.warnings.len() > 1 || failure.warnings.iter().any(|w| w != "READINESS_UNKNOWN") {
        failure.warnings = vec!["READINESS_UNKNOWN".into()];
        omitted = true;
    }
    if omitted {
        failure.warnings.push("ERROR_DETAIL_OMITTED".into());
    }
    failure
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Advisory {
    requested: bool,
    read_generation: Option<crate::storage::control::GenerationRef>,
    warnings: Vec<String>,
}
fn diagnostic_bounds(error: &BatchError) -> bool {
    error.code.len() <= 64
        && error
            .code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b == b'_')
        && error.field.as_ref().is_none_or(|s| {
            s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._[]".contains(&b))
        })
}
fn display_bounds(out: &NativeBatchOutcome) -> bool {
    use crate::model::constants::*;
    if out.warnings.len() > 1 || out.warnings.iter().any(|w| w != "READINESS_UNKNOWN") {
        return false;
    }
    out.outcome.items.iter().all(|i| {
        if i.error.as_ref().is_some_and(|e| !diagnostic_bounds(e)) {
            return false;
        }
        let Some(value) = &i.result else { return true };
        let Some(object) = value.as_object() else {
            return false;
        };
        if object.keys().any(|k| {
            ![
                "id",
                "namespace",
                "phase",
                "strength",
                "stability",
                "createdAt",
                "supersedes",
                "nearDuplicates",
                "advisory",
            ]
            .contains(&k.as_str())
        }) {
            return false;
        }
        let Ok(stored) = serde_json::from_value::<crate::mcp::bridge::StoredMemory>(value.clone())
        else {
            return false;
        };
        if stored.id.parse::<uuid::Uuid>().is_err()
            || stored.namespace.len() > NAMESPACE_NAME_MAX_BYTES
            || stored.created_at.len() > 64
            || stored.phase != "Full"
            || !stored.strength.is_finite()
            || !stored.stability.is_finite()
            || stored.stability <= 0.
        {
            return false;
        }
        if stored.supersedes.as_ref().is_some_and(|s| {
            s.detail.is_some() || !matches!(s.status, crate::model::SupersedesStatus::Applied)
        }) {
            return false;
        }
        if let Some(report) = &stored.near_duplicates {
            let mut ids = std::collections::BTreeSet::new();
            if report.matches.len() > MAX_NEAR_DUPLICATE_MATCHES
                || report.threshold != DUPLICATE_SIMILARITY_THRESHOLD
                || report.matches.iter().any(|m| {
                    m.summary.len() > NEAR_DUPLICATE_SUMMARY_MAX_BYTES
                        || !m.score.is_finite()
                        || !(DUPLICATE_SIMILARITY_THRESHOLD as f32..=1.).contains(&m.score)
                        || !ids.insert(m.id)
                })
            {
                return false;
            }
        }
        if let Some(value) = object.get("advisory") {
            let Ok(advisory) = serde_json::from_value::<Advisory>(value.clone()) else {
                return false;
            };
            let _ = (advisory.requested, advisory.read_generation);
            if advisory.warnings.len() > 1
                || advisory.warnings.iter().any(|w| {
                    !["ADVISORY_UNAVAILABLE", "ADVISORY_CONTENT_UNAVAILABLE"].contains(&w.as_str())
                })
            {
                return false;
            }
        }
        true
    })
}
fn canonical(v: &impl Serialize) -> Value {
    serde_json::from_slice(&serde_json::to_vec(v).expect("typed finite response")).unwrap()
}
fn coherent(out: &NativeBatchOutcome, n: usize) -> bool {
    let o = &out.outcome;
    o.total == n
        && o.items.len() == n
        && !o.batch_operation_id.is_nil()
        && !o.store_id.is_nil()
        && o.items.iter().enumerate().all(|(index, i)| {
            i.index as usize == index
                && (i.status != BatchStatus::Ok || i.receipt.as_ref().is_some_and(|receipt| {
                    let crate::storage::control::MutationOutcome::NativeMemoryInsertedWithLinks {
                        transition,
                    } = &receipt.outcome
                    else {
                        return false;
                    };
                    receipt.store_id == o.store_id
                        && i.operation_id == Some(receipt.operation_id)
                        && i.namespace_id == Some(transition.namespace_id)
                        && i.result.as_ref().is_some_and(|r| {
                            r.get("id") == Some(&json!(transition.memory_id))
                                && r.get("namespace").and_then(Value::as_str).is_some()
                        })
                }))
        })
        && o.succeeded
            == o.items
                .iter()
                .filter(|i| i.status == BatchStatus::Ok)
                .count()
        && o.failed
            == o.items
                .iter()
                .filter(|i| i.status == BatchStatus::Error)
                .count()
        && o.succeeded
            .checked_add(o.failed)
            .and_then(|n| n.checked_add(o.unresolved))
            == Some(o.total)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Count the actual serializer, without allocating an encoded packet first.
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.checked_add(bytes.len()).unwrap();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    pub(super) fn bytes(value: &impl Serialize) -> usize {
        let mut count = Count(0);
        serde_json::to_writer(&mut count, value).unwrap();
        count.0
    }
    #[test]
    fn api1_batch_derived_compatibility_bound_counts_all_shape_fields() {
        let id = "ffffffff-ffff-ffff-ffff-ffffffffffff";
        // Valid scalar/display field ranges and counts, with maximal escaping.
        // This is a shape proof, not an authored canonical record/publication.
        let mut result = json!({"id":id,"namespace":"n".repeat(crate::model::constants::NAMESPACE_NAME_MAX_BYTES),"phase":"Full","strength":1.,"stability":f32::MAX,"createdAt":"+262142-12-31 23:59:59 +1245","supersedes":{"target":id,"status":"applied"},"nearDuplicates":{"threshold":0.85,"matches":(0..crate::model::constants::MAX_NEAR_DUPLICATE_MATCHES).map(|n|json!({"id":uuid::Uuid::from_u128(n as u128+1),"score":0.99999994,"summary":"\u{1}".repeat(crate::model::constants::NEAR_DUPLICATE_SUMMARY_MAX_BYTES)})).collect::<Vec<_>>()},"advisory":{"requested":true,"readGeneration":{"storeId":id,"generation":"9223372036854775807"},"warnings":["ADVISORY_CONTENT_UNAVAILABLE"]},"index":99,"status":"ok","ready":false,"error":"INDEX_NOT_READY","errorDetail":{"code":"INDEX_NOT_READY","field":"x".repeat(128)}});
        let measured = bytes(&result);
        assert!(measured <= crate::mcp::batch_budget::API1_COMPAT_ITEM_BYTES);
        // The original per-item allowance still covers the non-duplicated
        // runtime result and its indexed status/error metadata.
        for key in ["index", "status", "ready", "error", "errorDetail"] {
            result.as_object_mut().unwrap().remove(key);
        }
        assert!(bytes(&result) < 8 * 1024);
        println!(
            "API1 compatibility measured={measured} derived_bound={}",
            crate::mcp::batch_budget::API1_COMPAT_ITEM_BYTES
        );
    }
}
