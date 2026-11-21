//! Real legacy REST router/MCP dispatcher regressions. No native-v2 fixture.
use super::*;
use crate::mcp::{
    McpServer,
    bridge::McpBridge,
    server::tests::{initialize_request, modern_request, plain_request},
};
use crate::test_support::StubBridge;
use axum::{body::Body, http::Request};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn response(
    router: axum::Router,
    method: &str,
    path: &str,
    selectors: &[&str],
    body: &str,
) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    for value in selectors {
        request = request.header("Recalld-Contract-Version", *value);
    }
    let response = router
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn contract_http_rejects_before_parsing_provider_and_all_mutations() {
    let f = Fixture::new();
    let provider = Arc::new(CountingProvider::default());
    let router = crate::api::router(
        state(&f, provider.clone()),
        &crate::api::ApiConfig::default(),
    );
    let before = files(f.path());
    let generation = f.storage.read().unwrap().control_state().unwrap();
    for selector in [
        vec!["2"],
        vec!["02"],
        vec!["1.0"],
        vec!["null"],
        vec!["1", "2"],
        vec!["1", "1"],
    ] {
        for (method, path) in [
            ("POST", "/memories"),
            ("POST", "/memories/batch"),
            ("POST", "/search"),
            ("POST", "/namespaces"),
            ("PUT", "/namespaces/1/policy"),
            ("DELETE", "/memories/00000000-0000-0000-0000-000000000001"),
            (
                "POST",
                "/memories/00000000-0000-0000-0000-000000000001/reinforce",
            ),
            ("POST", "/decay/sweep"),
        ] {
            // #237: contract 2 is published for policy replacement only;
            // `native_policy_wire_gate_opens_for_v2_only` covers that pair.
            if selector == ["2"] && path == "/namespaces/1/policy" {
                continue;
            }
            // Intentionally malformed JSON: selector must win before extraction.
            let (status, value) = response(router.clone(), method, path, &selector, "{").await;
            assert_eq!(status, 400, "{selector:?} {path}: {value}");
            assert_eq!(value["field"], "contractVersion");
        }
    }
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.storage.read().unwrap().control_state().unwrap(),
        generation
    );
    assert_eq!(files(f.path()), before);
}

