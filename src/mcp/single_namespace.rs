//! `[namespaces] mode = "single"`: MCP agents see one shared store.
//!
//! A wrapper over [`McpBridge`] so multi mode is the bridge untouched. In
//! single mode the namespace-management tools are hidden and refused, the
//! namespace guidance is dropped from the instructions and tool
//! descriptions, and any argument that would reach a namespace other than
//! `default` is rejected rather than silently pinned (see
//! [`crate::mcp::args::namespace_or`]). The HTTP API and CLI are unaffected.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::config::NamespacesMode;
use crate::mcp::bridge::McpBridge;
use crate::mcp::protocol::{
    ResourceInfo, ResourceReadResult, ResourceTemplate, SINGLE_NAMESPACE_INSTRUCTIONS,
    ToolCallResult, ToolInfo,
};
use crate::mcp::server::McpHandler;
use crate::model::constants::DEFAULT_NAMESPACE_NAME as DEFAULT;

/// Tools that only make sense with several namespaces.
const HIDDEN: [&str; 3] = ["create_namespace", "delete_namespace", "move_memories"];

const SETTING: &str = "recalld is in single-namespace mode ([namespaces] mode = \"single\" \
    or RECALLD_NAMESPACES_MODE=single); every memory is in 'default'";

/// (tool, multi-mode sentence, single-mode replacement). A test asserts
/// every sentence still occurs, so a reworded description cannot drift
/// back into namespace guidance unnoticed.
const DESCRIPTIONS: [(&str, &str, &str); 5] = [
    (
        "get_session",
        "; pass namespace explicitly to choose shared, project, or topic context.",
        ". Single-namespace mode: everything is in default; do not pass namespace.",
    ),
    (
        "store_memory",
        "Choose namespace explicitly: shared context in default, project-only context in a \
         project namespace, and reusable topic context in an existing topic namespace when \
         useful. Omission uses the session default shown by get_session.",
        "Single-namespace mode: every memory is stored in default; do not pass namespace.",
    ),
    (
        "store_memories",
        "Each item uses store_memory fields and its own namespace (omitted: session default).",
        "Each item uses store_memory fields; do not pass namespace (single-namespace mode: \
         everything is in default).",
    ),
    (
        "recall_memories",
        "Omitted namespace uses the session default shown by get_session; shared and project \
         context require separate scoped calls.",
        "Single-namespace mode: everything is in default, so one call searches every memory; \
         do not pass namespace.",
    ),
    (
        "list_namespaces",
        "Call this first when you do not know which namespaces exist. Reuse a matching project \
         or topic scope instead of creating duplicates.",
        "Single-namespace mode: only default is reachable through this server; other \
         namespaces listed here are kept but not readable here.",
    ),
];

/// The MCP handler for `mode`: the bridge itself in multi mode.
///
/// In single mode a `.recalld.toml` namespace is ignored (with a WARN),
/// and a WARN names other namespaces that still hold live memories.
pub async fn handler(mut bridge: McpBridge, mode: NamespacesMode) -> Arc<dyn McpHandler> {
    if mode == NamespacesMode::Multi {
        return Arc::new(bridge);
    }
    if bridge.default_namespace != DEFAULT {
        tracing::warn!(
            namespace = %bridge.default_namespace,
            "single-namespace mode: ignoring the .recalld.toml namespace; MCP uses 'default'"
        );
        bridge.default_namespace = DEFAULT.into();
    }
    match bridge.namespaces.list_namespaces().await {
        Ok(list) => {
            let others: Vec<String> = list
                .iter()
                .filter(|ns| ns.name != DEFAULT && ns.memory_count > 0)
                .map(|ns| format!("{} ({})", ns.name, ns.memory_count))
                .collect();
            if !others.is_empty() {
                tracing::warn!(
                    namespaces = %others.join(", "),
                    "single-namespace mode: these namespaces hold live memories that are not \
                     reachable through MCP. They are kept; switch to multi mode and use \
                     move_memories to consolidate them into 'default'"
                );
            }
        }
        Err(error) => tracing::debug!(%error, "could not list namespaces at startup"),
    }
    Arc::new(SingleNamespace(bridge))
}

