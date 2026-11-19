//! Live activity bus for the local UI.
//!
//! A lossy `tokio::sync::broadcast` channel. Publishing never blocks a
//! request, and with no subscriber attached nothing is built: every hook
//! checks [`active`] (one relaxed atomic load) before it touches a payload.
//!
//! Hooks that sit below the daemon dispatcher (the graph, the RIF adapter)
//! learn who is acting from a task-local [`EventContext`]. The dispatcher
//! installs one only while somebody is subscribed.

use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use serde::Serialize;
use tokio::sync::broadcast;
use ts_rs::TS;

use crate::model::{DecayPhase, EdgeType, MemoryId};

/// Events buffered per subscriber before the oldest are dropped.
pub const CAPACITY: usize = 1024;

static BUS: LazyLock<broadcast::Sender<Arc<UiEvent>>> =
    LazyLock::new(|| broadcast::channel(CAPACITY).0);
static SUBSCRIBERS: AtomicUsize = AtomicUsize::new(0);
static SEQ: AtomicU64 = AtomicU64::new(1);

/// Whether anybody is listening. The only cost the hot path pays without a UI.
#[inline]
pub fn active() -> bool {
    SUBSCRIBERS.load(Ordering::Relaxed) > 0
}

/// A live subscription. Dropping it stops event construction once the
/// last one is gone.
pub struct Subscription {
    pub rx: broadcast::Receiver<Arc<UiEvent>>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        SUBSCRIBERS.fetch_sub(1, Ordering::Relaxed);
    }
}

pub fn subscribe() -> Subscription {
    let rx = BUS.subscribe();
    SUBSCRIBERS.fetch_add(1, Ordering::Relaxed);
    Subscription { rx }
}

/// Publish an event if anybody is listening; `make` runs only then.
#[inline]
pub fn publish(make: impl FnOnce(u64, i64) -> UiEvent) {
    if active() {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let _ = BUS.send(Arc::new(make(seq, now_millis())));
    }
}

pub(crate) fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ── Event payloads ─────────────────────────────────────────────────────

