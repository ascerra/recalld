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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supplied_query_certification_real_remedy_all_boundaries_and_profile_controls() {
    let root = tempfile::tempdir().unwrap();
    // This invokes real reembed_fresh using retained documents, not policy adoption.
    let d = certified_fixture(root.path());
    let f = Fixture::from_open_engine(root, RedbStorageEngine::open_native(&d.path).unwrap());
    completion(&f).rebuild_all().await.unwrap();
    let query = engine(&f);
    let before = bytes(&d.path);
    let arguments = args(&d);
    let request =
        crate::serialization::query::QueryRequest::from_mcp(&arguments, "wrong-default").unwrap();
    let result = query.query_contract(request).await;
    assert_eq!(result.http_status, 200, "{result:?}");
    assert_eq!(
        result.envelope.data.as_ref().unwrap()["results"][0]["memoryId"],
        json!(d.parent)
    );
    let mcp = McpSearchAdapter::new(
        query.clone(),
        Arc::new(NoProvider),
        f.storage.clone(),
        f.graph.clone(),
        chrono_tz::UTC,
    );
    let result = crate::mcp::tools::prepared_query(&mcp, "wrong-default", &arguments).await;
    assert_eq!(result.is_error, Some(false), "{result:?}");
    let expected = Value::Object(result.structured_content.unwrap());
    let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
    assert_eq!(serde_json::from_str::<Value>(text).unwrap(), expected);
    let http = SearchPipelineAdapter::new(
        query.clone(),
        Arc::new(NoProvider),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
    );
    let mut body = arguments.clone();
    body.as_object_mut().unwrap().remove("contractVersion");
    let result = crate::api::search_v2(&http, "wrong-default", &body).await;
    assert_eq!(result.status(), axum::http::StatusCode::OK);
    let raw = axum::body::to_bytes(result.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&raw).unwrap(), expected);
    for (field, value, status) in [
        ("embedding", json!([1., 0., 0.]), 422),
        ("embedding", json!([0., 0., 0., 0.]), 422),
        (
            "embeddingProfileId",
            json!(format!("ep1:{}", "a".repeat(64))),
            409,
        ),
    ] {
        let mut bad = body.clone();
        bad[field] = value;
        assert_eq!(
            crate::api::search_v2(&http, "wrong-default", &bad)
                .await
                .status()
                .as_u16(),
            status
        );
    }
    assert_eq!(bytes(&d.path), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supplied_query_certification_metadata_and_fts_do_not_require_vector_certification() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let f = Fixture::from_open_engine(root, RedbStorageEngine::open_native(&d.path).unwrap());
    completion(&f).rebuild_all().await.unwrap();
    let before = bytes(&d.path);
    let query = engine(&f);
    let request = crate::serialization::query::QueryRequest::from_mcp(
        &json!({"contractVersion":2,"namespace":"native","queryMode":"metadata_only","phases":["full"],"recordAccess":false}),
        "wrong-default",
    ).unwrap();
    let result = query.query_contract(request).await;
    assert_eq!(result.http_status, 200, "{result:?}");
    assert_eq!(
        result.envelope.data.as_ref().unwrap()["results"][0]["memoryId"],
        json!(d.parent)
    );
    // Existing nonsemantic FTS reader, not a new public FTS-only query mode.
    let fts = SharedFtsIndexRegistry::new(f.fts_index.clone());
    let hits = crate::search::FtsIndexRegistry::search(&fts, d.ns, "original", 10).unwrap();
    assert_eq!(hits[0].memory_id, d.parent);
    assert_eq!(bytes(&d.path), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supplied_query_certification_scope_and_stale_view_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let f = Fixture::from_open_engine(root, RedbStorageEngine::open_native(&d.path).unwrap());
    completion(&f).rebuild_all().await.unwrap();
    let view = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .capture_read_view()
        .unwrap();
    let ns = f
        .storage
        .read()
        .unwrap()
        .get_namespace(d.ns)
        .unwrap()
        .unwrap();
    view.check_native_vector_namespace(&ns).unwrap();
    let mut wrong = ns.clone();
    wrong.name.push_str("-wrong");
    assert!(view.check_native_vector_namespace(&wrong).is_err());
    wrong = ns.clone();
    wrong.initial_stability += 1.;
    assert!(view.check_native_vector_namespace(&wrong).is_err());
    {
        let mut storage = f.storage.write().unwrap();
        let policy = crate::storage::native_lifecycle::NativePolicyInputs::default();
        let intent = storage
            .meta_store()
            .capture_native_tombstone(d.parent, Some(d.ns), &policy)
            .unwrap();
        storage
            .tombstone_native_record(
                &MutationContext::unkeyed(d.ns),
                &intent,
                &policy,
                &crate::time::ManualClock::new(3000),
                None,
            )
            .unwrap();
    }
    assert!(
        view.check_native_vector_namespace(&ns).is_err(),
        "old pinned G must not borrow new owner certification"
    );
}
