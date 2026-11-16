use super::*;
use serde_json::json;

#[test]
fn contract_selectors_are_strict_and_v2_is_unconditionally_closed() {
    assert_eq!(
        ContractVersion::from_mcp(&json!({})).unwrap(),
        ContractVersion::V1
    );
    assert_eq!(
        ContractVersion::from_http(None).unwrap(),
        ContractVersion::V1
    );
    for (n, version) in [(1, ContractVersion::V1), (2, ContractVersion::V2)] {
        assert_eq!(
            ContractVersion::from_mcp(&json!({"contractVersion":n})).unwrap(),
            version
        );
        assert_eq!(
            ContractVersion::from_http(Some(&n.to_string())).unwrap(),
            version
        );
        assert_eq!(version.require_supported().is_ok(), n == 1);
    }
    for v in [
        json!(null),
        json!(true),
        json!("2"),
        json!(2.0),
        json!([]),
        json!({}),
        json!(-1),
        json!(0),
        json!(3),
    ] {
        assert!(
            ContractVersion::from_mcp(&json!({"contractVersion":v})).is_err(),
            "{v}"
        );
    }
    for v in ["", "02", "+2", "2.0", " 2", "2 ", "1,2", "0", "3"] {
        assert!(ContractVersion::from_http(Some(v)).is_err(), "{v}");
    }
    for v in [json!(null), json!([]), json!(1)] {
        assert!(ContractVersion::from_mcp(&v).is_err());
    }
    let error = reject_unknown_fields(json!({"a~/b":0}).as_object().unwrap(), &[]).unwrap_err();
    assert_eq!(error.code, "UNKNOWN_FIELD");
    assert_eq!(error.field, "/a~0~1b");
}

#[test]
fn contract_decimal_and_retry_options_reuse_a1_without_coercion_or_mutation() {
    let id = Uuid::from_u128(1);
    for decimal in ["0", "1", "9223372036854775807"] {
        let object = json!({"storeId":id,"expectedGeneration":decimal,"idempotencyKey":" K "});
        let original = object.clone();
        let options = WriteOptions::parse(object.as_object().unwrap(), Some(" K ")).unwrap();
        assert_eq!(object, original);
        assert_eq!(
            serde_json::to_value(options.expected_generation.unwrap()).unwrap(),
            decimal
        );
        let ctx = options.context(
            MutationScope::Namespace(crate::model::NamespaceId::new(2)),
            ContractVersion::V2,
        );
        assert_eq!(ctx.contract_version, 2);
        assert_eq!(ctx.idempotency_key.as_deref(), Some(" K "));
        assert!(ctx.validate().is_ok());
    }
    for v in [
        json!(0),
        json!(null),
        json!(true),
        json!(""),
        json!("00"),
        json!("01"),
        json!("+1"),
        json!("-1"),
        json!(" 1"),
        json!("1.0"),
        json!("9223372036854775808"),
        json!("18446744073709551616"),
    ] {
        let error = WriteOptions::parse(
            json!({"storeId":id,"expectedGeneration":v})
                .as_object()
                .unwrap(),
            None,
        )
        .unwrap_err();
        assert_eq!(error.field, "/expectedGeneration", "{v}");
    }
    for key in [" ".to_owned(), "x".repeat(128)] {
        assert!(
            WriteOptions::parse(json!({"storeId":id}).as_object().unwrap(), Some(&key)).is_ok()
        );
    }
    for key in [
        "".to_owned(),
        "x".repeat(129),
        "\n".into(),
        "é".into(),
        "\u{7f}".into(),
    ] {
        assert!(
            WriteOptions::parse(
                json!({"storeId":id,"idempotencyKey":key})
                    .as_object()
                    .unwrap(),
                None
            )
            .is_err()
        );
    }
    for object in [
        json!({"storeId":null}),
        json!({"storeId":Uuid::nil()}),
        json!({"storeId":1}),
        json!({"storeId":"invalid"}),
        json!({"idempotencyKey":"key"}),
        json!({"expectedGeneration":"0"}),
        json!({"storeId":id,"idempotencyKey":null}),
    ] {
        assert!(
            WriteOptions::parse(object.as_object().unwrap(), None).is_err(),
            "{object}"
        );
    }
    assert!(
        WriteOptions::parse(
            json!({"storeId":id,"idempotencyKey":"a"})
                .as_object()
                .unwrap(),
            Some("b")
        )
        .is_err()
    );
    assert!(WriteOptions::parse(json!({}).as_object().unwrap(), None).is_ok());
    assert!(WriteOptions::parse(json!({}).as_object().unwrap(), Some("key")).is_err());
}