/// One activity event. `kind` tags the variant on the wire.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiEvent {
    Recall(RecallEvent),
    Store(StoreEvent),
    Link(LinkEvent),
    Reinforce(ReinforceEvent),
    Forget(ForgetEvent),
    PhaseChange(PhaseChangeEvent),
    Sweep(SweepEvent),
    /// The subscriber fell behind and this many events were dropped.
    Lagged(LaggedEvent),
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecallEvent {
    pub seq: u64,
    pub at: i64,
    pub agent: Option<String>,
    pub namespace: String,
    pub query: String,
    pub seeds: Vec<Seed>,
    pub trace: Vec<TraceStep>,
    pub returned: Vec<String>,
    pub suppressed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct Seed {
    pub id: String,
    pub score: f32,
}

/// One spread of activation along an edge, in PQSA firing order.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TraceStep {
    pub from: String,
    pub to: String,
    /// `EdgeType` discriminant.
    pub edge_type: u8,
    pub amount: f32,
    /// Index of the firing node in PQSA order (0 = first expansion).
    pub order: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StoreEvent {
    pub seq: u64,
    pub at: i64,
    pub agent: Option<String>,
    pub id: String,
    pub namespace_id: u32,
    /// `DecayPhase` discriminant.
    pub phase: u8,
    /// Links that existed when the store returned; later auto-links arrive
    /// as `link` events.
    pub edges: Vec<EdgeRef>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EdgeRef {
    pub to: String,
    pub edge_type: u8,
    pub weight: f32,
}

/// An edge added to the graph (a store's auto-links arrive as these).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkEvent {
    pub seq: u64,
    pub at: i64,
    pub agent: Option<String>,
    pub src: String,
    pub dst: String,
    pub edge_type: u8,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReinforceEvent {
    pub seq: u64,
    pub at: i64,
    pub agent: Option<String>,
    pub id: String,
    pub rating: Option<u8>,
    pub stability_before: Option<f32>,
    pub stability_after: Option<f32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ForgetEvent {
    pub seq: u64,
    pub at: i64,
    pub agent: Option<String>,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PhaseChangeEvent {
    pub seq: u64,
    pub at: i64,
    pub id: String,
    pub from: u8,
    pub to: u8,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SweepEvent {
    pub seq: u64,
    pub at: i64,
    pub moved: SweepMoved,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SweepMoved {
    pub full_to_summary: u64,
    pub summary_to_ghost: u64,
    pub ghost_to_deleted: u64,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct LaggedEvent {
    pub missed: u64,
}

impl UiEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Recall(_) => "recall",
            Self::Store(_) => "store",
            Self::Link(_) => "link",
            Self::Reinforce(_) => "reinforce",
            Self::Forget(_) => "forget",
            Self::PhaseChange(_) => "phase_change",
            Self::Sweep(_) => "sweep",
            Self::Lagged(_) => "lagged",
        }
    }
}

// ── Task-local context ─────────────────────────────────────────────────

/// Who is acting, and the recall trace being collected, for hooks that run
/// below the daemon dispatcher.
#[derive(Default)]
pub struct EventContext {
    pub agent: Option<String>,
    pub recall: Option<Mutex<RecallTrace>>,
}

/// What one recall did inside the pipeline.
#[derive(Default, Debug)]
pub struct RecallTrace {
    pub seeds: Vec<(MemoryId, f32)>,
    pub steps: Vec<crate::graph::activation::SpreadStep>,
    pub suppressed: Vec<MemoryId>,
}

tokio::task_local! {
    static CONTEXT: Arc<EventContext>;
}

/// Run `fut` with `ctx` visible to the hooks below.
pub async fn scope<F: std::future::Future>(ctx: Arc<EventContext>, fut: F) -> F::Output {
    CONTEXT.scope(ctx, fut).await
}

/// The last agent that sent a write, for hooks that run outside its task
/// (derived-index completion). Best-effort: concurrent writers can blur it.
static RECENT_WRITER: Mutex<Option<(String, std::time::Instant)>> = Mutex::new(None);
const RECENT_WRITER_TTL: std::time::Duration = std::time::Duration::from_secs(5);

pub fn note_writer(agent: Option<&str>) {
    if let Some(agent) = agent {
        *RECENT_WRITER.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((agent.to_owned(), std::time::Instant::now()));
    }
}

fn current_agent() -> Option<String> {
    CONTEXT
        .try_with(|c| c.agent.clone())
        .ok()
        .flatten()
        .or_else(|| {
            RECENT_WRITER
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .filter(|(_, at)| at.elapsed() < RECENT_WRITER_TTL)
                .map(|(agent, _)| agent.clone())
        })
}

/// Whether a recall trace is being collected in this task.
pub fn tracing_recall() -> bool {
    CONTEXT.try_with(|c| c.recall.is_some()).unwrap_or(false)
}

/// Add to the recall trace collected in this task, if any.
pub fn with_recall(f: impl FnOnce(&mut RecallTrace)) {
    let _ = CONTEXT.try_with(|c| {
        if let Some(trace) = &c.recall {
            f(&mut trace.lock().unwrap_or_else(|e| e.into_inner()));
        }
    });
}

// ── Hooks called from core modules (only compiled with `ui`) ────────────

pub fn edge_added(src: MemoryId, dst: MemoryId, edge_type: EdgeType, weight: f32) {
    publish(|seq, at| {
        UiEvent::Link(LinkEvent {
            seq,
            at,
            agent: current_agent(),
            src: src.to_string(),
            dst: dst.to_string(),
            edge_type: edge_type as u8,
            weight,
        })
    });
}

pub fn phase_changed(id: MemoryId, from: DecayPhase, to: DecayPhase) {
    if from == to {
        return;
    }
    publish(|seq, at| {
        if to == DecayPhase::Tombstone {
            UiEvent::Forget(ForgetEvent {
                seq,
                at,
                agent: current_agent(),
                id: id.to_string(),
            })
        } else {
            UiEvent::PhaseChange(PhaseChangeEvent {
                seq,
                at,
                id: id.to_string(),
                from: from as u8,
                to: to as u8,
            })
        }
    });
}

pub fn node_removed(id: MemoryId) {
    publish(|seq, at| {
        UiEvent::Forget(ForgetEvent {
            seq,
            at,
            agent: current_agent(),
            id: id.to_string(),
        })
    });
}

pub fn suppressed(ids: impl Iterator<Item = MemoryId>) {
    if active() {
        with_recall(|t| t.suppressed.extend(ids));
    }
}

static LAST_SWEEP_AT: AtomicI64 = AtomicI64::new(0);
static SWEEP_INTERVAL_SECS: AtomicU64 = AtomicU64::new(0);

/// Called by the background decay sweep after every run.
pub fn sweep_finished(result: &crate::decay::sweep::SweepResult, interval: std::time::Duration) {
    LAST_SWEEP_AT.store(now_millis(), Ordering::Relaxed);
    SWEEP_INTERVAL_SECS.store(interval.as_secs(), Ordering::Relaxed);
    publish(|seq, at| {
        UiEvent::Sweep(SweepEvent {
            seq,
            at,
            moved: SweepMoved {
                full_to_summary: result.full_to_summary,
                summary_to_ghost: result.summary_to_ghost,
                ghost_to_deleted: result.deletions,
            },
        })
    });
}

/// `(last sweep millis, interval secs)`, zero when no background sweep ran yet.
pub fn sweep_schedule() -> (i64, u64) {
    (
        LAST_SWEEP_AT.load(Ordering::Relaxed),
        SWEEP_INTERVAL_SECS.load(Ordering::Relaxed),
    )
}

// ── Process counters read by Health → System ───────────────────────────

pub static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
pub static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static EMBED_LAST_US: AtomicU64 = AtomicU64::new(0);
static EMBED_EWMA_US: AtomicU64 = AtomicU64::new(0);

/// Record one embedding request's latency.
pub fn embedding_latency(elapsed: std::time::Duration) {
    let us = elapsed.as_micros() as u64;
    EMBED_LAST_US.store(us, Ordering::Relaxed);
    let prev = EMBED_EWMA_US.load(Ordering::Relaxed);
    let next = if prev == 0 { us } else { (prev * 7 + us) / 8 };
    EMBED_EWMA_US.store(next, Ordering::Relaxed);
}

/// `(last, smoothed)` embedding latency in microseconds; zero when none yet.
pub fn embedding_latency_us() -> (u64, u64) {
    (
        EMBED_LAST_US.load(Ordering::Relaxed),
        EMBED_EWMA_US.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_builds_nothing_without_subscribers() {
        // Other tests in this binary may hold subscriptions; only assert the
        // closure contract when none do.
        if active() {
            return;
        }
        let mut built = false;
        publish(|_, _| {
            built = true;
            UiEvent::Lagged(LaggedEvent { missed: 0 })
        });
        assert!(!built);
    }

    #[tokio::test]
    async fn subscribers_receive_and_counting_tracks_drops() {
        let mut sub = subscribe();
        assert!(active());
        let id = MemoryId::new();
        node_removed(id);
        loop {
            let event = sub.rx.recv().await.unwrap();
            if let UiEvent::Forget(f) = event.as_ref()
                && f.id == id.to_string()
            {
                break;
            }
        }
        drop(sub);
    }

    #[tokio::test]
    async fn context_scopes_agent_and_trace() {
        let ctx = Arc::new(EventContext {
            agent: Some("claude-code".into()),
            recall: Some(Mutex::new(RecallTrace::default())),
        });
        let inner = ctx.clone();
        scope(ctx.clone(), async move {
            assert_eq!(current_agent().as_deref(), Some("claude-code"));
            assert!(tracing_recall());
            with_recall(|t| t.suppressed.push(MemoryId::new()));
            drop(inner);
        })
        .await;
        assert_eq!(
            ctx.recall
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .suppressed
                .len(),
            1
        );
        assert!(!tracing_recall());
    }
}
