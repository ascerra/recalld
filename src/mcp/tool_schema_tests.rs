use crate::mcp::protocol::*;
use crate::mcp::server::{
    McpServer,
    tests::{initialize_request, modern_request, plain_request},
};
use crate::storage::StorageEngine as _;
use crate::test_support::{Fixture, MemorySpec, StubBridge};
use serde_json::{Value, json};
use std::sync::Arc;

fn bridge(f: &Fixture, default: &str) -> crate::mcp::bridge::McpBridge {
    let mut bridge = StubBridge::default().into_bridge();
    bridge.storage = Arc::new(f.storage_adapter());
    bridge.namespaces = Arc::new(f.namespace_adapter());
    bridge.default_namespace = default.into();
    bridge
}

fn record_fixture(label: &str, tool: &str, schema: &Value, value: &Value) {
    if let Some(dir) = std::env::var_os("RECALLD_LIVE_SCHEMA_FIXTURES") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{label}-{tool}.json")),
            serde_json::to_vec(&json!({"tool":tool,"schema":schema,"value":value})).unwrap(),
        )
        .unwrap();
    }
}

#[tokio::test]
async fn live_metadata_schemas_actual_outputs_preserve_eras_scope_and_truncation() {
    let f = Fixture::new();
    let project = f.create_namespace("Project");
    let mut ids = vec![];
    for namespace in [crate::model::NamespaceId::new(1), project] {
        ids.push(
            f.insert_memory_with(MemorySpec {
                tags: &[
                    "alpha",
                    "beta",
                    "entity/alice",
                    "topic/tools",
                    "emotion/calm",
                ],
                ..MemorySpec::new(namespace)
            })
            .await,
        );
    }
    let doomed = f.insert_memory().await;
    f.tombstone(doomed);
    ids.push(doomed);
    f.completion().rebuild_all().await.unwrap();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let records: Vec<_> = ids
        .iter()
        .map(|id| f.record(*id).unwrap().to_bytes())
        .collect();
    let mut schemas = std::collections::BTreeMap::new();
    for modern in [true, false] {
        for default in ["default", "Project"] {
            let mut server = McpServer::new(Arc::new(bridge(&f, default)));
            if !modern {
                assert!(
                    server
                        .handle_message(initialize_request(1, LEGACY_PROTOCOL_VERSION))
                        .await
                        .unwrap()
                        .error
                        .is_none()
                );
            }
            let message = |method: &str, args: Value| {
                if modern {
                    modern_request(2, method, args)
                } else {
                    plain_request(2, method, Some(args))
                }
            };
            let catalog = server
                .handle_message(message("tools/list", json!({})))
                .await
                .unwrap()
                .result
                .unwrap();
            assert_eq!(catalog["cacheScope"], "public");
            let tools = catalog["tools"].as_array().unwrap();
            assert_eq!(tools.len(), 17);
            for tool in tools {
                let name = tool["name"].as_str().unwrap();
                assert!(tool["inputSchema"]["properties"]["contractVersion"].is_null());
                if name == "get_memory" {
                    assert_eq!(
                        tool["inputSchema"]["properties"]["recordAccess"]["default"],
                        true
                    );
                    assert_eq!(tool["annotations"]["readOnlyHint"], false);
                    assert!(
                        tool["description"]
                            .as_str()
                            .unwrap()
                            .contains("recordAccess defaults to true")
                    );
                    assert!(
                        !tool["description"]
                            .as_str()
                            .unwrap()
                            .contains("does not record access")
                    );
                }
                let has_schema = modern
                    && matches!(
                        name,
                        "get_session"
                            | "namespace_stats"
                            | "list_tags"
                            | "list_memories"
                            | "recall_memories"
                            | "find_similar_memories"
                            | "reinforce_memory"
                            | "forget_memory"
                            | "get_memory"
                            | "store_memory"
                            | "store_memories"
                            | "forget_memories"
                            | "move_memories"
                            | "delete_namespace"
                    );
                assert_eq!(tool.get("outputSchema").is_some(), has_schema, "{name}");
                if has_schema {
                    schemas.insert(name.to_owned(), tool["outputSchema"].clone());
                }
            }
            for (tool, arguments) in [
                ("namespace_stats", json!({})),
                ("list_tags", json!({"limit":1})),
                ("list_tags", json!({"allNamespaces":true,"limit":1})),
            ] {
                let result = server
                    .handle_message(message(
                        "tools/call",
                        json!({"name":tool,"arguments":arguments}),
                    ))
                    .await
                    .unwrap()
                    .result
                    .unwrap();
                assert_ne!(result["isError"], true, "{result}");
                let value: Value =
                    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
                if modern {
                    assert_eq!(result["structuredContent"], value);
                } else {
                    assert!(result.get("structuredContent").is_none());
                }
                if tool == "namespace_stats" {
                    assert_eq!(value["name"], default);
                    assert_eq!(value["memoryCount"], 1);
                    assert_eq!(
                        value["tombstoneCount"],
                        if default == "default" { 1 } else { 0 }
                    );
                    assert_eq!(
                        value["phaseCounts"],
                        json!({"full":1,"summary":0,"ghost":0})
                    );
                } else {
                    assert_eq!(value["tags"]["total"], 2);
                    assert_eq!(value["tags"]["truncated"], true);
                    assert_eq!(value["tags"]["items"].as_array().unwrap().len(), 1);
                    if arguments["allNamespaces"] == true {
                        assert!(value["namespace"].is_null() && value["memoriesCounted"].is_null());
                    } else {
                        assert_eq!(value["namespace"], default);
                        assert_eq!(value["memoriesCounted"], 1);
                    }
                }
                let label = format!("{modern}-{default}-{}", arguments["allNamespaces"] == true);
                record_fixture(&label, tool, &schemas[tool], &value);
            }
            let rejected = server
                .handle_message(message(
                    "tools/call",
                    json!({"name":"namespace_stats","arguments":{"contractVersion":2}}),
                ))
                .await
                .unwrap()
                .result
                .unwrap();
            assert_eq!(rejected["isError"], true);
            assert!(rejected.get("structuredContent").is_none());
        }
    }
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    for (id, bytes) in ids.iter().zip(records) {
        assert_eq!(f.record(*id).unwrap().to_bytes(), bytes);
    }
    assert!(SERVER_INSTRUCTIONS.split_whitespace().count() < 150);
    assert!(SERVER_INSTRUCTIONS.starts_with("At task start, recall relevant context"));
}

