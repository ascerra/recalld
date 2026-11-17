//! Test-only API1 projection/carrier proposal. No production admission changes.
use super::*;
use crate::daemon::prepared_query::shape;
use crate::daemon::protocol::{DaemonResponse, MAX_MESSAGE_SIZE};
use crate::mcp::protocol::ToolCallResult;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Api1Failure<'a> {
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
    received_evidence: Option<&'a crate::serialization::query_response::ReceivedQueryEvidence>,
    warnings: &'a [String],
}

fn public(reply: &GetReply, timezone: chrono_tz::Tz) -> ToolCallResult {
    if let Some(error) = &reply.envelope.error {
        let failure = Api1Failure {
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
            // An available prior receipt must never become a fabricated unknown ID.
            operation_id: error.operation_id.or_else(|| {
                (error.commit_state == CommitState::Committed)
                    .then(|| reply.envelope.receipt.as_ref().map(|r| r.operation_id))
                    .flatten()
            }),
            received_evidence: reply.received_evidence.as_ref(),
            warnings: &reply.envelope.warnings,
        };
        let text = serde_json::to_string(&failure).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        let mut result = ToolCallResult::text(text);
        result.structured_content = value.as_object().cloned();
        result.is_error = Some(true);
        return result;
    }
    let memory = &reply.envelope.data.as_ref().unwrap().memory;
    ToolCallResult::json(&crate::mcp::bridge::MemoryRecord {
        labels: memory.labels.clone(),
        id: memory.id.to_string(),
        namespace: memory.namespace.clone(),
        summary: memory.summary.clone().unwrap_or_default(),
        full_text: memory.full_text.clone(),
        tags: memory.tags.iter().map(ToString::to_string).collect(),
        phase: format!("{:?}", memory.phase),
        strength: memory.strength,
        stability: memory.stability,
        created_at: crate::time::format_timestamp(memory.created_at, timezone),
        last_accessed_at: crate::time::format_timestamp(memory.last_accessed_at, timezone),
        is_permastore: memory.is_permastore,
        edge_count: memory.edge_count,
    })
    .unwrap()
}

fn tagged(reply: &GetReply) -> Vec<u8> {
    serde_json::to_vec(&DaemonResponse::success(
        u64::MAX,
        serde_json::json!({"wireVersion":1,"kind":"native","httpStatus":reply.http_status,
            "replyJson":serde_json::to_string(reply).unwrap()}),
    ))
    .unwrap()
}

