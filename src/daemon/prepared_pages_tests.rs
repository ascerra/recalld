//! Real owning adapter and same-connection Remote page regressions.
use crate::{
    daemon::server::native_lifecycle_test_remote,
    mcp::{bridge::StorageEngine as _, bridge_adapters::McpStorageAdapter},
    model::NamespaceId,
    serialization::{
        catalog_page::{CatalogKind, CatalogPageRequest},
        list::ListPageRequest,
    },
    storage::StorageEngine as _,
    test_support::{Fixture, MemorySpec},
};
use serde_json::json;
use std::sync::Arc;

struct NoProvider;
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for NoProvider {
    fn dimensions(&self) -> usize {
        panic!("metadata page consulted provider dimensions")
    }
    fn model_name(&self) -> &str {
        panic!("metadata page consulted provider name")
    }
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        panic!("metadata page consulted profile")
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        panic!("metadata page embedded")
    }
}
struct NoClock;
impl crate::time::Clock for NoClock {
    fn now_millis(&self) -> i64 {
        panic!("metadata page consulted clock")
    }
}

fn adapter(f: &Fixture) -> Arc<McpStorageAdapter> {
    Arc::new(
        McpStorageAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            Arc::new(NoProvider),
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
            f.graph.clone(),
            Arc::new(Default::default()),
            chrono_tz::UTC,
        )
        .with_clock(Arc::new(NoClock)),
    )
}

