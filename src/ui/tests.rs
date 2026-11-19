//! UI integration tests: a real `Recalld` in a temp dir behind a real
//! `DaemonServer` on a temp socket, and the HTTP security rules.
//! Nothing here touches personal data, the user's daemon socket or backups.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt as _;

use super::wire::*;
use crate::daemon::{DaemonClient, DaemonServer};

pub(crate) const DIM: usize = 16;

/// A daemon over an isolated store. Dropping it stops serving.
pub(crate) struct Daemon {
    pub dir: tempfile::TempDir,
    pub socket: PathBuf,
    pub server: Arc<DaemonServer>,
    accept: tokio::task::JoinHandle<()>,
    stub: tokio::task::JoinHandle<()>,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.accept.abort();
        self.stub.abort();
    }
}

/// Deterministic bag-of-words embedding: texts sharing words score close.
pub(crate) fn embed(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; DIM];
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
    {
        let h = word
            .to_lowercase()
            .bytes()
            .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
        v[h as usize % DIM] += 1.0;
        v[(h >> 8) as usize % DIM] += 0.5;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
    v.iter_mut().for_each(|x| *x /= norm);
    if v.iter().all(|x| *x == 0.0) {
        v[0] = 1.0;
    }
    v
}

/// An Ollama-compatible `/api/embed` on 127.0.0.1, private to one test.
pub(crate) async fn embedding_stub() -> (String, tokio::task::JoinHandle<()>) {
    async fn handler(axum::Json(body): axum::Json<Value>) -> axum::Json<Value> {
        let texts: Vec<String> = match &body["input"] {
            Value::String(s) => vec![s.clone()],
            Value::Array(items) => items
                .iter()
                .filter_map(|i| i.as_str().map(str::to_owned))
                .collect(),
            _ => vec![],
        };
        axum::Json(json!({ "embeddings": texts.iter().map(|t| embed(t)).collect::<Vec<_>>() }))
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().route("/api/embed", axum::routing::post(handler));
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (url, task)
}

pub(crate) fn offline_config(dir: &std::path::Path, embed_url: &str) -> crate::RecalldConfig {
    let mut config = crate::RecalldConfig::default();
    config.storage.data_dir = dir.join("store").to_str().unwrap().to_string();
    config.embedding.provider = crate::config::EmbeddingProvider::Ollama;
    config.embedding.model_name = "ui-test-embed".into();
    config.embedding.dimensions = DIM;
    config.embedding.base_url = embed_url.into();
    // Never read a personal credential even by accident.
    config.embedding.api_key_env = "RECALLD_UI_TEST_UNUSED_KEY".into();
    config.decay.disable_sweep = true;
    config
}

impl Daemon {
    pub async fn start() -> Self {
        Self::start_with(|_| {}).await
    }

    pub async fn start_with(adjust: impl FnOnce(&mut crate::RecalldConfig)) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (url, stub) = embedding_stub().await;
        let mut config = offline_config(dir.path(), &url);
        adjust(&mut config);
        let system = crate::Recalld::new(config).await.unwrap();
        let server = Arc::new(DaemonServer::new(system));
        let socket = dir.path().join("d.sock");
        server.ui_service().set_socket(&socket);
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let accept = {
            let server = server.clone();
            tokio::spawn(async move {
                while let Ok((stream, _)) = listener.accept().await {
                    server.serve_stream(stream);
                }
            })
        };
        Self {
            dir,
            socket,
            server,
            accept,
            stub,
        }
    }

    pub async fn client(&self) -> Arc<DaemonClient> {
        let client = DaemonClient::connect(&self.socket).await.unwrap();
        client.ping().await.unwrap();
        Arc::new(client)
    }

    /// A client named like an MCP agent.
    pub async fn agent(&self, name: &str) -> Arc<DaemonClient> {
        let client = DaemonClient::connect(&self.socket).await.unwrap();
        client.set_agent(name, "mcp").await;
        client.ping().await.unwrap();
        Arc::new(client)
    }

    /// Store a memory through the same remote adapter `recalld mcp` uses.
    pub async fn store(&self, client: &Arc<DaemonClient>, summary: &str) -> String {
        self.store_in(client, summary, "default").await
    }

    pub async fn store_in(
        &self,
        client: &Arc<DaemonClient>,
        summary: &str,
        namespace: &str,
    ) -> String {
        let result = self
            .tool(
                client,
                "store_memory",
                json!({
                    "summary": summary,
                    "fullText": format!("{summary}, in full."),
                    "tags": ["topic/ui-test"],
                    "entities": ["Recalld"],
                    "namespace": namespace,
                    "checkDuplicates": false,
                }),
            )
            .await;
        result["id"]
            .as_str()
            .unwrap_or_else(|| panic!("store failed: {result}"))
            .to_owned()
    }

    /// Call an MCP tool exactly as `recalld mcp` would, over `client`.
    pub async fn tool(&self, client: &Arc<DaemonClient>, name: &str, args: Value) -> Value {
        use crate::daemon::bridge_adapters::*;
        let bridge = crate::mcp::bridge::McpBridge {
            search: Arc::new(RemoteSearchAdapter::new(client.clone())),
            storage: Arc::new(RemoteStorageAdapter::new(client.clone())),
            namespaces: Arc::new(RemoteNamespaceAdapter::new(client.clone())),
            health: Arc::new(RemoteHealthAdapter::new(client.clone())),
            default_namespace: "default".into(),
            timezone: chrono_tz::UTC,
        };
        let result = crate::mcp::tools::dispatch_tool(&bridge, name, args).await;
        let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone()))
    }
}

