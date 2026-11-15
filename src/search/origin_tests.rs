use super::tests::{StubGraph, StubMetaStore, engine};
use super::*;
use crate::search::adapters::*;
use crate::storage::{
    StorageEngine as _,
    control::{self, ExposureScope},
};
use crate::test_support::Fixture;
use std::sync::atomic::{AtomicUsize, Ordering};

struct OldOnly {
    inner: StorageAccessRecorder,
    calls: AtomicUsize,
    at_calls: AtomicUsize,
}
#[async_trait::async_trait]
impl AccessRecorder for OldOnly {
    async fn record_access(
        &self,
        _: MemoryId,
        _: AccessKind,
    ) -> Result<Vec<control::WriteReceipt>> {
        panic!("wrong legacy dispatch")
    }
    async fn record_access_batch(
        &self,
        _: &[(MemoryId, AccessKind)],
        _: Option<&ExposureScope>,
    ) -> Result<Vec<control::WriteReceipt>> {
        panic!("wrong legacy dispatch")
    }
    async fn record_access_batch_with_work(
        &self,
        a: &[(MemoryId, AccessKind)],
        s: Option<&ExposureScope>,
        p: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<control::WriteReceipt>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.record_access_batch_with_work(a, s, p).await
    }
    async fn record_access_batch_at_with_work(
        &self,
        a: &[(MemoryId, AccessKind)],
        s: Option<&ExposureScope>,
        t: i64,
        p: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<Vec<control::WriteReceipt>> {
        self.at_calls.fetch_add(1, Ordering::SeqCst);
        self.inner
            .record_access_batch_at_with_work(a, s, t, p)
            .await
    }
}
fn real_engine(f: &Fixture, recorder: Arc<dyn AccessRecorder>) -> QueryEngine {
    let mut e = engine(StubGraph::default(), StubMetaStore::default());
    e.namespace_resolver = Arc::new(StorageNamespaceResolver::new(f.storage.clone()));
    e.meta_store = Arc::new(StorageMetadataAdapter::new(f.storage.clone()));
    e.access_recorder = recorder;
    e.rif_engine = Arc::new(RifProcessorAdapter::new(
        Arc::new(crate::rif::RifEngine::new(Default::default())),
        f.graph.clone(),
    ));
    e
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exposure_origin_old_only_dispatch_and_readonly_unused_observer() {
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let old = Arc::new(OldOnly {
        inner: StorageAccessRecorder::new(f.storage.clone(), f.cache.clone()),
        calls: AtomicUsize::new(0),
        at_calls: AtomicUsize::new(0),
    });
    let e = real_engine(&f, old.clone());
    let query:SearchQuery=serde_json::from_value(serde_json::json!({"namespace":"default","queryMode":"metadata_only","graphDepth":0,"filter":{"minStrength":0.0},"recordAccess":false})).unwrap();
    let before = f.record(id).unwrap().encode().unwrap();
    // The unused observer must not validate even an unsupported origin.
    for origin in [1, 2, 3] {
        e.search_checked_with_origin_work(
            query.clone(),
            RetrievalSemantics::V1,
            false,
            None,
            origin,
        )
        .await
        .unwrap();
        e.get_memory_with_origin_work(id, false, None, origin)
            .await
            .unwrap();
    }
    let empty_scope = ExposureScope {
        namespace: "default".into(),
        source: Some(id),
    };
    assert!(
        e.record_exposures_with_origin_work(&[], empty_scope, None, 3)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(old.calls.load(Ordering::SeqCst), 0);
    assert_eq!(old.at_calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.record(id).unwrap().encode().unwrap(), before);
    let mut query = query;
    query.record_access = true;
    assert!(
        e.search_checked_with_origin_work(query.clone(), RetrievalSemantics::V1, false, None, 2)
            .await
            .is_err()
    );
    assert_eq!(f.record(id).unwrap().encode().unwrap(), before);
    assert_eq!(old.calls.load(Ordering::SeqCst), 0);
    e.search(query).await.unwrap();
    assert_eq!(old.calls.load(Ordering::SeqCst), 1);
    assert_eq!(old.at_calls.load(Ordering::SeqCst), 0);
    let scope = ExposureScope {
        namespace: "default".into(),
        source: Some(id),
    };
    e.record_exposures_at_with_work(
        &[(id, AccessKind::DirectRetrieval)],
        scope,
        1_900_000_000_000,
        None,
    )
    .await
    .unwrap();
    assert_eq!(old.at_calls.load(Ordering::SeqCst), 1);
    assert_eq!(old.calls.load(Ordering::SeqCst), 1);
}
