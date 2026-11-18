use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Duration;

use tokio::io::BufReader;
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

use super::protocol::{
    DaemonRequest, FrameError, LEGACY_MAX_MESSAGE_SIZE, MAX_MESSAGE_SIZE, PROTOCOL_VERSION,
    read_framed_response, write_framed_request,
};
use crate::mcp::bridge::BridgeError;

/// Connect attempts made before a reconnect is declared hopeless.
const RECONNECT_ATTEMPTS: usize = 3;
/// Backoff between reconnect attempts. The final entry is never slept on.
const RECONNECT_BACKOFF_MS: [u64; RECONNECT_ATTEMPTS] = [50, 200, 500];
/// How many stale (already-abandoned) responses may be skipped before the
/// stream is treated as corrupt.
const MAX_STALE_SKIPS: usize = 8;

/// Whether a method may be replayed on a fresh connection after the original
/// attempt failed with an unknown outcome.
///
/// The bar is **no observable state change of any kind**, not "not a write".
/// Access and decay bookkeeping counts: recording an access advances
/// `last_accessed_at` and appends an `AccessEvent` that feeds the FSRS
/// schedule, so a transparent replay would silently age a memory twice off
/// one caller request. That is what keeps `search` and `find_similar` off
/// this list even though neither creates or edits a memory — both record
/// accesses for every result they return (`SearchPipeline::search`,
/// `SearchPipeline::similar`).
///
/// The methods that remain are pure reads of storage, the namespace table,
/// or process health. Ordinary `get_memory` records exposure too; only its
/// explicit `get_memory_read_only` alias is eligible for automatic replay.
///
/// Mutating methods are excluded for the usual reasons: they mint new
/// server-side state (`store_memory` allocates a fresh id, so a replay
/// silently creates a duplicate the caller can never reconcile), advance
/// state on each call (`reinforce_memory` moves the FSRS schedule), or have
/// a return value that is not idempotent even where the effect is
/// (`delete_memory` returns whether the memory existed).
///
/// The destructive methods are excluded for a sharper reason than
/// "mutating". `delete_memories` returns a per-id flag vector whose
/// values all flip to false on a replay, so a caller that got no answer
/// the first time cannot distinguish "nothing was deleted" from
/// "everything was deleted, twice". `delete_namespace` is worse: it
/// removes a directory from disk, and a replay after an unknown outcome
/// could destroy a namespace of the same name that the user recreated in
/// between. Neither belongs on this list, and neither belongs on
/// `records_accesses_only` — the honest report for a destructive call
/// with an unknown outcome is the "may or may not have been applied"
/// warning.
fn is_retry_safe(method: &str) -> bool {
    matches!(
        method,
        "ping"
            | "session_context_v1"
            | "check_health"
            | "scan_duplicates"
            | "search_read_only"
            | "get_memory_read_only"
            | "find_similar_read_only"
            | "list_namespaces"
            | "namespace_stats"
            | "list_memories"
            | "list_tags"
    )
}

/// Whether a method is barred from the automatic replay *only* because it
/// records accesses.
///
/// These write no memory content, so the honest report to the caller is
/// "ask again", not the "may or may not have been applied" warning that a
/// half-applied mutation deserves.
fn records_accesses_only(method: &str) -> bool {
    matches!(method, "search" | "find_similar")
}

/// An established socket connection to the daemon.
struct Connection {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

impl Connection {
    fn new(stream: UnixStream) -> Self {
        let (reader, writer) = stream.into_split();
        Self {
            reader: BufReader::new(reader),
            writer,
        }
    }
}

/// Mutable client state, serialized by the client's mutex.
struct ClientState {
    /// `None` once the connection has been poisoned; re-established lazily.
    conn: Option<Connection>,
    /// Monotonic *across reconnects*, so a frame left over from a dropped
    /// connection can never alias the id of a live call.
    next_id: u64,
    /// Largest frame the daemon is known to accept. Held at the protocol v1
    /// limit until a `ping` negotiates something larger.
    frame_limit: usize,
    api1_lifecycle: Option<bool>,
    api1_store: Option<bool>,
    api1_store_batch: Option<bool>,
    prepared_queries: Option<bool>,
    prepared_get: Option<bool>,
    api1_get: Option<bool>,
    exact_pairs_bits: bool,
    canonical_pages: bool,
}

#[derive(Clone, Copy)]
enum Api1Capability {
    Lifecycle,
    Store,
    StoreBatch,
}
impl Api1Capability {
    fn supported(self, state: &ClientState) -> Option<bool> {
        match self {
            Self::Lifecycle => state.api1_lifecycle,
            Self::Store => state.api1_store,
            Self::StoreBatch => state.api1_store_batch,
        }
    }
}

/// Attribution is independent of connection health. In particular a drained
/// malformed frame leaves a usable connection but cannot settle a sent write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransmissionPhase {
    BeforeSend,
    SentUnattributed,
    MatchedReply,
}
pub(crate) struct PreparedExchange {
    pub(crate) phase: TransmissionPhase,
    /// A fully admitted response result; no typed receipt has been acquired.
    pub(crate) result: Option<serde_json::Value>,
}

pub(crate) enum Api1GetExchange {
    Prepared(PreparedExchange),
    Compatibility(Result<serde_json::Value, BridgeError>),
}
fn api1_get_capability(value: &serde_json::Value) -> Result<bool, BridgeError> {
    if !value.is_object() {
        return Err(BridgeError::Unavailable(
            "malformed get capability handshake".into(),
        ));
    }
    match value.get("api1GetV1") {
        None => Ok(false),
        Some(serde_json::Value::Bool(value)) => Ok(*value),
        _ => Err(BridgeError::Unavailable(
            "malformed api1GetV1 capability".into(),
        )),
    }
}

/// A failed exchange, plus whether the connection survived it. Existing callers
/// retain their old `lost` behavior; new prepared callers use typed attribution.
struct ExchangeError {
    error: BridgeError,
    phase: TransmissionPhase,
    /// The connection was poisoned and must be re-established. When set, the
    /// request may or may not have been applied by the daemon: neither a
    /// failed write nor a failed read proves the daemon did not act.
    lost: bool,
}

