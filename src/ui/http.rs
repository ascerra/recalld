//! `recalld ui`'s HTTP side: `/api/ui/*` JSON over the daemon socket,
//! `/api/ui/events` (SSE), and the embedded SPA.

use std::convert::Infallible;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::wire::{DaemonState, UiError, UiSession};
use crate::daemon::DaemonClient;
use crate::mcp::bridge::BridgeError;

/// Starts the daemon; supplied by the binary, which owns auto-start.
pub type Starter =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;

#[derive(Clone)]
pub struct UiState {
    pub token: Arc<str>,
    pub port: u16,
    pub daemon: Arc<DaemonPool>,
    pub starter: Option<Starter>,
}

/// A few daemon connections, used round-robin. `DaemonClient` serializes
/// calls per connection; parallel UI reads (search fan-out) need several.
pub struct DaemonPool {
    socket: PathBuf,
    clients: Mutex<Vec<Arc<DaemonClient>>>,
    next: AtomicUsize,
    size: usize,
}

impl DaemonPool {
    pub fn new(socket: PathBuf, size: usize) -> Self {
        Self {
            socket,
            clients: Mutex::new(Vec::new()),
            next: AtomicUsize::new(0),
            size: size.max(1),
        }
    }

    pub fn socket(&self) -> &std::path::Path {
        &self.socket
    }

    async fn client(&self) -> Result<Arc<DaemonClient>, BridgeError> {
        let i = self.next.fetch_add(1, Ordering::Relaxed) % self.size;
        let mut clients = self.clients.lock().await;
        if let Some(client) = clients.get(i) {
            return Ok(client.clone());
        }
        let client = DaemonClient::connect(&self.socket)
            .await
            .map_err(|e| BridgeError::Unavailable(format!("daemon not reachable: {e}")))?;
        client.set_agent("recalld ui", "ui").await;
        client.ping().await?;
        let client = Arc::new(client);
        clients.push(client.clone());
        Ok(client)
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        self.client().await?.call(method, params).await
    }

    /// Run an MCP tool exactly as `recalld mcp` would for an agent, over
    /// one of this pool's connections. Used for the UI's two writes.
    pub async fn tool(&self, name: &str, args: Value) -> Result<Value, BridgeError> {
        use crate::daemon::bridge_adapters::*;
        let client = self.client().await?;
        let bridge = crate::mcp::bridge::McpBridge {
            search: Arc::new(RemoteSearchAdapter::new(client.clone())),
            storage: Arc::new(RemoteStorageAdapter::new(client.clone())),
            namespaces: Arc::new(RemoteNamespaceAdapter::new(client.clone())),
            health: Arc::new(RemoteHealthAdapter::new(client)),
            default_namespace: "default".into(),
            timezone: chrono_tz::UTC,
        };
        let result = crate::mcp::tools::dispatch_tool(&bridge, name, args).await;
        let text = result
            .content
            .first()
            .map(|crate::mcp::protocol::ContentBlock::Text { text }| text.clone())
            .unwrap_or_default();
        let value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
        if result.is_error == Some(true) || value.get("error").is_some() {
            return Err(BridgeError::InvalidInput(
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or(&text)
                    .to_owned(),
            ));
        }
        Ok(value)
    }

    /// Drop connections so the next call reconnects (after a daemon restart).
    pub async fn reset(&self) {
        self.clients.lock().await.clear();
    }
}

pub fn router(state: UiState) -> Router {
    let api = Router::new()
        .route("/session", get(session))
        .route("/namespaces", get(namespaces))
        .route("/graph", get(graph))
        .route("/health", get(health))
        .route("/system", get(system))
        .route("/memories", get(memories))
        .route("/memories/{id}", get(memory))
        .route("/memories/{id}/history", get(history))
        .route("/memories/{id}/neighbors", get(neighbors))
        .route("/memories/{id}/reinforce", post(reinforce))
        .route("/memories/{id}/forget", post(forget))
        .route("/tags", get(tags))
        .route("/search", get(search))
        .route("/activity", get(activity))
        .route("/namespaces/{name}/stats", get(namespace_stats))
        .route("/daemon/start", post(start_daemon))
        .route("/events", get(events))
        .fallback(|| async { error(StatusCode::NOT_FOUND, "notFound", "no such endpoint") })
        .layer(tower_http::compression::CompressionLayer::new().gzip(true));
    Router::new()
        .nest("/api/ui", api)
        .fallback(super::assets::serve)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            super::security::guard,
        ))
        .with_state(state)
}

