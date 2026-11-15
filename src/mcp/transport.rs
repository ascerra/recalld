//! Stdio transport for MCP JSON-RPC communication.
//!
//! Reads newline-delimited JSON-RPC from stdin, dispatches to the
//! MCP server, and writes responses to stdout. All non-protocol
//! output (logging, debug) goes to stderr, never stdout.
//!
//! stdio is **dual-era**, and the era is decided PER MESSAGE, not per
//! process. Under MCP 2026-07-28 an open connection is explicitly not a
//! conversation or a session: a stdio process may serve a modern client
//! that never sends `initialize` (it probes `server/discover` first and
//! carries its protocol version in every `params._meta`), a legacy client
//! that does, or both. The single `McpServer` held for the process
//! lifetime therefore holds legacy lifecycle state only; modern requests
//! never consult it.

use std::sync::Arc;

use tokio::io::{self, AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::mcp::McpError;
use crate::mcp::protocol::*;
use crate::mcp::server::McpServer;

/// Handle one line of stdin, returning the line to write to stdout.
///
/// Returns `Ok(None)` for blank input and for notifications, which produce
/// no response.
pub(crate) async fn process_line(
    server: &Arc<tokio::sync::Mutex<McpServer>>,
    line: &str,
) -> Result<Option<String>, McpError> {
    crate::performance::measure_transport("mcp.process_line", async {
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        let decode = crate::performance::Timer::new("mcp.decode");
        let parsed = serde_json::from_str(line);
        decode.outcome(parsed.is_ok());
        let message: JsonRpcMessage = match parsed {
            Ok(msg) => msg,
            Err(e) => {
                crate::performance::event!(
                    kind = "transport_protocol",
                    transport = "mcp_stdio",
                    outcome = "parse_error"
                );
                // We have no id to respond with, but JSON-RPC still wants a
                // response -- so say so explicitly with a null id rather than
                // fabricating `0` and colliding with a real request.
                let err_resp = JsonRpcResponse::error(
                    JsonRpcId::Null,
                    PARSE_ERROR,
                    format!("Parse error: {e}"),
                );
                return Ok(Some(serde_json::to_string(&err_resp)?));
            }
        };

        crate::performance::event!(
            kind = "transport_operation",
            operation = if message.method == "tools/call"
                && message
                    .params
                    .as_ref()
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    == Some("store_memory")
            {
                "store_memory"
            } else {
                "other"
            }
        );
        let wait = crate::performance::Timer::new("mcp.server_lock.wait");
        let server = server.lock().await;
        wait.complete();
        let mut server = crate::performance::Hold::new("mcp.server_lock.hold", server);
        let dispatch = crate::performance::Timer::new("mcp.dispatch");
        let response = server.handle_message(message).await;
        dispatch.complete();
        let encode = crate::performance::Timer::new("mcp.serialize");
        let result = match response {
            Some(response) => {
                crate::performance::event!(
                    kind = "transport_protocol",
                    transport = "mcp_stdio",
                    protocol_error = response.error.is_some()
                );
                if let Some(tool_error) = response
                    .result
                    .as_ref()
                    .and_then(|r| r.get("isError"))
                    .and_then(|v| v.as_bool())
                {
                    crate::performance::event!(kind = "transport_tool_result", tool_error);
                }
                serde_json::to_string(&response)
                    .map(Some)
                    .map_err(McpError::from)
            }
            None => Ok(None),
        };
        encode.outcome(result.is_ok());
        result
    })
    .await
}

/// Run the MCP server over stdin/stdout.
///
/// Reads one JSON-RPC message per line from stdin, dispatches to
/// `McpServer::handle_message`, and writes the response (if any)
/// as a single line to stdout.
///
/// Returns when stdin is closed (EOF) or an I/O error occurs.
pub async fn run_stdio(server: Arc<tokio::sync::Mutex<McpServer>>) -> Result<(), McpError> {
    serve_io(server, BufReader::new(io::stdin()), io::stdout()).await
}

/// Production stdio loop with injectable I/O for actual flush/error tests.
pub(crate) async fn serve_io<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin>(
    server: Arc<tokio::sync::Mutex<McpServer>>,
    reader: R,
    mut writer: W,
) -> Result<(), McpError> {
    let mut lines = reader.lines();
    tracing::info!("MCP stdio transport started");
    while let Some(line) = lines.next_line().await? {
        crate::performance::measure_transport("mcp.stdio", async {
            // Excludes the delimiter stripped by AsyncBufReadExt::lines.
            crate::performance::event!(
                kind = "transport_input",
                transport = "mcp_stdio",
                source_bytes = line.len()
            );
            let mut timeline = crate::performance::Timeline::new();
            let output = process_line(&server, &line).await?;
            timeline.mark("transport.decode_dispatch_serialize");
            if let Some(json) = output {
                crate::performance::event!(
                    kind = "transport_output",
                    transport = "mcp_stdio",
                    phase = "attempt",
                    source_bytes = json.len() + 1
                );
                let delivery = crate::performance::Timer::new("mcp.write_flush");
                let result = async {
                    writer.write_all(json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await
                }
                .await;
                crate::performance::event!(
                    kind = "transport_delivery",
                    transport = "mcp_stdio",
                    success = result.is_ok()
                );
                delivery.outcome(result.is_ok());
                result?;
                crate::performance::event!(
                    kind = "transport_output",
                    transport = "mcp_stdio",
                    phase = "complete",
                    source_bytes = json.len() + 1
                );
            } else {
                crate::performance::event!(
                    kind = "transport_notification",
                    transport = "mcp_stdio"
                );
            }
            timeline.mark("transport.write_flush");
            Ok::<_, McpError>(())
        })
        .await?;
    }
    tracing::info!("MCP stdio transport closed (stdin EOF)");
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::server::tests::stub_handler;

    fn server() -> Arc<tokio::sync::Mutex<McpServer>> {
        Arc::new(tokio::sync::Mutex::new(McpServer::new(stub_handler())))
    }

    async fn line(server: &Arc<tokio::sync::Mutex<McpServer>>, raw: &str) -> serde_json::Value {
        let out = process_line(server, raw).await.unwrap().unwrap();
        serde_json::from_str(&out).unwrap()
    }

    #[tokio::test]
    async fn parse_error_responds_with_a_null_id() {
        let srv = server();
        let resp = line(&srv, "{not json").await;
        assert_eq!(resp["error"]["code"], PARSE_ERROR);
        assert!(resp["id"].is_null(), "expected null id, got {}", resp["id"]);
    }

    /// A modern client probes `server/discover` first, then works without
    /// ever sending `initialize` -- a stdio process is not a session.
    #[tokio::test]
    async fn modern_client_discovers_then_lists_tools_without_initialize() {
        let srv = server();

        let discover = line(
            &srv,
            r#"{"jsonrpc":"2.0","id":1,"method":"server/discover"}"#,
        )
        .await;
        assert_eq!(
            discover["result"]["supportedVersions"],
            serde_json::json!([PROTOCOL_VERSION])
        );

        let raw = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {
                "_meta": {
                    META_PROTOCOL_VERSION: PROTOCOL_VERSION,
                    META_CLIENT_CAPABILITIES: {},
                }
            }
        })
        .to_string();
        let tools = line(&srv, &raw).await;
        assert_eq!(tools["result"]["tools"][0]["name"], "store_memory");
        assert_eq!(tools["result"]["resultType"], "complete");
        assert!(!srv.lock().await.is_initialized());
    }

    #[tokio::test]
    async fn legacy_client_initializes_then_lists_tools() {
        let srv = server();

        let raw = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "LegacyClient", "version": "1.0.0" }
            }
        })
        .to_string();
        let init = line(&srv, &raw).await;
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert_ne!(init["result"]["protocolVersion"], PROTOCOL_VERSION);

        // The initialized notification produces no output.
        assert!(
            process_line(
                &srv,
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
            )
            .await
            .unwrap()
            .is_none()
        );

        let tools = line(&srv, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).await;
        assert_eq!(tools["result"]["tools"][0]["name"], "store_memory");
    }

    #[tokio::test]
    async fn blank_lines_are_ignored() {
        let srv = server();
        assert!(process_line(&srv, "   ").await.unwrap().is_none());
    }
}