impl ExchangeError {
    /// The call failed but the connection is still usable and the outcome is
    /// known (nothing was sent, or the daemon answered definitively).
    fn settled(error: BridgeError) -> Self {
        Self {
            error,
            lost: false,
            phase: TransmissionPhase::BeforeSend,
        }
    }

    /// The connection was poisoned; the outcome of the call is unknown.
    fn lost(error: BridgeError) -> Self {
        Self {
            error,
            lost: true,
            phase: TransmissionPhase::SentUnattributed,
        }
    }

    fn unattributed(error: BridgeError) -> Self {
        Self {
            error,
            lost: false,
            phase: TransmissionPhase::SentUnattributed,
        }
    }

    fn matched(error: BridgeError) -> Self {
        Self {
            error,
            lost: false,
            phase: TransmissionPhase::MatchedReply,
        }
    }
}

/// Client for communicating with the Recalld daemon over a Unix socket.
///
/// The connection is self-healing: a transport failure poisons it, and the
/// next call transparently re-establishes it. An oversized request is
/// rejected before a single byte reaches the socket, so it cannot corrupt
/// the stream for subsequent calls.
pub struct DaemonClient {
    #[cfg(test)]
    api1_get_pause: std::sync::Mutex<
        Option<(
            std::sync::Arc<tokio::sync::Notify>,
            std::sync::Arc<tokio::sync::Notify>,
        )>,
    >,
    socket_path: PathBuf,
    state: Mutex<ClientState>,
    /// Mirrors `state.conn.is_some()` so [`DaemonClient::is_connected`] can
    /// stay synchronous.
    connected: AtomicBool,
}

impl DaemonClient {
    /// Prepared-only single exchange, without reconnect/replay after send. The
    /// shared mutex covers negotiation and the entire attributed response.
    pub(crate) async fn call_api1_get(
        &self,
        params: serde_json::Value,
        request: &crate::serialization::get_api1::Request,
    ) -> Api1GetExchange {
        let mut state = self.state.lock().await;
        let ready = async {
            if state.conn.is_none() {
                self.reconnect(&mut state).await?;
            }
            if state.api1_get.is_none() {
                state.negotiate().await?;
            }
            Ok::<_, BridgeError>(state.api1_get == Some(true))
        }
        .await;
        match ready {
            Err(_) => Api1GetExchange::Prepared(PreparedExchange {
                phase: TransmissionPhase::BeforeSend,
                result: None,
            }),
            Ok(true) => {
                // The attributed exchange rechecks capability under its mutex.
                // A reconnect/downgrade here fails closed, never changes lanes.
                drop(state);
                #[cfg(test)]
                {
                    let pause = self.api1_get_pause.lock().unwrap().take();
                    if let Some((entered, release)) = pause {
                        entered.notify_one();
                        release.notified().await;
                    }
                }
                Api1GetExchange::Prepared(self.call_prepared(super::api1_get::GET, params).await)
            }
            Ok(false) => {
                let method = if request.record_access {
                    "get_memory"
                } else {
                    "get_memory_read_only"
                };
                let mut old = serde_json::json!({"id":request.id.to_string(),"recordAccess":request.record_access});
                let method = if let Some(namespace) = &request.namespace {
                    old["namespace"] = serde_json::json!(namespace);
                    old.as_object_mut().unwrap().remove("recordAccess");
                    if request.record_access {
                        "get_memory_scoped".to_owned()
                    } else {
                        "get_memory_scoped_read_only".to_owned()
                    }
                } else {
                    method.to_owned()
                };
                let result = self.call_locked(&mut state, &method, old).await;
                self.connected
                    .store(state.conn.is_some(), AtomicOrdering::Relaxed);
                Api1GetExchange::Compatibility(result)
            }
        }
    }