#[tokio::test]
async fn prepared_pages_remote_continuation_filters_and_stale_generation() {
    let f = Fixture::new();
    let other = f.create_namespace("other");
    for i in 0..203 {
        let tags = [format!("entity/e{i:03}"), "entity/common".into()];
        let tags = tags.iter().map(String::as_str).collect::<Vec<_>>();
        f.insert_memory_with(MemorySpec {
            tags: &tags,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    }
    f.insert_memory_with(MemorySpec {
        tags: &["entity/excluded"],
        ..MemorySpec::new(other)
    })
    .await;
    let owned = adapter(&f);
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) =
        native_lifecycle_test_remote(owned.clone(), &socket.path().join("pages.sock")).await;
    let before = canonical(&f);
    let mut request = ListPageRequest::from_mcp(&json!({"contractVersion":2,
        "namespace":"DEFAULT","entities":["common"],"limit":100}))
    .unwrap();
    let mut seen = Vec::new();
    let mut saved = None;
    loop {
        let local = owned.list_canonical_page(request.clone()).await.unwrap();
        let page = remote.list_canonical_page(request.clone()).await.unwrap();
        assert_eq!(
            serde_json::to_vec(&page.data).unwrap(),
            serde_json::to_vec(&local.data).unwrap()
        );
        assert_eq!(page.binding, local.binding);
        assert_eq!(page.data.total.get(), 203);
        seen.extend(page.data.memories.iter().map(|e| e.id));
        request.cursor = page.data.next_cursor;
        if saved.is_none() {
            saved = request.cursor.clone();
        }
        if request.cursor.is_none() {
            assert!(page.data.complete);
            break;
        }
        request.limit = 51;
    }
    assert_eq!(seen.len(), 203);
    assert!(seen.windows(2).all(|w| w[0] < w[1]));
    let mut tags = CatalogPageRequest::from_mcp(
        &json!({"contractVersion":2,"namespace":"default","limit":100}),
        CatalogKind::Tags,
    )
    .unwrap();
    let mut names = Vec::new();
    loop {
        let local = owned.list_catalog_page(tags.clone()).await.unwrap();
        let page = remote.list_catalog_page(tags.clone()).await.unwrap();
        assert_eq!(
            serde_json::to_vec(&page.data).unwrap(),
            serde_json::to_vec(&local.data).unwrap()
        );
        let crate::catalog_page::CatalogData::Tags(data) = page.data else {
            panic!("wrong catalog");
        };
        assert_eq!(data.memories_counted.get(), 203);
        assert_eq!(data.entities.total.get(), 204);
        assert!(data.topics.items.is_empty() && !data.topics.truncated);
        names.extend(data.entities.items.iter().map(|e| e.name.clone()));
        tags.cursor = data.next_cursor;
        if tags.cursor.is_none() {
            assert!(data.complete);
            break;
        }
        tags.limit = 51;
    }
    assert_eq!(names.len(), 204);
    assert_eq!(names[0], "common");
    assert_eq!(canonical(&f), before);
    request.cursor = saved;
    let mut changed = request.clone();
    changed.filters.labels.entities = vec!["other".into()];
    assert!(matches!(
        remote.list_canonical_page(changed).await,
        Err(crate::canonical_list::ListError::Input(_))
    ));
    // A real unrelated write invalidates the store-wide generation binding.
    f.insert_memory_with(MemorySpec::new(other)).await;
    assert!(matches!(
        remote.list_canonical_page(request).await,
        Err(crate::canonical_list::ListError::Stale)
    ));
    drop(remote);
    task.await.unwrap();
}

fn canonical(f: &Fixture) -> Vec<u8> {
    let storage = f.storage.read().unwrap();
    let records = storage
        .scan_all()
        .unwrap()
        .into_iter()
        .map(|(id, record)| (id.to_string(), record.to_bytes()))
        .collect::<Vec<_>>();
    serde_json::to_vec(&(storage.control_state().unwrap(), records)).unwrap()
}

async fn malformed_peer(
    value: serde_json::Value,
    method: &'static str,
) -> (
    crate::daemon::bridge_adapters::RemoteStorageAdapter,
    tokio::task::JoinHandle<()>,
    tempfile::TempDir,
) {
    use crate::daemon::protocol::*;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("peer.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(ping.method, "ping");
        // Metadata capability alone: no query capability and no provider.
        let caps = json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE,"canonicalPagesV1":true,"preparedQueriesV1":false});
        write_framed_message(
            &mut stream,
            &DaemonResponse::success(ping.id, caps.clone()),
            MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
        let request = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(request.method, method);
        write_framed_message(
            &mut stream,
            &DaemonResponse::success(request.id, value),
            MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
        let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(
            ping.method, "ping",
            "malformed response must not trigger fallback/retry"
        );
        write_framed_message(
            &mut stream,
            &DaemonResponse::success(ping.id, caps),
            MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
    });
    let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
    (
        crate::daemon::bridge_adapters::RemoteStorageAdapter::new(client),
        task,
        root,
    )
}

#[tokio::test]
async fn prepared_pages_remote_rejects_total_omitting_prior_or_future_entries() {
    use crate::daemon::{
        prepared_pages as wire, prepared_query::response_limit, protocol::MAX_MESSAGE_SIZE,
    };
    let f = Fixture::new();
    f.create_namespace("second");
    f.create_namespace("third");
    for tag in ["entity/a", "entity/b", "entity/c"] {
        f.insert_memory_with(MemorySpec {
            tags: &[tag],
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    }
    let owned = adapter(&f);
    let before = canonical(&f);
    let args = json!({"contractVersion":2,"namespace":"default","limit":1});
    let first = owned
        .list_canonical_page(ListPageRequest::from_mcp(&args).unwrap())
        .await
        .unwrap();
    let mut continued = args.clone();
    continued["cursor"] = json!(first.data.next_cursor.unwrap());
    let mut raw = wire::dispatch(
        owned.as_ref(),
        wire::LIST,
        json!({"arguments":continued,"responseLimit":response_limit()}),
        Some(MAX_MESSAGE_SIZE),
    )
    .await
    .unwrap();
    assert_eq!(raw["result"]["data"]["total"], "3");
    assert_eq!(raw["result"]["data"]["complete"], false);
    raw["result"]["data"]["total"] = json!("2");
    let (remote, task, _root) = malformed_peer(raw, wire::LIST).await;
    let list = remote
        .list_canonical_page(ListPageRequest::from_mcp(&continued).unwrap())
        .await;
    let list_rejected = list.is_err();
    eprintln!("list prior+current+future total2 rejected={list_rejected}");
    remote.test_client().ping().await.unwrap();
    drop(remote);
    task.await.unwrap();
    let mut rejected = vec![list_rejected];
    for kind in [CatalogKind::Namespaces, CatalogKind::Tags] {
        let mut a = json!({"contractVersion":2,"limit":1});
        if kind == CatalogKind::Tags {
            a["namespace"] = json!("default");
        }
        let request = CatalogPageRequest::from_mcp(&a, kind).unwrap();
        let first = owned.list_catalog_page(request).await.unwrap();
        if let crate::catalog_page::CatalogData::Namespaces(data) = first.data {
            a["cursor"] = json!(data.next_cursor.unwrap());
        }
        let mut raw = wire::dispatch(
            owned.as_ref(),
            wire::CATALOG,
            json!({"arguments":a,"catalogKind":kind,"responseLimit":response_limit()}),
            Some(MAX_MESSAGE_SIZE),
        )
        .await
        .unwrap();
        match kind {
            CatalogKind::Namespaces => {
                assert_eq!(raw["result"]["data"]["total"], "3");
                raw["result"]["data"]["total"] = json!("2");
            }
            CatalogKind::Tags => {
                assert_eq!(raw["result"]["data"]["entities"]["truncated"], true);
                raw["result"]["data"]["entities"]["total"] = json!("1");
            }
        }
        let (remote, task, _root) = malformed_peer(raw, wire::CATALOG).await;
        let result = remote
            .list_catalog_page(CatalogPageRequest::from_mcp(&a, kind).unwrap())
            .await;
        eprintln!(
            "catalog {kind:?} missing required count rejected={}",
            result.is_err()
        );
        rejected.push(result.is_err());
        remote.test_client().ping().await.unwrap();
        drop(remote);
        task.await.unwrap();
    }
    assert_eq!(canonical(&f), before);
    assert_eq!(
        rejected,
        vec![true; 3],
        "untrusted totals must include prior anchor and promised continuation"
    );
}

#[tokio::test]
async fn prepared_pages_remote_strict_peer_fields_identity_and_context() {
    use crate::daemon::{
        prepared_pages as wire, prepared_query::response_limit, protocol::MAX_MESSAGE_SIZE,
    };
    let f = Fixture::new();
    f.insert_memory_with(MemorySpec::new(NamespaceId::new(1)))
        .await;
    let owned = adapter(&f);
    let before = canonical(&f);
    let args = json!({"contractVersion":2,"namespace":"default","limit":1});
    let good = wire::dispatch(
        owned.as_ref(),
        wire::LIST,
        json!({"arguments":args,"responseLimit":response_limit()}),
        Some(MAX_MESSAGE_SIZE),
    )
    .await
    .unwrap();
    for (path, value) in [
        ("/extra", json!(true)),
        ("/result/namespace", json!("other")),
        ("/result/namespaceId", json!(0)),
        ("/result/at/storeId", json!(uuid::Uuid::nil())),
        ("/result/at/generation", json!("01")),
        ("/result/data/total", json!(1)),
        ("/result/data/limit", json!(2)),
        ("/result/data/memories/0/id", json!(uuid::Uuid::nil())),
        ("/result/data/memories/0/entities", json!(["invented"])),
    ] {
        let mut raw = good.clone();
        if path == "/extra" {
            raw["extra"] = value;
        } else {
            *raw.pointer_mut(path).unwrap() = value;
        }
        let (remote, task, _root) = malformed_peer(raw, wire::LIST).await;
        assert!(
            matches!(
                remote
                    .list_canonical_page(ListPageRequest::from_mcp(&args).unwrap())
                    .await,
                Err(crate::canonical_list::ListError::Unavailable)
            ),
            "{path}"
        );
        remote.test_client().ping().await.unwrap();
        drop(remote);
        task.await.unwrap();
    }
    let args = json!({"contractVersion":2});
    let good = wire::dispatch(
        owned.as_ref(),
        wire::CATALOG,
        json!({"arguments":args,"catalogKind":"namespaces","responseLimit":response_limit()}),
        Some(MAX_MESSAGE_SIZE),
    )
    .await
    .unwrap();
    for (path, value) in [
        (
            "/result/data/namespaces/0/profileCompatibility",
            json!("compatible"),
        ),
        (
            "/result/data/configuredEmbedding",
            json!({"identity":"unknown","dimension":0}),
        ),
        (
            "/result/data/configuredEmbedding",
            json!({"identity":"configuration","dimension":4,"profileId":"invented"}),
        ),
    ] {
        let mut raw = good.clone();
        *raw.pointer_mut(path).unwrap() = value;
        let (remote, task, _root) = malformed_peer(raw, wire::CATALOG).await;
        assert!(
            matches!(
                remote
                    .list_catalog_page(
                        CatalogPageRequest::from_mcp(&args, CatalogKind::Namespaces).unwrap()
                    )
                    .await,
                Err(crate::catalog_page::CatalogError::Unavailable)
            ),
            "{path}"
        );
        remote.test_client().ping().await.unwrap();
        drop(remote);
        task.await.unwrap();
    }
    // A claimed Bound ID never makes an unsupported width a valid profile.
    // This is authored peer evidence, not a genuine native namespace.
    let mut bound = good.clone();
    bound["result"]["data"]["namespaces"][0]["embeddingDim"] = json!(8193);
    bound["result"]["data"]["namespaces"][0]["embeddingProfileId"] =
        json!(format!("ep1:{}", "0".repeat(64)));
    bound["result"]["data"]["namespaces"][0]["bindingProvenance"] = json!("operator_attestation");
    bound["result"]["data"]["namespaces"][0]["profileCompatibility"] =
        json!("unknownActiveProfile");
    let (remote, task, _root) = malformed_peer(bound, wire::CATALOG).await;
    assert!(matches!(
        remote
            .list_catalog_page(
                CatalogPageRequest::from_mcp(&args, CatalogKind::Namespaces).unwrap()
            )
            .await,
        Err(crate::catalog_page::CatalogError::Unavailable)
    ));
    remote.test_client().ping().await.unwrap();
    drop(remote);
    task.await.unwrap();
    assert_eq!(canonical(&f), before);
}

#[tokio::test]
async fn prepared_pages_old_and_limited_peers_refuse_without_page_send() {
    use crate::daemon::protocol::*;
    for (cap, limit) in [(false, MAX_MESSAGE_SIZE), (true, LEGACY_MAX_MESSAGE_SIZE)] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("old.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let peer = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            for _ in 0..2 {
                let ping = read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
                assert_eq!(
                    ping.method, "ping",
                    "no page request or fallback may be sent"
                );
                let result =
                    json!({"protocolVersion":2,"maxMessageSize":limit,"canonicalPagesV1":cap});
                write_framed_message(
                    &mut stream,
                    &DaemonResponse::success(ping.id, result),
                    MAX_MESSAGE_SIZE,
                )
                .await
                .unwrap();
            }
        });
        let client = Arc::new(crate::daemon::DaemonClient::connect(&path).await.unwrap());
        let remote = crate::daemon::bridge_adapters::RemoteStorageAdapter::new(client.clone());
        let request =
            ListPageRequest::from_mcp(&json!({"contractVersion":2,"namespace":"default"})).unwrap();
        assert!(matches!(
            remote.list_canonical_page(request).await,
            Err(crate::canonical_list::ListError::Unavailable)
        ));
        client.ping().await.unwrap();
        drop((client, remote));
        peer.await.unwrap();
    }
    let f = Fixture::new();
    let before = canonical(&f);
    let response=crate::daemon::server::native_method_test_rpc_with_limit(adapter(&f),
        json!({"arguments":{"contractVersion":2,"namespace":"default"},"responseLimit":crate::daemon::prepared_query::response_limit()}),
        crate::daemon::prepared_pages::LIST,None).await;
    assert!(response.error.is_some());
    assert!(response.result.is_none());
    assert_eq!(canonical(&f), before);
}

