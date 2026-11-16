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
