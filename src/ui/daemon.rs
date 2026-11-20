//! Daemon-side half of the UI: read-only methods, connection registry,
//! event observation around ordinary methods, and the `subscribe` stream.
//!
//! Every method here is read-only and records no access. Writes the UI
//! makes (Reinforce, Forget) go through the daemon's existing methods.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::Engine as _;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::sync::watch;

use super::events::{self, EventContext, RecallTrace, UiEvent};
use super::wire::*;
use crate::Recalld;
use crate::embedding::EmbeddingProvider;
use crate::graph::SharedGraph;
use crate::mcp::bridge::{BridgeError, HealthChecker};
use crate::model::{MemoryId, NamespaceConfig};
use crate::storage::{RedbStorageEngine, StorageEngine as _};

/// Largest edge page `graph_snapshot` returns (≈ 3.4 MiB of base64).
pub const MAX_EDGE_PAGE: usize = 200_000;

pub struct UiService {
    pub(super) storage: Arc<std::sync::RwLock<RedbStorageEngine>>,
    pub(super) graph: SharedGraph,
    embedding: Arc<dyn EmbeddingProvider>,
    config: crate::RecalldConfig,
    pub(super) query_engine: Arc<crate::search::QueryEngine>,
    pub(super) rows: Mutex<super::memories::RowCache>,
    pub(super) api_storage: Arc<dyn crate::api::StorageEngine>,
    api_namespaces: Arc<dyn crate::api::NamespaceRegistry>,
    health: Arc<dyn HealthChecker>,
    started_at: i64,
    started: Instant,
    socket: std::sync::OnceLock<String>,
    clients: Mutex<BTreeMap<u64, ClientInfo>>,
    next_client: AtomicU64,
    cpu_sample: Mutex<Option<(Instant, f64)>>,
}

impl UiService {
    pub fn new(system: &Recalld) -> Self {
        use crate::api::adapters::{NamespaceRegistryAdapter, StorageEngineAdapter};
        let api_storage: Arc<dyn crate::api::StorageEngine> = Arc::new(
            StorageEngineAdapter::new(
                system.storage().clone(),
                system.cache().clone(),
                system.vector_index().clone(),
                system.fts_index().clone(),
                system.entity_index().clone(),
                system.graph().clone(),
            )
            .with_clock(system.clock().clone())
            .with_native_lifecycle_policy(system.query_engine().native_policy_inputs())
            .with_catalog_profile_context(system.embedding().as_ref()),
        );
        let api_namespaces: Arc<dyn crate::api::NamespaceRegistry> =
            Arc::new(NamespaceRegistryAdapter::from_system(system));
        Self {
            storage: system.storage().clone(),
            graph: system.graph().clone(),
            embedding: system.embedding().clone(),
            config: system.config().clone(),
            query_engine: system.query_engine().clone(),
            rows: Mutex::new(super::memories::RowCache::new()),
            api_storage,
            api_namespaces,
            health: Arc::new(crate::mcp::bridge_adapters::McpHealthAdapter::new(
                system.storage().clone(),
            )),
            started_at: events::now_millis(),
            started: Instant::now(),
            socket: std::sync::OnceLock::new(),
            clients: Mutex::new(BTreeMap::new()),
            next_client: AtomicU64::new(1),
            cpu_sample: Mutex::new(None),
        }
    }

    /// The socket the daemon listens on, for Health → System.
    pub fn set_socket(&self, path: &std::path::Path) {
        let _ = self.socket.set(path.display().to_string());
    }

