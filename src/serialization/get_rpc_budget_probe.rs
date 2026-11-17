//! Diagnostic of the proposed private carrier; not the production preflight.
use super::*;
use crate::daemon::{
    prepared_get,
    protocol::{DaemonResponse, MAX_MESSAGE_SIZE},
};
use serde_json::value::RawValue;

fn charge(total: usize, token: usize) -> Option<usize> {
    total.checked_add(32usize.saturating_sub(token))
}
fn normalized(raw: &str) -> Option<(usize, usize)> {
    fn visit(raw: &RawValue, total: &mut usize, numbers: &mut usize) -> Option<()> {
        let text = raw.get();
        match text.as_bytes()[0] {
            b'{' => {
                let fields: std::collections::BTreeMap<String, &RawValue> =
                    serde_json::from_str(text).ok()?;
                for value in fields.values() {
                    visit(value, total, numbers)?;
                }
            }
            b'[' => {
                let items: Vec<&RawValue> = serde_json::from_str(text).ok()?;
                for value in items {
                    visit(value, total, numbers)?;
                }
            }
            b'"' | b't' | b'f' | b'n' => {}
            _ => {
                let number: serde_json::Number = serde_json::from_str(text).ok()?;
                assert!(number.to_string().len() <= 32);
                *total = charge(*total, text.len())?;
                *numbers = numbers.checked_add(1)?;
            }
        }
        Some(())
    }
    let value: &RawValue = serde_json::from_str(raw).ok()?;
    let mut total = raw.len();
    let mut count = 0;
    visit(value, &mut total, &mut count)?;
    Some((total, count))
}
fn quoted(raw: &str, status: u16) -> Vec<u8> {
    serde_json::to_vec(&DaemonResponse::success(
        u64::MAX,
        serde_json::json!({"wireVersion":1,"httpStatus":status,"replyJson":raw}),
    ))
    .unwrap()
}
fn typed_copy(reply: &GetReply) -> GetReply {
    let mut copied: GetReply =
        serde_json::from_str(&serde_json::to_string(reply).unwrap()).unwrap();
    copied.http_status = reply.http_status;
    copied
}
fn receipt_charge_bound(value: &Value) -> usize {
    match value {
        Value::Null => 4,
        Value::Bool(_) => 5,
        Value::Number(_) => 32,
        Value::String(v) => {
            assert!(v.is_ascii() && serde_json::to_vec(v).unwrap().len() <= 38);
            38
        }
        Value::Array(v) => {
            2 + v.len().saturating_sub(1) + v.iter().map(receipt_charge_bound).sum::<usize>()
        }
        Value::Object(v) => {
            2 + v.len().saturating_sub(1)
                + v.iter()
                    .map(|(k, v)| k.len() + 3 + receipt_charge_bound(v))
                    .sum::<usize>()
        }
    }
}
fn check_shape(reply: &GetReply, label: &str) {
    let raw = serde_json::to_string(reply).unwrap();
    let (charged, count) = normalized(&raw).unwrap();
    let value: Value = serde_json::from_str(&raw).unwrap();
    assert!(
        serde_json::to_vec(&value).unwrap().len() <= charged,
        "{label}"
    );
    let private = quoted(&raw, reply.http_status);
    let public = serde_json::to_vec(&DaemonResponse::success(
        u64::MAX,
        serde_json::to_value(reply.mcp()).unwrap(),
    ))
    .unwrap();
    assert!(
        charged <= prepared_get::response_limit(),
        "{label}: charged={charged}"
    );
    assert!(
        private.len() <= prepared_get::response_limit(),
        "{label}: private={}",
        private.len()
    );
    assert!(
        public.len() <= MAX_MESSAGE_SIZE,
        "{label}: public={}",
        public.len()
    );
    eprintln!(
        "candidate {label}: inner={} charged={charged} numeric={count} quoted={} public={}",
        raw.len(),
        private.len(),
        public.len()
    );
}

