// Real owned fixture consumers; included in read_view::tests.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_actual_prepared_mcp_rest_current_history_and_false_bytes() {
    use crate::storage::StorageEngine as _;
    let f = Fixture::new();
    let owner = owner(&f);
    let old = payload_record(&owner, 101, [1., 0., 0., 0.], None, None, true).await;
    let current = payload_record(&owner, 102, [1., 0., 0., 0.], None, Some(old), true).await;
    let engine = Arc::new(payload_engine(&f, &owner));
    let provider = Arc::new(crate::embedding::PassthroughProvider::new(4));
    let mcp = crate::mcp::bridge_adapters::McpSearchAdapter::new(
        engine.clone(),
        provider.clone(),
        f.storage.clone(),
        f.graph.clone(),
        chrono_tz::UTC,
    );
    let http = crate::api::adapters::SearchPipelineAdapter::new(
        engine,
        provider,
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
    );
    f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f);
    let args = serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"recordAccess":false});
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        crate::mcp::tools::prepared_query(&mcp, "default", &args),
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, Some(false), "{result:?}");
    let value = serde_json::Value::Object(result.structured_content.unwrap());
    let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text).unwrap(),
        value
    );
    assert_eq!(value["scope"]["namespaceId"], 1);
    assert_eq!(value["data"]["results"].as_array().unwrap().len(), 1);
    assert_eq!(value["data"]["results"][0]["memoryId"], current.to_string());
    assert_eq!(value["data"]["results"][0]["fullText"], "full payload 102");
    assert!(value.get("receipt").is_none());
    assert!(value["data"].get("explanation").is_none());
    assert!(value["data"]["coverage"]["channels"]["vector"]["examined"].is_null());
    assert!(value["data"]["coverage"]["channels"]["entity"]["candidateLimited"].is_null());
    assert_eq!(
        value["data"]["coverage"]["supplemental"]["requested"],
        false
    );
    assert!(value["readGeneration"]["generation"].is_string());
    let mut body = args.clone();
    body.as_object_mut().unwrap().remove("contractVersion");
    let response = crate::api::search_v2(&http, "default", &body).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let raw = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&raw).unwrap(),
        value
    );
    body["contractVersion"] = serde_json::json!(2);
    body["versionMode"] = serde_json::json!("history");
    let history = crate::mcp::tools::prepared_query(&mcp, "default", &body).await;
    let history = serde_json::Value::Object(history.structured_content.unwrap());
    let ids: Vec<_> = history["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["memoryId"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&old.to_string().as_str()));
    assert_eq!(bytes(&f), before);
    body["namespace"] = serde_json::json!("missing");
    let missing = crate::mcp::tools::prepared_query(&mcp, "default", &body).await;
    assert_eq!(missing.is_error, Some(true));
    assert_eq!(
        serde_json::Value::Object(missing.structured_content.unwrap())["error"]["code"],
        "NAMESPACE_NOT_FOUND"
    );
    assert_eq!(bytes(&f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_actual_original2_observer_and_current_barrier() {
    use crate::search::adapters::StorageAccessRecorder;
    let f = Fixture::new();
    let owner = owner(&f);
    let id = payload_record(&owner, 103, [1., 0., 0., 0.], None, None, true).await;
    let mut engine = payload_engine(&f, &owner);
    engine.access_recorder = Arc::new(
        StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
            .with_clock(Arc::new(crate::time::ManualClock::new(999)))
            .with_completion(owner.clone()),
    );
    let request = crate::serialization::query::QueryRequest::from_mcp(
        &serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"]}),
        "default",
    )
    .unwrap();
    let before = f.record(id).unwrap().access_history.len();
    let reply = engine.query_contract(request).await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    assert_eq!(reply.observation_receipts.len(), 1);
    let receipt = reply.envelope.receipt.as_ref().unwrap();
    assert!(matches!(
        receipt.outcome,
        crate::storage::control::MutationOutcome::ExposuresRecorded { .. }
    ));
    assert_eq!(f.record(id).unwrap().access_history.len(), before + 1);
    assert_eq!(
        f.record(id)
            .unwrap()
            .access_history
            .last()
            .unwrap()
            .timestamp,
        123
    );
    let ready = reply.envelope.readiness.as_ref().unwrap();
    assert!(ready.ready);
    assert_eq!(ready.pending_work, 0);
    assert_eq!(ready.derived.len(), 4);
    assert!(
        ready
            .derived
            .iter()
            .all(|d| d.ready && d.applied_generation == ready.committed_generation)
    );
    assert!(reply.envelope.read_generation.unwrap().generation < receipt.committed_generation);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_actual_whole_response_budget_before_false_and_true_observation() {
    use crate::search::adapters::StorageAccessRecorder;
    use crate::storage::StorageEngine as _;
    let f = Fixture::new();
    let owner = owner(&f);
    for _ in 0..4 {
        owner
            .insert(
                MutationContext::unkeyed(NamespaceId::new(1)),
                InsertMemory {
                    id: None,
                    summary: "large query payload".into(),
                    full_text: Some("x".repeat(crate::model::constants::FULL_TEXT_MAX_BYTES)),
                    tags: vec![crate::model::Tag::new("allow").unwrap()],
                    initial_stability: None,
                    created_at: Some(100),
                    embedding: Some(vec![1., 0., 0., 0.]),
                    parent_id: None,
                    supersedes: None,
                },
                Vec::new().into(),
                100,
            )
            .await
            .unwrap();
    }
    let mut engine = payload_engine(&f, &owner);
    engine.access_recorder = Arc::new(
        StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
            .with_completion(owner.clone()),
    );
    f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f);
    for record_access in [false, true] {
        let request=crate::serialization::query::QueryRequest::from_mcp(&serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"recordAccess":record_access,"limit":4}),"default").unwrap();
        let reply = engine.query_contract(request).await;
        assert_eq!(reply.http_status, 400, "{reply:?}");
        assert!(reply.envelope.data.is_none());
        assert!(reply.envelope.receipt.is_none());
        let error = reply.envelope.error.as_ref().unwrap();
        assert_eq!(error.code, "RESPONSE_TOO_LARGE");
        assert_eq!(
            error.commit_state,
            crate::serialization::contract::CommitState::NotCommitted
        );
        assert_eq!(bytes(&f), before);
    }
    let request=crate::serialization::query::QueryRequest::from_mcp(&serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"limit":3}),"default").unwrap();
    let reply = engine.query_contract(request).await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    assert_eq!(reply.observation_receipts.len(), 1);
    assert_eq!(
        reply.envelope.data.as_ref().unwrap()["results"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let framed =
        serde_json::to_vec(&serde_json::json!({"jsonrpc":"2.0","id":1,"result":reply.mcp()}))
            .unwrap();
    assert!(framed.len() < crate::daemon::protocol::MAX_MESSAGE_SIZE);
    assert!(reply.envelope.readiness.as_ref().unwrap().ready);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_actual_final_barrier_failure_keeps_full_receipt_and_identity() {
    use crate::search::adapters::StorageAccessRecorder;
    let f = Fixture::new();
    let owner = owner(&f);
    let id = payload_record(&owner, 104, [1., 0., 0., 0.], None, None, true).await;
    let mut engine = payload_engine(&f, &owner);
    engine.access_recorder = Arc::new(
        StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
            .with_completion(owner.clone()),
    );
    let mut request = crate::serialization::query::QueryRequest::from_mcp(
        &serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"]}),
        "default",
    )
    .unwrap();
    request.before_barrier = Some(|owner| {
        owner
            .storage
            .read()
            .unwrap()
            .meta_store()
            .invalidate_derived_view(crate::storage::publication::DerivedView::Fts)
    });
    let reply = engine.query_contract(request).await;
    assert_eq!(reply.http_status, 503, "{reply:?}");
    let receipt = reply.envelope.receipt.as_ref().unwrap();
    assert_eq!(reply.observation_receipts, vec![receipt.clone()]);
    assert_eq!(
        reply.envelope.error.as_ref().unwrap().commit_state,
        crate::serialization::contract::CommitState::Committed
    );
    assert!(reply.envelope.read_generation.is_some());
    assert!(reply.envelope.scope.is_some());
    assert!(reply.envelope.readiness.is_none());
    assert!(
        reply
            .envelope
            .warnings
            .iter()
            .any(|w| w.starts_with("READINESS_UNKNOWN"))
    );
    assert_eq!(
        f.record(id)
            .unwrap()
            .access_history
            .last()
            .unwrap()
            .timestamp,
        123
    );
    let original = receipt.clone();
    let at = owner
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    owner.complete().await.unwrap();
    assert_eq!(
        owner
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        at
    );
    assert_eq!(original, *reply.envelope.receipt.as_ref().unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_actual_native_bound_supplied_priority_unknown_and_metadata_only() {
    let f = similar_fixture().await; // Real migration/attestation, not authored schema rows.
    let profile = crate::embedding::runtime::tests::profile()
        .profile_id()
        .unwrap();
    let before = bytes(&f.f);
    let args = serde_json::json!({"contractVersion":2,"namespace":"default","recordAccess":false,
        "query":"provider must not run","embedding":[1.,0.,0.,0.],"embeddingProfileId":profile,
        "queryMode":"embedding_only","tags":["allow"]});
    let request =
        crate::serialization::query::QueryRequest::from_mcp(&args, "wrongDefault").unwrap();
    let reply = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        f.engine.query_contract(request),
    )
    .await
    .unwrap();
    assert_eq!(reply.http_status, 200, "{reply:?}");
    assert_eq!(
        reply.envelope.data.as_ref().unwrap()["results"][0]["memoryId"],
        f.live.to_string()
    );
    assert_eq!(
        reply.envelope.data.as_ref().unwrap()["results"][0]["labels"]["provenance"],
        "legacy_retained_only"
    );
    assert_eq!(
        reply.envelope.data.as_ref().unwrap()["results"][0]["labels"]["entities"],
        serde_json::json!([])
    );
    assert_eq!(bytes(&f.f), before);
    let mut wrong = args.clone();
    wrong["namespace"] = serde_json::json!("mismatch");
    let request = crate::serialization::query::QueryRequest::from_mcp(&wrong, "default").unwrap();
    assert_eq!(f.engine.query_contract(request).await.http_status, 409);
    wrong["namespace"] = serde_json::json!("unknown");
    let request = crate::serialization::query::QueryRequest::from_mcp(&wrong, "default").unwrap();
    assert_eq!(f.engine.query_contract(request).await.http_status, 409);
    assert_eq!(bytes(&f.f), before);
    let request=crate::serialization::query::QueryRequest::from_mcp(&serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"recordAccess":false}),"unknown").unwrap();
    let reply = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        f.engine.query_contract(request),
    )
    .await
    .unwrap();
    assert_eq!(reply.http_status, 200, "{reply:?}");
    assert_eq!(bytes(&f.f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_actual_similar_scope_current_history_empty_identity_and_parity() {
    let f = similar_fixture().await;
    let before = bytes(&f.f);
    let source = f.source;
    let current = f.terminal;
    let old = f.old;
    let ns_other = f.other;
    let unknown = f.unknown;
    let mismatch = f.mismatch;
    let provider = Arc::new(crate::embedding::PassthroughProvider::new(4));
    let engine = Arc::new(f.engine);
    let mcp = crate::mcp::bridge_adapters::McpSearchAdapter::new(
        engine.clone(),
        provider.clone(),
        f.f.storage.clone(),
        f.f.graph.clone(),
        chrono_tz::UTC,
    );
    let http = crate::api::adapters::SearchPipelineAdapter::new(
        engine,
        provider,
        f.f.vector_index.clone(),
        f.f.fts_index.clone(),
        f.f.entity_index.clone(),
    );
    let mut args = serde_json::json!({"contractVersion":2,"id":source.to_string(),"recordAccess":false,"namespaceScope":"allCompatible"});
    let result = crate::mcp::tools::prepared_similar(&mcp, &args).await;
    assert_eq!(result.is_error, Some(false), "{result:?}");
    let value = serde_json::Value::Object(result.structured_content.unwrap());
    let ids: Vec<_> = value["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["memoryId"].as_str().unwrap().to_owned())
        .collect();
    assert!(ids.contains(&current.to_string()));
    assert!(!ids.contains(&old.to_string()));
    assert!(!ids.contains(&source.to_string()));
    assert_eq!(value["data"]["sourceNamespace"]["namespace"], "default");
    assert!(
        value["data"]["coverage"]["searchedNamespaceIds"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(ns_other))
    );
    let excluded = value["data"]["coverage"]["excludedNamespaces"]
        .as_array()
        .unwrap();
    assert!(
        excluded.contains(&serde_json::json!({"namespaceId":unknown,"reason":"unknownProfile"}))
    );
    assert!(
        excluded.contains(&serde_json::json!({"namespaceId":mismatch,"reason":"differentProfile"}))
    );
    let mut body = args.clone();
    body.as_object_mut().unwrap().remove("contractVersion");
    body.as_object_mut().unwrap().remove("id");
    let response = crate::api::similar_v2(&http, source, &body).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let raw = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&raw).unwrap(),
        value
    );
    args["versionMode"] = serde_json::json!("history");
    let result = crate::mcp::tools::prepared_similar(&mcp, &args).await;
    let value = serde_json::Value::Object(result.structured_content.unwrap());
    assert!(
        value["data"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["memoryId"] == old.to_string())
    );
    args["tags"] = serde_json::json!(["missing"]);
    let result = crate::mcp::tools::prepared_similar(&mcp, &args).await;
    let value = serde_json::Value::Object(result.structured_content.unwrap());
    assert!(value["data"]["results"].as_array().unwrap().is_empty());
    assert_eq!(value["data"]["sourceNamespace"]["namespaceId"], 1);
    assert!(value["readGeneration"]["generation"].is_string());
    assert_eq!(bytes(&f.f), before);
    args["namespace"] = serde_json::json!("other");
    let result = crate::mcp::tools::prepared_similar(&mcp, &args).await;
    let value = serde_json::Value::Object(result.structured_content.unwrap());
    assert_eq!(value["error"]["code"], "NAMESPACE_MISMATCH");
    assert_eq!(bytes(&f.f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_actual_similar_original2_true_preserves_all_namespace_receipts() {
    let f = similar_fixture().await;
    let request=crate::serialization::query::SimilarRequest::from_mcp(&serde_json::json!({"contractVersion":2,"id":f.source.to_string(),"namespaceScope":"allCompatible","tags":["allow"]})).unwrap();
    let reply = f.engine.similar_contract(request).await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    assert_eq!(reply.observation_receipts.len(), 2);
    assert!(reply.envelope.receipt.is_none());
    assert!(reply.envelope.readiness.as_ref().unwrap().ready);
    assert_eq!(reply.envelope.readiness.as_ref().unwrap().derived.len(), 4);
    for id in [f.live, f.foreign] {
        assert_eq!(
            f.f.record(id)
                .unwrap()
                .access_history
                .last()
                .unwrap()
                .timestamp,
            10_000
        );
    }
    assert!(
        reply
            .observation_receipts
            .iter()
            .all(|r| r.committed_generation > reply.envelope.read_generation.unwrap().generation)
    );
}