#[tokio::test]
async fn prepared_pages_whole_size_error_smaller_retry_actual_transport_frames() {
    let f = Fixture::new();
    let tags = (0..64)
        .map(|i| format!("entity/t{i:02}{}", "x".repeat(115)))
        .collect::<Vec<_>>();
    let refs = tags.iter().map(String::as_str).collect::<Vec<_>>();
    let summary = "\"\\\n".repeat(666);
    for _ in 0..40 {
        f.insert_memory_with(MemorySpec {
            summary: &summary,
            tags: &refs,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    }
    let owned = adapter(&f);
    let before = canonical(&f);
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) =
        native_lifecycle_test_remote(owned, &socket.path().join("size.sock")).await;
    let mut r =
        ListPageRequest::from_mcp(&json!({"contractVersion":2,"namespace":"default","limit":40}))
            .unwrap();
    assert!(matches!(
        remote.list_canonical_page(r.clone()).await,
        Err(crate::canonical_list::ListError::TooLarge)
    ));
    r.limit = 10;
    let page = remote.list_canonical_page(r).await.unwrap();
    assert_eq!(page.data.total.get(), 40);
    assert!(!page.data.complete);
    let envelope = page.into_envelope();
    let rest = serde_json::to_vec(&envelope).unwrap();
    let tool = crate::mcp::protocol::ToolCallResult::json(&envelope).unwrap();
    let mut lengths = Vec::new();
    for legacy in [false, true] {
        let mut tool = tool.clone();
        if legacy {
            tool.structured_content = None;
        }
        let value = serde_json::to_value(tool).unwrap();
        let wrapped = crate::mcp::protocol::ResultEnvelope::complete(value.clone());
        let mcp = crate::mcp::protocol::JsonRpcResponse::success(
            crate::mcp::protocol::JsonRpcId::Number(i64::MAX),
            serde_json::to_value(wrapped).unwrap(),
        );
        let daemon = crate::daemon::protocol::DaemonResponse::success(u64::MAX, value);
        let sizes = (
            serde_json::to_vec(&mcp).unwrap().len(),
            serde_json::to_vec(&daemon).unwrap().len(),
        );
        assert!(
            sizes.0 < crate::daemon::protocol::MAX_MESSAGE_SIZE
                && sizes.1 < crate::daemon::protocol::MAX_MESSAGE_SIZE
        );
        lengths.push(sizes);
    }
    eprintln!(
        "actual legacy-store escaped-field page REST={} modern/legacy MCP+daemon={lengths:?}; not an arbitrary-ID or RSS bound",
        rest.len()
    );
    assert!(rest.len() < crate::daemon::protocol::MAX_MESSAGE_SIZE);
    drop(remote);
    task.await.unwrap();
    assert_eq!(canonical(&f), before);
}

#[tokio::test]
async fn prepared_pages_actual_mcp_http_catalog_errors_have_kind_schemas() {
    let f = Fixture::new();
    let before = canonical(&f);
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = adapter(&f);
    let rest = crate::api::adapters::StorageEngineAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
    );
    for (kind, tool) in [
        (CatalogKind::Namespaces, "list_namespaces"),
        (CatalogKind::Tags, "list_tags"),
    ] {
        let mcp =
            crate::mcp::tools::dispatch_v2(&bridge, tool, json!({"contractVersion":2,"limit":0}))
                .await;
        assert_eq!(mcp.is_error, Some(true));
        let value = serde_json::to_value(mcp).unwrap();
        let mcp: serde_json::Value =
            serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(value["structuredContent"], mcp);
        let response =
            crate::api::catalog_v2(&rest, "default", &[("limit".into(), "0".into())], kind).await;
        assert_eq!(response.status(), 400);
        let http: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 8192)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(http, mcp);
        assert_eq!(http["error"]["commitState"], "notCommitted");
        assert!(http.get("data").is_none());
        assert!(http.get("receipt").is_none());
    }
    assert_eq!(canonical(&f), before);
}

