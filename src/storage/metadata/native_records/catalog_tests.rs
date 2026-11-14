//! Actual native writes/migration and reopen, not authored schema-row probes.
use super::*;
use crate::{
    catalog_page::{self, CatalogData, ProfileCompatibility},
    serialization::catalog_page::{CatalogKind, CatalogPageRequest},
    test_support::Fixture,
};
use serde_json::{Value, json};
struct NoProvider;
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for NoProvider {
    fn dimensions(&self) -> usize {
        panic!("catalog consulted provider dimensions")
    }
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        panic!("catalog consulted provider profile")
    }
    fn model_name(&self) -> &str {
        panic!("catalog consulted provider name")
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        panic!("catalog embedded text")
    }
}
fn storage_adapter(
    f: &Fixture,
    provider: Arc<dyn crate::embedding::EmbeddingProvider>,
) -> crate::mcp::bridge_adapters::McpStorageAdapter {
    crate::mcp::bridge_adapters::McpStorageAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        provider,
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
        Arc::new(Default::default()),
        chrono_tz::UTC,
    )
}
fn http_adapter(f: &Fixture) -> crate::api::adapters::StorageEngineAdapter {
    crate::api::adapters::StorageEngineAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
    )
}
async fn mcp(bridge: &crate::mcp::bridge::McpBridge, tool: &str, args: Value) -> Value {
    let result = crate::mcp::tools::dispatch_v2(bridge, tool, args).await;
    assert_ne!(result.is_error, Some(true));
    let wire = serde_json::to_value(result).unwrap();
    let text: Value = serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, wire["structuredContent"]);
    text
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_page_native_unicode_empty_provenance_actual_transports_and_reopen() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture_with_ghost(root.path(), true);
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    for i in 0..4 {
        let mut original = input(&d);
        original.memory.parent_id = None;
        original.memory.tags = vec![
            crate::model::Tag::new("entity/compatibility-only").unwrap(),
            crate::model::Tag::new("plain/keep").unwrap(),
        ];
        original.entities = match i {
            0 => vec![
                "José".into(),
                "John Smith".into(),
                "Alice's".into(),
                "José".into(),
            ],
            1 | 2 => vec!["José".into()],
            _ => vec![],
        };
        if i == 3 {
            original.topics.clear();
            original.emotions.clear();
        }
        let intent = session.capture_insert(scope(&d), &original).unwrap();
        session
            .insert_prepared(
                &ctx(&session, &d, &format!("catalog-{i}")),
                &intent,
                &generated(),
            )
            .unwrap();
    }
    let f = Fixture::from_open_engine(root, session.engine);
    let (router, mut bridge, query) = readers::readers(&f, clock.clone());
    bridge.storage = Arc::new(storage_adapter(&f, Arc::new(NoProvider)));
    let before = readers::bytes(&d.path);
    let clock_before = clock.0.load(Ordering::SeqCst);
    let wrong =
        crate::mcp::tools::dispatch_v2(&bridge, "list_tags", json!({"contractVersion":2})).await;
    assert_eq!(wrong.is_error, Some(true));
    let args = json!({"contractVersion":2,"namespace":"NATIVE","limit":1});
    let first = mcp(&bridge, "list_tags", args.clone()).await;
    assert_eq!(first["data"]["memoriesCounted"], "5");
    assert_eq!(first["data"]["entities"]["total"], "3");
    assert_eq!(first["data"]["entities"]["items"][0]["name"], "José");
    assert_eq!(first["data"]["entities"]["items"][0]["count"], "3");
    assert_eq!(
        first["data"]["entities"]["items"][0]["sources"]["canonicalNativeV2"],
        "3"
    );
    assert_eq!(
        first["data"]["entities"]["items"][0]["sources"]["rawTags"],
        "0"
    );
    assert_eq!(first["data"]["tags"]["items"][0]["name"], "plain/keep");
    assert_eq!(first["data"]["tags"]["items"][0]["count"], "4");
    let adapter = http_adapter(&f);
    let response = crate::api::catalog_v2(
        &adapter,
        "native",
        &[("limit".into(), "1".into())],
        CatalogKind::Tags,
    )
    .await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        serde_json::from_slice::<Value>(
            &axum::body::to_bytes(
                response.into_body(),
                crate::daemon::protocol::MAX_MESSAGE_SIZE
            )
            .await
            .unwrap()
        )
        .unwrap(),
        first
    );
    let namespaces = mcp(&bridge, "list_namespaces", json!({"contractVersion":2})).await;
    assert_eq!(
        namespaces["data"]["namespaces"][0]["profileCompatibility"],
        "unknownActiveProfile"
    );
    assert!(namespaces["data"]["configuredEmbedding"].is_null());
    use tower::ServiceExt;
    for path in ["/tags?limit=bad", "/namespaces?limit=bad"] {
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::get(path)
                    .header("Recalld-Contract-Version", "2")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        let value: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 8192)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["field"], "contractVersion");
    }
    assert_eq!(clock.0.load(Ordering::SeqCst), clock_before);
    assert_eq!(readers::bytes(&d.path), before);
    let cursor = first["data"]["nextCursor"].as_str().unwrap().to_owned();
    drop((adapter, router, bridge, query));
    let root = f.close();
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, session.engine);
    let mut r = CatalogPageRequest::from_mcp(&args, CatalogKind::Tags).unwrap();
    r.cursor = Some(cursor);
    r.limit = 2;
    let next = catalog_page::read(f.storage.clone(), r)
        .await
        .unwrap()
        .into_envelope();
    let next = serde_json::to_value(next).unwrap();
    assert_eq!(next["readGeneration"], first["readGeneration"]);
    assert_eq!(
        next["data"]["entities"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Alice's", "John Smith"]
    );
    assert_eq!(next["data"]["complete"], true);
    assert!(next["data"]["nextCursor"].is_null());
}
#[tokio::test]
async fn catalog_page_native_unknown_profile_preserves_retained_only_without_provider() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("old");
    let mut old = RedbStorageEngine::open(&source).unwrap();
    let mut ns = NamespaceConfig::default_namespace(0);
    ns.embedding_dim = 4;
    let id = old.create_namespace(&ns).unwrap();
    let item = crate::storage::insertion::InsertMemory {
        id: None,
        summary: "retained labels".into(),
        full_text: None,
        tags: vec![crate::model::Tag::new("entity/retained").unwrap()],
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
    let target = root.path().join("native");
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
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = Arc::new(storage_adapter(&f, Arc::new(NoProvider)));
    let before = readers::bytes(&target);
    let tags = mcp(&bridge, "list_tags", json!({"contractVersion":2})).await;
    let entry = &tags["data"]["entities"]["items"][0];
    assert_eq!(entry["name"], "retained");
    assert_eq!(entry["sources"]["canonicalLegacyRetainedOnly"], "1");
    assert_eq!(entry["sources"]["canonicalNativeV2"], "0");
    assert_eq!(entry["sources"]["rawTags"], "0");
    let namespaces = mcp(&bridge, "list_namespaces", json!({"contractVersion":2})).await;
    assert_eq!(
        namespaces["data"]["namespaces"][0]["profileCompatibility"],
        "unknownNamespaceProfile"
    );
    assert!(namespaces["data"]["namespaces"][0]["embeddingProfileId"].is_null());
    assert_eq!(readers::bytes(&target), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
}
struct ConfiguredProvider {
    descriptor: crate::model::schema::EmbeddingProfile,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for ConfiguredProvider {
    fn dimensions(&self) -> usize {
        4
    }
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Some(self.descriptor.clone())
    }
    fn model_name(&self) -> &str {
        "configuration only"
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        panic!("metadata catalog called embed")
    }
}
#[tokio::test]
async fn catalog_page_profile_compatibility_uses_supplied_configuration_not_equal_dimensions() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock);
    let f = Fixture::from_open_engine(root, session.engine);
    let before = readers::bytes(&d.path);
    for mismatch in [false, true] {
        let mut descriptor = d.profile.clone();
        if mismatch {
            descriptor.model_id.push_str("-different");
        }
        let provider = Arc::new(ConfiguredProvider {
            descriptor,
            calls: AtomicUsize::new(0),
        });
        let adapter =
            storage_adapter(&f, provider.clone()).with_catalog_profile_context(provider.as_ref());
        let rest = http_adapter(&f).with_catalog_profile_context(provider.as_ref());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.storage = Arc::new(adapter);
        let m = mcp(&bridge, "list_namespaces", json!({"contractVersion":2})).await;
        assert_eq!(
            m["data"]["namespaces"][0]["profileCompatibility"],
            if mismatch {
                "profileMismatch"
            } else {
                "compatible"
            }
        );
        let response = crate::api::catalog_v2(&rest, "default", &[], CatalogKind::Namespaces).await;
        assert_eq!(response.status(), 200);
        let h: Value = serde_json::from_slice(
            &axum::body::to_bytes(
                response.into_body(),
                crate::daemon::protocol::MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap(),
        )
        .unwrap();
        assert_eq!(h, m);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    }
    assert_eq!(readers::bytes(&d.path), before);
}