    /// Register a daemon connection for the clients list.
    pub fn connect(self: &Arc<Self>) -> Connection {
        let id = self.next_client.fetch_add(1, Ordering::Relaxed);
        self.clients
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id,
                ClientInfo {
                    id,
                    agent: None,
                    kind: "unknown".into(),
                    since: events::now_millis(),
                },
            );
        Connection {
            id,
            service: self.clone(),
            subscribe: Mutex::new(None),
        }
    }

    /// Answer a UI method, or `None` when `method` belongs to the daemon.
    pub async fn dispatch(
        &self,
        conn: &Connection,
        method: &str,
        params: &Value,
    ) -> Option<Result<Value, BridgeError>> {
        fn decode<T: serde::de::DeserializeOwned + Default>(v: &Value) -> Result<T, BridgeError> {
            if v.is_null() {
                return Ok(T::default());
            }
            serde_json::from_value(v.clone()).map_err(|e| BridgeError::InvalidInput(e.to_string()))
        }
        fn encode<T: serde::Serialize>(v: Result<T, BridgeError>) -> Result<Value, BridgeError> {
            v.and_then(|v| {
                serde_json::to_value(v).map_err(|e| BridgeError::Internal(e.to_string()))
            })
        }
        Some(match method {
            "graph_snapshot" => match decode::<GraphSnapshotParams>(params) {
                Ok(p) => encode(self.graph_snapshot(p).await),
                Err(e) => Err(e),
            },
            "health_report" => {
                let namespace = params
                    .get("namespace")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                encode(
                    crate::api::compute_health_report(
                        self.api_storage.as_ref(),
                        self.api_namespaces.as_ref(),
                        namespace,
                    )
                    .await
                    .map_err(app_error),
                )
            }
            "system_status" => encode(Ok(self.system_status().await)),
            "memory_page" => match decode::<MemoryPageParams>(params) {
                Ok(p) => encode(self.memory_page(p).await),
                Err(e) => Err(e),
            },
            "memory_neighbors" => match serde_json::from_value::<NeighborsParams>(params.clone()) {
                Ok(p) => encode(self.memory_neighbors(p).await),
                Err(e) => Err(BridgeError::InvalidInput(e.to_string())),
            },
            "access_history" => match serde_json::from_value::<HistoryParams>(params.clone()) {
                Ok(p) => encode(self.access_history(p).await),
                Err(e) => Err(BridgeError::InvalidInput(e.to_string())),
            },
            "activity" => match decode::<ActivityParams>(params) {
                Ok(p) => encode(self.activity(p).await),
                Err(e) => Err(e),
            },
            "search_explain_read_only" => {
                match serde_json::from_value::<ExplainParams>(params.clone()) {
                    Ok(p) => encode(self.search_explain(p).await),
                    Err(e) => Err(BridgeError::InvalidInput(e.to_string())),
                }
            }
            "hello" => match serde_json::from_value::<HelloParams>(params.clone()) {
                Ok(p) => {
                    let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(entry) = clients.get_mut(&conn.id) {
                        entry.agent = p.agent.filter(|a| !a.is_empty() && a.len() <= 128);
                        entry.kind = match p.kind.as_deref() {
                            Some("ui") => "ui",
                            Some("mcp") => "mcp",
                            _ => "unknown",
                        }
                        .into();
                    }
                    Ok(json!({}))
                }
                Err(e) => Err(BridgeError::InvalidInput(e.to_string())),
            },
            "subscribe" => match decode::<SubscribeParams>(params) {
                Ok(p) => {
                    let kinds = p.kinds.clone();
                    *conn.subscribe.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
                    encode(Ok(Subscribed { subscribed: kinds }))
                }
                Err(e) => Err(e),
            },
            _ => return None,
        })
    }

    fn namespace_configs(&self) -> Result<Vec<NamespaceConfig>, BridgeError> {
        self.storage
            .read()
            .map_err(|_| BridgeError::Internal("storage lock poisoned".into()))?
            .list_namespaces()
            .map_err(|e| BridgeError::Internal(e.to_string()))
    }

    fn namespace_ref(&self, ns: &NamespaceConfig) -> NamespaceRef {
        NamespaceRef {
            id: ns.id.get(),
            name: ns.name.clone(),
            thresholds: [
                ns.phase_thresholds.full_to_summary,
                ns.phase_thresholds.summary_to_ghost,
                ns.phase_thresholds.ghost_to_delete,
            ],
            permastore_days: ns.permastore_threshold,
            decay_multiplier: ns
                .decay_rate_multiplier
                .unwrap_or(self.config.decay.decay_rate_multiplier as f32),
        }
    }

    pub async fn graph_snapshot(
        &self,
        p: GraphSnapshotParams,
    ) -> Result<GraphSnapshot, BridgeError> {
        let configs = self.namespace_configs()?;
        let filter = match &p.namespace {
            Some(name) => Some(
                configs
                    .iter()
                    .find(|ns| &ns.name == name)
                    .ok_or_else(|| BridgeError::NotFound(format!("namespace '{name}'")))?
                    .id,
            ),
            None => None,
        };
        let namespaces = configs
            .iter()
            .filter(|ns| filter.is_none_or(|id| id == ns.id))
            .map(|ns| self.namespace_ref(ns))
            .collect();
        let first_page = p.edge_offset == 0;
        // Record columns only matter on the first page; skip the scan otherwise.
        let records = if first_page {
            let storage = self.storage.clone();
            tokio::task::spawn_blocking(move || {
                storage
                    .read()
                    .map_err(|_| BridgeError::Internal("storage lock poisoned".into()))?
                    .scan_all()
                    .map_err(|e| BridgeError::Internal(e.to_string()))
            })
            .await
            .map_err(|e| BridgeError::Internal(e.to_string()))??
            .into_iter()
            .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };

        let graph = self.graph.read().await;
        let mut index = HashMap::with_capacity(graph.node_count());
        let mut ids = Vec::new();
        let (mut ns, mut phase, mut degree, mut permastore) = (vec![], vec![], vec![], vec![]);
        let (mut strength, mut decay_strength, mut stability) = (vec![], vec![], vec![]);
        let (mut created_at, mut last_access) = (Vec::<f64>::new(), Vec::<f64>::new());
        for (key, node) in graph.iter_nodes() {
            if filter.is_some_and(|id| id != node.namespace_id) {
                continue;
            }
            index.insert(key, index.len() as u32);
            if !first_page {
                continue;
            }
            ids.push(node.memory_id.to_string());
            ns.push(node.namespace_id.get());
            degree.push(node.degree() as u32);
            match records.get(&node.memory_id) {
                Some(r) => {
                    phase.push(r.phase as u8);
                    strength.push(r.strength);
                    decay_strength.push(r.decay_strength);
                    stability.push(r.stability);
                    created_at.push(r.created_at as f64);
                    last_access.push(r.last_accessed_at as f64);
                    permastore.push(u8::from(r.is_permastore != 0));
                }
                None => {
                    phase.push(node.decay_phase as u8);
                    strength.push(node.strength);
                    decay_strength.push(node.strength);
                    stability.push(0.0);
                    created_at.push(0.0);
                    last_access.push(0.0);
                    permastore.push(0);
                }
            }
        }
        let (mut src, mut dst, mut kind, mut weight) = (vec![], vec![], vec![], vec![]);
        let limit = p
            .edge_limit
            .unwrap_or(MAX_EDGE_PAGE)
            .clamp(1, MAX_EDGE_PAGE);
        let mut edge_total = 0usize;
        for (_, edge) in graph.iter_edges() {
            let (Some(&s), Some(&d)) = (index.get(&edge.source), index.get(&edge.target)) else {
                continue;
            };
            if edge_total >= p.edge_offset && src.len() < limit {
                src.push(s);
                dst.push(d);
                kind.push(edge.edge_type as u8);
                weight.push(edge.weight);
            }
            edge_total += 1;
        }
        let node_count = index.len();
        drop(graph);
        let end = p.edge_offset + src.len();
        Ok(GraphSnapshot {
            generated_at: events::now_millis(),
            namespaces,
            node_count,
            nodes: first_page.then(|| SnapshotNodes {
                ids,
                ns: b64(&ns),
                phase: b64(&phase),
                strength: b64(&strength),
                decay_strength: b64(&decay_strength),
                stability: b64(&stability),
                created_at: b64(&created_at),
                last_access: b64(&last_access),
                degree: b64(&degree),
                permastore: b64(&permastore),
            }),
            edges: SnapshotEdges {
                src: b64(&src),
                dst: b64(&dst),
                kind: b64(&kind),
                weight: b64(&weight),
            },
            edge_total,
            edge_offset: p.edge_offset,
            next_edge_offset: (end < edge_total).then_some(end),
        })
    }

    pub async fn system_status(&self) -> SystemStatus {
        let (last_us, smooth_us) = events::embedding_latency_us();
        let (last_sweep, interval) = events::sweep_schedule();
        let sweep_enabled = !self.config.decay.disable_sweep;
        let interval_secs = if interval > 0 {
            interval
        } else {
            (self.config.decay.sweep_interval_hours * 3600.0) as u64
        };
        let cpu_percent = crate::ui::sysinfo::cpu_seconds().map(|cpu| {
            let now = Instant::now();
            let mut sample = self.cpu_sample.lock().unwrap_or_else(|e| e.into_inner());
            let (since, before) = sample.unwrap_or((self.started, 0.0));
            *sample = Some((now, cpu));
            let wall = now.duration_since(since).as_secs_f64();
            if wall > 0.0 {
                ((cpu - before) / wall * 100.0).max(0.0)
            } else {
                0.0
            }
        });
        SystemStatus {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol_version: crate::daemon::protocol::PROTOCOL_VERSION,
            pid: std::process::id(),
            started_at: self.started_at,
            uptime_secs: self.started.elapsed().as_secs(),
            data_dir: self.config.storage.data_dir.clone(),
            socket: self.socket.get().cloned().unwrap_or_default(),
            health: self.health.check_health().await,
            embedding: EmbeddingStatus {
                provider: serde_json::to_value(&self.config.embedding.provider)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                model: self.embedding.model_name().to_owned(),
                dimensions: self.embedding.dimensions(),
                latency_ms: (last_us > 0).then(|| LatencyMs {
                    last: last_us as f64 / 1000.0,
                    smoothed: smooth_us as f64 / 1000.0,
                }),
            },
            process: ProcessStatus {
                rss_bytes: crate::ui::sysinfo::rss_bytes(),
                cpu_percent,
            },
            clients: self
                .clients
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .values()
                .cloned()
                .collect(),
            sweep: SweepStatus {
                enabled: sweep_enabled,
                interval_secs,
                last_at: (last_sweep > 0).then_some(last_sweep),
                next_at: (sweep_enabled && last_sweep > 0)
                    .then(|| last_sweep + interval_secs as i64 * 1000),
            },
        }
    }

    // ── Observing ordinary daemon methods ──────────────────────────────

    /// Prepare to observe one ordinary (non-UI) daemon call. `None` when
    /// nobody is subscribed, so an unwatched daemon does no extra work.
    pub fn observe(&self, conn: &Connection, method: &str, params: &Value) -> Option<Observation> {
        let active = events::active();
        let recall = matches!(method, "search" | "query_contract_v1");
        let reinforce = matches!(
            method,
            "reinforce_memory"
                | "reinforce_memory_scoped"
                | "feedback_api1_v1"
                | "feedback_native_v1"
                | "feedback_lifecycle_v1"
        );
        // Unwatched, only recalls and reinforcements are noted (for attribution).
        if !active && !recall && !reinforce {
            return None;
        }
        let agent = self
            .clients
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&conn.id)
            .and_then(|c| c.agent.clone());
        let store = active
            && matches!(
                method,
                "store_memory"
                    | "store_api1_v1"
                    | "store_batch_api1_v1"
                    | "store_native_single_observed_v1"
                    | "mutate_batch"
                    | "mutate_native_batch_v1"
                    | "mutate_native_batch_observed_v1"
            );
        if active && !recall && !method.contains("read_only") {
            events::note_writer(agent.as_deref());
        }
        let reinforce = reinforce.then(|| find_id(params)).flatten().map(|id| {
            let rating = find_u8(params, &["quality", "rating"]);
            (id, rating, active.then(|| self.stability(id)).flatten())
        });
        Some(Observation {
            ctx: Arc::new(EventContext {
                agent: agent.clone(),
                recall: (active && recall).then(|| Mutex::new(RecallTrace::default())),
            }),
            agent,
            query: recall.then(|| {
                (
                    find_str(params, "query").unwrap_or_default(),
                    find_str(params, "namespace").unwrap_or_else(|| "default".into()),
                )
            }),
            reinforce,
            store,
            started_at: events::now_millis(),
        })
    }

    fn stability(&self, id: MemoryId) -> Option<f32> {
        self.storage
            .read()
            .ok()?
            .get_record(id)
            .ok()
            .flatten()
            .map(|r| r.stability)
    }

    /// Publish what an observed call did, once it succeeded.
    pub fn finish(&self, obs: Observation, result: &Result<Value, BridgeError>) {
        let Ok(value) = result else { return };
        if let Some((query, namespace)) = obs.query {
            let trace = obs
                .ctx
                .recall
                .as_ref()
                .map(|m| std::mem::take(&mut *m.lock().unwrap_or_else(|e| e.into_inner())))
                .unwrap_or_default();
            let returned = returned_ids(value);
            let agent = obs.agent.clone();
            events::journal(events::Note {
                at: events::now_millis(),
                agent: agent.clone(),
                query: Some(query.clone()),
                ids: returned.clone(),
            });
            if !events::active() {
                return;
            }
            // The light shows what mattered: the strongest matches (not every
            // candidate the pipeline scored) and a deduplicated, bounded set of
            // suppressed competitors.
            let mut best: std::collections::HashMap<MemoryId, f32> = Default::default();
            for (id, score) in &trace.seeds {
                let e = best.entry(*id).or_insert(*score);
                *e = e.max(*score);
            }
            let mut seeds: Vec<(MemoryId, f32)> = best.into_iter().collect();
            seeds.sort_by(|a, b| b.1.total_cmp(&a.1));
            let top = seeds.first().map_or(0.0, |s| s.1);
            seeds.retain(|s| s.1 >= top * 0.6);
            seeds.truncate(returned.len().max(8));
            let mut seen = std::collections::HashSet::new();
            let suppressed: Vec<String> = trace
                .suppressed
                .iter()
                .map(ToString::to_string)
                .filter(|id| !returned.contains(id) && seen.insert(id.clone()))
                .take(24)
                .collect();
            let steps: Vec<_> = trace.steps.iter().take(500).collect();
            events::publish(|seq, at| {
                UiEvent::Recall(events::RecallEvent {
                    seq,
                    at,
                    agent,
                    namespace,
                    query,
                    seeds: seeds
                        .iter()
                        .map(|(id, score)| events::Seed {
                            id: id.to_string(),
                            score: *score,
                        })
                        .collect(),
                    trace: steps
                        .iter()
                        .map(|s| events::TraceStep {
                            from: s.from.to_string(),
                            to: s.to.to_string(),
                            edge_type: s.edge_type as u8,
                            amount: s.amount,
                            order: s.order,
                        })
                        .collect(),
                    returned,
                    suppressed,
                })
            });
        }
        if obs.store {
            // Created memories, attributed to this connection's agent. The
            // graph insert itself runs on a shared completion worker.
            let mut ids = Vec::new();
            uuids(value, 0, &mut ids);
            ids.sort();
            ids.dedup();
            let created: Vec<(MemoryId, u32, u8)> = self
                .storage
                .read()
                .ok()
                .map(|s| {
                    ids.iter()
                        .filter_map(|id| {
                            let r = s.get_record(*id).ok()??;
                            (r.created_at >= obs.started_at - 2_000).then_some((
                                *id,
                                r.namespace_id,
                                r.phase as u8,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let graph = self.graph.try_read().ok();
            for (id, namespace_id, phase) in created {
                let edges = graph
                    .as_ref()
                    .map(|g| {
                        g.edges_for(&id)
                            .iter()
                            .filter_map(|e| {
                                let (s, t) = (g.nodes.get(e.source)?, g.nodes.get(e.target)?);
                                let other = if s.memory_id == id { t } else { s };
                                Some(events::EdgeRef {
                                    to: other.memory_id.to_string(),
                                    edge_type: e.edge_type as u8,
                                    weight: e.weight,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let agent = obs.agent.clone();
                events::publish(|seq, at| {
                    UiEvent::Store(events::StoreEvent {
                        seq,
                        at,
                        agent,
                        id: id.to_string(),
                        namespace_id,
                        phase,
                        edges,
                    })
                });
            }
        }
        if let Some((id, rating, before)) = obs.reinforce {
            events::journal(events::Note {
                at: events::now_millis(),
                agent: obs.agent.clone(),
                query: None,
                ids: vec![id.to_string()],
            });
            if !events::active() {
                return;
            }
            let after = self.stability(id);
            events::publish(|seq, at| {
                UiEvent::Reinforce(events::ReinforceEvent {
                    seq,
                    at,
                    agent: obs.agent,
                    id: id.to_string(),
                    rating,
                    stability_before: before,
                    stability_after: after,
                })
            });
        }
    }
}

fn app_error(e: crate::api::AppError) -> BridgeError {
    use crate::api::AppError;
    match e {
        AppError::NotFound { resource, id } => BridgeError::NotFound(format!("{resource} '{id}'")),
        other => BridgeError::Internal(format!("{other:?}")),
    }
}

fn b64<T: bytemuck::Pod>(values: &[T]) -> String {
    #[cfg(target_endian = "big")]
    compile_error!("graph_snapshot columns are little-endian");
    base64::engine::general_purpose::STANDARD.encode(bytemuck::cast_slice(values))
}

/// Per-connection state, unregistered on drop.
pub struct Connection {
    id: u64,
    service: Arc<UiService>,
    subscribe: Mutex<Option<SubscribeParams>>,
}

impl Connection {
    /// Set once this connection asked to `subscribe`.
    pub fn take_subscription(&self) -> Option<SubscribeParams> {
        self.subscribe
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.service
            .clients
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}

pub struct Observation {
    pub ctx: Arc<EventContext>,
    agent: Option<String>,
    query: Option<(String, String)>,
    reinforce: Option<(MemoryId, Option<u8>, Option<f32>)>,
    store: bool,
    started_at: i64,
}

/// Stream events to a subscribed connection until it closes.
pub async fn serve_subscription<R, W>(
    reader: &mut R,
    writer: &mut W,
    params: SubscribeParams,
    peer_limit: usize,
    mut shutdown: watch::Receiver<bool>,
) where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    use crate::daemon::protocol::write_framed_value;
    use tokio::sync::broadcast::error::RecvError;
    let mut sub = events::subscribe();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(15));
    heartbeat.tick().await;
    let mut buf = [0u8; 256];
    loop {
        let message = tokio::select! {
            event = sub.rx.recv() => match event {
                Ok(event) => {
                    if !params.kinds.is_empty() && !params.kinds.iter().any(|k| k == event.kind()) {
                        continue;
                    }
                    json!({"jsonrpc": "2.0", "method": "event", "params": &*event})
                }
                Err(RecvError::Lagged(missed)) => json!({
                    "jsonrpc": "2.0", "method": "event",
                    "params": UiEvent::Lagged(events::LaggedEvent { missed }),
                }),
                Err(RecvError::Closed) => break,
            },
            _ = heartbeat.tick() => json!({"jsonrpc": "2.0", "method": "heartbeat", "params": {}}),
            // Anything the peer sends on a subscription is ignored; EOF ends it.
            read = reader.read(&mut buf) => match read {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            },
            _ = shutdown.changed() => break,
        };
        if write_framed_value(writer, &message, peer_limit)
            .await
            .is_err()
        {
            break;
        }
    }
}

// ── Loose parameter / result readers ───────────────────────────────────
// Observation spans several RPC generations; read their shared field names
// without binding to any one shape. Depth-limited.

fn find<'a>(v: &'a Value, key: &str, depth: u8) -> Option<&'a Value> {
    let obj = v.as_object()?;
    if let Some(found) = obj.get(key) {
        return Some(found);
    }
    if depth == 0 {
        return None;
    }
    obj.values()
        .filter(|v| v.is_object())
        .find_map(|v| find(v, key, depth - 1))
}

fn find_str(v: &Value, key: &str) -> Option<String> {
    find(v, key, 3)?.as_str().map(str::to_owned)
}

fn find_u8(v: &Value, keys: &[&str]) -> Option<u8> {
    keys.iter()
        .find_map(|k| find(v, k, 3)?.as_u64())
        .and_then(|n| u8::try_from(n).ok())
}

fn find_id(v: &Value) -> Option<MemoryId> {
    ["id", "memoryId"].iter().find_map(|k| {
        let s = find(v, k, 3)?.as_str()?;
        uuid::Uuid::parse_str(s).ok().map(MemoryId::from_uuid)
    })
}

/// Every UUID-shaped `id` / `memoryId` string in a result (bounded depth).
fn uuids(v: &Value, depth: u8, out: &mut Vec<MemoryId>) {
    if depth > 6 {
        return;
    }
    match v {
        Value::Object(obj) => {
            for (k, v) in obj {
                if matches!(k.as_str(), "id" | "memoryId")
                    && let Some(id) = v.as_str().and_then(|s| uuid::Uuid::parse_str(s).ok())
                {
                    out.push(MemoryId::from_uuid(id));
                } else {
                    uuids(v, depth + 1, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| uuids(i, depth + 1, out)),
        _ => {}
    }
}

/// Memory ids a recall returned, across result shapes.
fn returned_ids(v: &Value) -> Vec<String> {
    fn walk(v: &Value, depth: u8, out: &mut Vec<String>) {
        if depth > 4 {
            return;
        }
        if let Some(obj) = v.as_object() {
            for (key, value) in obj {
                if matches!(key.as_str(), "hits" | "results" | "memories")
                    && let Some(items) = value.as_array()
                {
                    out.extend(
                        items
                            .iter()
                            .filter_map(|i| i.get("id")?.as_str().map(str::to_owned)),
                    );
                    continue;
                }
                walk(value, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(v, 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loose_readers_cover_rpc_generations() {
        let id = MemoryId::new();
        let v = json!({"arguments": {"memoryId": id.to_string(), "quality": 4}});
        assert_eq!(find_id(&v), Some(id));
        assert_eq!(find_u8(&v, &["quality", "rating"]), Some(4));
        let r = json!({"reply": {"results": [{"id": "a"}, {"id": "b"}]}, "hits": [{"id": "c"}]});
        let mut got = returned_ids(&r);
        got.sort();
        assert_eq!(got, ["a", "b", "c"]);
    }
}