// Measured through the actual production checked borrowed visitor, not a second
// approximation of raw-number length. Used only for small accounting specimens.
fn charged(raw: &[u8]) -> usize {
    let mut low = raw.len();
    let mut high = MAX_MESSAGE_SIZE;
    assert!(shape::check_typed(raw, high).is_ok());
    while low < high {
        let mid = low + (high - low) / 2;
        if shape::check_typed(raw, mid).is_ok() {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    low
}

fn projected_bytes(reply: &GetReply, timezone: chrono_tz::Tz) -> usize {
    serde_json::to_vec(&public(reply, timezone)).unwrap().len()
}

fn public_fits(reply: &GetReply, timezone: chrono_tz::Tz, limit: usize, reserve: usize) -> bool {
    projected_bytes(reply, timezone)
        .checked_add(reserve)
        .is_some_and(|size| size <= limit)
}

fn encoding_specimen() -> GetReply {
    let mut reply = specimen();
    let memory = &mut reply.envelope.data.as_mut().unwrap().memory;
    memory.created_at = chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis();
    memory.last_accessed_at = chrono::DateTime::<chrono::Utc>::MAX_UTC.timestamp_millis();
    memory.tags = vec![crate::model::Tag::new("topic/accounting").unwrap(); 32];
    memory.labels = Some(crate::model::schema::StructuredLabels {
        entities: vec!["José".into(), "John Smith".into()],
        topics: vec!["encoding".into()],
        emotions: vec![],
        provenance: crate::model::schema::LabelProvenance::LegacyRetainedOnly,
    });
    reply
}

#[test]
fn api1_point_budget_projection_versions_receipts_and_unknown_identity() {
    let receipt = maximal_point_receipt();
    for cause in [
        crate::storage::StorageError::RecoveryRequired("not echoed".repeat(4096)),
        crate::storage::StorageError::WorkerOutcomeUnknown("not echoed".repeat(4096)),
    ] {
        let reply = GetReply::failure(SearchError::Observation(
            crate::decay::observation::ObservationError::storage(
                crate::storage::StorageError::CommittedMutation {
                    receipt: Box::new(receipt.clone()),
                    source: Box::new(cause),
                },
                vec![],
            ),
        ));
        let output = public(&reply, chrono_tz::UTC);
        let text = output
            .content
            .iter()
            .find_map(|block| match block {
                crate::mcp::protocol::ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .unwrap();
        let raw: std::collections::BTreeMap<String, &serde_json::value::RawValue> =
            serde_json::from_str(text).unwrap();
        let canonical = serde_json::to_string(&receipt).unwrap();
        assert_eq!(raw["receipt"].get(), canonical);
        let aliases: Vec<&serde_json::value::RawValue> =
            serde_json::from_str(raw["observationReceipts"].get()).unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].get(), canonical);
        let value = output.structured_content.unwrap();
        assert!(!value.contains_key("contractVersion"));
        assert!(!value.contains_key("data"));
        let typed: WriteReceipt = serde_json::from_value(value["receipt"].clone()).unwrap();
        let typed_aliases: Vec<WriteReceipt> =
            serde_json::from_value(value["observationReceipts"].clone()).unwrap();
        assert_eq!(serde_json::to_string(&typed).unwrap(), canonical);
        assert_eq!(typed_aliases.len(), 1);
        assert_eq!(serde_json::to_string(&typed_aliases[0]).unwrap(), canonical);
        if reply.envelope.error.as_ref().unwrap().commit_state == CommitState::Unknown {
            assert_eq!(
                value.get("operationId").cloned(),
                reply
                    .envelope
                    .error
                    .as_ref()
                    .unwrap()
                    .operation_id
                    .map(|id| Value::String(id.to_string()))
            );
        } else {
            assert_eq!(value["operationId"], receipt.operation_id.to_string());
        }
        // The private O1 envelope remains2: API1 public projection must not emit it.
        assert_eq!(
            reply.mcp().structured_content.unwrap()["contractVersion"],
            2
        );
        eprintln!(
            "API1 typed failure={} bytes, tagged private={} bytes",
            projected_bytes(&reply, chrono_tz::UTC),
            tagged(&reply).len()
        );
    }
    for sent in [false, true] {
        let reply =
            GetReply::remote_failure(true, sent, vec![serde_json::to_value(&receipt).unwrap()]);
        let value = public(&reply, chrono_tz::UTC).structured_content.unwrap();
        assert!(!value.contains_key("receipt"));
        assert!(!value.contains_key("operationId"));
        assert_eq!(
            value["commitState"],
            if sent { "unknown" } else { "notCommitted" }
        );
        assert_eq!(
            value["receivedEvidence"]["receipts"],
            serde_json::json!([receipt])
        );
    }
}

#[test]
fn api1_point_budget_actual_pretty_emitter_and_tagged_completion() {
    // Independent proposal allowance, reviewed from actual point-shape bounds:
    // two normalized aliases <=2*8192, fixed completion <=8192; structured plus
    // escaped compact error <=3*(2*8192+8192)+1024=74752. Choose80KiB, not O1's64KiB.
    const RESERVE: usize = 80 * 1024;
    let mut overhead = 0;
    for access in [false, true] {
        for sent in [false, true] {
            for evidence in [false, true] {
                let mut reply = GetReply::remote_failure(access, sent, vec![]);
                if evidence {
                    reply.received_evidence = Some(
                        crate::serialization::query_response::ReceivedQueryEvidence {
                            receipts: vec![],
                        },
                    );
                    reply
                        .envelope
                        .warnings
                        .push("UNVERIFIED_RECEIVED_EVIDENCE".into());
                }
                overhead = overhead.max(projected_bytes(&reply, chrono_tz::UTC) + 1024);
            }
        }
    }
    let allowance = (MAX_MESSAGE_SIZE - overhead - RESERVE) / 3;
    let receipt = maximal_point_receipt();
    let receipt_shape =
        super::rpc_budget_probe::receipt_charge_bound(&serde_json::to_value(&receipt).unwrap());
    assert!(receipt_shape <= 8192);
    // New API1 flat completion, including scope/readG absent from API1 success:
    // views + readiness header + error/escaped pointer + warnings + fixed keys
    // + scope + readG + new projection keys. Source limits, not sampled lengths.
    let fixed = 4 * 128 + 256 + 2560 + 256 + 512 + 256 + 128 + 512;
    assert!(fixed < 8192);
    assert!(3 * (2 * receipt_shape + fixed) + 1024 < RESERVE);
    assert!(3 * allowance + overhead + RESERVE <= MAX_MESSAGE_SIZE);
    eprintln!(
        "API1 derived receipt scalar/key bound={receipt_shape}, new fixed completion bound={fixed}; reserve={RESERVE}"
    );
    for timezone in [
        chrono_tz::UTC,
        chrono_tz::America::Los_Angeles,
        chrono_tz::Asia::Kathmandu,
    ] {
        let mut reply = encoding_specimen();
        let initial = projected_bytes(&reply, timezone);
        let raw = serde_json::to_vec(&reply).unwrap();
        let base = charged(&raw);
        assert!(base < allowance);
        let old = reply.mcp().structured_content.unwrap();
        let value = public(&reply, timezone).structured_content.unwrap();
        assert!(!value.contains_key("contractVersion"));
        assert!(value["createdAt"].is_string());
        assert!(!value.contains_key("accessCount"));
        assert_eq!(
            value["strength"].as_f64().unwrap() as f32,
            f32::from_bits(1)
        );
        assert_ne!(
            serde_json::to_vec(&value).unwrap().len(),
            initial,
            "count both actual public representations"
        );
        assert!(old.contains_key("contractVersion"));
        reply.envelope.receipt = Some(receipt.clone());
        reply.observation_receipts = vec![receipt.clone()];
        reply.envelope.readiness = Some(readiness(&receipt));
        assert_eq!(
            projected_bytes(&reply, timezone),
            initial,
            "success projection must not be recaptured after exposure"
        );
        let completed = charged(&serde_json::to_vec(&reply).unwrap());
        assert!(completed - base < RESERVE);
        assert!(tagged(&reply).len() < allowance);
        let quoted = serde_json::to_vec(&serde_json::to_string(&reply).unwrap()).unwrap();
        assert!(tagged(&reply).len() - quoted.len() < 1024);
        let public_limit = initial.checked_add(RESERVE).unwrap();
        for limit in [public_limit - 1, public_limit, public_limit + 1] {
            assert_eq!(
                public_fits(&reply, timezone, limit, RESERVE),
                limit >= public_limit
            );
        }
        eprintln!(
            "API1 timezone={timezone}: pretty+structured={initial} normalized_pre={base} completed={completed} tagged={} allowance={allowance} reserve={RESERVE}",
            tagged(&reply).len()
        );
    }
    // Independent exact inner and outer boundaries; encoding specimens only.
    let mut reply = encoding_specimen();
    reply.envelope.data.as_mut().unwrap().memory.full_text = Some(String::new());
    let empty_charge = charged(&serde_json::to_vec(&reply).unwrap());
    let empty_outer = tagged(&reply).len();
    for delta in [-1isize, 0, 1] {
        let target = allowance.checked_add_signed(delta).unwrap();
        reply.envelope.data.as_mut().unwrap().memory.full_text =
            Some("x".repeat(target - empty_charge));
        assert_eq!(
            shape::check_typed(&serde_json::to_vec(&reply).unwrap(), allowance).is_ok(),
            delta <= 0
        );
        reply.envelope.data.as_mut().unwrap().memory.full_text =
            Some("x".repeat(target - empty_outer));
        assert_eq!(tagged(&reply).len(), target);
        assert_eq!(tagged(&reply).len() <= allowance, delta <= 0);
    }
    eprintln!(
        "API1 actual-emitter diagnostic overhead={overhead}, private allowance={allowance}; no production bound installed"
    );
    let mut foreign = receipt;
    foreign.outcome = MutationOutcome::ExposuresRecorded {
        accesses: (1..=15_000)
            .map(|n| crate::storage::control::ExposureOutcome {
                memory_id: MemoryId::from_uuid(uuid::Uuid::from_u128(n)),
                kind: AccessKind::DirectRetrieval,
                accessed_at: chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis(),
            })
            .collect(),
    };
    // Supported foreign encoding specimen, not a fabricated committed history.
    let raw = serde_json::to_vec(&foreign).unwrap();
    assert!(shape::check_typed(&raw, allowance).is_ok());
    let reply = GetReply::remote_failure(true, true, vec![serde_json::from_slice(&raw).unwrap()]);
    let output = public(&reply, chrono_tz::UTC);
    let value = output.structured_content.as_ref().unwrap();
    assert!(!value.contains_key("contractVersion"));
    assert!(!value.contains_key("receipt"));
    assert!(!value.contains_key("operationId"));
    assert_eq!(value["commitState"], "unknown");
    let output_bytes = serde_json::to_vec(&output).unwrap().len();
    assert!(output_bytes < MAX_MESSAGE_SIZE);
    eprintln!(
        "API1 foreign15000 specimen raw={} public={output_bytes}; unverified only",
        raw.len()
    );
}