    pub(crate) async fn call_prepared(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> PreparedExchange {
        let mut state = self.state.lock().await;
        let api1 = method == super::api1_get::GET;
        let point = method == super::prepared_get::GET;
        let limit = if api1 {
            super::api1_get::response_limit()
        } else if point {
            super::prepared_get::response_limit()
        } else {
            super::prepared_query::response_limit()
        };
        let outcome = async {
            if state.conn.is_none() && self.reconnect(&mut state).await.is_err() {
                return PreparedExchange {
                    phase: TransmissionPhase::BeforeSend,
                    result: None,
                };
            }
            if (if api1 {
                state.api1_get
            } else if point {
                state.prepared_get
            } else {
                state.prepared_queries
            })
            .is_none()
                && state.negotiate().await.is_err()
            {
                return PreparedExchange {
                    phase: TransmissionPhase::BeforeSend,
                    result: None,
                };
            }
            let supported = match method {
                super::api1_get::GET => state.api1_get == Some(true),
                super::prepared_get::GET => state.prepared_get == Some(true),
                super::prepared_pages::LIST | super::prepared_pages::CATALOG => {
                    state.canonical_pages
                }
                super::prepared_query::PAIRS => {
                    state.prepared_queries == Some(true) && state.exact_pairs_bits
                }
                super::prepared_query::QUERY | super::prepared_query::SIMILAR => {
                    state.prepared_queries == Some(true)
                }
                _ => false,
            };
            if !supported || state.frame_limit < MAX_MESSAGE_SIZE {
                return PreparedExchange {
                    phase: TransmissionPhase::BeforeSend,
                    result: None,
                };
            }
            let mut request = DaemonRequest {
                jsonrpc: "2.0".into(),
                id: 0,
                method: method.into(),
                params,
            };
            let id = match state.send_request(&mut request).await {
                Ok(id) => id,
                Err(error) => {
                    return PreparedExchange {
                        phase: error.phase,
                        result: None,
                    };
                }
            };
            for _ in 0..=MAX_STALE_SKIPS {
                let frame = match state.conn.as_mut() {
                    Some(conn) => super::protocol::read_frame(&mut conn.reader, limit).await,
                    None => {
                        return PreparedExchange {
                            phase: TransmissionPhase::SentUnattributed,
                            result: None,
                        };
                    }
                };
                let frame = match frame {
                    Ok(frame) => frame,
                    Err(FrameError::OversizedDiscarded { .. } | FrameError::Malformed(_)) => {
                        return PreparedExchange {
                            phase: TransmissionPhase::SentUnattributed,
                            result: None,
                        };
                    }
                    Err(_) => {
                        state.conn = None;
                        return PreparedExchange {
                            phase: TransmissionPhase::SentUnattributed,
                            result: None,
                        };
                    }
                };
                if !(if api1 {
                    super::api1_get::admitted_frame(&frame)
                } else if point {
                    super::prepared_get::admitted_frame(&frame)
                } else {
                    super::prepared_query::admitted_frame(&frame)
                }) {
                    return PreparedExchange {
                        phase: TransmissionPhase::SentUnattributed,
                        result: None,
                    };
                }
                // The bounded raw structural/scalar pass above precedes this
                // owned tree, including all malformed-reply evidence paths.
                let response: serde_json::Value = match serde_json::from_slice(&frame) {
                    Ok(value) => value,
                    Err(_) => {
                        return PreparedExchange {
                            phase: TransmissionPhase::SentUnattributed,
                            result: None,
                        };
                    }
                };
                if let Some(old) = response.get("id").and_then(serde_json::Value::as_u64) {
                    if old != 0 && old < id {
                        continue;
                    }
                }
                let correlated = response.get("id").and_then(serde_json::Value::as_u64) == Some(id)
                    && response.get("jsonrpc").and_then(serde_json::Value::as_str) == Some("2.0")
                    && response.as_object().is_some_and(|o| {
                        o.keys()
                            .all(|k| matches!(k.as_str(), "jsonrpc" | "id" | "result" | "error"))
                    })
                    && response.get("error").is_none()
                    && response.get("result").is_some();
                let result = response.get("result").cloned();
                if response
                    .get("id")
                    .and_then(serde_json::Value::as_u64)
                    .is_some_and(|other| other > id)
                {
                    state.conn = None;
                }
                return PreparedExchange {
                    phase: if correlated {
                        TransmissionPhase::MatchedReply
                    } else {
                        TransmissionPhase::SentUnattributed
                    },
                    result,
                };
            }
            state.conn = None;
            PreparedExchange {
                phase: TransmissionPhase::SentUnattributed,
                result: None,
            }
        }
        .await;
        self.connected
            .store(state.conn.is_some(), AtomicOrdering::Relaxed);
        outcome
    }
    /// Connects to the daemon at the given Unix socket path.
    ///
    /// The frame limit starts at the conservative protocol v1 value; call
    /// [`DaemonClient::ping`] to negotiate a larger one.
    pub async fn connect(socket_path: &Path) -> std::io::Result<Self> {
        let stream = UnixStream::connect(socket_path).await?;
        Ok(Self {
            #[cfg(test)]
            api1_get_pause: std::sync::Mutex::new(None),
            socket_path: socket_path.to_path_buf(),
            state: Mutex::new(ClientState {
                conn: Some(Connection::new(stream)),
                next_id: 1,
                frame_limit: LEGACY_MAX_MESSAGE_SIZE,
                api1_lifecycle: None,
                api1_store: None,
                api1_store_batch: None,
                prepared_queries: None,
                prepared_get: None,
                api1_get: None,
                exact_pairs_bits: false,
                canonical_pages: false,
            }),
            connected: AtomicBool::new(true),
        })
    }

    /// Whether the client currently holds a live connection.
    ///
    /// `false` only means the next call will reconnect first, not that the
    /// daemon is gone.
    pub fn is_connected(&self) -> bool {
        self.connected.load(AtomicOrdering::Relaxed)
    }

    /// Sends a request and awaits its response.
    ///
    /// The mutex is held for the whole exchange, so calls are serialized on
    /// the connection.
    ///
    /// **Not cancel-safe.** Dropping the returned future between the write
    /// and the read abandons a response that a later call must then skip;
    /// the client tolerates a bounded number of such strays but does not
    /// treat cancellation as a supported pattern.
    pub async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, BridgeError> {
        let mut state = self.state.lock().await;
        let outcome = self.call_locked(&mut state, method, params).await;
        self.connected
            .store(state.conn.is_some(), AtomicOrdering::Relaxed);
        outcome
    }

    /// Choose the old/new API1 mutation route on the negotiated connection,
    /// before writing. A reconnect after a lost mutation never retries it.
    pub(crate) async fn call_api1_lifecycle(
        &self,
        new_method: &str,
        new_params: serde_json::Value,
        old_method: &str,
        old_params: serde_json::Value,
    ) -> Result<(bool, serde_json::Value), BridgeError> {
        self.call_api1_mutation(
            Api1Capability::Lifecycle,
            new_method,
            new_params,
            Some((old_method, old_params)),
        )
        .await
    }

    pub(crate) async fn call_api1_store(
        &self,
        params: serde_json::Value,
        legacy_params: Option<serde_json::Value>,
    ) -> Result<(bool, serde_json::Value), BridgeError> {
        self.call_api1_mutation(
            Api1Capability::Store,
            "store_api1_v1",
            params,
            legacy_params.map(|p| ("store_memory", p)),
        )
        .await
    }

    pub(crate) async fn call_api1_store_batch(
        &self,
        params: serde_json::Value,
        legacy_params: Option<serde_json::Value>,
    ) -> Result<(bool, serde_json::Value), BridgeError> {
        self.call_api1_mutation(
            Api1Capability::StoreBatch,
            "store_batch_api1_v1",
            params,
            legacy_params.map(|v| ("mutate_batch", v)),
        )
        .await
    }

    async fn call_api1_mutation(
        &self,
        capability: Api1Capability,
        new_method: &str,
        new_params: serde_json::Value,
        legacy: Option<(&str, serde_json::Value)>,
    ) -> Result<(bool, serde_json::Value), BridgeError> {
        let mut state = self.state.lock().await;
        let result = async {
            if state.conn.is_none() {
                self.reconnect(&mut state).await?;
            }
            if capability.supported(&state).is_none() {
                if let Err(error) = state.negotiate().await {
                    if state.conn.is_none() {
                        self.reconnect(&mut state).await?;
                    } else {
                        return Err(error);
                    }
                }
            }
            let capable = capability.supported(&state) == Some(true);
            if capable && state.frame_limit < MAX_MESSAGE_SIZE {
                return Err(BridgeError::TooLarge(
                    "API1 mutation requires negotiated full receipt frames before work".into(),
                ));
            }
            let (method, params) = if capable {
                (new_method, new_params)
            } else {
                legacy.ok_or_else(|| {
                    BridgeError::InvalidInput(
                        "API1 store constraint requires a capable daemon; no mutation was sent"
                            .into(),
                    )
                })?
            };
            let value = self.call_locked(&mut state, method, params).await?;
            Ok((capable, value))
        }
        .await;
        self.connected
            .store(state.conn.is_some(), AtomicOrdering::Relaxed);
        result
    }

    async fn call_locked(
        &self,
        state: &mut ClientState,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, BridgeError> {
        if state.conn.is_none() {
            self.reconnect(state).await?;
        }

        let mut request = DaemonRequest {
            jsonrpc: "2.0".into(),
            id: 0,
            method: method.into(),
            params,
        };

        let failure = match state.exchange(&mut request).await {
            Ok(result) => return Ok(result),
            Err(e) => e,
        };

        // The connection is intact: the answer, good or bad, is final.
        if !failure.lost {
            return Err(failure.error);
        }

        let reconnected = self.reconnect(state).await;

        if !is_retry_safe(method) {
            let mut message = format!(
                "daemon connection lost while awaiting the response to `{method}` ({}); ",
                failure.error
            );
            // A read that is withheld from the automatic replay only because
            // it records accesses must not be described as a write that may
            // have half-happened: the caller can simply ask again.
            if records_accesses_only(method) {
                message.push_str(
                    "no memory was created, changed, or deleted, and reissuing the call is safe.",
                );
                if reconnected.is_ok() {
                    message.push_str(" The connection has been re-established.");
                } else {
                    message.push_str(" The daemon is currently unreachable.");
                }
            } else {
                message.push_str("the operation may or may not have been applied.");
                if reconnected.is_ok() {
                    message.push_str(
                        " The connection has been re-established - verify with recall_memories \
                         before retrying.",
                    );
                } else {
                    message.push_str(
                        " The daemon is currently unreachable - verify with recall_memories \
                         once it is back before retrying.",
                    );
                }
            }
            return Err(BridgeError::Internal(message));
        }

        reconnected?;
        state.exchange(&mut request).await.map_err(|e| e.error)
    }

    /// Sends a ping to verify the daemon is responsive and negotiate the
    /// frame limit for this connection.
    ///
    /// The mutex is held across the whole exchange *and* the write that
    /// records the answer. Releasing it in between would make this a
    /// read-modify-write on shared state: a reconnect landing in the gap
    /// negotiates its own (possibly legacy 1 MiB) limit, and this method
    /// would then stamp a stale 8 MiB over it. The client would go on to
    /// send frames a v1 daemon rejects *without draining*, permanently
    /// desynchronizing the stream — the precise failure the negotiation
    /// exists to prevent.
    pub async fn ping(&self) -> Result<(), BridgeError> {
        let mut state = self.state.lock().await;
        let outcome = self
            .call_locked(&mut state, "ping", handshake_params())
            .await;
        self.connected
            .store(state.conn.is_some(), AtomicOrdering::Relaxed);
        // The answer describes the connection the lock is still protecting.
        let outcome = outcome?;
        state.api1_get = None;
        state.api1_get = Some(api1_get_capability(&outcome)?);
        state.frame_limit = negotiated_limit(&outcome);
        state.prepared_get = Some(
            outcome
                .get("preparedGetV1")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
        );
        state.api1_lifecycle = Some(
            outcome
                .get("api1LifecycleV1")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
        );
        state.api1_store_batch = Some(
            outcome
                .get("api1StoreBatchV1")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        );
        state.api1_store = Some(
            outcome
                .get("api1StoreV1")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
        );
        Ok(())
    }

    /// Re-establishes the connection, negotiating the frame limit on the way.
    ///
    /// Bounded: at most [`RECONNECT_ATTEMPTS`] attempts with a fixed backoff,
    /// so this can never spin.
    async fn reconnect(&self, state: &mut ClientState) -> Result<(), BridgeError> {
        state.conn = None;
        state.frame_limit = LEGACY_MAX_MESSAGE_SIZE;
        state.api1_lifecycle = None;
        state.api1_store = None;
        state.api1_store_batch = None;
        state.prepared_queries = None;
        state.prepared_get = None;
        state.api1_get = None;
        state.exact_pairs_bits = false;
        state.canonical_pages = false;
        let mut last: Option<String> = None;

        for (attempt, backoff_ms) in RECONNECT_BACKOFF_MS.iter().enumerate() {
            match UnixStream::connect(&self.socket_path).await {
                Ok(stream) => {
                    state.conn = Some(Connection::new(stream));
                    state.frame_limit = LEGACY_MAX_MESSAGE_SIZE;
                    state.api1_lifecycle = None;
                    state.api1_store = None;
                    state.api1_store_batch = None;
                    state.prepared_queries = None;
                    state.prepared_get = None;
                    state.api1_get = None;
                    state.exact_pairs_bits = false;
                    state.canonical_pages = false;
                    match state.negotiate().await {
                        Ok(()) => return Ok(()),
                        Err(e) => {
                            state.conn = None;
                            last = Some(e.to_string());
                        }
                    }
                }
                Err(e) => last = Some(e.to_string()),
            }

            // No point sleeping after the final attempt.
            if attempt + 1 < RECONNECT_ATTEMPTS {
                tokio::time::sleep(Duration::from_millis(*backoff_ms)).await;
            }
        }

        Err(BridgeError::Internal(format!(
            "daemon unavailable at {} after {RECONNECT_ATTEMPTS} attempts: {}",
            self.socket_path.display(),
            last.unwrap_or_else(|| "unknown error".into())
        )))
    }

    /// Test hook: the frame limit currently in force.
    #[cfg(test)]
    async fn frame_limit(&self) -> usize {
        self.state.lock().await.frame_limit
    }
}

impl ClientState {
    /// Performs the handshake on a freshly established connection.
    ///
    /// Never recurses into reconnect: it drives [`ClientState::exchange`]
    /// directly.
    async fn negotiate(&mut self) -> Result<(), BridgeError> {
        let mut request = DaemonRequest {
            jsonrpc: "2.0".into(),
            id: 0,
            method: "ping".into(),
            params: handshake_params(),
        };
        let result = self.exchange(&mut request).await.map_err(|e| e.error)?;
        self.api1_get = None;
        self.api1_get = Some(api1_get_capability(&result)?);
        self.frame_limit = negotiated_limit(&result);
        self.api1_lifecycle = Some(
            result
                .get("api1LifecycleV1")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
        );
        self.api1_store_batch = Some(
            result
                .get("api1StoreBatchV1")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        );
        self.api1_store = Some(
            result
                .get("api1StoreV1")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
        );
        self.prepared_get = Some(
            result
                .get("preparedGetV1")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
        );
        self.prepared_queries = Some(
            result
                .get("preparedQueriesV1")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
        );
        self.canonical_pages = result
            .get("canonicalPagesV1")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        self.exact_pairs_bits = result
            .get("exactPairsBitsV1")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        Ok(())
    }

    async fn send_request(&mut self, request: &mut DaemonRequest) -> Result<u64, ExchangeError> {
        let method = request.method.clone();
        let limit = self.frame_limit;
        request.id = self.next_id;
        self.next_id += 1;
        let id = request.id;

        let written = match self.conn.as_mut() {
            Some(conn) => write_framed_request(&mut conn.writer, request, limit).await,
            None => {
                return Err(ExchangeError {
                    phase: TransmissionPhase::BeforeSend,
                    ..ExchangeError::lost(BridgeError::Internal(
                        "daemon connection is not established".into(),
                    ))
                });
            }
        };

        if let Err(e) = written {
            return Err(match e {
                // Rejected before anything reached the socket: the connection
                // is untouched and stays usable. This is what keeps one
                // oversized request from breaking every call after it.
                FrameError::TooLarge { size, limit } => {
                    ExchangeError::settled(BridgeError::too_large(size, limit))
                }
                FrameError::Malformed(detail) => ExchangeError::settled(BridgeError::InvalidInput(
                    format!("could not serialize `{method}` request: {detail}"),
                )),
                other => {
                    self.conn = None;
                    ExchangeError::lost(BridgeError::Internal(format!(
                        "daemon write error on `{method}`: {other}"
                    )))
                }
            });
        }

        Ok(id)
    }

    /// One request/response round trip on the current connection.
    ///
    /// Assigns `request.id` so a retry gets a fresh id while reusing the
    /// (potentially large) params without cloning them.
    async fn exchange(
        &mut self,
        request: &mut DaemonRequest,
    ) -> Result<serde_json::Value, ExchangeError> {
        let method = request.method.clone();
        let limit = self.frame_limit;
        let id = self.send_request(request).await?;

        for _ in 0..=MAX_STALE_SKIPS {
            let read = match self.conn.as_mut() {
                Some(conn) => read_framed_response(&mut conn.reader, limit).await,
                None => {
                    return Err(ExchangeError::lost(BridgeError::Internal(
                        "daemon connection is not established".into(),
                    )));
                }
            };

            let response = match read {
                Ok(response) => response,
                // Both arms below leave the stream on a frame boundary, so
                // only this call is lost - but they are different failures
                // and must not be reported as the same one. `TooLarge` sends
                // the caller off to shrink their input, which is exactly the
                // wrong move when the daemon simply emitted a frame we could
                // not parse.
                Err(e @ FrameError::OversizedDiscarded { .. }) => {
                    return Err(ExchangeError::unattributed(BridgeError::TooLarge(format!(
                        "the daemon's response to `{method}` could not be delivered: {e}"
                    ))));
                }
                Err(FrameError::Malformed(detail)) => {
                    return Err(ExchangeError::unattributed(BridgeError::Internal(format!(
                        "the daemon's response to `{method}` could not be parsed: {detail}; \
                         the connection is still usable and the call can be retried"
                    ))));
                }
                Err(e) => {
                    self.conn = None;
                    return Err(ExchangeError::lost(BridgeError::Internal(format!(
                        "daemon connection lost while awaiting the response to `{method}`: {e}"
                    ))));
                }
            };

            // The daemon uses id 0 when a framing error hid the request id.
            if response.id == 0 {
                let error = response
                    .error
                    .map(|e| e.into_bridge_error())
                    .unwrap_or_else(|| {
                        BridgeError::Internal(
                            "daemon reported an unattributable protocol error".into(),
                        )
                    });
                return Err(ExchangeError::unattributed(error));
            }

            match response.id.cmp(&id) {
                Ordering::Equal => {
                    if let Some(err) = response.error {
                        return Err(ExchangeError::matched(err.into_bridge_error()));
                    }
                    return match response.result {
                        Some(result) => Ok(result),
                        None => Err(ExchangeError::unattributed(BridgeError::Internal(
                            "empty daemon response".into(),
                        ))),
                    };
                }
                // Ids are monotonic, so a lower id belongs to a call that was
                // already abandoned. Provably not ours; discard it.
                Ordering::Less => {
                    tracing::debug!(
                        got = response.id,
                        expected = id,
                        "discarding stale daemon response"
                    );
                }
                // Impossible under monotonic ids: the stream is corrupt.
                Ordering::Greater => {
                    self.conn = None;
                    return Err(ExchangeError::lost(BridgeError::Internal(format!(
                        "daemon response id mismatch: expected {id}, got {}; \
                         connection reset to resynchronize",
                        response.id
                    ))));
                }
            }
        }

        self.conn = None;
        Err(ExchangeError::lost(BridgeError::Internal(format!(
            "daemon sent more than {MAX_STALE_SKIPS} stale responses while awaiting \
             `{method}`; connection reset to resynchronize"
        ))))
    }
}

/// Params announcing this build's protocol capabilities on `ping`.
fn handshake_params() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "maxMessageSize": MAX_MESSAGE_SIZE,
    })
}

