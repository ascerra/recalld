use super::*;
use crate::{model::NamespaceId, storage::control::GenerationRef};
use crate::{
    serialization::list::ListPageRequest,
    storage::StorageEngine,
    test_support::{Fixture, MemorySpec},
};
use serde_json::json;
fn request(value: serde_json::Value) -> ListPageRequest {
    let mut value = value;
    value["contractVersion"] = json!(2);
    value["namespace"] = json!("default");
    ListPageRequest::from_mcp(&value).unwrap()
}
#[test]
fn canonical_list_strict_parser_schema_and_cursor_bindings() {
    let base = json!({"contractVersion":2,"namespace":null,"entities":["José","comma, label"]});
    let r = ListPageRequest::from_mcp(&base).unwrap();
    assert!(r.namespace.is_none());
    assert_eq!(r.limit, crate::model::constants::DEFAULT_LIST_LIMIT);
    for field in ["offset", "sort", "order", "recordAccess", "unknown"] {
        let mut v = base.clone();
        v[field] = json!(null);
        let e = ListPageRequest::from_mcp(&v).unwrap_err();
        assert_eq!(e.code, "UNKNOWN_FIELD");
    }
    for field in [
        "limit",
        "cursor",
        "entities",
        "tags",
        "phase",
        "timeRangeStart",
        "timeRangeEnd",
    ] {
        let mut v = base.clone();
        v[field] = json!(null);
        assert_eq!(
            ListPageRequest::from_mcp(&v).unwrap_err().field,
            format!("/{field}")
        );
    }
    for v in [json!(0), json!(201), json!(1.5), json!("1"), json!(-1)] {
        let mut b = base.clone();
        b["limit"] = v;
        assert!(ListPageRequest::from_mcp(&b).is_err());
    }
    for v in [
        json!({"timeRangeStart":2,"timeRangeEnd":1}),
        json!({"entities":["valid",2]}),
        json!({"phase":"tombstone"}),
        json!({"timeRangeStart":i64::MAX}),
    ] {
        let mut b = v;
        b["contractVersion"] = json!(2);
        assert!(ListPageRequest::from_mcp(&b).is_err());
    }
    let http =
        ListPageRequest::from_http(&[("entities".into(), "[\"José\",\"comma, label\"]".into())])
            .unwrap();
    assert_eq!(http.filter_hash(), r.filter_hash());
    assert!(
        ListPageRequest::from_http(&[("limit".into(), "1".into()), ("limit".into(), "2".into())])
            .is_err()
    );
    let cursor = ListCursor {
        version: 1,
        contract_version: 2,
        eligibility_version: 1,
        at: GenerationRef {
            store_id: uuid::Uuid::new_v4(),
            generation: Generation::MAX,
        },
        namespace_id: NamespaceId::new(1),
        namespace_sha256: [1; 32],
        filter_sha256: r.filter_hash(),
        after: MemoryId::from_uuid(uuid::Uuid::new_v4()),
    };
    let token = cursor.encode().unwrap();
    assert!(ListCursor::decode(&token, &r).is_ok());
    let mut different = r.clone();
    different.filters.labels.entities.push("other".into());
    assert!(ListCursor::decode(&token, &different).is_err());
    assert!(ListCursor::decode(&(token + "="), &r).is_err());
    assert_eq!(ListPageRequest::schema()["additionalProperties"], false);
}
#[tokio::test]
async fn canonical_list_legacy_unknown_profile_filters_before_page_and_stales_without_writes() {
    let f = Fixture::new();
    let other = f.create_namespace("other");
    for i in 0..9 {
        f.insert_memory_with(MemorySpec {
            summary: if i % 2 == 0 { "selected" } else { "other" },
            tags: if i % 2 == 0 { &["entity/alice"] } else { &[] },
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
        f.insert_memory_with(MemorySpec::new(other)).await;
    }
    let before = f.storage.read().unwrap().control_state().unwrap();
    let records = f
        .storage
        .read()
        .unwrap()
        .scan_all()
        .unwrap()
        .into_iter()
        .map(|(id, r)| (id, r.to_bytes()))
        .collect::<Vec<_>>();
    let mut r = request(json!({"limit":2,"entities":["Alice"]}));
    let mut ids = vec![];
    let mut first = None;
    loop {
        let page = list_page(f.storage.clone(), r.clone()).await.unwrap();
        assert_eq!(page.binding.at, before.generation_ref());
        assert_eq!(page.data.total.get(), 5);
        for e in page.data.memories {
            assert_eq!(e.summary, "selected");
            assert!(e.labels.is_none());
            ids.push(e.id);
        }
        let Some(token) = page.data.next_cursor else {
            assert!(page.data.complete);
            break;
        };
        first.get_or_insert(token.clone());
        r.cursor = Some(token);
    }
    assert_eq!(ids.len(), 5);
    assert!(ids.windows(2).all(|w| w[0].as_bytes() < w[1].as_bytes()));
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .scan_all()
            .unwrap()
            .into_iter()
            .map(|(id, r)| (id, r.to_bytes()))
            .collect::<Vec<_>>(),
        records
    );
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .status()
            .pending,
        0
    );
    f.insert_memory_with(MemorySpec::new(other)).await;
    r.cursor = first;
    assert!(matches!(
        list_page(f.storage.clone(), r).await,
        Err(ListError::Stale)
    ));
}