#[test]
fn prepared_get_rpc_candidate_joint_budget_charges_every_number_before_evidence() {
    assert!(charge(usize::MAX, 31).is_none());
    assert_eq!(charge(usize::MAX - 31, 1), Some(usize::MAX));
    for v in [
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::MAX,
        -0.0,
        f64::from_bits(1.0f64.to_bits() - 1),
    ] {
        let raw = serde_json::to_string(&v).unwrap();
        assert!(raw.len() <= 24);
        assert!(
            normalized(&raw).unwrap().0
                >= serde_json::to_string(&serde_json::from_str::<Value>(&raw).unwrap())
                    .unwrap()
                    .len()
        );
    }
    let receipt = maximal_point_receipt();
    let value = serde_json::to_value(&receipt).unwrap();
    let bound = receipt_charge_bound(&value);
    let exact = normalized(&serde_json::to_string(&receipt).unwrap()).unwrap();
    assert!(exact.0 <= bound && bound <= 8192);
    eprintln!(
        "normalized native receipt actual={} numeric={} key/scalar-bound={bound} <=8192",
        exact.0, exact.1
    );
    // Tight source-shape guard: false success has exactly ten numeric leaves.
    let mut initial = specimen();
    initial.envelope.data.as_mut().unwrap().memory.full_text = None;
    let raw = serde_json::to_string(&initial).unwrap();
    assert_eq!(normalized(&raw).unwrap().1, 10);
    assert!(normalized(&raw).unwrap().0 <= serde_json::to_vec(&initial.mcp()).unwrap().len() + 320);
    for access in [false, true] {
        let mut before = typed_copy(&initial);
        before.envelope.data.as_mut().unwrap().record_access = access;
        let prefix = "quoted\"\\\n\u{0}é".repeat(1000);
        let mut lo = 0;
        let mut hi = prepared_get::response_limit();
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            before.envelope.data.as_mut().unwrap().memory.full_text =
                Some(format!("{prefix}{}", "x".repeat(mid)));
            if before.fits(prepared_get::response_limit(), access) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        before.envelope.data.as_mut().unwrap().memory.full_text =
            Some(format!("{prefix}{}", "x".repeat(lo)));
        assert!(before.fits(prepared_get::response_limit(), access));
        check_shape(
            &before,
            if access {
                "true before"
            } else {
                "false at admission"
            },
        );
        let mut beyond = typed_copy(&before);
        beyond
            .envelope
            .data
            .as_mut()
            .unwrap()
            .memory
            .full_text
            .as_mut()
            .unwrap()
            .push('x');
        assert!(!beyond.fits(prepared_get::response_limit(), access));
        if access {
            let mut after = typed_copy(&before);
            after.envelope.receipt = Some(receipt.clone());
            after.observation_receipts = vec![receipt.clone()];
            after.envelope.readiness = Some(readiness(&receipt));
            check_shape(&after, "true after maximal receipt + readiness");
            for state in ["committed", "unknown"] {
                let mut failure = GetReply::failure(SearchError::Observation(
                    crate::decay::observation::ObservationError {
                        cause: None,
                        commit_state: state,
                        ready: false,
                        receipts: vec![receipt.clone()],
                        unknown_operation_id: (state == "unknown")
                            .then(|| uuid::Uuid::from_u128(u128::MAX)),
                        message: "not echoed".into(),
                    },
                ));
                failure.envelope.scope = before.envelope.scope.clone();
                failure.envelope.read_generation = before.envelope.read_generation;
                check_shape(&failure, state);
            }
        } else {
            let mut marker = typed_copy(&before);
            let m = &mut marker.envelope.data.as_mut().unwrap().memory;
            m.phase = DecayPhase::Tombstone;
            m.summary = None;
            m.full_text = None;
            m.tags.clear();
            m.labels = None;
            check_shape(&marker, "false terminal");
        }
    }
    for failure in [
        GetReply::failure(SearchError::MemoryNotFound(MemoryId::from_uuid(
            uuid::Uuid::from_u128(1),
        ))),
        GetReply::input(InputError {
            code: "OUT_OF_RANGE",
            field: "\u{0}".repeat(256),
            message: "bounded",
        }),
    ] {
        check_shape(&failure, "missing/input");
    }
    // A supported foreign parent receipt remains UNVERIFIED, not current get.
    let mut parent = receipt.clone();
    parent.outcome = MutationOutcome::BatchReserved {
        batch_kind: crate::storage::batch::BatchKind::Insert,
        item_count: u32::MAX,
        caller_hash: [255; 32],
        resolution_hash: [255; 32],
    };
    let mut large = receipt.clone();
    large.outcome = MutationOutcome::ExposuresRecorded {
        accesses: (1..=15000)
            .map(|n| crate::storage::control::ExposureOutcome {
                memory_id: MemoryId::from_uuid(uuid::Uuid::from_u128(n)),
                kind: AccessKind::DirectRetrieval,
                accessed_at: 0,
            })
            .collect(),
    };
    for foreign in [receipt, parent, large] {
        let fragment = serde_json::to_string(&foreign).unwrap();
        let parsed: WriteReceipt = serde_json::from_str(&fragment).unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), fragment);
        let inner = format!("{{\"receipt\":{fragment}}}");
        let (base, _) = normalized(&inner).unwrap();
        assert!(base < prepared_get::response_limit());
        // Inner-charge boundary independently of the raw quoted-frame boundary.
        let empty = format!("{{\"receipt\":{fragment},\"padding\":\"\"}}");
        let charged = normalized(&empty).unwrap().0;
        for delta in [-1isize, 0, 1] {
            let n = (prepared_get::response_limit() as isize + delta) as usize - charged;
            let candidate = format!(
                "{{\"receipt\":{fragment},\"padding\":\"{}\"}}",
                "x".repeat(n)
            );
            let cost = normalized(&candidate).unwrap().0;
            assert_eq!(
                cost,
                (prepared_get::response_limit() as isize + delta) as usize
            );
            assert!(quoted(&candidate, 503).len() < prepared_get::response_limit());
            if cost > prepared_get::response_limit() {
                continue;
            }
            let raw_value: Value = serde_json::from_str(&fragment).unwrap();
            assert!(serde_json::to_vec(&raw_value).unwrap().len() <= cost);
            for access in [false, true] {
                for sent in [false, true] {
                    let failure = GetReply::remote_failure(access, sent, vec![raw_value.clone()]);
                    let outer = serde_json::to_vec(&DaemonResponse::success(
                        u64::MAX,
                        serde_json::to_value(failure.mcp()).unwrap(),
                    ))
                    .unwrap();
                    assert!(outer.len() <= MAX_MESSAGE_SIZE);
                }
            }
        }
        // Quoted-frame boundary independently: escaped padding supplies more
        // raw quoting bytes than numeric growth; no receipt needs to be lost.
        let gap = normalized(&empty)
            .unwrap()
            .0
            .saturating_sub(quoted(&empty, 503).len());
        let escaped = "\\".repeat(8192 + gap.div_ceil(2));
        let empty = format!(
            "{{\"receipt\":{fragment},\"padding\":{}}}",
            serde_json::to_string(&escaped).unwrap()
        );
        let base = quoted(&empty, 503).len();
        for delta in [-1isize, 0, 1] {
            let n = (prepared_get::response_limit() as isize + delta) as usize - base;
            let candidate = format!(
                "{{\"receipt\":{fragment},\"padding\":{}}}",
                serde_json::to_string(&format!("{escaped}{}", "x".repeat(n))).unwrap()
            );
            let frame = quoted(&candidate, 503);
            assert_eq!(
                frame.len(),
                (prepared_get::response_limit() as isize + delta) as usize
            );
            assert!(normalized(&candidate).unwrap().0 < prepared_get::response_limit());
            assert_eq!(
                crate::daemon::prepared_get::admitted_frame(&frame),
                delta <= 0
            );
        }
    }
    eprintln!(
        "candidate getF={} MAX={} reserve={} checked32 jointbudget",
        prepared_get::response_limit(),
        MAX_MESSAGE_SIZE,
        POINT_EVIDENCE_RESERVE
    );
}