#[test]
fn contract_partial_batch_classification_never_relabels_committed_items() {
    use ItemState::*;
    for (items, counts, status, http) in [
        (vec![Ok { ready: true }], (1, 0, 0), Status::Ok, 200),
        (vec![Ok { ready: false }], (1, 0, 0), Status::Partial, 207),
        (
            vec![Ok { ready: true }, Error, Unknown, NotAttempted],
            (1, 1, 2),
            Status::Partial,
            207,
        ),
        (vec![Error, Error], (0, 2, 0), Status::Error, 422),
        (
            vec![Error, Unknown, NotAttempted],
            (0, 1, 2),
            Status::Error,
            503,
        ),
    ] {
        let (actual, s, h) = BatchCounts::classify(&items).unwrap();
        assert_eq!((actual.succeeded, actual.failed, actual.unresolved), counts);
        assert_eq!(
            actual.total,
            actual.succeeded + actual.failed + actual.unresolved
        );
        assert_eq!((s, h), (status, http));
        assert!(
            serde_json::to_value(actual)
                .unwrap()
                .get("committedGeneration")
                .is_none()
        );
    }
    assert!(BatchCounts::classify(&[]).is_err());
}

#[test]
fn contract_envelope_fixture_serializes_decimal_counters_and_identical_mcp_object() {
    // Authored serializer fixture ONLY: these values are not backend readiness
    // evidence, a coherent ReadView, or native publication proof.
    use crate::storage::control::MutationOutcome;
    let store_id = Uuid::from_u128(1);
    let generation = Generation::MAX;
    let receipt = WriteReceipt {
        store_id,
        operation_id: Uuid::from_u128(2),
        committed_generation: generation,
        receipt_version: 1,
        outcome: MutationOutcome::GraphChanged {
            edges_put: 0,
            edges_removed: 1,
            memories_removed: 0,
        },
        replayed: true,
    };
    let mut envelope = Envelope::success(
        Scope::Catalog {
            kind: CatalogKind::Catalog,
        },
        json!({"count":1}),
    );
    envelope.receipt = Some(receipt);
    envelope.read_generation = Some(GenerationRef {
        store_id,
        generation: Generation::default(),
    });
    envelope.readiness = Some(Readiness {
        store_id,
        committed_generation: generation,
        ready: false,
        pending_work: 1,
        derived: vec![DerivedReadiness {
            name: "vector".into(),
            applied_generation: Some(Generation::default()),
            ready: false,
        }],
        pending_work_scope: None,
        observed_work: None,
    });
    envelope.status = Status::Error;
    envelope.error = Some(ExecutionError {
        category: ErrorCategory::Unavailable,
        code: "INDEX_NOT_READY".into(),
        message: "required view is pending".into(),
        field: None,
        retryable: false,
        commit_state: CommitState::Committed,
        operation_id: Some(Uuid::from_u128(2)),
    });
    let value = serde_json::to_value(&envelope).unwrap();
    assert_eq!(value["contractVersion"], 2);
    assert_eq!(value["scope"], json!({"kind":"catalog"}));
    assert_eq!(
        value["receipt"]["committedGeneration"],
        "9223372036854775807"
    );
    assert_eq!(value["readGeneration"]["generation"], "0");
    assert_eq!(value["readiness"]["derived"][0]["appliedGeneration"], "0");
    assert_eq!(value["readiness"]["pendingWork"], 1);
    assert_eq!(value["error"]["commitState"], "committed");
    assert_eq!(value["warnings"], json!([]));
    let wire = serde_json::to_value(crate::mcp::protocol::ToolCallResult::json(&envelope).unwrap())
        .unwrap();
    assert_eq!(wire["structuredContent"], value);
    assert_eq!(
        serde_json::from_str::<Value>(wire["content"][0]["text"].as_str().unwrap()).unwrap(),
        value
    );
}
