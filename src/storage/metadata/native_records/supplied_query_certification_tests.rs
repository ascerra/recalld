//! Actual migrated native stores and prepared transports; public2 remains closed.
use super::*;
use crate::search::{QueryEngine, RetrievalSemantics, VersionMode};

fn engine(f: &Fixture) -> Arc<QueryEngine> {
    let provider = Arc::new(NoProvider);
    let rif = Arc::new(crate::rif::RifEngine::new(crate::rif::RifConfig {
        enabled: false,
        ..Default::default()
    }));
    Arc::new(
        QueryEngine::new(
            Arc::new(StorageNamespaceResolver::new(f.storage.clone())),
            Arc::new(SingleEmbeddingRegistry::new(provider)),
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
        .with_coherent_owner(completion(f), rif),
        // Deliberately leave native_query_admission false. Format authority,
        // not an optional generated-provider flag, must protect supplied bytes.
    )
}

fn args(d: &Data) -> Value {
    json!({
        "contractVersion":2, "namespace":"native", "recordAccess":false,
        "queryMode":"embedding_only", "embedding":[1.,0.,0.,0.],
        "embeddingProfileId":d.profile.profile_id().unwrap(), "limit":1
    })
}

async fn uncertified_boundary(surface: u8) {
    let root = tempfile::tempdir().unwrap();
    // Real migration + profile/policy attestation. This does NOT certify vectors.
    let d = fixture(root.path());
    let f = Fixture::from_open_engine(root, RedbStorageEngine::open_native(&d.path).unwrap());
    completion(&f).rebuild_all().await.unwrap();
    let at = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    {
        let storage = f.storage.read().unwrap();
        let view = storage.meta_store().capture_read_view().unwrap();
        assert!(view.is_native_owned());
        assert_eq!(view.at, at);
        let ns = storage.get_namespace(d.ns).unwrap().unwrap();
        assert!(view.check_native_vector_namespace(&ns).is_err());
    }
    let query = engine(&f);
    let before = bytes(&d.path);
    let arguments = args(&d);
    let rejected = match surface {
        0 => {
            let request =
                crate::serialization::query::QueryRequest::from_mcp(&arguments, "wrong-default")
                    .unwrap();
            let result = query
                .prepare_coherent_payload_with_semantics(
                    request.query,
                    RetrievalSemantics::V2(VersionMode::Current),
                    false,
                    false,
                    None,
                )
                .await;
            match &result {
                Ok(Some(response)) => eprintln!(
                    "supplied certification INTERNAL success G={:?} results={}",
                    response.read_generation,
                    response.response.response.results.len(),
                ),
                Ok(None) => eprintln!("supplied certification INTERNAL no owned response"),
                Err(error) => eprintln!("supplied certification INTERNAL error={error}"),
            }
            matches!(result, Err(crate::search::SearchError::Unavailable(_)))
        }
        1 => {
            let adapter = McpSearchAdapter::new(
                query,
                Arc::new(NoProvider),
                f.storage.clone(),
                f.graph.clone(),
                chrono_tz::UTC,
            );
            let result =
                crate::mcp::tools::prepared_query(&adapter, "wrong-default", &arguments).await;
            eprintln!("supplied certification MCP result={result:?}");
            result.is_error == Some(true)
        }
        2 => {
            let adapter = SearchPipelineAdapter::new(
                query,
                Arc::new(NoProvider),
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
            );
            let mut body = arguments;
            body.as_object_mut().unwrap().remove("contractVersion");
            let response = crate::api::search_v2(&adapter, "wrong-default", &body).await;
            let status = response.status();
            let payload = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
                .await
                .unwrap();
            eprintln!(
                "supplied certification REST status={status} body={}",
                String::from_utf8_lossy(&payload)
            );
            status == axum::http::StatusCode::SERVICE_UNAVAILABLE
        }
        _ => unreachable!(),
    };
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref(),
        at
    );
    assert_eq!(
        bytes(&d.path),
        before,
        "false query changed canonical store bytes"
    );
    assert!(
        rejected,
        "uncertified native supplied vector escaped surface {surface}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supplied_query_certification_internal_rejects_uncertified_native() {
    uncertified_boundary(0).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supplied_query_certification_mcp_rejects_uncertified_native() {
    uncertified_boundary(1).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supplied_query_certification_rest_rejects_uncertified_native() {
    uncertified_boundary(2).await;
}