// ── Responses ──────────────────────────────────────────────────────────

fn error(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    (
        status,
        Json(UiError {
            error: code.into(),
            message: message.into(),
        }),
    )
        .into_response()
}

pub(crate) fn bridge_error(e: BridgeError) -> Response {
    let message = e.to_string();
    match e {
        BridgeError::NotFound(_) => error(StatusCode::NOT_FOUND, "notFound", message),
        BridgeError::InvalidInput(m) if m.starts_with("unknown method") => error(
            StatusCode::NOT_IMPLEMENTED,
            "unsupported",
            "the running daemon was started without UI support; run `recalld daemon stop`, then `recalld ui`",
        ),
        BridgeError::InvalidInput(_) => error(StatusCode::BAD_REQUEST, "invalidInput", message),
        BridgeError::Unavailable(_) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "daemonUnavailable",
            message,
        ),
        BridgeError::Internal(m) if m.contains("connect") || m.contains("connection") => {
            error(StatusCode::SERVICE_UNAVAILABLE, "daemonUnavailable", m)
        }
        _ => error(StatusCode::INTERNAL_SERVER_ERROR, "internal", message),
    }
}

async fn relay(state: &UiState, method: &str, params: Value) -> Response {
    match state.daemon.call(method, params).await {
        Ok(value) => Json(value).into_response(),
        Err(e) => bridge_error(e),
    }
}

// ── Handlers ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scope {
    namespace: Option<String>,
}

async fn session(State(state): State<UiState>) -> Json<UiSession> {
    let daemon = match state.daemon.call("system_status", json!({})).await {
        Ok(_) => DaemonState {
            status: "connected".into(),
            message: None,
        },
        Err(BridgeError::InvalidInput(m)) if m.starts_with("unknown method") => DaemonState {
            status: "unsupported".into(),
            message: Some(
                "the running daemon was started without UI support; run `recalld daemon stop`, then `recalld ui`"
                    .into(),
            ),
        },
        Err(e) => DaemonState {
            status: "unavailable".into(),
            message: Some(e.to_string()),
        },
    };
    Json(UiSession {
        version: env!("CARGO_PKG_VERSION").into(),
        daemon,
        socket: state.daemon.socket().display().to_string(),
    })
}

async fn namespaces(State(state): State<UiState>) -> Response {
    relay(&state, "list_namespaces", Value::Null).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQuery {
    namespace: Option<String>,
    #[serde(default)]
    edge_offset: usize,
}

async fn graph(State(state): State<UiState>, Query(q): Query<GraphQuery>) -> Response {
    relay(
        &state,
        "graph_snapshot",
        json!({ "namespace": q.namespace, "edgeOffset": q.edge_offset }),
    )
    .await
}

async fn health(State(state): State<UiState>, Query(q): Query<Scope>) -> Response {
    relay(&state, "health_report", json!({ "namespace": q.namespace })).await
}

async fn system(State(state): State<UiState>) -> Response {
    relay(&state, "system_status", json!({})).await
}

/// Repeated query keys (`?phase=1&phase=2`) as lists.
fn multi(pairs: Vec<(String, String)>) -> std::collections::HashMap<String, Vec<String>> {
    let mut out: std::collections::HashMap<String, Vec<String>> = Default::default();
    for (k, v) in pairs {
        out.entry(k).or_default().push(v);
    }
    out
}

async fn memories(
    State(state): State<UiState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let q = multi(pairs);
    let one = |k: &str| q.get(k).and_then(|v| v.first()).cloned();
    let list = |k: &str| q.get(k).cloned().unwrap_or_default();
    let phases: Vec<u8> = list("phase")
        .iter()
        .filter_map(|p| p.parse().ok())
        .collect();
    let params = json!({
        "namespace": one("namespace"),
        "ids": list("id"),
        "phases": phases,
        "tags": list("tag"),
        "entities": list("entity"),
        "sort": one("sort"),
        "desc": one("desc").map(|d| d == "true"),
        "offset": one("offset").and_then(|o| o.parse::<usize>().ok()).unwrap_or(0),
        "limit": one("limit").and_then(|l| l.parse::<usize>().ok()),
    });
    relay(&state, "memory_page", params).await
}

/// One memory, read the way an agent's `get_memory` does but with
/// `recordAccess:false` (native stores close the legacy read RPC).
async fn memory(State(state): State<UiState>, Path(id): Path<String>) -> Response {
    match state
        .daemon
        .tool("get_memory", json!({ "id": id, "recordAccess": false }))
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => bridge_error(e),
    }
}

