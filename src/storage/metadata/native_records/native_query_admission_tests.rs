//! Actual owned native read view/profile before provider; no authored rows.
use super::*;
use crate::embedding::{EmbeddingError, EmbeddingProvider};
use crate::search::{QueryEngine, SearchQuery};

struct Provider {
    profile: crate::model::schema::EmbeddingProfile,
    calls: Arc<AtomicUsize>,
}
#[async_trait::async_trait]
impl EmbeddingProvider for Provider {
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        Some(self.profile.clone())
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        &self.profile.model_id
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![1., 0., 0., 0.])
    }
}
fn query_engine(f: &Fixture, provider: Provider, admit: bool) -> QueryEngine {
    let rif = Arc::new(crate::rif::RifEngine::new(crate::rif::RifConfig {
        enabled: false,
        ..Default::default()
    }));
    let engine = QueryEngine::new(
        Arc::new(StorageNamespaceResolver::new(f.storage.clone())),
        Arc::new(SingleEmbeddingRegistry::new(Arc::new(provider))),
        Arc::new(SharedVectorIndexRegistry::new(f.vector_index.clone())),
        Arc::new(SharedFtsIndexRegistry::new(f.fts_index.clone())),
        Arc::new(SharedEntityIndexReader::new(f.entity_index.clone())),
        Arc::new(CacheManagerAdapter::new(f.cache.clone())),
        Arc::new(StorageMetadataAdapter::new(f.storage.clone())),
        Arc::new(SharedGraphReader::new(f.graph.clone())),
        Arc::new(RifProcessorAdapter::new(rif.clone(), f.graph.clone())),
        Arc::new(StorageAccessRecorder::new(
            f.storage.clone(),
            f.cache.clone(),
        )),
    )
    .with_lifecycle_inputs(
        Arc::new(crate::time::ManualClock::new(2000)),
        Default::default(),
        1.0,
    )
    .with_coherent_owner(completion(f), rif);
    if admit {
        engine.with_native_query_admission()
    } else {
        engine
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_query_admission_matches_canonical_profile_before_provider_and_preserves_legacy_gate()
 {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let engine = RedbStorageEngine::open_native(&d.path).unwrap();
    let f = Fixture::from_open_engine(root, engine);
    completion(&f).rebuild_all().await.unwrap();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let request = || SearchQuery {
        namespace: "native".into(),
        text: Some("original parent".into()),
        record_access: false,
        limit: 1,
        ..Default::default()
    };
    for (admit, profile) in [
        (false, d.profile.clone()),
        (true, {
            let mut p = d.profile.clone();
            p.model_id.push_str("-wrong");
            p
        }),
        (true, {
            let mut p = d.profile.clone();
            p.model_revision = Some("synthetic-revision-mismatch".into());
            p
        }),
        (true, {
            let mut p = d.profile.clone();
            p.query_preprocessing.prefix.push_str("wrong:");
            p
        }),
    ] {
        let query = query_engine(
            &f,
            Provider {
                profile,
                calls: calls.clone(),
            },
            admit,
        );
        assert!(
            query
                .prepare_coherent_payload(request(), false, false, None)
                .await
                .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
    let query = query_engine(
        &f,
        Provider {
            profile: d.profile.clone(),
            calls: calls.clone(),
        },
        true,
    );
    let response = query
        .prepare_coherent_payload(request(), false, false, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(response.response.response.results.len(), 1);
    assert_eq!(response.response.response.results[0].memory_id, d.parent);
    assert_eq!(response.read_generation, before.generation_ref());
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    let storage = f.storage.read().unwrap();
    let view = storage.meta_store().capture_read_view().unwrap();
    let mut ns = storage.get_namespace(d.ns).unwrap().unwrap();
    assert!(view.check_native_query_profile(&ns, None).is_err());
    ns.name.push_str("-foreign");
    assert!(
        view.check_native_query_profile(&ns, Some(&d.profile))
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_query_admission_flag_cannot_grant_legacy_owner_authority() {
    let root = tempfile::tempdir().unwrap();
    let mut engine = RedbStorageEngine::open(root.path()).unwrap();
    engine
        .create_namespace(&NamespaceConfig::default_namespace(0))
        .unwrap();
    let f = Fixture::from_open_engine(root, engine);
    completion(&f).rebuild_all().await.unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let query = query_engine(
        &f,
        Provider {
            profile: crate::embedding::runtime::tests::profile(),
            calls: calls.clone(),
        },
        true,
    );
    assert!(
        query
            .prepare_coherent_payload(
                SearchQuery {
                    namespace: "default".into(),
                    text: Some("read".into()),
                    record_access: false,
                    ..Default::default()
                },
                false,
                false,
                None
            )
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