#[tokio::test]
async fn live_metadata_schemas_actual_http_mcp_catalog_and_output() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    let f = Fixture::new();
    let app =
        crate::mcp::http_transport::mcp_router(Arc::new(bridge(&f, "default")), Default::default());
    let mut schema = Value::Null;
    for (method, args) in [
        ("tools/list", json!({})),
        (
            "tools/call",
            json!({"name":"namespace_stats","arguments":{}}),
        ),
    ] {
        let request = modern_request(1, method, args);
        let http_request = |with_name: bool| {
            let mut builder = Request::post("/mcp")
                .header("content-type", "application/json")
                .header("MCP-Protocol-Version", PROTOCOL_VERSION)
                .header("MCP-Method", method);
            if with_name && method == "tools/call" {
                builder = builder.header("MCP-Name", "namespace_stats");
            }
            builder
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap()
        };
        if method == "tools/call" {
            let rejected = app.clone().oneshot(http_request(false)).await.unwrap();
            assert_eq!(rejected.status(), 400);
            let bytes = to_bytes(rejected.into_body(), 1024 * 1024).await.unwrap();
            let rejected: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(rejected["error"]["code"], HEADER_MISMATCH);
            assert!(
                rejected["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("missing required Mcp-Name")
            );
        }
        let response = app.clone().oneshot(http_request(true)).await.unwrap();
        let status = response.status();
        let raw = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let response: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(status, 200, "{method}: {response}");
        assert!(response.get("error").is_none(), "{response}");
        let result = &response["result"];
        if method == "tools/list" {
            schema = result["tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == "namespace_stats")
                .unwrap()["outputSchema"]
                .clone();
            assert!(schema.is_object());
        } else {
            let value: Value =
                serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(result["structuredContent"], value);
            assert_eq!(value["memoryCount"], 0);
            record_fixture("http-empty", "namespace_stats", &schema, &value);
        }
    }
}

/// Version selectors are accepted but no longer advertised to agents.
#[tokio::test]
async fn version_selectors_are_hidden_but_still_accepted() {
    let f = Fixture::new();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let mut server = McpServer::new(Arc::new(bridge(&f, "default")));
    let call = |id, name: &str, arguments: Value| {
        modern_request(id, "tools/call", json!({"name":name,"arguments":arguments}))
    };
    let catalog = server
        .handle_message(modern_request(1, "tools/list", json!({})))
        .await
        .unwrap()
        .result
        .unwrap();
    for tool in catalog["tools"].as_array().unwrap() {
        // Output receipts still report the contract a record was written under.
        let text = format!("{} {}", tool["inputSchema"], tool["description"]);
        assert!(!text.contains("contractVersion"), "{text}");
        assert!(!text.contains("batchResultVersion"), "{text}");
    }
    for instructions in [SERVER_INSTRUCTIONS, SINGLE_NAMESPACE_INSTRUCTIONS] {
        assert!(!instructions.to_lowercase().contains("contract"));
    }
    for (name, arguments, accepted) in [
        ("get_session", json!({"contractVersion":1}), true),
        ("namespace_stats", json!({"contractVersion":1}), true),
        ("get_session", json!({"contractVersion":2}), false),
        ("namespace_stats", json!({"contractVersion":2}), false),
        (
            "store_memories",
            json!({"memories":[{"summary":"never stored"}],"batchResultVersion":2}),
            false,
        ),
    ] {
        let result = server
            .handle_message(call(2, name, arguments.clone()))
            .await
            .unwrap()
            .result
            .unwrap();
        assert_eq!(
            result["isError"] != true,
            accepted,
            "{name} {arguments}: {result}"
        );
        if arguments["contractVersion"] == 2 {
            assert!(
                result["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains("contractVersion 2 is not supported"),
                "{result}"
            );
        }
    }
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
}