/// Frame limit implied by a `ping` result.
///
/// A daemon that says nothing is protocol v1 and is held at 1 MiB, which
/// keeps us from ever handing it a frame it would reject without draining.
fn negotiated_limit(result: &serde_json::Value) -> usize {
    let version = result
        .get("protocolVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);
    if version < PROTOCOL_VERSION as u64 {
        return LEGACY_MAX_MESSAGE_SIZE;
    }
    let announced = result
        .get("maxMessageSize")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(LEGACY_MAX_MESSAGE_SIZE as u64);
    (announced as usize).clamp(LEGACY_MAX_MESSAGE_SIZE, MAX_MESSAGE_SIZE)
}

#[cfg(test)]
#[path = "store_capability_tests.rs"]
mod store_capability_tests;

#[cfg(test)]
mod tests {
    include!("api1_get_before_tests.rs");
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

    use serde_json::json;
    use tokio::net::UnixListener;

    use super::super::protocol::{DaemonResponse, read_framed_message, write_framed_message};
    use super::*;

    /// Scripted daemon behaviours used to drive the client through each
    /// failure mode. All counters below are global across connections, so a
    /// behaviour that fires "on the first call" does not fire again on the
    /// connection the client reconnects with.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Behavior {
        /// Answer every call successfully; negotiate protocol v2 on ping.
        Healthy,
        /// Answer ping with `{}`, as recalld <= 0.1.10 does.
        LegacyPing,
        /// Answer the first call with a response larger than the client's limit.
        HugeResponse,
        /// Read the first call, then hang up without replying.
        CloseAfterFirstCall,
        /// Prefix the second call's response with a stale one.
        StaleThenCorrect,
        /// Answer the first call with an id from the future.
        FutureId,
        /// Answer the first call with a well-framed but unparseable payload.
        MalformedResponse,
        /// Answer the first connection's `ping` slowly and hang up on its
        /// first real call; negotiate as a v1 daemon on every connection
        /// after that. Drives the ping/reconnect interleaving.
        SlowPingThenLegacyReconnect,
    }

    struct Stub {
        path: PathBuf,
        received: Arc<StdMutex<Vec<DaemonRequest>>>,
        _dir: tempfile::TempDir,
        handle: tokio::task::JoinHandle<()>,
    }

    impl Stub {
        fn start(behavior: Behavior) -> Self {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("t.sock");
            let listener = UnixListener::bind(&path).expect("bind");
            let received = Arc::new(StdMutex::new(Vec::new()));
            let handle = tokio::spawn(serve(listener, behavior, received.clone()));
            Self {
                path,
                received,
                _dir: dir,
                handle,
            }
        }

        fn methods(&self) -> Vec<String> {
            self.received
                .lock()
                .unwrap()
                .iter()
                .map(|r| r.method.clone())
                .collect()
        }

        fn count(&self, method: &str) -> usize {
            self.methods().iter().filter(|m| *m == method).count()
        }

        /// Stops listening and removes the socket so reconnects fail.
        async fn shutdown(&self) {
            self.handle.abort();
            let _ = std::fs::remove_file(&self.path);
            // Give the abort a chance to land so the live connection drops.
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn serve(
        listener: UnixListener,
        behavior: Behavior,
        received: Arc<StdMutex<Vec<DaemonRequest>>>,
    ) {
        use tokio::io::AsyncWriteExt;

        let mut calls = 0usize;
        let mut connections = 0usize;
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            connections += 1;
            let (reader, writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut writer = writer;

            loop {
                let Ok(request) = read_framed_message(&mut reader, MAX_MESSAGE_SIZE).await else {
                    break;
                };
                received.lock().unwrap().push(request.clone());

                if request.method == "ping" {
                    let legacy = behavior == Behavior::LegacyPing
                        || (behavior == Behavior::SlowPingThenLegacyReconnect && connections > 1);
                    let result = if legacy {
                        json!({})
                    } else {
                        json!({
                            "protocolVersion": PROTOCOL_VERSION,
                            "maxMessageSize": MAX_MESSAGE_SIZE,
                        })
                    };
                    // A slow answer keeps the client's mutex held long enough
                    // for a second caller to queue behind it.
                    if behavior == Behavior::SlowPingThenLegacyReconnect && connections == 1 {
                        tokio::time::sleep(Duration::from_millis(300)).await;
                    }
                    let response = DaemonResponse::success(request.id, result);
                    if write_framed_message(&mut writer, &response, MAX_MESSAGE_SIZE)
                        .await
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }

                calls += 1;

                // A well-framed reply the client cannot parse. Written raw,
                // because the framing helpers only emit valid messages.
                if behavior == Behavior::MalformedResponse && calls == 1 {
                    if writer.write_u32(4).await.is_err()
                        || writer.write_all(b"junk").await.is_err()
                    {
                        break;
                    }
                    continue;
                }

                let replies: Vec<DaemonResponse> = match behavior {
                    Behavior::SlowPingThenLegacyReconnect if connections == 1 => break,
                    Behavior::CloseAfterFirstCall if calls == 1 => break,
                    Behavior::HugeResponse if calls == 1 => vec![DaemonResponse::success(
                        request.id,
                        json!({ "blob": "x".repeat(2 * 1024 * 1024) }),
                    )],
                    Behavior::StaleThenCorrect if calls == 2 => vec![
                        DaemonResponse::success(request.id - 1, json!({ "stale": true })),
                        DaemonResponse::success(request.id, json!({ "ok": true })),
                    ],
                    Behavior::FutureId if calls == 1 => {
                        vec![DaemonResponse::success(request.id + 5, json!({}))]
                    }
                    _ => vec![DaemonResponse::success(request.id, json!({ "ok": true }))],
                };

                let mut failed = false;
                for reply in &replies {
                    if write_framed_message(&mut writer, reply, MAX_MESSAGE_SIZE)
                        .await
                        .is_err()
                    {
                        failed = true;
                        break;
                    }
                }
                if failed {
                    break;
                }
            }
        }
    }

    /// A params blob whose serialized form comfortably exceeds `bytes`.
    fn params_of_bytes(bytes: usize) -> serde_json::Value {
        json!({ "fullText": "y".repeat(bytes) })
    }

    // 14 — direct regression for the reported bug.
    #[tokio::test]
    async fn oversize_request_fails_fast_and_connection_survives() {
        let stub = Stub::start(Behavior::Healthy);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        client
            .call("get_memory_read_only", json!({ "id": "a" }))
            .await
            .unwrap();

        let err = client
            .call("store_memory", params_of_bytes(2 * 1024 * 1024))
            .await
            .unwrap_err();
        let text = err.to_string();
        assert!(matches!(err, BridgeError::TooLarge(_)), "got {text}");
        assert!(
            text.contains(&LEGACY_MAX_MESSAGE_SIZE.to_string()),
            "{text}"
        );
        assert!(text.contains("bytes"), "{text}");

        // The socket was never touched, so the next call still works.
        assert!(client.is_connected());
        client
            .call("get_memory_read_only", json!({ "id": "b" }))
            .await
            .unwrap();

        // Exactly the two small calls reached the wire.
        assert_eq!(stub.methods().len(), 2);
        assert_eq!(stub.count("get_memory_read_only"), 2);
        assert_eq!(stub.count("store_memory"), 0);
    }

    // 15
    #[tokio::test]
    async fn reconnects_after_server_closes_connection() {
        let stub = Stub::start(Behavior::CloseAfterFirstCall);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        // The first call is swallowed; the client reconnects and retries.
        client
            .call("get_memory_read_only", json!({ "id": "a" }))
            .await
            .unwrap();
        assert!(client.is_connected());

        client
            .call("get_memory_read_only", json!({ "id": "b" }))
            .await
            .unwrap();
        assert_eq!(stub.count("ping"), 1, "reconnect renegotiates exactly once");
    }

    // 16
    #[tokio::test]
    async fn read_only_method_retries_transparently_after_mid_call_close() {
        let stub = Stub::start(Behavior::CloseAfterFirstCall);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        let result = client.call("list_memories", json!({})).await.unwrap();
        assert_eq!(result, json!({ "ok": true }));
        assert_eq!(
            stub.count("list_memories"),
            2,
            "the side-effect-free call was replayed"
        );
    }

    // 16b — regression: `search` and `find_similar` look read-only but
    // record an access for every result, which advances `last_accessed_at`
    // and the FSRS schedule. A transparent replay would age those memories
    // twice off one caller request, so the call is reported instead.
    #[tokio::test]
    async fn access_recording_reads_are_not_replayed() {
        for method in ["search", "find_similar"] {
            let stub = Stub::start(Behavior::CloseAfterFirstCall);
            let client = DaemonClient::connect(&stub.path).await.unwrap();

            let err = client.call(method, json!({})).await.unwrap_err();
            assert_eq!(
                stub.count(method),
                1,
                "{method} must reach the daemon exactly once"
            );

            // ...and the report must not describe a read as a half-applied
            // write, which is the other way to get this wrong.
            let text = err.to_string();
            assert!(text.contains("safe"), "{text}");
            assert!(!text.contains("may or may not have been applied"), "{text}");
            assert!(client.is_connected());
        }
    }

    // 17
    #[tokio::test]
    async fn mutating_method_is_not_retried() {
        let stub = Stub::start(Behavior::CloseAfterFirstCall);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        let err = client
            .call("store_memory", json!({ "summary": "s" }))
            .await
            .unwrap_err();
        let text = err.to_string();
        assert!(text.contains("may or may not have been applied"), "{text}");
        assert!(text.contains("re-established"), "{text}");

        assert_eq!(
            stub.count("store_memory"),
            1,
            "the mutating call must reach the daemon exactly once"
        );
        // The client is healthy again for the next call.
        assert!(client.is_connected());
        client
            .call("get_memory_read_only", json!({ "id": "a" }))
            .await
            .unwrap();
    }

    // 18
    #[tokio::test]
    async fn reconnect_gives_up_after_bounded_attempts() {
        let stub = Stub::start(Behavior::Healthy);
        let client = DaemonClient::connect(&stub.path).await.unwrap();
        client
            .call("get_memory_read_only", json!({ "id": "a" }))
            .await
            .unwrap();

        stub.shutdown().await;

        let err = tokio::time::timeout(Duration::from_secs(5), async {
            // The first call discovers the dead peer; the second starts with
            // no connection at all. Both must terminate.
            let _ = client
                .call("get_memory_read_only", json!({ "id": "b" }))
                .await;
            client
                .call("get_memory_read_only", json!({ "id": "c" }))
                .await
        })
        .await
        .expect("reconnect must not loop forever")
        .unwrap_err();

        let text = err.to_string();
        assert!(text.contains(&stub.path.display().to_string()), "{text}");
        assert!(text.contains("3 attempts"), "{text}");
        assert!(!client.is_connected());
    }

    // 19
    #[tokio::test]
    async fn stale_response_id_is_skipped() {
        let stub = Stub::start(Behavior::StaleThenCorrect);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        client
            .call("get_memory_read_only", json!({ "id": "a" }))
            .await
            .unwrap();
        let result = client
            .call("get_memory_read_only", json!({ "id": "b" }))
            .await
            .unwrap();

        assert_eq!(result, json!({ "ok": true }), "the stale reply was skipped");
        assert!(client.is_connected());
        assert_eq!(stub.count("ping"), 0, "no reconnect was needed");
    }

    // 20
    #[tokio::test]
    async fn future_response_id_poisons_and_next_call_recovers() {
        let stub = Stub::start(Behavior::FutureId);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        // A mutating method, so the mismatch surfaces instead of being
        // silently replayed.
        let err = client.call("store_memory", json!({})).await.unwrap_err();
        let text = err.to_string();
        assert!(text.contains("id mismatch"), "{text}");

        let result = client
            .call("get_memory_read_only", json!({ "id": "a" }))
            .await
            .unwrap();
        assert_eq!(result, json!({ "ok": true }));
    }

    // 21 — regression for the silently-desyncing response path.
    #[tokio::test]
    async fn oversized_response_is_drained_and_connection_survives() {
        let stub = Stub::start(Behavior::HugeResponse);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        let err = client
            .call("get_memory_read_only", json!({ "id": "a" }))
            .await
            .unwrap_err();
        assert!(matches!(err, BridgeError::TooLarge(_)), "{err}");

        // The oversized frame was drained, so the stream is still aligned.
        assert!(client.is_connected());
        let result = client
            .call("get_memory_read_only", json!({ "id": "b" }))
            .await
            .unwrap();
        assert_eq!(result, json!({ "ok": true }));
        assert_eq!(stub.count("ping"), 0, "no reconnect was needed");
    }

    // 21b — regression: a response we cannot parse is not a size problem.
    //
    // Both failures leave the stream on a frame boundary, and the client
    // used to collapse them into `TooLarge`, telling the caller to shrink
    // an input that was never the problem.
    #[tokio::test]
    async fn malformed_response_is_reported_as_a_protocol_error_not_a_size_error() {
        let stub = Stub::start(Behavior::MalformedResponse);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        let err = client
            .call("get_memory_read_only", json!({ "id": "a" }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, BridgeError::Internal(_)),
            "a malformed response is a protocol failure, got {err}"
        );
        let text = err.to_string();
        assert!(text.contains("could not be parsed"), "{text}");
        assert!(!text.contains("Too large"), "{text}");

        // The stream is still aligned, so the next call goes through.
        assert!(client.is_connected());
        let result = client
            .call("get_memory_read_only", json!({ "id": "b" }))
            .await
            .unwrap();
        assert_eq!(result, json!({ "ok": true }));
        assert_eq!(stub.count("ping"), 0, "no reconnect was needed");
    }

    // 22
    #[tokio::test]
    async fn ping_negotiates_frame_limit() {
        let stub = Stub::start(Behavior::Healthy);
        let client = DaemonClient::connect(&stub.path).await.unwrap();
        assert_eq!(client.frame_limit().await, LEGACY_MAX_MESSAGE_SIZE);

        client.ping().await.unwrap();
        assert_eq!(client.frame_limit().await, MAX_MESSAGE_SIZE);
    }

    // 22c — regression: `ping` must not be a read-modify-write.
    //
    // The pinger holds the client mutex while the (slow) first connection
    // answers, so the concurrent `get_memory` queues behind it. When that
    // call finds the connection dead it reconnects to what is now a v1
    // daemon and correctly clamps the limit to 1 MiB. If `ping` released the
    // lock between its exchange and its write, tokio's FIFO mutex hands the
    // waiter in first and the stale 8 MiB lands on top of that clamp -
    // after which the client sends frames a v1 daemon rejects *without*
    // draining, desynchronizing the stream for good.
    #[tokio::test]
    async fn concurrent_reconnect_cannot_have_its_clamp_overwritten_by_ping() {
        let stub = Stub::start(Behavior::SlowPingThenLegacyReconnect);
        let client = Arc::new(DaemonClient::connect(&stub.path).await.unwrap());

        let pinger = {
            let client = client.clone();
            tokio::spawn(async move { client.ping().await })
        };

        // Long enough for the ping to own the lock and block on the answer.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let reader = {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .call("get_memory_read_only", json!({ "id": "a" }))
                    .await
            })
        };

        pinger.await.unwrap().unwrap();
        reader.await.unwrap().unwrap();

        assert_eq!(
            client.frame_limit().await,
            LEGACY_MAX_MESSAGE_SIZE,
            "the live connection is a v1 daemon; its clamp must be the one in force"
        );
    }

    // 22b
    #[tokio::test]
    async fn legacy_daemon_returning_empty_ping_clamps_to_1mib() {
        let stub = Stub::start(Behavior::LegacyPing);
        let client = DaemonClient::connect(&stub.path).await.unwrap();

        client.ping().await.unwrap();
        assert_eq!(client.frame_limit().await, LEGACY_MAX_MESSAGE_SIZE);
    }

    #[test]
    fn only_methods_without_observable_effects_are_retry_safe() {
        for method in [
            "store_memory",
            "create_namespace",
            "reinforce_memory",
            "delete_memory",
            "delete_memories",
            "forget_native_v1",
            "feedback_native_v1",
            "forget_lifecycle_v1",
            "feedback_lifecycle_v1",
            "feedback_api1_v1",
            "forget_api1_v1",
            "shutdown",
            "get_memory",
            "get_memory_scoped",
            "get_api1_v1",
            // Not mutations, but not effect-free either: both record an
            // access per result, which moves the decay schedule.
            "search",
            "find_similar",
        ] {
            assert!(!is_retry_safe(method), "{method} must not be replayed");
        }
        for method in [
            "ping",
            "check_health",
            "scan_duplicates",
            "get_memory_read_only",
            "list_namespaces",
            "namespace_stats",
            "list_memories",
            "list_tags",
        ] {
            assert!(is_retry_safe(method), "{method} should be replayable");
        }
    }

    /// The destructive methods must be on neither list.
    /// `records_accesses_only` downgrades the caller's warning from "may
    /// or may not have been applied" to "just ask again", which is the
    /// wrong thing to tell someone whose namespace may have been
    /// destroyed.
    #[test]
    fn w42_the_destructive_methods_are_neither_retry_safe_nor_access_only() {
        for method in ["delete_memories", "delete_namespace"] {
            assert!(!is_retry_safe(method), "{method} must not be replayed");
            assert!(
                !records_accesses_only(method),
                "{method} is destructive, not merely access-recording"
            );
        }
    }

    #[test]
    fn negotiated_limit_ignores_unknown_and_absurd_announcements() {
        assert_eq!(negotiated_limit(&json!({})), LEGACY_MAX_MESSAGE_SIZE);
        assert_eq!(
            negotiated_limit(&json!({ "protocolVersion": 1, "maxMessageSize": 99_999_999 })),
            LEGACY_MAX_MESSAGE_SIZE
        );
        assert_eq!(
            negotiated_limit(&json!({ "protocolVersion": 2, "maxMessageSize": 99_999_999 })),
            MAX_MESSAGE_SIZE
        );
        assert_eq!(
            negotiated_limit(&json!({ "protocolVersion": 2, "maxMessageSize": 1024 })),
            LEGACY_MAX_MESSAGE_SIZE
        );
    }
}