struct SingleNamespace(McpBridge);

fn refuse(name: &str) -> ToolCallResult {
    ToolCallResult::error(format!(
        "{name} is disabled: {SETTING}. Set mode = \"multi\" to manage namespaces."
    ))
}

/// Reject any argument that would reach beyond `default`.
fn check(arguments: &Value) -> Result<(), String> {
    let items = arguments.get("memories").and_then(Value::as_array);
    for args in std::iter::once(arguments).chain(items.into_iter().flatten()) {
        if let Some(ns) = args.get("namespace").and_then(Value::as_str)
            && ns != DEFAULT
        {
            return Err(format!(
                "namespace '{ns}' is not available: {SETTING}. Omit namespace."
            ));
        }
    }
    if arguments.get("allNamespaces") == Some(&Value::Bool(true)) {
        return Err(format!("allNamespaces is not available: {SETTING}."));
    }
    if arguments.get("sameNamespace") == Some(&Value::Bool(false)) {
        return Err(format!("sameNamespace:false is not available: {SETTING}."));
    }
    Ok(())
}

#[async_trait]
impl McpHandler for SingleNamespace {
    fn tools(&self) -> Vec<ToolInfo> {
        let mut tools = self.0.tools();
        tools.retain(|tool| !HIDDEN.contains(&tool.name.as_str()));
        for tool in &mut tools {
            for (name, multi, single) in DESCRIPTIONS {
                if tool.name == name {
                    tool.description = tool.description.replacen(multi, single, 1);
                }
            }
        }
        tools
    }

    fn resources(&self) -> Vec<ResourceInfo> {
        self.0.resources()
    }

