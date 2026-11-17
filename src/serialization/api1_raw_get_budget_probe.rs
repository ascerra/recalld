//! New API1 error-only raw carrier diagnostic; original O1 is unchanged.
use super::*;
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

const MAX_FRAGMENTS: usize = 128;
const RESERVE: usize = 80 * 1024;

struct Acquired<'a> {
    raw: Vec<&'a RawValue>,
}

fn acquire(raw: &str, limit: usize) -> Result<Acquired<'_>, ()> {
    // All structure/count/byte admission precedes typed receipt acquisition.
    shape::check_typed(raw.as_bytes(), limit).map_err(|_| ())?;
    let fields: BTreeMap<String, &RawValue> = serde_json::from_str(raw).map_err(|_| ())?;
    if fields.contains_key("receivedEvidence") {
        return Err(());
    }
    let groups: Vec<&RawValue> = fields
        .get("observationReceipts")
        .map(|raw| serde_json::from_str(raw.get()))
        .transpose()
        .map_err(|_| ())?
        .unwrap_or_default();
    let count = usize::from(fields.contains_key("receipt"))
        .checked_add(groups.len())
        .ok_or(())?;
    if count > MAX_FRAGMENTS {
        return Err(());
    }
    let candidates: Vec<_> = fields
        .get("receipt")
        .copied()
        .into_iter()
        .chain(groups)
        .collect();
    let total = candidates
        .iter()
        .try_fold(0usize, |n, r| n.checked_add(r.get().len()))
        .ok_or(())?;
    if total > limit {
        return Err(());
    }
    let mut seen = BTreeSet::new();
    let raw = candidates
        .into_iter()
        .filter(|r| {
            crate::daemon::prepared_query::decode::receipt_raw(r.get()).is_ok()
                && seen.insert(r.get())
        })
        .collect();
    Ok(Acquired { raw })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawEvidence<'a> {
    receipts_json: Vec<&'a str>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Failure<'a> {
    #[serde(flatten)]
    fields: Api1Failure<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    received_evidence: Option<RawEvidence<'a>>,
}

fn output(reply: &GetReply, acquired: &Acquired<'_>) -> ToolCallResult {
    let mut fields = failure_fields(reply);
    fields.received_evidence = None;
    let body = Failure {
        fields,
        received_evidence: (!acquired.raw.is_empty()).then(|| RawEvidence {
            receipts_json: acquired.raw.iter().map(|r| r.get()).collect(),
        }),
    };
    let text = serde_json::to_string(&body).unwrap();
    let value = serde_json::to_value(&body).unwrap();
    let mut result = ToolCallResult::text(text);
    result.structured_content = value.as_object().cloned();
    result.is_error = Some(true);
    result
}

fn failure_for(acquired: &Acquired<'_>) -> GetReply {
    GetReply::remote_failure(
        true,
        true,
        acquired
            .raw
            .iter()
            .map(|r| serde_json::from_str(r.get()).unwrap())
            .collect(),
    )
}

