//! Actual emitters plus explicitly authored peer evidence, not new proof rules.
use super::*;

#[tokio::test]
async fn live_record_schema_retains_foreign_typed_receipt_without_attribution() {
    use crate::serialization::{batch_api1, native_single, store_api1};
    let (d, f, a, provider, clock) = setup().await;
    let input = crate::storage::batch::BatchInput {
        request_context: None,
        expected_generation: None,
        result_version: 1,
        mutation_version: 1,
        kind: crate::storage::batch::BatchKind::Insert,
        items: vec![json!({"summary":"genuine reservation", "checkDuplicates":false})],
        default_namespace: "native".into(),
        store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
        idempotency_key: Some("record-schema-parent".into()),
        batch_operation_id: None,
    };
    let batch_api1::Reply::Owned(Ok(out)) = a
        .store_batch_api1(batch_api1::Request {
            surface: batch_api1::Surface::Mcp,
            input,
        })
        .await
    else {
        panic!("genuine completed batch expected")
    };
    let receipt = out.outcome.reservation_receipt.unwrap();
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::BatchReserved { .. }
    ));
    let request = store_api1::Request {
        input: crate::mcp::args::parse_store_input_fields(
            &json!({"summary":"different single write"}),
            "native",
        )
        .unwrap(),
        profile_id: None,
        created_at: None,
        require_native: false,
    };
    let before = bytes(&d.path);
    let calls = provider.calls.load(Ordering::SeqCst);
    let ticks = clock.0.load(Ordering::SeqCst);
    let mut reply = native_single::Reply::uncertain();
    reply.receipt = Some(receipt.clone());
    let peer = canonical(&store_api1::Reply::Owned(reply));
    let result = store_api1::Reply::decode(peer.clone(), &request).mcp();
    let value = Value::Object(result.structured_content.clone().unwrap());
    assert_eq!(value["commitState"], "unknown");
    assert_eq!(value["receipt"], canonical(&receipt));
    assert!(value.get("readiness").is_none());
    crate::test_support::record_schema_result(
        "store-foreign-retained",
        "store_memory",
        &result,
        "authored typed peer error using a genuine separately committed BatchReserved receipt; unknown current single-write attribution, no new producer work",
    );
    let mut malformed = peer;
    malformed["unexpected"] = json!(true);
    let result = store_api1::Reply::decode(malformed, &request).mcp();
    let value = Value::Object(result.structured_content.clone().unwrap());
    assert_eq!(value["commitState"], "unknown");
    assert!(value.get("receipt").is_none());
    assert_eq!(value["receivedEvidence"]["receipt"], canonical(&receipt));
    crate::test_support::record_schema_result(
        "store-foreign-unverified",
        "store_memory",
        &result,
        "malformed authored Remote wrapper retains genuine parent receipt in explicitly unverified evidence, not current-write commitment",
    );
    assert_eq!(bytes(&d.path), before);
    assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
    assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
}

#[tokio::test]
async fn live_record_schema_actual_legacy_durable_forget_mixed() {
    let (_d, _native, _a, provider, clock) = setup().await;
    let f = Fixture::new();
    let id = f.insert_memory().await;
    readers::completion(&f).complete().await.unwrap();
    let a = Arc::new(adapter(&f, provider, clock, Default::default()));
    let mut b = bridge(a);
    b.default_namespace = "default".into();
    let result = crate::mcp::tools::dispatch_tool(
        &b,
        "forget_memories",
        json!({"ids":[id, MemoryId::new(), "invalid"]}),
    )
    .await;
    let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
    let value: Value = serde_json::from_str(text).unwrap();
    assert_eq!(value["total"], 3);
    assert_eq!(value["deleted"], 1);
    assert!(value["errors"].as_u64().unwrap() >= 1);
    assert_eq!(result.is_error, Some(true));
    assert!(value["batch"]["reservationReceipt"].is_object());
    crate::test_support::record_schema_result(
        "forget-durable-mixed",
        "forget_memories",
        &result,
        "actual public MCP durable legacy batch: one existing ID, missing ID and malformed ID; original indexed partial failure and full receipts",
    );
}