#[tokio::test]
async fn canonical_list_oversize_whole_error_smaller_retry_transport_budget_and_closed_admission() {
    let f = Fixture::new();
    let tags = (0..64)
        .map(|i| format!("entity/t{i:02}{}", "x".repeat(115)))
        .collect::<Vec<_>>();
    let refs = tags.iter().map(String::as_str).collect::<Vec<_>>();
    let summary = "\"\\\n".repeat(666);
    // Actual legacy storage path with valid maximum-width fields. This is not a
    // native-write proof; the separate native fixture covers that boundary.
    for _ in 0..40 {
        f.insert_memory_with(MemorySpec {
            summary: &summary,
            tags: &refs,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    }
    let original = request(json!({"limit":40}));
    assert!(matches!(
        list_page(f.storage.clone(), original.clone()).await,
        Err(ListError::TooLarge)
    ));
    let mut retry = original;
    retry.limit = 10;
    let first = list_page(f.storage.clone(), retry.clone()).await.unwrap();
    assert_eq!(first.data.total.get(), 40);
    assert!(!first.data.complete);
    retry.cursor = first.data.next_cursor.clone();
    let envelope = first.into_envelope();
    let result = crate::mcp::protocol::ToolCallResult::json(&envelope).unwrap();
    let rpc = crate::mcp::protocol::JsonRpcResponse::success(
        crate::mcp::protocol::JsonRpcId::Number(i64::MAX),
        serde_json::to_value(result).unwrap(),
    );
    assert!(
        serde_json::to_vec_pretty(&rpc).unwrap().len() < crate::daemon::protocol::MAX_MESSAGE_SIZE
    );
    let daemon = crate::daemon::protocol::DaemonResponse::success(
        u64::MAX,
        serde_json::to_value(&envelope).unwrap(),
    );
    let (mut writer, mut reader) = tokio::io::duplex(4096);
    let (sent, got) = tokio::join!(
        crate::daemon::protocol::write_framed_message(
            &mut writer,
            &daemon,
            crate::daemon::protocol::MAX_MESSAGE_SIZE
        ),
        crate::daemon::protocol::read_framed_response(
            &mut reader,
            crate::daemon::protocol::MAX_MESSAGE_SIZE
        )
    );
    sent.unwrap();
    assert_eq!(got.unwrap().result, daemon.result);
    retry.limit = 7; // page size is deliberately not a cursor binding
    let second = list_page(f.storage.clone(), retry.clone()).await.unwrap();
    assert_eq!(second.data.memories.len(), 7);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let freeze = tracker
        .close_and_drain(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .unwrap();
    assert!(matches!(
        list_page(f.storage.clone(), retry.clone()).await,
        Err(ListError::Unavailable)
    ));
    drop(freeze);
    assert!(list_page(f.storage.clone(), retry).await.is_ok());
    assert_eq!(tracker.status().pending, 0);
}

#[tokio::test]
async fn canonical_list_cursor_scope_filter_anchor_and_empty_exhaustion() {
    let f = Fixture::new();
    f.create_namespace("other");
    let empty = list_page(f.storage.clone(), request(json!({})))
        .await
        .unwrap();
    assert_eq!(empty.data.total.get(), 0);
    assert!(empty.data.complete);
    assert!(empty.data.next_cursor.is_none());
    f.insert_memory().await;
    f.insert_memory().await;
    let r = request(json!({"limit":1}));
    let first = list_page(f.storage.clone(), r.clone()).await.unwrap();
    let token = first.data.next_cursor.unwrap();
    let cursor = ListCursor::decode(&token, &r).unwrap();
    let mut forged = cursor.clone();
    forged.after = MemoryId::from_uuid(uuid::Uuid::nil());
    let mut next = r.clone();
    next.cursor = Some(forged.encode().unwrap());
    assert!(matches!(
        list_page(f.storage.clone(), next.clone()).await,
        Err(ListError::Input(_))
    ));
    next.cursor = Some(token.clone());
    next.namespace = Some("other".into());
    assert!(matches!(
        list_page(f.storage.clone(), next).await,
        Err(ListError::Stale)
    ));
    let mut changed = r;
    changed.cursor = Some(token);
    changed.filters.time_range_start = Some(1);
    assert!(matches!(
        list_page(f.storage.clone(), changed).await,
        Err(ListError::Input(_))
    ));
}