fn decode_b64(s: &str) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(s).unwrap()
}

// ── Contract tests: wire shapes of every UI-used daemon method ─────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_graph_snapshot() {
    let d = Daemon::start().await;
    let c = d.client().await;
    let a = d.store(&c, "Alpha decision").await;
    let b = d.store(&c, "Beta decision").await;

    let value = c.call("graph_snapshot", json!({})).await.unwrap();
    let snap: GraphSnapshot = serde_json::from_value(value).unwrap();
    assert_eq!(snap.node_count, 2);
    assert_eq!(snap.edge_offset, 0);
    assert_eq!(snap.next_edge_offset, None);
    assert!(snap.namespaces.iter().any(|n| n.name == "default"));
    let nodes = snap.nodes.expect("first page carries nodes");
    let mut ids = nodes.ids.clone();
    ids.sort();
    let mut expected = vec![a.clone(), b.clone()];
    expected.sort();
    assert_eq!(ids, expected);
    let n = nodes.ids.len();
    assert_eq!(decode_b64(&nodes.ns).len(), 4 * n);
    assert_eq!(
        decode_b64(&nodes.phase),
        vec![1u8; n],
        "fresh memories are Full"
    );
    assert_eq!(decode_b64(&nodes.strength).len(), 4 * n);
    assert_eq!(decode_b64(&nodes.stability).len(), 4 * n);
    assert_eq!(decode_b64(&nodes.created_at).len(), 8 * n);
    assert_eq!(decode_b64(&nodes.last_access).len(), 8 * n);
    assert_eq!(decode_b64(&nodes.degree).len(), 4 * n);
    assert_eq!(decode_b64(&nodes.permastore).len(), n);
    let e = snap.edge_total;
    assert_eq!(decode_b64(&snap.edges.src).len(), 4 * e);
    assert_eq!(decode_b64(&snap.edges.kind).len(), e);
    assert_eq!(decode_b64(&snap.edges.weight).len(), 4 * e);

    // Paging: an edge page past the first carries no node columns.
    let page: GraphSnapshot = serde_json::from_value(
        c.call(
            "graph_snapshot",
            json!({"edgeOffset": e.max(1), "namespace": "default"}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(page.nodes.is_none());

    let missing = c
        .call("graph_snapshot", json!({"namespace": "no-such"}))
        .await;
    assert!(missing.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_health_report_matches_http_report() {
    let d = Daemon::start().await;
    let c = d.client().await;
    d.store(&c, "Gamma").await;
    let via_daemon = c.call("health_report", json!({})).await.unwrap();
    for key in [
        "scope",
        "overview",
        "decayForecast",
        "atRisk",
        "ageDistribution",
        "storage",
        "metadata",
    ] {
        assert!(via_daemon.get(key).is_some(), "missing {key}");
    }
    assert_eq!(via_daemon["scope"], "all");
    assert_eq!(via_daemon["overview"]["totalMemories"], 1);
    let scoped = c
        .call("health_report", json!({"namespace": "default"}))
        .await
        .unwrap();
    assert_eq!(scoped["scope"], "default");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_system_status_and_clients() {
    let d = Daemon::start().await;
    let c = d.agent("claude-code").await;
    let status: SystemStatus =
        serde_json::from_value(c.call("system_status", json!({})).await.unwrap()).unwrap();
    assert_eq!(status.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(status.socket, d.socket.display().to_string());
    assert!(status.process.rss_bytes.is_some_and(|b| b > 0));
    assert!(status.process.cpu_percent.is_some());
    assert!(
        status
            .clients
            .iter()
            .any(|c| c.agent.as_deref() == Some("claude-code") && c.kind == "mcp")
    );
    assert_eq!(status.embedding.provider, "ollama");
    assert_eq!(status.embedding.model, "ui-test-embed");
    assert_eq!(status.embedding.dimensions, DIM);
    assert!(!status.sweep.enabled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_existing_methods_the_ui_uses() {
    use crate::mcp::bridge::{NamespaceInfo, NamespaceStats};
    let d = Daemon::start().await;
    let c = d.client().await;
    d.store(&c, "Delta").await;
    let list: Vec<NamespaceInfo> =
        serde_json::from_value(c.call("list_namespaces", Value::Null).await.unwrap()).unwrap();
    let default = list.iter().find(|n| n.name == "default").unwrap();
    assert_eq!(default.memory_count, 1);
    let _: NamespaceStats = serde_json::from_value(
        c.call("namespace_stats", json!({"name": "default"}))
            .await
            .unwrap(),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_streams_store_events_with_agent() {
    use crate::daemon::protocol::{
        DaemonRequest, MAX_MESSAGE_SIZE, read_frame, write_framed_request,
    };
    let d = Daemon::start().await;

    let stream = tokio::net::UnixStream::connect(&d.socket).await.unwrap();
    let (r, mut w) = stream.into_split();
    let mut r = tokio::io::BufReader::new(r);
    let req = DaemonRequest {
        jsonrpc: "2.0".into(),
        id: 1,
        method: "subscribe".into(),
        params: json!({"kinds": ["store"]}),
    };
    write_framed_request(&mut w, &req, MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    let reply: Value =
        serde_json::from_slice(&read_frame(&mut r, MAX_MESSAGE_SIZE).await.unwrap()).unwrap();
    assert_eq!(reply["result"]["subscribed"], json!(["store"]));

    let c = d.agent("test-agent").await;
    let id = d.store(&c, "Epsilon").await;

    let event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let frame = read_frame(&mut r, MAX_MESSAGE_SIZE).await.unwrap();
            let message: Value = serde_json::from_slice(&frame).unwrap();
            if message["method"] == "event" && message["params"]["id"] == id.as_str() {
                return message["params"].clone();
            }
        }
    })
    .await
    .expect("store event within 5s");
    assert_eq!(event["kind"], "store");
    assert_eq!(event["agent"], "test-agent");
    assert_eq!(event["phase"], 1);
}

// ── HTTP security (SPEC §1.1), one test per rule ───────────────────────

fn state(socket: PathBuf) -> super::UiState {
    super::UiState {
        token: "a".repeat(64).into(),
        port: 7690,
        daemon: Arc::new(super::DaemonPool::new(socket, 1)),
        starter: None,
    }
}

fn request(method: &str, uri: &str, host: Option<&str>, cookie: bool) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(host) = host {
        b = b.header(header::HOST, host);
    }
    if cookie {
        b = b.header(
            header::COOKIE,
            format!("recalld_ui_7690={}", "a".repeat(64)),
        );
    }
    b.body(Body::empty()).unwrap()
}

async fn send(req: Request<Body>) -> axum::response::Response {
    super::http::router(state(PathBuf::from("/nonexistent/recalld-ui-test.sock")))
        .oneshot(req)
        .await
        .unwrap()
}

#[tokio::test]
async fn security_api_requires_the_session_cookie() {
    let res = send(request(
        "GET",
        "/api/ui/namespaces",
        Some("127.0.0.1:7690"),
        false,
    ))
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let mut wrong = request("GET", "/api/ui/namespaces", Some("127.0.0.1:7690"), false);
    wrong.headers_mut().insert(
        header::COOKIE,
        format!("recalld_ui_7690={}", "b".repeat(64))
            .parse()
            .unwrap(),
    );
    assert_eq!(send(wrong).await.status(), StatusCode::FORBIDDEN);
    // With the cookie the request reaches the handler (no daemon here → 503).
    let ok = send(request(
        "GET",
        "/api/ui/namespaces",
        Some("127.0.0.1:7690"),
        true,
    ))
    .await;
    assert_eq!(ok.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn security_rejects_foreign_or_missing_host() {
    for host in [Some("evil.example:7690"), Some("127.0.0.1:80"), None] {
        let api = send(request("GET", "/api/ui/namespaces", host, true)).await;
        assert_eq!(api.status(), StatusCode::FORBIDDEN, "{host:?}");
        let page = send(request("GET", "/", host, true)).await;
        assert_eq!(page.status(), StatusCode::FORBIDDEN, "{host:?}");
    }
    let localhost = send(request(
        "GET",
        "/api/ui/namespaces",
        Some("localhost:7690"),
        true,
    ))
    .await;
    assert_ne!(localhost.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn security_mutations_need_the_ui_header() {
    let res = send(request(
        "POST",
        "/api/ui/daemon/start",
        Some("127.0.0.1:7690"),
        true,
    ))
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let mut with = request("POST", "/api/ui/daemon/start", Some("127.0.0.1:7690"), true);
    with.headers_mut()
        .insert(super::MUTATION_HEADER, "1".parse().unwrap());
    // Passes security; this state has no starter.
    assert_eq!(send(with).await.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn security_no_cors() {
    let mut pre = request(
        "OPTIONS",
        "/api/ui/namespaces",
        Some("127.0.0.1:7690"),
        true,
    );
    pre.headers_mut()
        .insert(header::ORIGIN, "http://evil.example".parse().unwrap());
    pre.headers_mut().insert(
        header::ACCESS_CONTROL_REQUEST_METHOD,
        "POST".parse().unwrap(),
    );
    let res = send(pre).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let mut get = request("GET", "/api/ui/namespaces", Some("127.0.0.1:7690"), true);
    get.headers_mut()
        .insert(header::ORIGIN, "http://evil.example".parse().unwrap());
    for res in [res, send(get).await] {
        assert!(
            res.headers()
                .keys()
                .all(|k| !k.as_str().starts_with("access-control-")),
            "{:?}",
            res.headers()
        );
    }
}

#[tokio::test]
async fn security_token_is_exchanged_for_a_strict_cookie() {
    let token = "a".repeat(64);
    let res = send(request(
        "GET",
        &format!("/map?t={token}"),
        Some("127.0.0.1:7690"),
        false,
    ))
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/map");
    let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.starts_with(&format!("recalld_ui_7690={token}")));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    let bad = send(request("GET", "/?t=nope", Some("127.0.0.1:7690"), false)).await;
    assert_eq!(bad.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn security_binds_loopback_only() {
    let bound = super::BoundUi::bind(&super::UiOptions {
        port: None,
        open_browser: false,
        socket: PathBuf::from("/nonexistent/recalld-ui-test.sock"),
        starter: None,
    })
    .await
    .unwrap();
    assert!(bound.listener.local_addr().unwrap().ip().is_loopback());
    assert!(bound.url().starts_with("http://127.0.0.1:"));
    assert!(bound.url().contains(&format!("?t={}", bound.token())));
}