#[derive(Deserialize)]
struct Limit {
    limit: Option<usize>,
    depth: Option<u8>,
}

async fn history(
    State(state): State<UiState>,
    Path(id): Path<String>,
    Query(q): Query<Limit>,
) -> Response {
    relay(
        &state,
        "access_history",
        json!({ "id": id, "limit": q.limit }),
    )
    .await
}

async fn neighbors(
    State(state): State<UiState>,
    Path(id): Path<String>,
    Query(q): Query<Limit>,
) -> Response {
    relay(
        &state,
        "memory_neighbors",
        json!({ "id": id, "depth": q.depth }),
    )
    .await
}

async fn tags(State(state): State<UiState>, Query(q): Query<Scope>) -> Response {
    relay(
        &state,
        "list_tags",
        json!({ "namespace": q.namespace, "limit": 200 }),
    )
    .await
}

#[derive(Deserialize)]
struct ActivityQuery {
    namespace: Option<String>,
    since: Option<i64>,
    limit: Option<usize>,
}

async fn activity(State(state): State<UiState>, Query(q): Query<ActivityQuery>) -> Response {
    relay(
        &state,
        "activity",
        json!({ "namespace": q.namespace, "since": q.since, "limit": q.limit }),
    )
    .await
}

async fn namespace_stats(State(state): State<UiState>, Path(name): Path<String>) -> Response {
    relay(&state, "namespace_stats", json!({ "name": name })).await
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    namespace: Option<String>,
    limit: Option<usize>,
}

/// recalld's hybrid search, read-only. For All namespaces, one search per
/// namespace in parallel, merged by score when their embedding profiles
/// match and grouped by namespace otherwise (SPEC §2.5).
async fn search(State(state): State<UiState>, Query(q): Query<SearchQuery>) -> Response {
    let limit = q.limit.unwrap_or(10).clamp(1, 50);
    let names: Vec<String> = match q.namespace {
        Some(ns) => vec![ns],
        None => match state.daemon.call("list_namespaces", Value::Null).await {
            Ok(list) => serde_json::from_value::<Vec<crate::mcp::bridge::NamespaceInfo>>(list)
                .map(|l| {
                    l.into_iter()
                        .filter(|n| n.memory_count > 0)
                        .map(|n| n.name)
                        .collect()
                })
                .unwrap_or_default(),
            Err(e) => return bridge_error(e),
        },
    };
    let tasks: Vec<_> = names
        .into_iter()
        .map(|ns| {
            let (daemon, query) = (state.daemon.clone(), q.q.clone());
            tokio::spawn(async move {
                daemon
                    .call(
                        "search_explain_read_only",
                        json!({ "query": query, "namespace": ns, "limit": limit }),
                    )
                    .await
            })
        })
        .collect();
    let mut per = Vec::new();
    for task in tasks {
        let result = task
            .await
            .map_err(|e| BridgeError::Internal(e.to_string()))
            .and_then(|r| r)
            .and_then(|v| {
                serde_json::from_value::<super::wire::SearchExplain>(v)
                    .map_err(|e| BridgeError::Internal(e.to_string()))
            });
        match result {
            Ok(s) => per.push(s),
            Err(e) => return bridge_error(e),
        }
    }
    Json(super::memories::merge(per, q.q, limit)).into_response()
}

#[derive(Deserialize)]
struct Write {
    namespace: String,
    rating: Option<u8>,
}

async fn reinforce(
    State(state): State<UiState>,
    Path(id): Path<String>,
    Json(body): Json<Write>,
) -> Response {
    let args =
        json!({ "id": id, "namespace": body.namespace, "quality": body.rating.unwrap_or(3) });
    match state.daemon.tool("reinforce_memory", args).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => bridge_error(e),
    }
}

