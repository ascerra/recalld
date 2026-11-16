// Real owned fixture consumers; included in read_view::tests.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_remote_native_query_similar_exact_and_known_publication_failure() {
    use crate::mcp::bridge::SearchPipeline as _;
    use crate::daemon::server::prepared_query_tests::PreparedDaemon;
    let f=similar_fixture().await;
    let engine=Arc::new(f.engine);
    let provider=Arc::new(crate::embedding::PassthroughProvider::new(4));
    let adapter=Arc::new(crate::mcp::bridge_adapters::McpSearchAdapter::new(engine.clone(),provider,
        f.f.storage.clone(),f.f.graph.clone(),chrono_tz::UTC));
    let daemon=PreparedDaemon::start(adapter.clone()).await;
    let remote=crate::daemon::bridge_adapters::RemoteSearchAdapter::new(daemon.client.clone());
    let before=bytes(&f.f);
    let assert_wire = |name: &str, local: &crate::serialization::query_response::QueryReply,
                       received: &crate::serialization::query_response::QueryReply| {
        // Reproduce the actual private emitter exactly once: typed Reply ->
        // compact Value -> DaemonResponse bytes -> the client's Value parse.
        // Direct to_value(local) takes a different f32/f64 representation path.
        let result=crate::daemon::prepared_query::compact_value(&crate::daemon::prepared_query::Reply {
            wire_version:1,http_status:local.http_status,reply:local,
        }).unwrap();
        let frame=serde_json::to_vec(&crate::daemon::DaemonResponse::success(u64::MAX,result)).unwrap();
        assert!(crate::daemon::prepared_query::admitted_frame(&frame));
        let expected:serde_json::Value=serde_json::from_slice(&frame).unwrap();
        if let Some(root)=std::env::var_os("RECALLD_QUERY_SCHEMA_FIXTURES") {
            let root=std::path::Path::new(&root).parent().unwrap().join("remote-wire");
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join(format!("{name}-local-typed.json")),serde_json::to_vec(local).unwrap()).unwrap();
            std::fs::write(root.join(format!("{name}-expected-private-frame.json")),frame).unwrap();
            std::fs::write(root.join(format!("{name}-received-projected.json")),serde_json::to_vec(received).unwrap()).unwrap();
        }
        assert_eq!(received.http_status,local.http_status,"{name}");
        assert_eq!(serde_json::to_value(received).unwrap(),expected["result"]["reply"],"{name}");
        // Independent canonical f32 bits, not a numeric tolerance. The response
        // already owns all payload fields; this check performs no write.
        let rows=received.envelope.data.as_ref().unwrap()["results"].as_array().unwrap();
        let source=f.f.storage.read().unwrap();
        for row in rows {
            let id:MemoryId=serde_json::from_value(row["memoryId"].clone()).unwrap();
            let record=source.get_record(id).unwrap().unwrap();
            assert_eq!((row["stability"].as_f64().unwrap() as f32).to_bits(),record.stability.to_bits());
            if let Some(vector)=row.get("embedding") {
                let stored=source.get_vector(NamespaceId::new(record.namespace_id),record.vector_slot).unwrap().unwrap();
                let actual:Vec<_>=vector.as_array().unwrap().iter().map(|v|(v.as_f64().unwrap() as f32).to_bits()).collect();
                assert_eq!(actual,stored.iter().map(|v|v.to_bits()).collect::<Vec<_>>());
            }
        }

    };
    for mode in ["metadata_only","fts_only","embedding_only"] {
        let mut args=serde_json::json!({"contractVersion":2,"namespace":"DeFaUlT","queryMode":mode,
            "tags":["allow"],"recordAccess":false,"explain":true,"compact":false,"includeEmbeddings":true});
        if mode=="fts_only" {args["ftsQuery"]=serde_json::json!("similar");}
        if mode=="embedding_only" {
            args["query"]=serde_json::json!("provider must not run");
            args["embedding"]=serde_json::json!([1.,0.,0.,0.]);
            args["embeddingProfileId"]=serde_json::json!(crate::embedding::runtime::tests::profile().profile_id().unwrap());
        }
        let input=||crate::serialization::query::QueryRequest::from_mcp(&args,"wrong-default").unwrap();
        let local=adapter.query_contract(input()).await;
        assert_eq!(local.http_status,200,"{mode}: {local:?}");
        let received=remote.query_contract(input()).await;
        assert_wire(mode,&local,&received);
    }
    for scope in ["sameNamespace","allCompatible"] {
        for version in ["current","history"] {
            let args=serde_json::json!({"contractVersion":2,"id":f.source.to_string(),"namespace":"DeFaUlT",
                "recordAccess":false,"namespaceScope":scope,"versionMode":version,"explain":true,"includeEmbeddings":true});
            let input=||crate::serialization::query::SimilarRequest::from_mcp(&args).unwrap();
            let local=adapter.similar_contract(input()).await;
            assert_eq!(local.http_status,200,"{local:?}");
            let received=remote.similar_contract(input()).await;
            assert_wire(&format!("similar-{scope}-{version}"),&local,&received);
        }
    }
    let mut input=crate::serialization::scan::ExactScanRequest::from_http(&serde_json::json!({"pairBudget":2,"threshold":0.}),"DeFaUlT").unwrap();
    loop {
        let local=adapter.scan_duplicate_pairs(input.clone()).await.unwrap();
        let received=remote.scan_duplicate_pairs(input.clone()).await.unwrap();
        assert_eq!(serde_json::to_value(&received).unwrap(),serde_json::to_value(&local).unwrap());
        assert!(received.diagnostics.is_none(),"Remote must not fabricate capture metrics");
        input.cursor=received.data.next_cursor;
        if input.cursor.is_none() {break;}
    }
    assert_eq!(bytes(&f.f),before,"all false/scan calls preserve canonical bytes");
    // Fault occurs in actual certification publication after native exposure TX.
    f.f.storage.read().unwrap().meta_store().vector_publication_failure_nth.store(1,Ordering::SeqCst);
    let args=serde_json::json!({"contractVersion":2,"namespace":"default","queryMode":"metadata_only","tags":["allow"],"recordAccess":true});
    let failed=remote.query_contract(crate::serialization::query::QueryRequest::from_mcp(&args,"wrong-default").unwrap()).await;
    assert_eq!(failed.http_status,503,"{failed:?}");
    assert_eq!(failed.envelope.error.as_ref().unwrap().commit_state,crate::serialization::contract::CommitState::Committed);
    assert!(failed.envelope.scope.is_some() && failed.envelope.read_generation.is_some());
    assert!(failed.envelope.readiness.is_none() && failed.received_evidence.is_none());
    assert_eq!(failed.observation_receipts.len(),1);
    let receipt=failed.envelope.receipt.as_ref().unwrap();
    assert_eq!(failed.observation_receipts[0],*receipt);
    let crate::storage::control::MutationOutcome::LifecycleExposureRecorded {transition}=&receipt.outcome else {panic!("full native exposure receipt");};
    transition.validate(receipt,false).unwrap();
    assert_eq!(transition.records.len(),1);
    assert_eq!(transition.records[0].memory_id,f.live);
    // The injected publication fault makes the live owner unavailable. Inspect
    // the actual retained record/receipt only after ordinary native recovery.
    assert!(f.f.storage.read().unwrap().get_record(f.live).is_err());
    let receipt=receipt.clone();
    drop(remote);
    daemon.finish().await;
    drop(adapter);
    drop(engine);
    drop(f.owner);
    let root=f.f.close();
    let reopened=crate::storage::RedbStorageEngine::open_native(root.path()).unwrap();
    assert_eq!(reopened.get_record(f.live).unwrap().unwrap().access_history.last().unwrap().timestamp,10_000);
    assert_eq!(reopened.lookup_operation(receipt.store_id,crate::storage::control::MutationScope::Namespace(NamespaceId::new(1)),receipt.operation_id).unwrap(),Some(receipt));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_evidence_rejects_operator_rif_before_work_on_actual_transports() {
    struct NoProviderOrObservation;
    #[async_trait::async_trait]
    impl EmbeddingProviderRegistry for NoProviderOrObservation {
        async fn embed(&self, _: &str, _: &str) -> Result<Vec<f32>> {
            panic!("prepared non-Off query must reject before provider");
        }
    }
    #[async_trait::async_trait]
    impl AccessRecorder for NoProviderOrObservation {
        async fn record_access(
            &self,
            _: MemoryId,
            _: AccessKind,
        ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
            panic!("prepared non-Off query must reject before observation");
        }
        async fn record_access_batch(
            &self,
            _: &[(MemoryId, AccessKind)],
            _: Option<&crate::storage::control::ExposureScope>,
        ) -> Result<Vec<crate::storage::control::WriteReceipt>> {
            panic!("prepared non-Off query must reject before batch observation");
        }
    }
    for mode in [RifMode::Shadow, RifMode::Experimental] {
        for access in [false, true] {
            let f = Fixture::new();
            let owner = owner(&f);
            payload_record(&owner, 9201, [1., 0., 0., 0.], None, None, true).await;
            let mut engine = payload_engine(&f, &owner);
            engine.rif_mode = mode;
            engine.embedding_providers = Arc::new(NoProviderOrObservation);
            engine.access_recorder = Arc::new(NoProviderOrObservation);
            let engine = Arc::new(engine);
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
            let before = bytes(&f);
            for query_mode in ["metadata_only", "embedding_only"] {
                let mut args = serde_json::json!({"contractVersion":2,"queryMode":query_mode,"tags":["allow"],"recordAccess":access,"explain":true});
                if query_mode == "embedding_only" {
                    args["query"] = serde_json::json!("fixture");
                }
                let reply = crate::mcp::tools::prepared_query(&mcp, "default", &args).await;
                assert_eq!(
                    reply.is_error,
                    Some(true),
                    "configured {mode:?} must not reach prepared query: {reply:?}"
                );
                let value = serde_json::Value::Object(reply.structured_content.unwrap());
                assert_eq!(value["error"]["commitState"], "notCommitted");
                assert!(value.get("receipt").is_none());
                args.as_object_mut().unwrap().remove("contractVersion");
                let response = crate::api::search_v2(&http, "default", &args).await;
                assert_eq!(
                    response.status(),
                    axum::http::StatusCode::SERVICE_UNAVAILABLE
                );
                let raw = axum::body::to_bytes(
                    response.into_body(),
                    crate::daemon::protocol::MAX_MESSAGE_SIZE,
                )
                .await
                .unwrap();
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&raw).unwrap(),
                    value
                );
                assert_eq!(bytes(&f), before);
                let status = f
                    .storage
                    .read()
                    .unwrap()
                    .meta_store()
                    .work_tracker()
                    .status();
                assert_eq!(status.pending, 0);
                assert_eq!(status.unresolved, 0);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_evidence_similar_actual_mcp_rest_cosine_and_bounded_origins() {
    for dense in [false, true] {
        let f = if dense {
            similar_fixture_with_origins(0, Some((false, 0.0))).await
        } else {
            similar_fixture().await
        };
        let before = bytes(&f.f);
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
        let mut args = serde_json::json!({"contractVersion":2,"id":f.source.to_string(),"recordAccess":false,"limit":1,"minScore":0.5});
        if dense {
            args["tags"] = serde_json::json!(["explain"]);
        }
        let plain = crate::mcp::tools::prepared_similar(&mcp, &args).await;
        assert_eq!(plain.is_error, Some(false), "{plain:?}");
        let plain = serde_json::Value::Object(plain.structured_content.unwrap());
        args["explain"] = serde_json::json!(true);
        let reply = crate::mcp::tools::prepared_similar(&mcp, &args).await;
        assert_eq!(
            reply.is_error,
            Some(false),
            "actual cosine evidence must be consumed: {reply:?}"
        );
        let value = serde_json::Value::Object(reply.structured_content.clone().unwrap());
        let crate::mcp::protocol::ContentBlock::Text { text } = &reply.content[0];
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(text).unwrap(),
            value
        );
        let row = &value["data"]["results"][0];
        assert_eq!(row["memoryId"], f.terminal.to_string());
        let e = &row["explanation"];
        assert_eq!(e["algorithm"], "cosineMax");
        assert!(e.get("rankingProfile").is_none());
        assert_eq!(e["ownCosine"], 0.0);
        assert_eq!(e["ownAdmission"], "rejectedByMinScore");
        assert!(e["admittedOwn"].is_null());
        assert_eq!(e["finalSimilarity"], row["compositeScore"]);
        assert_eq!(e["inheritedFloor"], e["finalSimilarity"]);
        assert_eq!(
            e["origins"].as_array().unwrap().len(),
            if dense { 64 } else { 1 }
        );
        assert_eq!(e["omittedOrigins"], if dense { 6 } else { 0 });
        if !dense {
            assert_eq!(e["origins"][0]["originId"], f.old.to_string());
        }
        let mut stripped = value.clone();
        stripped["data"]["results"][0]
            .as_object_mut()
            .unwrap()
            .remove("explanation");
        assert_eq!(
            stripped, plain,
            "evidence must not alter selection or scores"
        );
        args.as_object_mut().unwrap().remove("contractVersion");
        args.as_object_mut().unwrap().remove("id");
        let response = crate::api::similar_v2(&http, f.source, &args).await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let raw = axum::body::to_bytes(
            response.into_body(),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&raw).unwrap(),
            value
        );
        assert_eq!(bytes(&f.f), before);
        crate::serialization::query_schema::record_fixture(
            if dense {
                "similar-evidence-dense"
            } else {
                "similar-evidence"
            },
            true,
            &value,
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_evidence_actual_metadata_counts_units_and_first_rejection() {
    let f = Fixture::new();
    let owner = owner(&f);
    payload_record(&owner, 9101, [1., 0., 0., 0.], None, None, true).await;
    payload_record(&owner, 9102, [1., 0., 0., 0.], None, None, false).await;
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
    let before = bytes(&f);
    let mut args = serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"recordAccess":false});
    let reply = crate::mcp::tools::prepared_query(&mcp, "default", &args).await;
    assert_eq!(reply.is_error, Some(false), "{reply:?}");
    let value = serde_json::Value::Object(reply.structured_content.unwrap());
    let c = &value["data"]["coverage"];
    assert_eq!(
        c["canonicalEligibility"]["examinedUnit"],
        "canonicalRecords"
    );
    assert_eq!(c["canonicalEligibility"]["examined"], 2);
    assert_eq!(c["canonicalEligibility"]["passed"], 1);
    assert_eq!(
        c["canonicalEligibility"]["firstRejections"]["requiredTag"],
        1
    );
    assert_eq!(
        c["channels"]["metadata"]["examinedUnit"],
        "canonicalRecords"
    );
    assert_eq!(c["channels"]["metadata"]["examined"], 2);
    assert_eq!(c["channels"]["metadata"]["eligible"], 1);
    assert!(c["channels"]["vector"]["examinedUnit"].is_null());
    assert!(c["channels"]["vector"]["examined"].is_null());
    args.as_object_mut().unwrap().remove("contractVersion");
    let response = crate::api::search_v2(&http, "default", &args).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let raw = axum::body::to_bytes(
        response.into_body(),
        crate::daemon::protocol::MAX_MESSAGE_SIZE,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&raw).unwrap(),
        value
    );
    assert_eq!(bytes(&f), before);
    crate::serialization::query_schema::record_fixture("recall-canonical-evidence", false, &value);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_evidence_actual_operator_origin_rif_projection() {
    let f = Fixture::new();
    let owner = owner(&f);
    let old = payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    let terminal = payload_record(&owner, 2, [1., 0., 0., 0.], None, Some(old), true).await;
    payload_record(&owner, 3, [1., 0., 0., 0.], Some(old), None, true).await;
    let mut engine = payload_engine(&f, &owner);
    engine.rif_mode = RifMode::Experimental;
    let before = bytes(&f);
    let query: SearchQuery = serde_json::from_value(serde_json::json!({"queryMode":"metadata_only","filter":{"minStrength":0.0},"limit":3,"graphDepth":0,"recordAccess":false})).unwrap();
    let prepared = engine
        .prepare_coherent_payload_with_semantics(
            query,
            RetrievalSemantics::V2(VersionMode::Current),
            false,
            false,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    let typed = prepared.response.diagnostics.provenance.as_ref().unwrap()[&terminal]
        .explanation
        .as_ref()
        .unwrap();
    assert_eq!(typed.rif_multiplier, 1.0);
    assert!(typed.inherited_sources[0].own.rif_multiplier < 1.0);
    let projected = crate::serialization::query_response::project(&prepared, true).unwrap();
    let row = projected.envelope.data.as_ref().unwrap()["results"].as_array().unwrap()
        .iter().find(|r| r["memoryId"] == terminal.to_string()).unwrap();
    let value = &row["explanation"];
    assert_eq!(value["inheritedSources"][0]["source"], old.to_string());
    assert_eq!(
        value["inheritedSources"][0]["own"]["rifMultiplier"],
        serde_json::json!(typed.inherited_sources[0].own.rif_multiplier)
    );
    assert_eq!(
        value["own"]["rifMultiplier"],
        serde_json::json!(typed.own.as_ref().unwrap().rif_multiplier)
    );
    assert_eq!(bytes(&f), before);
    // This uses the actual operator producer directly, not an experimental REST selector.
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_actual_supplemental_coverage_after_filter_and_cap() {
    let f = Fixture::new();
    let owner = owner(&f);
    let anchor = payload_record(&owner, 1, [1., 0., 0., 0.], None, None, true).await;
    for number in (2..=25).rev() {
        payload_record(
            &owner,
            number,
            [0., 1., 0., 0.],
            Some(anchor),
            None,
            number > 13,
        )
        .await;
    }
    let engine = payload_engine(&f, &owner);
    f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f);
    let request = crate::serialization::query::QueryRequest::from_mcp(
        &serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"limit":1,"depth":1,"compact":false,"recordAccess":false,"explain":true}), "default"
    ).unwrap();
    let reply = engine.query_contract(request).await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    let data = reply.envelope.data.as_ref().unwrap();
    assert_eq!(data["results"][0]["memoryId"], anchor.to_string());
    let neighbors = data["neighbors"].as_array().unwrap();
    assert_eq!(neighbors.len(), 10);
    for (neighbor, number) in neighbors.iter().zip(14..=23) {
        assert_eq!(neighbor["memoryId"], fixed_id(number).to_string());
        assert!(neighbor.get("explanation").is_none());
    }
    let coverage = &data["coverage"]["supplemental"];
    assert_eq!(coverage["ineligibleNeighbors"], 12);
    assert_eq!(coverage["eligibleUniqueNeighbors"], 12);
    assert_eq!(coverage["neighborLimitApplied"], true);
    assert_eq!(coverage["requested"], true);
    assert_eq!(bytes(&f), before);
    crate::serialization::query_schema::record_fixture(
        "recall-supplemental",
        false,
        &serde_json::to_value(&reply).unwrap(),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_query_explanation_preserves_actual_exposure_classification() {
    use crate::search::adapters::StorageAccessRecorder;
    let mut observations = Vec::new();
    for explain in [false, true] {
        let f = Fixture::new();
        let owner = owner(&f);
        let old = payload_record(&owner, 105, [1., 0., 0., 0.], None, None, true).await;
        let current = payload_record(&owner, 106, [1., 0., 0., 0.], None, Some(old), true).await;
        let old_before = f.record(old).unwrap().access_history.clone();
        let current_before = f.record(current).unwrap().access_history.clone();
        let mut engine = payload_engine(&f, &owner);
        engine.access_recorder = Arc::new(
            StorageAccessRecorder::new(f.storage.clone(), f.cache.clone()).with_completion(owner),
        );
        let request = crate::serialization::query::QueryRequest::from_mcp(
            &serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"explain":explain}), "default"
        ).unwrap();
        let reply = engine.query_contract(request).await;
        assert_eq!(reply.http_status, 200, "{reply:?}");
        assert!(reply.envelope.readiness.as_ref().unwrap().ready);
        let hits = reply.envelope.data.as_ref().unwrap()["results"]
            .as_array()
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["memoryId"], current.to_string());
        assert_eq!(hits[0].get("explanation").is_some(), explain);
        assert_eq!(f.record(old).unwrap().access_history, old_before);
        let history = f.record(current).unwrap().access_history;
        assert_eq!(history.len(), current_before.len() + 1);
        observations.push(history.last().unwrap().clone());
    }
    assert_eq!(observations[0], observations[1]);
}

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
    crate::serialization::query_schema::record_fixture("recall-plain", false, &value);
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
    // Ranked-query evidence is optional projection, not a different scorer.
    let mut explained_args = args.clone();
    explained_args["explain"] = serde_json::json!(true);
    let explained = crate::mcp::tools::prepared_query(&mcp, "default", &explained_args).await;
    assert_eq!(explained.is_error, Some(false), "{explained:?}");
    let explained = serde_json::Value::Object(explained.structured_content.unwrap());
    crate::serialization::query_schema::record_fixture("recall-explained", false, &explained);
    let mut without_evidence = explained.clone();
    for hit in without_evidence["data"]["results"].as_array_mut().unwrap() {
        let evidence = hit.as_object_mut().unwrap().remove("explanation").unwrap();
        assert_eq!(evidence["rankingProfile"], "baseline-v1");
        assert_eq!(evidence["retrievalSemanticsVersion"], 2);
        assert_eq!(evidence["finalScore"], hit["compositeScore"]);
        assert_eq!(evidence["version"], hit["version"]);
        assert_eq!(hit["memoryId"], current.to_string());
    }
    assert_eq!(without_evidence, value);
    explained_args
        .as_object_mut()
        .unwrap()
        .remove("contractVersion");
    let response = crate::api::search_v2(&http, "default", &explained_args).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let raw = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&raw).unwrap(),
        explained
    );
    assert_eq!(bytes(&f), before);
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
    crate::serialization::query_schema::record_fixture(
        "recall-legacy-observed",
        false,
        &serde_json::to_value(&reply).unwrap(),
    );
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
            .all(|d| d.ready && d.applied_generation == Some(ready.committed_generation))
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
        let request=crate::serialization::query::QueryRequest::from_mcp(&serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"recordAccess":record_access,"limit":4,"explain":true}),"default").unwrap();
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
    let request=crate::serialization::query::QueryRequest::from_mcp(&serde_json::json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"],"limit":3,"explain":true}),"default").unwrap();
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
    crate::serialization::query_schema::record_fixture(
        "recall-known-failure",
        false,
        &serde_json::to_value(&reply).unwrap(),
    );
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
    crate::serialization::query_schema::record_fixture("similar-plain", true, &value);
    let ids: Vec<_> = value["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["memoryId"].as_str().unwrap().to_owned())
        .collect();
    assert!(ids.contains(&current.to_string()));
    assert!(!ids.contains(&old.to_string()));
    assert!(!ids.contains(&source.to_string()));
    for hit in value["data"]["results"].as_array().unwrap() {
        assert!(hit.get("explanation").is_none());
        assert!(hit.get("rankingProfile").is_none());
    }
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
    // The optional explanation selector is strict: null still fails before
    // exposure in both modes. Actual true cosine evidence is tested separately.
    for access in [false, true] {
        let mut unsupported = args.clone();
        unsupported["explain"] = serde_json::Value::Null;
        unsupported["recordAccess"] = serde_json::json!(access);
        let result = crate::mcp::tools::prepared_similar(&mcp, &unsupported).await;
        assert_eq!(result.is_error, Some(true));
        let rejected = serde_json::Value::Object(result.structured_content.unwrap());
        crate::serialization::query_schema::record_fixture(
            if access {
                "similar-true-rejected"
            } else {
                "similar-false-rejected"
            },
            true,
            &rejected,
        );
        assert_eq!(rejected["error"]["code"], "INVALID_TYPE");
        assert_eq!(rejected["error"]["field"], "/explain");
        assert!(rejected.get("receipt").is_none());
        unsupported
            .as_object_mut()
            .unwrap()
            .remove("contractVersion");
        unsupported.as_object_mut().unwrap().remove("id");
        let response = crate::api::similar_v2(&http, source, &unsupported).await;
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let raw = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&raw).unwrap(),
            rejected
        );
        assert_eq!(bytes(&f.f), before);
    }
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
    let request=crate::serialization::query::SimilarRequest::from_mcp(&serde_json::json!({"contractVersion":2,"id":f.source.to_string(),"namespaceScope":"allCompatible","tags":["allow"],"explain":true})).unwrap();
    let reply = f.engine.similar_contract(request).await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    crate::serialization::query_schema::record_fixture(
        "similar-native-observed",
        true,
        &serde_json::to_value(&reply).unwrap(),
    );
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
