//! Real owning adapter and same-connection Remote page regressions.
use crate::{
    daemon::server::native_lifecycle_test_remote,
    mcp::{bridge::StorageEngine as _, bridge_adapters::McpStorageAdapter},
    model::NamespaceId,
    serialization::{catalog_page::{CatalogKind, CatalogPageRequest}, list::ListPageRequest},
    storage::StorageEngine as _,
    test_support::{Fixture, MemorySpec},
};
use serde_json::json;
use std::sync::Arc;

fn adapter(f: &Fixture) -> Arc<McpStorageAdapter> {
    Arc::new(McpStorageAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        Arc::new(crate::embedding::PassthroughProvider::new(crate::test_support::DIM)),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
        Arc::new(Default::default()),
        chrono_tz::UTC,
    ))
}

fn canonical(f: &Fixture) -> Vec<u8> {
    let storage = f.storage.read().unwrap();
    let records = storage.scan_all().unwrap().into_iter()
        .map(|(id, record)| (id.to_string(), record.to_bytes()))
        .collect::<Vec<_>>();
    serde_json::to_vec(&(storage.control_state().unwrap(), records)).unwrap()
}

#[tokio::test]
async fn prepared_pages_remote_list_matches_actual_owner_before() {
    let f = Fixture::new();
    f.insert_memory_with(MemorySpec {
        summary: "canonical page control",
        tags: &["entity/alice", "selected"],
        ..MemorySpec::new(NamespaceId::new(1))
    }).await;
    let owned = adapter(&f);
    let request = ListPageRequest::from_mcp(&json!({
        "contractVersion":2, "namespace":"default", "limit":1,
        "tags":["selected"]
    })).unwrap();
    let before = canonical(&f);
    let local = owned.list_canonical_page(request.clone()).await.unwrap();
    assert_eq!(local.data.total.get(), 1);
    assert_eq!(local.data.memories[0].summary, "canonical page control");
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = native_lifecycle_test_remote(owned, &socket.path().join("pages.sock")).await;
    remote.test_client().ping().await.unwrap();
    let received = remote.list_canonical_page(request).await;
    eprintln!("actual owned list succeeded; Remote result: {received:?}");
    drop(remote);
    task.await.unwrap();
    assert_eq!(canonical(&f), before);
    let received = received.expect("actual Remote canonical list must match owner");
    assert_eq!(serde_json::to_vec(&received.into_envelope()).unwrap(),
        serde_json::to_vec(&local.into_envelope()).unwrap());
}

#[tokio::test]
async fn prepared_pages_remote_catalogs_match_actual_owner_before() {
    let f = Fixture::new();
    f.create_namespace("other");
    f.insert_memory_with(MemorySpec {
        tags: &["entity/alice", "selected"],
        ..MemorySpec::new(NamespaceId::new(1))
    }).await;
    let owned = adapter(&f);
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = native_lifecycle_test_remote(owned.clone(), &socket.path().join("pages.sock")).await;
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
        assert_eq!(serde_json::to_vec(&received.into_envelope()).unwrap(),
            serde_json::to_vec(&local.into_envelope()).unwrap());
    }
}