#[tokio::test]
async fn native_policy_wire_gate_opens_for_v2_only() {
    let f = Fixture::new();
    let provider = Arc::new(CountingProvider::default());
    let router = crate::api::router(
        state(&f, provider.clone()),
        &crate::api::ApiConfig::default(),
    );
    let before = files(f.path());
    // Contract 1 (explicit or omitted) still cannot replace a policy.
    for selectors in [vec![], vec!["1"]] {
        let (status, value) = response(
            router.clone(),
            "PUT",
            "/namespaces/1/policy",
            &selectors,
            "{",
        )
        .await;
        assert_eq!(status, 400);
        assert_eq!(value["field"], "contractVersion");
    }
    // #237: contract 2 reaches the policy handler (this legacy fixture has no
    // native catalog, so it answers with a contract-2 envelope, not a gate error).
    let body = json!({"policy":{
        "initialStability":1.0,"defaultDifficulty":5,"permastoreThreshold":100,"desiredRetention":0.9,
        "phaseThresholds":{"fullToSummary":0.8,"summaryToGhost":0.3,"ghostToDelete":0.1},
        "decayRateMultiplier":null
    }});
    let (status, value) = response(
        router.clone(),
        "PUT",
        "/namespaces/1/policy",
        &["2"],
        &body.to_string(),
    )
    .await;
    assert_ne!(value["field"], "contractVersion", "{status} {value}");
    assert_eq!(value["contractVersion"], 2, "{value}");
    assert_eq!(value["status"], "error", "{value}");
    use crate::api::middleware::is_policy_replacement;
    use axum::http::Method;
    assert!(is_policy_replacement(
        &Method::PUT,
        "/namespaces/default/policy"
    ));
    for (method, path) in [
        (Method::POST, "/namespaces/1/policy"),
        (Method::PUT, "/namespaces//policy"),
        (Method::PUT, "/namespaces/1/2/policy"),
        (Method::PUT, "/namespaces/1/policy/x"),
        (Method::PUT, "/memories/1/policy"),
    ] {
        assert!(!is_policy_replacement(&method, path), "{method} {path}");
    }
    assert_eq!(files(f.path()), before);
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);
    // MCP keeps refusing contract 2 for every tool, this one included; its
    // published form is the contract-1 partial change.
    let bridge = StubBridge::default().into_bridge();
    let result = crate::mcp::tools::dispatch_tool(
        &bridge,
        "update_namespace_policy",
        json!({"contractVersion":2}),
    )
    .await;
    assert_eq!(result.is_error, Some(true));
    assert!(
        serde_json::to_string(&result)
            .unwrap()
            .contains("contractVersion 2 is not supported")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_http_v1_success_shape_and_unknown_fields_stay_legacy() {
    let f = Fixture::new();
    let router = crate::api::router(
        state(&f, Arc::new(CountingProvider::default())),
        &crate::api::ApiConfig::default(),
    );
    let (status, stored) = response(
        router.clone(),
        "POST",
        "/memories",
        &["1"],
        &json!({
            "summary":"legacy wire", "namespace":"default", "embedding":[1.,0.,0.,0.],
            "entities":["Alice"],"topics":["Rust"],"emotions":["Curious"],"ignoredLegacyField":true
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, 201, "{stored}");
    // Use the real returned UUID; do not fabricate a successful payload.
    let id = stored["data"]["id"].as_str().unwrap();
    for path in [
        "/namespaces".to_owned(),
        format!("/memories/{id}?recordAccess=false"),
    ] {
        let (s1, omitted) = response(router.clone(), "GET", &path, &[], "").await;
        let (s2, explicit) = response(router.clone(), "GET", &path, &["1"], "").await;
        assert_eq!((s1, s2), (200, 200));
        assert_eq!(omitted["data"], explicit["data"]);
        assert_eq!(
            omitted.as_object().unwrap().keys().collect::<Vec<_>>(),
            explicit.as_object().unwrap().keys().collect::<Vec<_>>()
        );
        // tookUs measures each execution; its numeric value is not payload identity.
        assert!(omitted.get("contractVersion").is_none());
        assert!(omitted.get("readGeneration").is_none());
        if path.starts_with("/memories/") {
            for field in ["entities", "topics", "emotions"] {
                assert!(omitted["data"][field].is_array(), "{omitted}");
            }
            assert!(omitted["data"].get("labels").is_none());
        }
    }
}

fn bridge(f: &Fixture) -> Arc<McpBridge> {
    Arc::new(McpBridge {
        search: Arc::new(f.search_adapter()),
        storage: Arc::new(f.storage_adapter()),
        namespaces: Arc::new(f.namespace_adapter()),
        health: Arc::new(StubBridge::default()),
        default_namespace: "default".into(),
        timezone: chrono_tz::UTC,
    })
}

#[tokio::test]
async fn contract_mcp_guards_every_tool_and_generic_dispatch_without_backend_work() {
    let f = Fixture::new();
    let mut server = McpServer::new(bridge(&f));
    let before = files(f.path());
    let generation = f.storage.read().unwrap().control_state().unwrap();
    let panic_bridge = StubBridge::default().into_bridge();
    for tool in crate::mcp::tools::tool_definitions() {
        assert!(tool.input_schema["properties"]["contractVersion"].is_null());
        for selector in [
            json!(2),
            json!(3),
            json!(null),
            json!(true),
            json!("2"),
            json!(2.0),
        ] {
            let args = json!({"contractVersion":selector});
            let direct =
                crate::mcp::tools::dispatch_tool(&panic_bridge, &tool.name, args.clone()).await;
            assert_eq!(direct.is_error, Some(true));
            let response = server
                .handle_message(modern_request(
                    1,
                    "tools/call",
                    json!({"name":tool.name,"arguments":args}),
                ))
                .await
                .unwrap();
            assert!(response.error.is_none());
            assert_eq!(response.result.unwrap()["isError"], true);
        }
    }
    // Preserve the complete legacy non-object error, not just rejection.
    for args in [json!(null), json!([]), json!(true), json!(1), json!("bad")] {
        let direct =
            crate::mcp::tools::dispatch_tool(&panic_bridge, "list_namespaces", args.clone()).await;
        let dispatched = server
            .handle_message(modern_request(
                1,
                "tools/call",
                json!({"name":"list_namespaces","arguments":args}),
            ))
            .await
            .unwrap()
            .result
            .unwrap();
        let direct = serde_json::to_value(direct).unwrap();
        assert_eq!(direct["content"], dispatched["content"]);
        assert_eq!(direct["isError"], dispatched["isError"]);
        assert!(dispatched.get("structuredContent").is_none());
        // The modern protocol adds resultType/_meta outside the tool payload.
        assert_eq!(dispatched["resultType"], "complete");
    }
    // The dispatcher also gates handlers other than the production bridge.
    let mut generic = crate::mcp::server::tests::server();
    let rejected = generic
        .handle_message(modern_request(
            1,
            "tools/call",
            json!({"name":"structured_test","arguments":{"contractVersion":2}}),
        ))
        .await
        .unwrap()
        .result
        .unwrap();
    assert_eq!(rejected["isError"], true);
    assert!(
        rejected["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("UNSUPPORTED_CONTRACT_VERSION")
    );
    assert_eq!(
        f.storage.read().unwrap().control_state().unwrap(),
        generation
    );
    assert_eq!(files(f.path()), before);
}

#[tokio::test]
async fn contract_mcp_omitted_arguments_v1_and_legacy_eras_keep_success_payloads() {
    let f = Fixture::new();
    let bridge = bridge(&f);
    let mut modern = McpServer::new(bridge.clone());
    let mut reference = None;
    for arguments in [
        None,
        Some(json!({})),
        Some(json!({"contractVersion":1,"unknownV1":true})),
    ] {
        let mut params = json!({"name":"list_namespaces"});
        if let Some(args) = arguments {
            params["arguments"] = args;
        }
        let wire = modern
            .handle_message(modern_request(1, "tools/call", params))
            .await
            .unwrap()
            .result
            .unwrap();
        assert_ne!(wire["isError"], true, "{wire}");
        let parsed: Value =
            serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(parsed.is_array());
        assert!(wire.get("structuredContent").is_none());
        assert!(parsed.get("contractVersion").is_none());
        if let Some(prior) = &reference {
            assert_eq!(&wire, prior);
        } else {
            reference = Some(wire);
        }
    }
    let object_args =
        json!({"name":"namespace_stats","arguments":{"namespace":"default","contractVersion":1}});
    let object_wire = modern
        .handle_message(modern_request(1, "tools/call", object_args.clone()))
        .await
        .unwrap()
        .result
        .unwrap();
    assert_ne!(object_wire["isError"], true, "{object_wire}");
    let object_value: Value =
        serde_json::from_str(object_wire["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(object_value.is_object());
    assert_eq!(object_wire["structuredContent"], object_value);
    for version in crate::mcp::SUPPORTED_LEGACY_VERSIONS {
        let mut legacy = McpServer::new(bridge.clone());
        legacy
            .handle_message(initialize_request(1, version))
            .await
            .unwrap();
        for params in [
            json!({"name":"list_namespaces"}),
            json!({"name":"list_namespaces","arguments":{"contractVersion":1}}),
        ] {
            let wire = legacy
                .handle_message(plain_request(2, "tools/call", Some(params)))
                .await
                .unwrap()
                .result
                .unwrap();
            assert_ne!(wire["isError"], true);
            assert!(wire.get("structuredContent").is_none());
            assert_eq!(wire["content"], reference.as_ref().unwrap()["content"]);
        }
        let legacy_object = legacy
            .handle_message(plain_request(2, "tools/call", Some(object_args.clone())))
            .await
            .unwrap()
            .result
            .unwrap();
        assert_ne!(legacy_object["isError"], true, "{legacy_object}");
        assert!(legacy_object.get("structuredContent").is_none());
        assert_eq!(legacy_object["content"], object_wire["content"]);
        let rejected = legacy
            .handle_message(plain_request(
                3,
                "tools/call",
                Some(json!({"name":"list_namespaces","arguments":{"contractVersion":2}})),
            ))
            .await
            .unwrap()
            .result
            .unwrap();
        assert_eq!(rejected["isError"], true);
        assert!(rejected.get("structuredContent").is_none());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_duplicates_v1_preserves_original_extractor_errors() {
    let fixture = Fixture::new();
    let state = state(&fixture, Arc::new(CountingProvider::default()));
    let legacy = axum::Router::new()
        .route(
            "/namespaces/{name}/duplicates",
            axum::routing::post(crate::api::handlers::scan_duplicates),
        )
        .with_state(state.clone());
    let dispatch = crate::api::router(state, &crate::api::ApiConfig::default());
    for (body, content_type) in [
        ("{", "application/json"),
        (r#"{"threshold":null}"#, "application/json"),
        (r#"{"maxMemories":"bad"}"#, "application/json"),
        ("{}", "text/plain"),
    ] {
        let request = || {
            Request::builder()
                .method("POST")
                .uri("/namespaces/default/duplicates")
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap()
        };
        let before = legacy.clone().oneshot(request()).await.unwrap();
        let after = dispatch.clone().oneshot(request()).await.unwrap();
        assert_eq!(before.status(), after.status());
        let before = axum::body::to_bytes(before.into_body(), usize::MAX)
            .await
            .unwrap();
        let after = axum::body::to_bytes(after.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(before, after);
    }
}
