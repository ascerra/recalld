//! Encoding bounds, not historical commits or a fabricated readiness authority.
use super::*;
use crate::model::{AccessEvent, AccessKind, DecayPhase, NamespaceId};
use crate::storage::control::{Generation, MutationOutcome};

fn maximal_point_receipt() -> WriteReceipt {
    use crate::model::schema::{LifecycleFields, LifecycleVersion, TimestampOrigin};
    use crate::storage::native_lifecycle::{LifecycleChange, LifecycleState, NativeLifecycleWrite};
    let time = chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis();
    let prior: Generation = (Generation::MAX.get() - 1).to_string().try_into().unwrap();
    let before = LifecycleState {
        lifecycle: LifecycleFields {
            observed_at: time,
            retrieved_at: Some(time),
            reinforced_at: Some(time),
            decay_anchor_at: time,
            lifecycle_version: LifecycleVersion::ExplicitFeedbackV2,
            lifecycle_updated_at: time,
            timestamp_origin: TimestampOrigin::LegacyInferred,
            phase_changed_at: time,
            policy_revision: Generation::MAX,
            record_generation: prior,
        },
        created_at: time,
        last_accessed_at: time,
        stability: f32::MAX.try_into().unwrap(),
        difficulty: f32::from_bits(0x411fffff).try_into().unwrap(),
        strength: f32::from_bits(1).try_into().unwrap(),
        decay_strength: f32::from_bits(0x3f7fffff).try_into().unwrap(),
        phase: DecayPhase::Summary,
        is_permastore: false,
        has_summary: true,
        has_text: false,
    };
    let after = crate::storage::native_lifecycle::exposed(&before, time).unwrap();
    let store_id = uuid::Uuid::from_u128(u128::MAX);
    let transition = NativeLifecycleWrite {
        namespace_id: NamespaceId::new(u32::MAX),
        lifecycle_version: LifecycleVersion::ExplicitFeedbackV2,
        policy_revision: Generation::MAX,
        previous_generation: prior,
        capture_generation: GenerationRef {
            store_id,
            generation: prior,
        },
        sampled_at: time,
        records: vec![LifecycleChange {
            memory_id: MemoryId::from_uuid(uuid::Uuid::from_u128(u128::MAX - 1)),
            kind: AccessKind::DirectRetrieval,
            before,
            after,
            history_before: [255; 32],
            history_after: [255; 32],
            evicted_event: Some(AccessEvent {
                timestamp: time,
                kind: AccessKind::ManualReinforcement,
            }),
        }],
        feedback: None,
    };
    let receipt = WriteReceipt {
        store_id,
        operation_id: uuid::Uuid::from_u128(u128::MAX - 2),
        committed_generation: Generation::MAX,
        receipt_version: 1,
        outcome: MutationOutcome::LifecycleExposureRecorded { transition },
        replayed: false,
    };
    if let MutationOutcome::LifecycleExposureRecorded { transition } = &receipt.outcome {
        transition.validate(&receipt, false).unwrap();
    }
    receipt
}

// Object keys/counts come from the actual serialized typed shape. Each filled
// optional branch is present; feedback=None is mandatory for point exposure.
// This bounds all scalar values, not just the chosen specimen's numeric bytes.
fn receipt_bound(value: &Value, key: &str) -> usize {
    match value {
        Value::Null => 4,
        Value::Bool(_) => 5,
        Value::Number(n) => {
            if n.is_f64() {
                32
            } else {
                20
            }
        }
        Value::String(s) => {
            assert!(s.is_ascii());
            assert!(serde_json::to_vec(s).unwrap().len() <= 38);
            38 // UUID including quotes; also bounds fixed enums and quoted G.
        }
        Value::Array(values) if matches!(key, "historyBefore" | "historyAfter") => {
            assert_eq!(values.len(), 32);
            assert!(values.iter().all(|v| v.as_u64().is_some_and(|b| b <= 255)));
            2 + 32 * 3 + 31
        }
        Value::Array(values) => {
            2 + values.len().saturating_sub(1)
                + values.iter().map(|v| receipt_bound(v, "")).sum::<usize>()
        }
        Value::Object(fields) => {
            2 + fields.len().saturating_sub(1)
                + fields
                    .iter()
                    .map(|(k, v)| k.len() + 3 + receipt_bound(v, k))
                    .sum::<usize>()
        }
    }
}

