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
    extract::{Query, State},
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
