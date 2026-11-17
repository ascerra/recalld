// Test-only ordinary API1 consumer probes; no runtime routing change.
async fn api1_point_projection_probe(remote: bool) {
    let f = similar_fixture().await;
    let engine = Arc::new(f.engine);
    let mut input = point_request(f.foreign, false);
    input.original_contract_version = 1;
    let expected = engine
        .get_contract(input, crate::daemon::protocol::MAX_MESSAGE_SIZE)
        .await;
    assert_eq!(expected.http_status, 200, "{expected:?}");
    let expected = expected.envelope.data.unwrap().memory;
    let materialized = f.f.record(f.foreign).unwrap().strength;
    assert_ne!(expected.strength.to_bits(), materialized.to_bits());
    let adapter = Arc::new(
        f.f.storage_adapter()
            .with_clock(f.clock.clone())
            .with_query_engine(engine),
    );
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.default_namespace = "wrong-session-scope".into();
    let root = tempfile::tempdir().unwrap();
    let daemon = if remote {
        let (storage, task) = crate::daemon::server::native_lifecycle_test_remote(
            adapter,
            &root.path().join("api1-point.sock"),
        )
        .await;
        bridge.storage = Arc::new(storage);
        Some(task)
    } else {
        bridge.storage = adapter;
        None
    };
    f.clock.0.store(0, Ordering::SeqCst);
    let before = bytes(&f.f);
    let result = crate::mcp::tools::dispatch_tool(
        &bridge,
        "get_memory",
        serde_json::json!({"id":f.foreign.to_string(),"recordAccess":false,
            "historicallyIgnored":true}),
    )
    .await;
    drop(bridge);
    if let Some(task) = daemon {
        tokio::time::timeout(std::time::Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
    }
    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert_eq!(bytes(&f.f), before, "false mode changed canonical files");
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 1);
    let value = result.structured_content.unwrap();
    assert_eq!(value["id"], f.foreign.to_string());
    assert_eq!(value["namespace"], "other");
    assert_eq!(value["summary"], expected.summary.unwrap());
    assert_eq!(value["fullText"], expected.full_text.unwrap());
    let actual = value["strength"].as_f64().unwrap() as f32;
    eprintln!(
        "API1 projection remote={remote}: actual={actual:?} captured={:?} materialized={materialized:?}; false bytes/scope/payload/one-clock controls passed",
        expected.strength
    );
    assert_eq!(actual.to_bits(), expected.strength.to_bits());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_point_before_direct_native_uses_captured_projection() {
    api1_point_projection_probe(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_point_before_remote_native_uses_captured_projection() {
    api1_point_projection_probe(true).await;
}