fn specimen() -> GetReply {
    let receipt = maximal_point_receipt();
    let at = GenerationRef {
        store_id: receipt.store_id,
        generation: receipt.committed_generation,
    };
    GetReply::success(
        Scope::Namespace {
            namespace_id: NamespaceId::new(u32::MAX),
            namespace: "a".repeat(64),
        },
        at,
        GetData {
            evaluated_at: i64::MIN,
            record_access: true,
            memory: MemoryResponse {
                labels: None,
                id: MemoryId::from_uuid(uuid::Uuid::from_u128(u128::MAX - 1)),
                namespace: "a".repeat(64),
                created_at: i64::MIN,
                last_accessed_at: i64::MIN,
                summary: Some("quoted\"\\\n\u{0}é".repeat(16)),
                full_text: Some("\\\"\u{1f}\né".repeat(2048)),
                tags: vec![],
                phase: DecayPhase::Full,
                strength: f32::from_bits(1),
                decay_strength: f32::from_bits(0x3f7fffff),
                stability: f32::MAX,
                is_permastore: false,
                edge_count: u16::MAX,
                access_count: usize::MAX,
            },
        },
    )
}

fn readiness(receipt: &WriteReceipt) -> Readiness {
    crate::storage::Readiness {
        store_id: receipt.store_id,
        committed_generation: Generation::MAX,
        ready: true,
        pending_work: 0,
        derived: ["graph", "vectors", "fts", "entities"]
            .into_iter()
            .map(|name| crate::storage::publication::DerivedReadiness {
                name,
                applied_generation: Generation::MAX,
                ready: true,
            })
            .collect(),
    }
    .into()
}

#[test]
fn prepared_get_bound_covers_actual_typed_receipt_aliases_and_completion_fields() {
    let native = maximal_point_receipt();
    let mut legacy = native.clone();
    legacy.outcome = MutationOutcome::ExposuresRecorded {
        accesses: vec![crate::storage::control::ExposureOutcome {
            memory_id: MemoryId::from_uuid(uuid::Uuid::from_u128(u128::MAX)),
            kind: AccessKind::DirectRetrieval,
            accessed_at: chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis(),
        }],
    };
    for receipt in [native, legacy] {
        let value = serde_json::to_value(&receipt).unwrap();
        let bound = receipt_bound(&value, "");
        let encoded = serde_json::to_vec(&value).unwrap();
        eprintln!(
            "point receipt {}: actual {} bytes, scalar/key bound {bound}",
            value["outcome"]["kind"],
            encoded.len()
        );
        assert!(
            encoded.len() <= bound && bound <= 4096,
            "{} <= {bound}",
            encoded.len()
        );
        // f32->Value may expand to an f64 spelling. Count that exact encoding.
        let compact = value.to_string();
        assert_eq!(encoded, compact.as_bytes());
        let text = serde_json::to_vec(&compact).unwrap();
        assert!(text.len() <= 2 * encoded.len() + 2);
        let mut reply = specimen();
        let before = serde_json::to_vec(&reply.mcp()).unwrap().len();
        reply.envelope.receipt = Some(receipt.clone());
        reply.observation_receipts = vec![receipt.clone()];
        reply.envelope.readiness = Some(readiness(&receipt));
        let frame = serde_json::to_vec(&reply.mcp()).unwrap();
        assert!(frame.len() - before + POINT_FRAME_RESERVE <= POINT_EVIDENCE_RESERVE);
        assert_eq!(reply.mcp().structured_content.unwrap()["receipt"], value);
        let compact = serde_json::to_value(&reply).unwrap();
        let wrapper = crate::daemon::protocol::DaemonResponse::success(u64::MAX, compact.clone());
        let framed = serde_json::to_vec(&wrapper).unwrap().len() + 4;
        assert!(framed <= serde_json::to_vec(&compact).unwrap().len() + POINT_FRAME_RESERVE);
        // Fixed failure fields cannot smuggle a storage error string or payload.
        let failure = GetReply::failure(SearchError::Observation(
            crate::decay::observation::ObservationError::storage(
                crate::storage::StorageError::CommittedMutation {
                    receipt: Box::new(receipt.clone()),
                    source: Box::new(crate::storage::StorageError::WorkerOutcomeUnknown(
                        "x".repeat(100_000),
                    )),
                },
                vec![],
            ),
        ));
        let failed = serde_json::to_value(&failure).unwrap();
        assert!(failure.envelope.data.is_none());
        assert_eq!(
            failure.envelope.error.as_ref().unwrap().commit_state,
            CommitState::Unknown
        );
        assert_eq!(failed["receipt"], value);
        assert_eq!(failed["observationReceipts"], serde_json::json!([value]));
        assert!(
            serde_json::to_vec(&failure.mcp()).unwrap().len() + POINT_FRAME_RESERVE
                <= POINT_EVIDENCE_RESERVE
        );
        let mut fixed = serde_json::to_value(&reply).unwrap();
        let fixed = fixed.as_object_mut().unwrap();
        for key in [
            "data",
            "scope",
            "readGeneration",
            "receipt",
            "observationReceipts",
        ] {
            fixed.remove(key);
        }
        assert!(serde_json::to_vec(fixed).unwrap().len() <= 8192);
    }
}