async fn forget(
    State(state): State<UiState>,
    Path(id): Path<String>,
    Json(body): Json<Write>,
) -> Response {
    let args = json!({ "id": id, "namespace": body.namespace });
    match state.daemon.tool("forget_memory", args).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => bridge_error(e),
    }
}

async fn start_daemon(State(state): State<UiState>) -> Response {
    let Some(start) = state.starter.clone() else {
        return error(
            StatusCode::NOT_IMPLEMENTED,
            "unsupported",
            "start the daemon with `recalld daemon start`",
        );
    };
    match start().await {
        Ok(()) => {
            state.daemon.reset().await;
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => error(StatusCode::SERVICE_UNAVAILABLE, "daemonUnavailable", e),
    }
}

/// `GET /api/ui/events`: the daemon's `subscribe` stream as SSE. Each SSE
/// event is named after the activity kind and carries the JSON payload.
async fn events(State(state): State<UiState>) -> Response {
    let socket = state.daemon.socket().to_path_buf();
    let rx = match subscribe(&socket).await {
        Ok(rx) => rx,
        Err(e) => return bridge_error(e),
    };
    let stream = futures_stream(rx);
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

fn futures_stream(
    rx: tokio::sync::mpsc::Receiver<Value>,
) -> impl futures_core::Stream<Item = Result<Event, Infallible>> {
    ReceiverStream(rx)
}

/// Open a dedicated daemon connection and turn its notifications into a
/// channel. The task ends when either side closes.
async fn subscribe(
    socket: &std::path::Path,
) -> Result<tokio::sync::mpsc::Receiver<Value>, BridgeError> {
    use crate::daemon::protocol::{
        DaemonRequest, MAX_MESSAGE_SIZE, PROTOCOL_VERSION, read_frame, write_framed_request,
    };
    let unavailable = |e: &dyn std::fmt::Display| BridgeError::Unavailable(e.to_string());
    let stream = tokio::net::UnixStream::connect(socket)
        .await
        .map_err(|e| unavailable(&e))?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = tokio::io::BufReader::new(reader);
    let mut limit = crate::daemon::protocol::LEGACY_MAX_MESSAGE_SIZE;
    let calls = [
        (
            "ping",
            json!({"protocolVersion": PROTOCOL_VERSION, "maxMessageSize": MAX_MESSAGE_SIZE}),
        ),
        ("hello", json!({"agent": "recalld ui", "kind": "ui"})),
        ("subscribe", json!({})),
    ];
    for (id, (method, params)) in calls.into_iter().enumerate() {
        let request = DaemonRequest {
            jsonrpc: "2.0".into(),
            id: id as u64 + 1,
            method: method.into(),
            params,
        };
        write_framed_request(&mut writer, &request, limit)
            .await
            .map_err(|e| unavailable(&e))?;
        let frame = read_frame(&mut reader, MAX_MESSAGE_SIZE)
            .await
            .map_err(|e| unavailable(&e))?;
        let reply: Value = serde_json::from_slice(&frame).map_err(|e| unavailable(&e))?;
        if let Some(e) = reply.get("error") {
            let message = e.get("message").and_then(Value::as_str).unwrap_or("error");
            return Err(BridgeError::InvalidInput(message.to_owned()));
        }
        if method == "ping" {
            limit = MAX_MESSAGE_SIZE;
        }
    }
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(async move {
        let _writer = writer; // keep the connection open
        loop {
            let frame = tokio::select! {
                frame = read_frame(&mut reader, MAX_MESSAGE_SIZE) => frame,
                _ = tx.closed() => break,
            };
            let Ok(frame) = frame else { break };
            let Ok(message) = serde_json::from_slice::<Value>(&frame) else {
                continue;
            };
            if message.get("method").and_then(Value::as_str) != Some("event") {
                continue;
            }
            if tx.send(message["params"].clone()).await.is_err() {
                break;
            }
        }
    });
    Ok(rx)
}

/// `Stream` over an mpsc receiver of activity events, as named SSE events.
struct ReceiverStream(tokio::sync::mpsc::Receiver<Value>);

impl futures_core::Stream for ReceiverStream {
    type Item = Result<Event, Infallible>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_recv(cx).map(|event| {
            event.map(|value| {
                let kind = value
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("message")
                    .to_owned();
                Ok(Event::default().event(kind).data(value.to_string()))
            })
        })
    }
}