#[tokio::test]
async fn prepared_pages_remote_list_matches_actual_owner_before() {
    let f = Fixture::new();
    f.insert_memory_with(MemorySpec {
        summary: "canonical page control",
        tags: &["entity/alice", "selected"],
        ..MemorySpec::new(NamespaceId::new(1))
    })
    .await;
    let owned = adapter(&f);
    let request = ListPageRequest::from_mcp(&json!({
        "contractVersion":2, "namespace":"default", "limit":1,
        "tags":["selected"]
    }))
    .unwrap();
    let before = canonical(&f);
    let local = owned.list_canonical_page(request.clone()).await.unwrap();
    assert_eq!(local.data.total.get(), 1);
    assert_eq!(local.data.memories[0].summary, "canonical page control");
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) =
        native_lifecycle_test_remote(owned, &socket.path().join("pages.sock")).await;
    remote.test_client().ping().await.unwrap();
    let received = remote.list_canonical_page(request).await;
    eprintln!("actual owned list succeeded; Remote result: {received:?}");
    drop(remote);
    task.await.unwrap();
    assert_eq!(canonical(&f), before);
    let received = received.expect("actual Remote canonical list must match owner");
    assert_eq!(
        serde_json::to_vec(&received.into_envelope()).unwrap(),
        serde_json::to_vec(&local.into_envelope()).unwrap()
    );
}