#[test]
fn prepared_get_bound_exact_limit_and_value_normalized_compact_text() {
    for observe in [false, true] {
        let reply = specimen();
        let tool = reply.mcp();
        let raw = serde_json::to_vec(&tool).unwrap().len();
        let reserve = if observe {
            POINT_EVIDENCE_RESERVE
        } else {
            POINT_FRAME_RESERVE
        };
        let limit = raw + reserve;
        assert!(!reply.fits(limit - 1, observe));
        assert!(reply.fits(limit, observe));
        assert!(reply.fits(limit + 1, observe));
        let crate::mcp::protocol::ContentBlock::Text { text } = &tool.content[0];
        let structured = Value::Object(tool.structured_content.clone().unwrap());
        assert_eq!(text, &structured.to_string());
        // Compare at equal parse boundaries. Without float_roundtrip serde_json
        // can round a long f64 spelling on parse; an in-memory Value is not the
        // same boundary as a decoded peer value. Both actual wire copies must
        // contain identical numeric bytes and decode to the same result.
        let wire: Value = serde_json::from_slice(&serde_json::to_vec(&tool).unwrap()).unwrap();
        let parsed_text: Value =
            serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(parsed_text, wire["structuredContent"]);
    }
}

#[test]
fn prepared_get_bound_rejects_unbounded_input_pointer_without_echoing_it() {
    let mut value =
        serde_json::json!({"contractVersion":2,"id":uuid::Uuid::from_u128(1).to_string()});
    value["~/\"\u{0}".repeat(100_000)] = Value::Null;
    let error = GetRequest::from_mcp(&value).unwrap_err();
    assert!(error.field.len() > 256);
    let reply = GetReply::input(error);
    let error = reply.envelope.error.as_ref().unwrap();
    assert_eq!(error.code, "UNKNOWN_FIELD");
    assert_eq!(error.field.as_deref(), Some(""));
    assert_eq!(error.commit_state, CommitState::NotCommitted);
    assert!(reply.observation_receipts.is_empty());
    assert!(serde_json::to_vec(&reply.mcp()).unwrap().len() < 4096);
}