    fn resource_templates(&self) -> Vec<ResourceTemplate> {
        self.0.resource_templates()
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> ToolCallResult {
        if HIDDEN.contains(&name) {
            return refuse(name);
        }
        match check(&arguments) {
            Ok(()) => self.0.call_tool(name, arguments).await,
            Err(error) => ToolCallResult::error(error),
        }
    }

    async fn call_catalog_tool(
        &self,
        _arguments: Value,
        _admission: crate::serialization::catalog_api1::CatalogReplyAdmission,
    ) -> ToolCallResult {
        refuse("create_namespace")
    }

    async fn call_move_tool(
        &self,
        _arguments: Value,
        _admission: crate::serialization::move_memories::Admission,
    ) -> ToolCallResult {
        refuse("move_memories")
    }

    async fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, String> {
        self.0.read_resource(uri).await
    }

    fn instructions(&self) -> &'static str {
        SINGLE_NAMESPACE_INSTRUCTIONS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::bridge::NamespaceRegistry as _;
    use crate::mcp::protocol::SERVER_INSTRUCTIONS;
    use crate::mcp::server::McpServer;
    use crate::mcp::server::tests::{initialize_request, modern_request};
    use crate::test_support::{Fixture, StubBridge};
    use serde_json::json;

    fn bridge(f: &Fixture, default: &str) -> McpBridge {
        let mut bridge = StubBridge::default().into_bridge();
        bridge.storage = Arc::new(f.storage_adapter());
        bridge.namespaces = Arc::new(f.namespace_adapter());
        bridge.default_namespace = default.into();
        bridge
    }

    async fn call(server: &mut McpServer, name: &str, arguments: Value) -> Value {
        let params = json!({"name": name, "arguments": arguments});
        server
            .handle_message(modern_request(2, "tools/call", params))
            .await
            .unwrap()
            .result
            .unwrap()
    }

    fn error_text(result: &Value) -> &str {
        assert_eq!(result["isError"], true, "{result}");
        result["content"][0]["text"].as_str().unwrap()
    }

    #[test]
    fn every_replaced_description_sentence_still_exists() {
        let tools = crate::mcp::tools::tool_definitions();
        for (name, multi, _) in DESCRIPTIONS {
            let tool = tools.iter().find(|t| t.name == name).unwrap();
            assert!(tool.description.contains(multi), "{name}: {multi}");
        }
        for name in HIDDEN {
            assert!(tools.iter().any(|t| t.name == name), "{name}");
        }
    }

    #[tokio::test]
    async fn multi_mode_is_the_bridge_unchanged() {
        let f = Fixture::new();
        let handler = handler(bridge(&f, "default"), NamespacesMode::Multi).await;
        assert_eq!(handler.instructions(), SERVER_INSTRUCTIONS);
        let names = |tools: Vec<ToolInfo>| tools.into_iter().map(|t| t.name).collect::<Vec<_>>();
        assert_eq!(
            names(handler.tools()),
            names(crate::mcp::tools::tool_definitions())
        );
    }

    #[tokio::test]
    async fn single_mode_hides_refuses_and_pins_to_default() {
        let f = Fixture::new();
        f.create_namespace("Project");
        // A `.recalld.toml` namespace is ignored.
        let mut server =
            McpServer::new(handler(bridge(&f, "Project"), NamespacesMode::Single).await);

        let init = server
            .handle_message(initialize_request(1, "2025-06-18"))
            .await
            .unwrap()
            .result
            .unwrap();
        assert_eq!(init["instructions"], SINGLE_NAMESPACE_INSTRUCTIONS);
        assert_ne!(SINGLE_NAMESPACE_INSTRUCTIONS, SERVER_INSTRUCTIONS);
        assert!(SINGLE_NAMESPACE_INSTRUCTIONS.split_whitespace().count() < 150);
        let discover = server
            .handle_message(modern_request(1, "server/discover", json!({})))
            .await
            .unwrap()
            .result
            .unwrap();
        assert_eq!(discover["instructions"], SINGLE_NAMESPACE_INSTRUCTIONS);

        let catalog = server
            .handle_message(modern_request(2, "tools/list", json!({})))
            .await
            .unwrap()
            .result
            .unwrap();
        let tools = catalog["tools"].as_array().unwrap();
        assert_eq!(tools.len(), crate::mcp::tools::tool_definitions().len() - 3);
        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            assert!(!HIDDEN.contains(&name), "{name}");
            let description = tool["description"].as_str().unwrap();
            assert!(!description.contains("project namespace"), "{name}");
            assert!(!description.contains("Choose namespace"), "{name}");
        }

        for name in HIDDEN {
            let refused = call(&mut server, name, json!({"name": "project-x"})).await;
            assert!(
                error_text(&refused).contains("RECALLD_NAMESPACES_MODE"),
                "{name}"
            );
        }
        let names: Vec<_> = f.namespace_adapter().list_namespaces().await.unwrap();
        assert_eq!(names.len(), 2, "nothing was created");

        for (name, args) in [
            ("list_memories", json!({"namespace": "Project"})),
            (
                "recall_memories",
                json!({"query": "x", "namespace": "Project"}),
            ),
            (
                "store_memories",
                json!({"memories": [{"summary": "x", "namespace": "Project"}]}),
            ),
            ("list_tags", json!({"allNamespaces": true})),
            (
                "find_similar_memories",
                json!({"id": "x", "sameNamespace": false}),
            ),
        ] {
            let rejected = call(&mut server, name, args).await;
            assert!(
                error_text(&rejected).contains("[namespaces] mode"),
                "{name}"
            );
        }

        for args in [json!({"namespace": "default"}), json!({})] {
            let listed = call(&mut server, "list_memories", args).await;
            assert_ne!(listed["isError"], true, "{listed}");
        }
        let session = call(&mut server, "get_session", json!({})).await;
        assert_eq!(
            session["structuredContent"]["defaultNamespace"]["requestedName"],
            "default"
        );
    }
}
