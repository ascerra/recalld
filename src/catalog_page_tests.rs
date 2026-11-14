use super::*;
use crate::{
    serialization::catalog_page::*,
    test_support::{Fixture, MemorySpec},
};
use serde_json::{Value, json};
fn request(kind: CatalogKind, mut value: Value) -> CatalogPageRequest {
    value["contractVersion"] = json!(2);
    if kind == CatalogKind::Tags
        && value.get("allNamespaces") != Some(&json!(true))
        && value.get("namespace").is_none()
    {
        value["namespace"] = json!("default");
    }
    CatalogPageRequest::from_mcp(&value, kind).unwrap()
}
fn files(path: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn visit(
        root: &std::path::Path,
        dir: &std::path::Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                visit(root, &p, out)
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().into(),
                    std::fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut out = Default::default();
    visit(path, path, &mut out);
    out
}
#[test]
fn catalog_page_strict_inputs_and_cursor_schema() {
    for kind in [CatalogKind::Namespaces, CatalogKind::Tags] {
        let base = json!({"contractVersion":2});
        let parsed = CatalogPageRequest::from_mcp(&base, kind).unwrap();
        assert_eq!(parsed.limit, 50);
        for field in ["limit", "cursor", "offset", "recordAccess", "unknown"] {
            let mut value = base.clone();
            value[field] = Value::Null;
            assert!(CatalogPageRequest::from_mcp(&value, kind).is_err());
        }
        for value in [json!(0), json!(-1), json!(1.5), json!("1"), json!(501)] {
            let mut body = base.clone();
            body["limit"] = value;
            assert!(CatalogPageRequest::from_mcp(&body, kind).is_err());
        }
        assert!(
            CatalogPageRequest::from_http(
                &[("limit".into(), "1".into()), ("limit".into(), "2".into())],
                kind
            )
            .is_err()
        );
        assert_eq!(
            CatalogPageRequest::schema(kind)["additionalProperties"],
            false
        );
    }
    for body in [
        json!({"contractVersion":2,"allNamespaces":null}),
        json!({"contractVersion":2,"namespace":"default","allNamespaces":true}),
        json!({"contractVersion":2,"allNamespaces":"true"}),
    ] {
        assert!(CatalogPageRequest::from_mcp(&body, CatalogKind::Tags).is_err());
    }
    assert!(
        CatalogPageRequest::from_mcp(
            &json!({"contractVersion":2,"namespace":"default"}),
            CatalogKind::Namespaces
        )
        .is_err()
    );
    let r = request(CatalogKind::Tags, json!({}));
    let c = CatalogCursor {
        version: 1,
        contract_version: 2,
        ordering_version: 1,
        kind: r.kind,
        at: GenerationRef {
            store_id: uuid::Uuid::new_v4(),
            generation: Generation::MAX,
        },
        namespace_id: Some(NamespaceId::new(1)),
        catalog_sha256: "a".repeat(64),
        namespace_after: None,
        tag_after: [Some("b".repeat(64)), None, None, None],
    };
    let token = c.encode().unwrap();
    assert!(token.len() < 4096);
    assert!(CatalogCursor::decode(&token, &r).is_ok());
    assert!(CatalogCursor::decode(&(token.clone() + "="), &r).is_err());
    let mut changed = r.clone();
    changed.all_namespaces = true;
    changed.namespace = None;
    assert!(CatalogCursor::decode(&token, &changed).is_err());
    let mut unknown = c;
    unknown.ordering_version = 2;
    assert!(CatalogCursor::decode(&unknown.encode().unwrap(), &r).is_err());
}
#[tokio::test]
async fn catalog_page_tag_counts_all_buckets_ties_continuation_and_mutation() {
    let f = Fixture::new();
    let other = f.create_namespace("other");
    for i in 0..203 {
        let tags = [
            format!("plain/p{i:03}"),
            format!("entity/e{i:03}"),
            format!("topic/t{i:03}"),
            format!("emotion/m{i:03}"),
            "entity/common".into(),
        ];
        let refs = tags.iter().map(String::as_str).collect::<Vec<_>>();
        f.insert_memory_with(MemorySpec {
            tags: &refs,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    }
    f.insert_memory_with(MemorySpec {
        tags: &["entity/excluded"],
        ..MemorySpec::new(other)
    })
    .await;
    let before = files(f.path());
    let mut r = request(CatalogKind::Tags, json!({"limit":100}));
    let mut seen: [Vec<String>; 4] = std::array::from_fn(|_| Vec::new());
    let mut saved = None;
    let mut pages = 0;
    loop {
        let page = read(f.storage.clone(), r.clone()).await.unwrap();
        let CatalogData::Tags(data) = page.data else {
            panic!()
        };
        assert_eq!(data.memories_counted.get(), 203);
        for (i, bucket) in [&data.tags, &data.entities, &data.topics, &data.emotions]
            .into_iter()
            .enumerate()
        {
            assert_eq!(bucket.total.get(), if i == 1 { 204 } else { 203 });
            for entry in &bucket.items {
                assert_eq!(entry.sources.raw_tags, entry.count);
                assert_eq!(entry.sources.canonical_native_v2.get(), 0);
                seen[i].push(entry.name.clone());
            }
        }
        pages += 1;
        let Some(token) = data.next_cursor else {
            assert!(data.complete);
            assert!(
                ![data.tags, data.entities, data.topics, data.emotions]
                    .iter()
                    .any(|b| b.truncated)
            );
            break;
        };
        assert!(!data.complete);
        saved.get_or_insert(token.clone());
        r.cursor = Some(token);
        r.limit = 73;
    }
    assert_eq!(pages, 3);
    assert_eq!(seen[1][0], "common");
    for i in [0, 2, 3] {
        assert_eq!(seen[i].len(), 203);
        assert!(seen[i].windows(2).all(|pair| pair[0] < pair[1]));
    }
    assert_eq!(seen[1].len(), 204);
    assert_eq!(files(f.path()), before);
    let mut cursor = CatalogCursor::decode(saved.as_ref().unwrap(), &r).unwrap();
    cursor.tag_after[1] = Some("0".repeat(64));
    r.cursor = Some(cursor.encode().unwrap());
    assert!(matches!(
        read(f.storage.clone(), r.clone()).await,
        Err(CatalogError::Input(_))
    ));
    r.cursor = saved;
    r.namespace = Some("other".into());
    assert!(matches!(
        read(f.storage.clone(), r.clone()).await,
        Err(CatalogError::Stale)
    ));
    r.namespace = Some("default".into());
    f.insert_memory_with(MemorySpec::new(other)).await;
    assert!(matches!(
        read(f.storage.clone(), r).await,
        Err(CatalogError::Stale)
    ));
}
#[tokio::test]
async fn catalog_page_namespace_totals_order_empty_and_reopen() {
    let f = Fixture::new();
    for name in ["z-last", "a-first", "middle", "empty"] {
        f.create_namespace(name);
    }
    f.insert_memory().await;
    let before = files(f.path());
    let r = request(CatalogKind::Namespaces, json!({"limit":2}));
    let first = read(f.storage.clone(), r.clone())
        .await
        .unwrap()
        .with_active_embedding(None);
    let at = first.at;
    let CatalogData::Namespaces(data) = first.data else {
        panic!()
    };
    assert_eq!(data.total.get(), 5);
    assert_eq!(data.namespaces[0].id.get(), 1);
    assert_eq!(data.namespaces[0].memory_count.get(), 1);
    assert!(matches!(
        data.namespaces[0].profile_compatibility,
        ProfileCompatibility::UnknownNamespaceProfile
    ));
    let token = data.next_cursor.unwrap();
    assert_eq!(files(f.path()), before);
    let dir = f.close();
    let engine = RedbStorageEngine::open(dir.path()).unwrap();
    let f = Fixture::from_open_engine(dir, engine);
    let mut next = r;
    next.cursor = Some(token);
    next.limit = 3;
    let page = read(f.storage.clone(), next).await.unwrap();
    assert_eq!(page.at, at);
    let CatalogData::Namespaces(data) = page.data else {
        panic!()
    };
    assert_eq!(
        data.namespaces
            .iter()
            .map(|e| e.id.get())
            .collect::<Vec<_>>(),
        vec![3, 4, 5]
    );
    assert!(data.complete);
    assert!(data.next_cursor.is_none());
    assert!(data.namespaces.iter().all(|e| e.memory_count.get() == 0));
    let page = read(
        f.storage.clone(),
        request(CatalogKind::Tags, json!({"namespace":"empty"})),
    )
    .await
    .unwrap();
    let CatalogData::Tags(data) = page.data else {
        panic!()
    };
    assert_eq!(data.memories_counted.get(), 0);
    assert!(data.complete);
    assert_eq!(data.tags.total.get(), 0);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let freeze = tracker
        .close_and_drain(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .unwrap();
    assert!(matches!(
        read(
            f.storage.clone(),
            request(CatalogKind::Namespaces, json!({}))
        )
        .await,
        Err(CatalogError::Unavailable)
    ));
    drop(freeze);
    assert_eq!(tracker.status().pending, 0);
}
#[tokio::test]
async fn catalog_page_response_budget_whole_error_and_serialized_transport_retry() {
    let f = Fixture::new();
    for row in 0..20 {
        let tags = (0..64)
            .map(|i| {
                let prefix = ["tag/", "entity/", "topic/", "emotion/"][i % 4];
                let value = format!("{prefix}{row:03}{i:03}");
                format!("{value}{}", "x".repeat(128 - value.len()))
            })
            .collect::<Vec<_>>();
        let refs = tags.iter().map(String::as_str).collect::<Vec<_>>();
        f.insert_memory_with(MemorySpec {
            tags: &refs,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    }
    let before = files(f.path());
    assert!(matches!(
        read(
            f.storage.clone(),
            request(CatalogKind::Tags, json!({"limit":500}))
        )
        .await,
        Err(CatalogError::ResponseTooLarge)
    ));
    assert_eq!(files(f.path()), before);
    let page = read(
        f.storage.clone(),
        request(CatalogKind::Tags, json!({"limit":50})),
    )
    .await
    .unwrap();
    let envelope = page.into_envelope();
    let result = crate::mcp::protocol::ToolCallResult::json(&envelope).unwrap();
    let wire = serde_json::to_value(&result).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(wire["content"][0]["text"].as_str().unwrap()).unwrap(),
        wire["structuredContent"]
    );
    let rpc = crate::mcp::protocol::JsonRpcResponse::success(
        crate::mcp::protocol::JsonRpcId::Number(i64::MAX),
        wire,
    );
    assert!(serde_json::to_vec(&rpc).unwrap().len() < crate::daemon::protocol::MAX_MESSAGE_SIZE);
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
    assert_eq!(files(f.path()), before);
}
