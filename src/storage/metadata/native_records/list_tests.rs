//! Actual native insertion/migration -> canonical list -> strict reopen. These
//! are inner transport consumers, not public contract2 activation evidence.
use super::*;
use crate::{
    canonical_list::{ListError, list_page},
    serialization::list::ListPageRequest,
    test_support::Fixture,
};
use serde_json::{Value, json};
fn request(cursor: Option<String>) -> ListPageRequest {
    let mut v = json!({"contractVersion":2,"namespace":"native","limit":2,"entities":["JOSÉ"]});
    if let Some(c) = cursor {
        v["cursor"] = json!(c)
    }
    ListPageRequest::from_mcp(&v).unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canonical_list_native_unicode_authority_transport_reopen_and_stale() {
    let mut root = tempfile::tempdir().unwrap();
    let d = certified_positive_fixture::certified_ghost_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let mut expected = vec![];
    for i in 0..7 {
        let mut original = input(&d);
        original.memory.parent_id = None;
        original.entities = if i < 5 {
            vec!["José".into(), "John Smith".into(), "Alice's".into()]
        } else {
            vec![]
        };
        // A canonical empty list must not fall back to this compatibility tag.
        original.memory.tags = vec![crate::model::Tag::new("entity/alice").unwrap()];
        let intent = session.capture_insert(scope(&d), &original).unwrap();
        let receipt = session
            .insert_prepared(
                &ctx(&session, &d, &format!("list-{i}")),
                &intent,
                &generated(),
            )
            .unwrap();
        if i < 5 {
            expected.push(inserted(&receipt));
        }
    }
    expected.sort_by_key(|id| *id.as_bytes());
    let mut engine = Some(session.engine);
    let mut cursor = None;
    let mut saved = None;
    let mut actual = vec![];
    let mut at = None;
    for _ in 0..3 {
        let last_cursor = cursor.clone();
        let f = Fixture::from_open_engine(root, engine.take().unwrap());
        // Canonical list is independent of derived publication and provider.
        let (router, bridge, query) = readers::readers(&f, clock.clone());
        let public = crate::mcp::tools::dispatch_tool(
            &bridge,
            "list_memories",
            json!({"contractVersion":2}),
        )
        .await;
        assert_eq!(public.is_error, Some(true));
        use tower::ServiceExt;
        let refused = router
            .clone()
            .oneshot(
                axum::http::Request::get("/memories?limit=malformed")
                    .header("Recalld-Contract-Version", "2")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(refused.status(), 400);
        let refused: Value = serde_json::from_slice(
            &axum::body::to_bytes(refused.into_body(), 8192)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(refused["field"], "contractVersion");
        let before = readers::bytes(&d.path);
        let omitted =
            crate::mcp::tools::dispatch_v2(&bridge, "list_memories", json!({"contractVersion":2}))
                .await;
        assert_eq!(omitted.is_error, Some(true)); // wrong frozen default never scans all scopes

        let page = list_page(f.storage.clone(), request(cursor.clone()))
            .await
            .unwrap();
        assert_eq!(page.data.total.get(), 5);
        assert_eq!(
            page.data.memories.len(),
            if actual.len() == 4 { 1 } else { 2 }
        );
        let first_at = *at.get_or_insert(page.binding.at);
        assert_eq!(page.binding.at, first_at);
        cursor = page.data.next_cursor.clone();
        if saved.is_none() {
            saved = cursor.clone();
        }
        for e in &page.data.memories {
            assert_eq!(e.entities, vec!["José", "John Smith", "Alice's"]);
            assert_eq!(
                e.labels.as_ref().unwrap().provenance,
                crate::model::schema::LabelProvenance::NativeV2
            );
            actual.push(e.id);
        }
        let expected_wire = serde_json::to_value(page.into_envelope()).unwrap();
        let mut args =
            json!({"contractVersion":2,"namespace":"NATIVE","limit":2,"entities":["JOSÉ"]});
        let original_cursor = last_cursor.clone();
        if let Some(c) = original_cursor {
            args["cursor"] = json!(c);
        }
        let mcp = crate::mcp::tools::dispatch_v2(&bridge, "list_memories", args).await;
        assert_ne!(mcp.is_error, Some(true));
        let wire = serde_json::to_value(mcp).unwrap();
        let text: Value =
            serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(text, wire["structuredContent"]);
        assert_eq!(text, expected_wire);
        let adapter = crate::api::adapters::StorageEngineAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
            f.graph.clone(),
        );
        let mut params = vec![
            ("limit".into(), "2".into()),
            ("entities".into(), "[\"JOSÉ\"]".into()),
        ];
        if let Some(c) = last_cursor.clone() {
            params.push(("cursor".into(), c));
        }
        let response = crate::api::list_memories_v2(&adapter, "native", &params).await;
        assert_eq!(response.status(), 200);
        let bytes = axum::body::to_bytes(
            response.into_body(),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap(),
            expected_wire
        );
        assert_eq!(readers::bytes(&d.path), before);
        drop((adapter, router, bridge, query));
        root = f.close();
        let reopened = open(&d, clock.clone());
        engine = Some(reopened.engine);
    }
    assert_eq!(actual, expected);
    assert!(cursor.is_none());
    let f = Fixture::from_open_engine(root, engine.take().unwrap());
    // All-mode retains migrated provenance after real native Ghost phase loss.
    let all = list_page(
        f.storage.clone(),
        ListPageRequest::from_mcp(&json!({"contractVersion":2,"namespace":"native"})).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(all.data.total.get(), 8);
    let rejected_fallback = list_page(
        f.storage.clone(),
        ListPageRequest::from_mcp(
            &json!({"contractVersion":2,"namespace":"native","entities":["alice"]}),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(rejected_fallback.data.total.get(), 0);
    assert!(
        all.data
            .memories
            .iter()
            .any(|e| e.labels.as_ref().unwrap().provenance
                == crate::model::schema::LabelProvenance::LegacyRetainedOnly)
    );
    assert_eq!(
        all.data
            .memories
            .iter()
            .filter(|e| e.entities.is_empty())
            .count(),
        3
    );
    let root = f.close();
    let mut session = open(&d, clock);
    let intent = session.capture_tombstone(expected[0], Some(d.ns)).unwrap();
    session
        .tombstone(&ctx(&session, &d, "list-delete"), &intent)
        .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    assert!(matches!(
        list_page(f.storage.clone(), request(saved)).await,
        Err(ListError::Stale)
    ));
    let fresh = list_page(f.storage.clone(), request(None)).await.unwrap();
    assert_eq!(fresh.data.total.get(), 4);
    assert!(!fresh.data.memories.iter().any(|r| r.id == expected[0]));
}

#[tokio::test]
async fn canonical_list_native_unknown_binding_is_readable_without_provider_or_vector_profile() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("unknown-legacy");
    let mut old = RedbStorageEngine::open(&source).unwrap();
    let mut ns = NamespaceConfig::default_namespace(0);
    ns.embedding_dim = 4;
    let id = old.create_namespace(&ns).unwrap();
    let item = crate::storage::insertion::InsertMemory {
        id: None,
        summary: "retained legacy".into(),
        full_text: None,
        tags: vec![],
        initial_stability: None,
        created_at: Some(100),
        embedding: Some(vec![1., 0., 0., 0.]),
        parent_id: None,
        supersedes: None,
    };
    old.insert_memory_with_receipt(&MutationContext::unkeyed(id), &item, &[], 100)
        .unwrap();
    old.sync().unwrap();
    drop(old);
    let target = root.path().join("unknown-native");
    migrate_schema(
        &source,
        &target,
        &MigrationOptions {
            migration_now: 200,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session =
        NativeLifecycleSession::open(&target, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    let before = readers::bytes(&target);
    let page = list_page(
        f.storage.clone(),
        ListPageRequest::from_mcp(&json!({"contractVersion":2,"namespace":"default"})).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(page.data.total.get(), 1);
    assert_eq!(page.data.memories[0].summary, "retained legacy");
    assert_eq!(
        page.data.memories[0].labels.as_ref().unwrap().provenance,
        crate::model::schema::LabelProvenance::LegacyRetainedOnly
    );
    assert!(matches!(
        f.storage
            .read()
            .unwrap()
            .get_namespace(id)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .embedding_binding,
        crate::model::schema::EmbeddingBinding::UnknownLegacy
    ));
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(readers::bytes(&target), before);
}