async fn framed(tool: ToolCallResult) -> Value {
    use crate::daemon::protocol::{read_framed_response, write_framed_message};
    let response = DaemonResponse::success(u64::MAX, serde_json::to_value(tool).unwrap());
    let (mut writer, mut reader) = tokio::net::UnixStream::pair().unwrap();
    let task = tokio::spawn(async move {
        write_framed_message(&mut writer, &response, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
    });
    let value = read_framed_response(&mut reader, MAX_MESSAGE_SIZE)
        .await
        .unwrap()
        .result
        .unwrap();
    task.await.unwrap();
    value
}

fn raw_from_public(value: &Value) -> Vec<&str> {
    value["receivedEvidence"]["receiptsJson"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

fn raw_limit() -> (usize, usize) {
    // Count actual new error keys, two representations, and worst admitted
    // fragment-count punctuation using empty RawValue strings. This is only
    // overhead accounting; these nulls are never claimed valid receipts.
    let null: &RawValue = serde_json::from_str("null").unwrap();
    let acquired = Acquired {
        raw: vec![null; MAX_FRAGMENTS],
    };
    let mut overhead = 0;
    for access in [false, true] {
        for sent in [false, true] {
            let mut reply = GetReply::remote_failure(access, sent, vec![]);
            reply
                .envelope
                .warnings
                .push("UNVERIFIED_RECEIVED_EVIDENCE".into());
            let tool = output(&reply, &acquired);
            let frame = serde_json::to_vec(&DaemonResponse::success(
                u64::MAX,
                serde_json::to_value(tool).unwrap(),
            ))
            .unwrap();
            // Keeping null bytes in overhead is deliberately conservative.
            overhead = overhead.max(frame.len() + 4);
        }
    }
    ((MAX_MESSAGE_SIZE - overhead - RESERVE) / 6, overhead)
}

#[tokio::test]
async fn api1_point_raw_actual_frames_preserve_canonical_acquisition() {
    let receipt = maximal_point_receipt();
    let first = serde_json::to_string(&receipt).unwrap();
    let mut other = receipt.clone();
    if let MutationOutcome::LifecycleExposureRecorded { transition } = &mut other.outcome {
        transition.records[0].before.strength = (-0.0f32).try_into().unwrap();
        transition.records[0].after.strength = (-0.0f32).try_into().unwrap();
    }
    let second = serde_json::to_string(&other).unwrap();
    let incoming =
        format!("{{\"receipt\":{first},\"observationReceipts\":[{first},{second},{first}]}}");
    let (limit, _) = raw_limit();
    let acquired = acquire(&incoming, limit).unwrap();
    assert_eq!(
        acquired.raw.iter().map(|r| r.get()).collect::<Vec<_>>(),
        [first.as_str(), second.as_str()]
    );
    let reply = failure_for(&acquired);
    for legacy in [false, true] {
        let mut tool = output(&reply, &acquired);
        if legacy {
            tool.structured_content = None;
        }
        let value = framed(tool).await;
        let text: Value =
            serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap();
        for body in std::iter::once(&text).chain((!legacy).then_some(&value["structuredContent"])) {
            assert_eq!(body["commitState"], "unknown");
            assert!(body.get("operationId").is_none());
            assert!(body.get("receipt").is_none());
            assert!(body.get("contractVersion").is_none());
            assert_eq!(raw_from_public(body), [first.as_str(), second.as_str()]);
            for (raw, original) in raw_from_public(body).iter().zip(&acquired.raw) {
                assert_eq!(
                    serde_json::from_str::<Value>(raw).unwrap(),
                    serde_json::from_str::<Value>(original.get()).unwrap()
                );
                let typed: WriteReceipt = serde_json::from_str(raw).unwrap();
                assert_eq!(serde_json::to_string(&typed).unwrap(), original.get());
            }
        }
    }
    eprintln!(
        "raw API1 actual full/legacy frames preserve both conflicting aliases, canonical bytes, acquired Values and typed bits"
    );
}

#[test]
fn api1_point_raw_admission_rejects_malformed_recursive_and_excess_count() {
    let raw = serde_json::to_string(&maximal_point_receipt()).unwrap();
    let (limit, _) = raw_limit();
    for bad in [
        format!("{{\"receipt\":{raw},\"receipt\":{raw}}}"),
        format!("{{\"receipt\":{raw},\"rece\\u0069pt\":{raw}}}"),
        format!("{{\"receivedEvidence\":{{\"receipt\":{raw}}}}"),
        format!(
            "{{\"observationReceipts\":[{}]}}",
            vec!["null"; MAX_FRAGMENTS + 1].join(",")
        ),
    ] {
        assert!(acquire(&bad, limit).is_err());
    }
    for bad in [
        raw.replacen("\"receiptVersion\":1", "\"receiptVersion\":9", 1),
        raw.replacen(
            "\"replayed\":false",
            "\"replayed\":false,\"unexpected\":1",
            1,
        ),
    ] {
        assert!(
            acquire(&format!("{{\"receipt\":{bad}}}"), limit)
                .unwrap()
                .raw
                .is_empty()
        );
    }
    assert!(acquire(&format!("{{\"receipt\":{raw}}}"), 10).is_err());
}

#[tokio::test]
async fn api1_point_raw_full_emitter_bound_and_foreign_receipt() {
    let (limit, overhead) = raw_limit();
    assert!(6 * limit + overhead + RESERVE <= MAX_MESSAGE_SIZE);
    let mut foreign = maximal_point_receipt();
    foreign.outcome = MutationOutcome::ExposuresRecorded {
        accesses: (1..=11_000)
            .map(|n| crate::storage::control::ExposureOutcome {
                memory_id: MemoryId::from_uuid(uuid::Uuid::from_u128(n)),
                kind: AccessKind::DirectRetrieval,
                accessed_at: chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis(),
            })
            .collect(),
    };
    let raw = serde_json::to_string(&foreign).unwrap();
    let incoming = format!("{{\"receipt\":{raw}}}");
    let acquired = acquire(&incoming, limit).unwrap();
    assert_eq!(acquired.raw.len(), 1);
    let reply = failure_for(&acquired);
    let tool = output(&reply, &acquired);
    let bytes = serde_json::to_vec(&DaemonResponse::success(
        u64::MAX,
        serde_json::to_value(&tool).unwrap(),
    ))
    .unwrap();
    let raw_bytes: usize = acquired.raw.iter().map(|r| r.get().len()).sum();
    assert!(bytes.len() + 4 <= 6 * raw_bytes + overhead);
    assert!(bytes.len() + 4 <= MAX_MESSAGE_SIZE);
    let parsed = framed(tool).await;
    assert_eq!(
        raw_from_public(&parsed["structuredContent"]),
        [raw.as_str()]
    );
    let text: Value = serde_json::from_str(parsed["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(raw_from_public(&text), [raw.as_str()]);
    eprintln!(
        "raw API1 foreign11000 raw={raw_bytes} fullframe={} limit={limit} overhead={overhead} reserve={RESERVE}",
        bytes.len() + 4
    );
}

#[test]
fn api1_point_raw_legacy_typed_failure_bounds_unbounded_source_message() {
    let mut receipt = maximal_point_receipt();
    receipt.outcome = MutationOutcome::ExposuresRecorded {
        accesses: vec![crate::storage::control::ExposureOutcome {
            memory_id: MemoryId::from_uuid(uuid::Uuid::from_u128(1)),
            kind: AccessKind::DirectRetrieval,
            accessed_at: 100,
        }],
    };
    let observation = crate::decay::observation::ObservationError::not_ready(
        vec![receipt.clone()],
        "x".repeat(MAX_MESSAGE_SIZE + 1),
    );
    assert!(
        observation.to_string().len() > MAX_MESSAGE_SIZE,
        "old string has no finite point-receipt bound"
    );
    let reply = GetReply::failure(SearchError::Observation(observation));
    let value = public(&reply, chrono_tz::UTC);
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(bytes.len() < RESERVE);
    let body = value.structured_content.unwrap();
    let typed: WriteReceipt = serde_json::from_value(body["receipt"].clone()).unwrap();
    assert_eq!(
        serde_json::to_vec(&typed).unwrap(),
        serde_json::to_vec(&receipt).unwrap()
    );
    assert_eq!(body["commitState"], "committed");
    assert!(!body.contains_key("contractVersion"));
    eprintln!(
        "legacy typed observation failure={}; source string >8MiB never echoed/parsed/truncated as a receipt carrier",
        bytes.len()
    );
}

#[tokio::test]
async fn api1_point_raw_escaping_and_exact_acquisition_limits() {
    let (limit, overhead) = raw_limit();
    let receipt = serde_json::to_string(&maximal_point_receipt()).unwrap();
    let base = format!("{{\"receipt\":{receipt},\"padding\":\"\"}}");
    let base_charge = charged(base.as_bytes());
    for delta in [-1isize, 0, 1] {
        let padding = "x".repeat(limit.checked_add_signed(delta).unwrap() - base_charge);
        let incoming = format!("{{\"receipt\":{receipt},\"padding\":\"{padding}\"}}");
        assert_eq!(acquire(&incoming, limit).is_ok(), delta <= 0);
    }
    // Pure serializer stress, explicitly NOT a valid receipt/committed history.
    // The real acquisition validator below rejects this JSON-string fragment.
    let escaped = serde_json::to_string(&"\\\"\0\né".repeat(20_000)).unwrap();
    let raw: &RawValue = serde_json::from_str(&escaped).unwrap();
    assert!(
        acquire(&format!("{{\"receipt\":{escaped}}}"), limit)
            .unwrap()
            .raw
            .is_empty()
    );
    let specimen = Acquired { raw: vec![raw] };
    let mut reply = GetReply::remote_failure(true, true, vec![]);
    reply
        .envelope
        .warnings
        .push("UNVERIFIED_RECEIVED_EVIDENCE".into());
    let tool = output(&reply, &specimen);
    let count = serde_json::to_vec(&DaemonResponse::success(
        u64::MAX,
        serde_json::to_value(&tool).unwrap(),
    ))
    .unwrap()
    .len()
        + 4;
    assert!(count <= 6 * escaped.len() + overhead);
    assert!(count <= MAX_MESSAGE_SIZE);
    let parsed = framed(tool).await;
    assert_eq!(
        raw_from_public(&parsed["structuredContent"]),
        [escaped.as_str()]
    );
    let text: Value = serde_json::from_str(parsed["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(raw_from_public(&text), [escaped.as_str()]);
    eprintln!(
        "raw API1 nonreceipt escaping geometry raw={} frame={count}; acquisition rejects it",
        escaped.len()
    );
}
