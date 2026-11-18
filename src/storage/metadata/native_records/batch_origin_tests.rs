use super::*;
use crate::storage::request_context::RetainedRequestContext;

#[tokio::test]
async fn native_origin_batch_export_import_fork_preserves_receipts_and_original_cas() {
    export_import_and_restore(true, 1).await;
}

#[test]
fn native_origin_batch_process_exits_keep_reserved_child_and_origin() {
    process_child_boundaries(true, 1);
}

#[test]
fn native_origin_batch_pending_resume_final_rejection_and_forged_child() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let mut r = request(&d, s.metadata().control_state().unwrap().store_id);
    r.request_context = RetainedRequestContext::from_contract(1).unwrap();
    r.expected_generation = Some(s.metadata().control_state().unwrap().committed_generation);
    let original = plans(&r);
    let p = reserve(&s, &r, &original);
    let child = p.child(0).unwrap();
    assert_eq!(child.context().contract_version, 1);
    assert!(child.context().expected_generation.is_none());
    drop(s);
    let s = open(&d, clock.clone());
    let replay = s
        .metadata()
        .reserve_native_store_batch_with_work(&r.context(), &original, None, None, || {
            panic!("pending resume consulted settings")
        })
        .unwrap();
    assert_eq!(replay.request_context, p.request_context);
    let before = readers::bytes(&d.path);
    let mut forged = child.clone();
    forged.request_context = None;
    assert!(matches!(
        s.metadata()
            .lookup_native_store_child(&forged, native(&original[0]), None),
        Err(StorageError::IdempotencyConflict)
    ));
    assert!(matches!(
        s.metadata()
            .finalize_batch_error(&forged, batch::BatchError::new("INVALID_INPUT")),
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(readers::bytes(&d.path), before);
    let completed = s
        .metadata()
        .finalize_batch_error(&child, batch::BatchError::new("INVALID_INPUT"))
        .unwrap();
    assert_eq!(completed.bookkeeping_receipts.len(), 1);
    assert_eq!(completed.unresolved(), 0);
    let receipt = completed.bookkeeping_receipts[0].clone();
    assert_eq!(
        receipt.committed_generation,
        p.reservation_receipt.committed_generation.next().unwrap()
    );
    drop(s);
    let s = open(&d, clock);
    let before = readers::bytes(&d.path);
    let replay = reserve(&s, &r, &original);
    assert_eq!(replay.bookkeeping_receipts, vec![receipt]);
    assert_eq!(replay.items, completed.items);
    assert_eq!(readers::bytes(&d.path), before);
}

#[test]
fn native_origin_batch_all_invalid_parent_is_anchored_before_any_child() {
    for corruption in 0..7 {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let s = open(&d, clock.clone());
        let mut r = request(&d, s.metadata().control_state().unwrap().store_id);
        r.items = vec![json!({"summary":false})];
        r.request_context = RetainedRequestContext::from_contract(1).unwrap();
        r.expected_generation = Some(s.metadata().control_state().unwrap().committed_generation);
        let p = reserve(&s, &r, &plans(&r));
        assert_eq!(p.unresolved(), 0);
        assert!(p.child(0).is_none());
        assert!(p.bookkeeping_receipts.is_empty());
        let before = readers::bytes(&d.path);
        let mut wrong = r.clone();
        wrong.request_context = None;
        assert!(matches!(
            s.metadata().reserve_native_store_batch_with_work(
                &wrong.context(),
                &plans(&wrong),
                None,
                None,
                || panic!("origin conflict consulted settings")
            ),
            Err(StorageError::IdempotencyConflict)
        ));
        assert_eq!(readers::bytes(&d.path), before);
        let mut value = serde_json::to_value(&p).unwrap();
        match corruption {
            0 => {
                value.as_object_mut().unwrap().remove("requestContext");
            }
            1 => value["requestContext"] = serde_json::Value::Null,
            2 => value["requestContext"]["proofVersion"] = json!(2),
            3 => value["requestContext"]["contractVersion"] = json!(2),
            4 => value["requestContext"]["extra"] = json!(true),
            5 => value["version"] = json!(1),
            _ => {
                value
                    .as_object_mut()
                    .unwrap()
                    .remove("reservationPrecondition");
            }
        }
        let tx = s.metadata().db.begin_write().unwrap();
        tx.open_table(crate::storage::metadata::batch::BATCHES)
            .unwrap()
            .insert(
                p.reservation_receipt.operation_id.as_bytes().as_slice(),
                serde_json::to_vec(&value).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
        drop(s);
        let before = readers::bytes(&d.path);
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
            "case {corruption}"
        );
        assert_eq!(readers::bytes(&d.path), before, "case {corruption}");
    }
}

#[test]
fn native_origin_batch_request_codec_rejects_null_redundant_and_foreign_context() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let r = request(&d, s.metadata().control_state().unwrap().store_id);
    let before = readers::bytes(&d.path);
    for value in [
        serde_json::Value::Null,
        json!({"proofVersion":1,"contractVersion":2}),
        json!({"proofVersion":1,"contractVersion":3}),
        json!({"proofVersion":2,"contractVersion":1}),
        json!({"proofVersion":1,"contractVersion":1,"extra":true}),
    ] {
        let mut wire = serde_json::to_value(&r).unwrap();
        wire["requestContext"] = value;
        if let Ok(decoded) = serde_json::from_value::<BatchInput>(wire) {
            assert!(crate::mcp::batches::prepare_native_store(&decoded).is_err());
        }
    }
    let mut legacy = r;
    legacy.mutation_version = 1;
    legacy.request_context = RetainedRequestContext::from_contract(1).unwrap();
    assert!(crate::mcp::batches::prepare(&legacy).is_err());
    assert_eq!(readers::bytes(&d.path), before);
}