#[tokio::test]
async fn prepared_pages_remote_catalogs_match_actual_owner_before() {
    let f = Fixture::new();
    f.create_namespace("other");
    f.insert_memory_with(MemorySpec {
        tags: &["entity/alice", "selected"],
        ..MemorySpec::new(NamespaceId::new(1))
    })
    .await;
    let owned = adapter(&f);
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) =
        native_lifecycle_test_remote(owned.clone(), &socket.path().join("pages.sock")).await;
    remote.test_client().ping().await.unwrap();
    let before = canonical(&f);
    let mut results = Vec::new();
    for kind in [CatalogKind::Namespaces, CatalogKind::Tags] {
        let mut value = json!({"contractVersion":2,"limit":1});
        if kind == CatalogKind::Tags {
            value["namespace"] = json!("default");
        }
        let request = CatalogPageRequest::from_mcp(&value, kind).unwrap();
        let local = owned.list_catalog_page(request.clone()).await.unwrap();
        let received = remote.list_catalog_page(request).await;
        eprintln!("actual owned {kind:?} succeeded; Remote result: {received:?}");
        results.push((local, received));
    }
    drop(remote);
    task.await.unwrap();
    assert_eq!(canonical(&f), before);
    for (local, received) in results {
        let received = received.expect("actual Remote canonical catalog must match owner");
        assert_eq!(
            serde_json::to_vec(&received.into_envelope()).unwrap(),
            serde_json::to_vec(&local.into_envelope()).unwrap()
        );
    }
}