#[test]
fn prepared_get_rpc_scalar_expansion_and_received_evidence_stay_inside_outer_frame() {
    use crate::daemon::{
        prepared_get,
        protocol::{DaemonResponse, MAX_MESSAGE_SIZE},
    };
    // Encoding specimen only: no assertion that these synthetic extreme values
    // are an actual canonical read, receipt history or current readiness proof.
    let receipt = maximal_point_receipt();
    let mut reply = specimen();
    reply.envelope.receipt = Some(receipt.clone());
    reply.observation_receipts = vec![receipt.clone()];
    reply.envelope.readiness = Some(readiness(&receipt));
    fn raw_frame(reply: &GetReply) -> Vec<u8> {
        // Same typed Reply and normalization used by the actual dispatcher.
        let bytes = serde_json::to_vec(&crate::daemon::prepared_query::Reply {
            wire_version: 1,
            http_status: reply.http_status,
            reply,
        })
        .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        serde_json::to_vec(&DaemonResponse::success(u64::MAX, value)).unwrap()
    }
    let limit = prepared_get::response_limit();
    let mut low = 0;
    let mut high = limit;
    while low < high {
        let mid = (low + high + 1) / 2;
        reply.envelope.data.as_mut().unwrap().memory.full_text = Some("x".repeat(mid));
        if raw_frame(&reply).len() <= limit {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    for delta in [0, 1] {
        reply.envelope.data.as_mut().unwrap().memory.full_text = Some("x".repeat(low + delta));
        let raw = raw_frame(&reply);
        assert_eq!(prepared_get::admitted_frame(&raw), delta == 0);
        let outer = DaemonResponse::success(u64::MAX, serde_json::to_value(reply.mcp()).unwrap());
        assert!(serde_json::to_vec(&outer).unwrap().len() <= MAX_MESSAGE_SIZE);
    }
    // Larger separately typed evidence exercises expansion without attributing
    // synthetic evidence to the current operation or inventing history hashes.
    // Use the real serializer's enum shape rather than the handwritten variant.
    let mut legacy = receipt;
    legacy.outcome = MutationOutcome::ExposuresRecorded {
        accesses: (1..=15000)
            .map(|n| crate::storage::control::ExposureOutcome {
                memory_id: MemoryId::from_uuid(uuid::Uuid::from_u128(n)),
                kind: AccessKind::DirectRetrieval,
                accessed_at: 0,
            })
            .collect(),
    };
    let foreign = serde_json::to_value(&legacy).unwrap();
    let bytes = serde_json::to_vec(&foreign).unwrap();
    assert!(bytes.len() < limit);
    let failure = GetReply::remote_failure(true, true, vec![foreign]);
    let outer = DaemonResponse::success(u64::MAX, serde_json::to_value(failure.mcp()).unwrap());
    let size = serde_json::to_vec(&outer).unwrap().len();
    assert!(size <= MAX_MESSAGE_SIZE);
    eprintln!(
        "get F={limit}; received-evidence encoding={}bytes outer={size}bytes",
        bytes.len()
    );
}

#[test]
fn prepared_get_rpc_direct_typed_json_probe_preserves_valid_domain_float_bits() {
    fn canonical(raw: &str) -> Option<GetReply> {
        let reply: GetReply = serde_json::from_str(raw).ok()?;
        (serde_json::to_string(&reply).ok()?.as_bytes() == raw.as_bytes()).then_some(reply)
    }
    let time = chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis();
    for bits in [0u32, 0x80000000, 1, 2, 0x007fffff, 0x00800000, 0x3f7fffff] {
        let strength = f32::from_bits(bits);
        let mut receipt = maximal_point_receipt();
        let MutationOutcome::LifecycleExposureRecorded { transition } = &mut receipt.outcome else {
            unreachable!()
        };
        transition.records[0].before.strength = strength.try_into().unwrap();
        transition.records[0].after =
            crate::storage::native_lifecycle::exposed(&transition.records[0].before, time).unwrap();
        let captured = transition.capture_generation;
        let state = transition.records[0].before.clone();
        if let MutationOutcome::LifecycleExposureRecorded { transition } = &receipt.outcome {
            transition.validate(&receipt, false).unwrap();
        }
        let mut reply = specimen();
        reply.envelope.read_generation = Some(captured);
        reply.envelope.receipt = Some(receipt.clone());
        reply.observation_receipts = vec![receipt.clone()];
        reply.envelope.readiness = Some(readiness(&receipt));
        let data = reply.envelope.data.as_mut().unwrap();
        data.evaluated_at = time;
        data.memory.created_at = time;
        data.memory.last_accessed_at = time;
        data.memory.phase = DecayPhase::Summary;
        data.memory.full_text = None;
        data.memory.strength = strength;
        data.memory.decay_strength = state.decay_strength.get();
        data.memory.stability = state.stability.get();
        let raw = serde_json::to_string(&reply).unwrap();
        let decoded = canonical(&raw).expect("whole flattened GetReply must roundtrip exact bytes");
        assert_eq!(
            decoded
                .envelope
                .data
                .as_ref()
                .unwrap()
                .memory
                .strength
                .to_bits(),
            bits
        );
        assert_eq!(
            decoded
                .envelope
                .data
                .as_ref()
                .unwrap()
                .memory
                .stability
                .to_bits(),
            f32::MAX.to_bits()
        );
        for alias in [
            decoded.envelope.receipt.as_ref().unwrap(),
            &decoded.observation_receipts[0],
        ] {
            let MutationOutcome::LifecycleExposureRecorded { transition } = &alias.outcome else {
                unreachable!()
            };
            assert_eq!(transition.records[0].before.strength.get().to_bits(), bits);
            assert_eq!(transition.records[0].after.strength.get().to_bits(), bits);
            assert_eq!(
                transition.records[0].before.stability.get().to_bits(),
                f32::MAX.to_bits()
            );
            transition.validate(alias, false).unwrap();
            let fragment = serde_json::to_string(alias).unwrap();
            let received: WriteReceipt = serde_json::from_str(&fragment).unwrap();
            assert_eq!(serde_json::to_string(&received).unwrap(), fragment);
        }
        eprintln!(
            "direct typed GetReply + both tagged receipt aliases bits={bits:08x} bytes={} exact",
            raw.len()
        );
        for replacement in ["1e999", "1e39", "NaN", "\"NaN\""] {
            let needle = format!("\"strength\":{}", serde_json::to_string(&strength).unwrap());
            let invalid = raw.replacen(&needle, &format!("\"strength\":{replacement}"), 1);
            assert!(canonical(&invalid).is_none(), "{replacement}");
        }
        assert!(
            canonical(&raw.replacen(
                "\"contractVersion\":2",
                "\"contractVersion\":2,\"contractVersion\":2",
                1
            ))
            .is_none()
        );
        assert!(
            canonical(&raw.replacen("\"memory\":{", "\"memory\":{\"unknown\":0,", 1)).is_none()
        );
        assert!(
            canonical(&raw.replacen("\"records\":[{", "\"records\":[{\"unknown\":0,", 1)).is_none()
        );
        assert!(
            canonical(&raw.replacen(&format!("\"evaluatedAt\":{time}"), "\"evaluatedAt\":1.5", 1))
                .is_none()
        );
        assert!(canonical(&raw.replacen(&format!("\"createdAt\":{time},"), "", 1)).is_none());
    }
}

#[path = "get_rpc_budget_probe.rs"]
mod rpc_budget_probe;
