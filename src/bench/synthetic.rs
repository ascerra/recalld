//! Synthetic stores for the UI's perf harness and screenshot fixtures, and a
//! deterministic Ollama-compatible embedding stub so neither needs a model.
//!
//! Stores go through the real MCP tool path on an in-process `Recalld` whose
//! clock is driven forward in time, so created/accessed timestamps, access
//! history, auto-links and phases (after a real decay sweep) are all produced
//! by recalld itself.

use std::net::SocketAddr;
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use serde_json::{Value, json};

const DAY: i64 = 86_400_000;

/// Deterministic bag-of-words embedding (FNV-1a per lowercase word). Texts
/// that share words land close together. Mirrored by nothing else: both the
/// generator and the stub server call this.
pub fn hash_embed(text: &str, dim: usize) -> Vec<f32> {
    let mut v = vec![0f32; dim.max(1)];
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
    {
        let h = word.to_lowercase().bytes().fold(2_166_136_261u32, |h, b| {
            (h ^ b as u32).wrapping_mul(16_777_619)
        });
        let n = v.len();
        v[h as usize % n] += 1.0;
        v[(h >> 11) as usize % n] += 0.5;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        v[0] = 1.0;
    } else {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

/// Serve `POST /api/embed` (Ollama's shape) on `addr` until the process exits.
pub async fn serve_embed_stub(addr: SocketAddr, dim: usize) -> std::io::Result<()> {
    let app = axum::Router::new().route(
        "/api/embed",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| async move {
            let texts: Vec<String> = match &body["input"] {
                Value::String(s) => vec![s.clone()],
                Value::Array(items) => items
                    .iter()
                    .filter_map(|i| i.as_str().map(str::to_owned))
                    .collect(),
                _ => vec![],
            };
            axum::Json(json!({
                "embeddings": texts.iter().map(|t| hash_embed(t, dim)).collect::<Vec<_>>()
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!(
        "embedding stub ({dim} dims) on http://{}",
        listener.local_addr()?
    );
    axum::serve(listener, app).await
}

pub struct SyntheticOptions {
    pub memories: usize,
    /// Approximate edge target; auto/entity link caps are derived from it.
    pub edges: usize,
    /// The reference screens' sample content instead of generated topics.
    pub sample: bool,
    pub seed: u64,
    pub embed_dim: usize,
}

pub struct SyntheticSummary {
    pub memories: usize,
    pub edges: usize,
    pub namespaces: Vec<String>,
}

struct Spec {
    namespace: &'static str,
    summary: String,
    full_text: String,
    tags: Vec<String>,
    entities: Vec<String>,
    created_days_ago: f64,
    last_access_days_ago: f64,
    stability: f32,
}

/// The reference screens' memories (DESIGN `reference/Explore`), plus filler
/// so the namespace counts match the screens (98 / 25 / 12).
fn sample_specs(rng: &mut StdRng) -> Vec<Spec> {
    #[rustfmt::skip]
    let named: &[(&str, &str, f64, f64, f32)] = &[
        ("project-recalld", "Connection bonus keeps well-linked memories in Full", 78.0, 0.0, 60.0),
        ("project-recalld", "R_eff = R + bonus·(1−R), capped at 0.15", 78.0, 0.0, 41.0),
        ("project-recalld", "Fan ≥ 7 stops spreading activation", 76.0, 0.0, 30.0),
        ("project-recalld", "Auto-link threshold 0.50, max 15 links", 71.0, 0.0, 22.0),
        ("project-recalld", "Late reviews grow stability ~10×", 77.0, 0.0, 55.0),
        ("project-recalld", "Sweep runs Ghost → Summary → Full", 75.0, 0.0, 30.0),
        ("project-recalld", "Local UI: Sigma v4 + d3-force in a worker", 0.5, 0.01, 4.0),
        ("project-recalld", "UI reads must never record access", 0.6, 0.04, 9.0),
        ("default", "Start every session with get_session", 121.0, 1.0, 210.0),
        ("default", "Use recordAccess:false to inspect memories", 99.0, 3.0, 60.0),
        ("default", "Clean up scratch data after a PR lands", 5.0, 5.0, 1600.0),
        ("project-recalld", "Release checklist: Cargo.toml, CHANGELOG, tag, GHCR", 48.0, 6.0, 25.0),
        ("project-recalld", "Spreading activation expands at most 100 nodes", 62.0, 8.0, 14.0),
        ("topic-rust", "Rust 1.94, edition 2024", 142.0, 9.0, 300.0),
        ("default", "Keep delete_namespace off the allowlist", 110.0, 12.0, 120.0),
        ("project-recalld", "Supersedes chains resolve after filters", 58.0, 14.0, 20.0),
        ("project-recalld", "Embeddings: embeddinggemma:300m at 768 dims", 140.0, 21.0, 40.0),
        ("default", "Daemon idles out after 30 minutes", 91.0, 28.0, 10.0),
        ("project-recalld", "GHCR image builds for amd64 and arm64", 103.0, 35.0, 15.0),
        ("project-recalld", "edges.db stays apart from meta.db", 97.0, 42.0, 18.0),
        ("project-recalld", "Bonus ignores 2-hop neighbors", 81.0, 42.0, 8.0),
        ("project-recalld", "FTS boost capped at 0.05 over vector score", 86.0, 49.0, 12.0),
        ("project-recalld", "Over-fetch 8× the limit, up to 1,600", 79.0, 60.0, 6.0),
        ("project-recalld", "Compact text logs at 20% dead space", 100.0, 61.0, 9.0),
        ("project-recalld", "Sweep every 12 h", 114.0, 90.0, 5.0),
        ("project-recalld", "Podman needs --add-host for host.docker.internal", 160.0, 120.0, 4.0),
        ("project-recalld", "Embedding dimensions fixed per namespace", 312.0, 300.0, 4.0),
        ("project-recalld", "Vector slots are reused after compaction", 231.0, 210.0, 2.0),
    ];
    let words = [
        "decay",
        "sweep",
        "stability",
        "graph",
        "edge",
        "vector",
        "search",
        "namespace",
        "daemon",
        "socket",
        "embedding",
        "summary",
        "tombstone",
        "cache",
        "release",
        "docker",
        "agent",
        "session",
        "recall",
        "reinforce",
        "entity",
        "topic",
        "storage",
        "index",
    ];
    let mut out: Vec<Spec> = named
        .iter()
        .map(|&(ns, summary, created, last, s)| Spec {
            namespace: ns,
            summary: summary.into(),
            full_text: format!("{summary}. Recorded while working on recalld."),
            tags: vec![format!("topic/{ns}")],
            entities: vec!["recalld".into()],
            created_days_ago: created,
            last_access_days_ago: last,
            stability: s,
        })
        .collect();
    for (ns, total) in [("project-recalld", 98), ("default", 25), ("topic-rust", 12)] {
        let have = out.iter().filter(|s| s.namespace == ns).count();
        for _ in have..total {
            let pick: Vec<&str> = words.choose_multiple(rng, 4).copied().collect();
            let created = rng.gen_range(1.0..300.0);
            out.push(Spec {
                namespace: ns,
                summary: format!("{} {} for {} {}", cap(pick[0]), pick[1], pick[2], pick[3]),
                full_text: format!("Notes on {} and {}.", pick[0], pick[2]),
                tags: vec![format!("topic/{}", pick[0])],
                entities: vec![pick[1].into()],
                created_days_ago: created,
                // Most memories were touched recently; a long tail decays.
                last_access_days_ago: created * rng.gen_range(0.0f64..1.0).powf(1.5),
                stability: rng.gen_range(2.0f32..20.0),
            });
        }
    }
    out
}

fn cap(w: &str) -> String {
    let mut c = w.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Topic clusters of pseudo-words for the perf store.
fn perf_specs(rng: &mut StdRng, n: usize) -> Vec<Spec> {
    let syllables = [
        "ka", "lo", "mi", "ra", "tu", "ve", "zo", "ne", "pa", "si", "do", "ri", "ul", "an", "or",
        "es",
    ];
    let word = |rng: &mut StdRng| -> String {
        (0..rng.gen_range(2..4))
            .map(|_| *syllables.choose(rng).unwrap())
            .collect()
    };
    let topics = (n / 50).max(1);
    let vocab: Vec<(Vec<String>, Vec<String>)> = (0..topics)
        .map(|_| {
            (
                (0..10).map(|_| word(rng)).collect(),
                (0..3).map(|_| cap(&word(rng))).collect(),
            )
        })
        .collect();
    let namespaces = ["default", "project-alpha", "project-beta", "topic-notes"];
    (0..n)
        .map(|i| {
            let t = rng.gen_range(0..topics);
            let (words, entities) = &vocab[t];
            let pick: Vec<&String> = words.choose_multiple(rng, 5).collect();
            let created = 365.0 * (1.0 - i as f64 / n as f64) + rng.gen_range(0.0..0.5);
            Spec {
                namespace: namespaces[t % namespaces.len()],
                summary: format!(
                    "{} {} {} {} {}",
                    cap(pick[0]),
                    pick[1],
                    pick[2],
                    pick[3],
                    pick[4]
                ),
                full_text: format!("{} {} {}", pick[0], pick[2], pick[4]),
                tags: vec![format!("topic/t{t}")],
                entities: vec![entities.choose(rng).unwrap().clone()],
                created_days_ago: created,
                last_access_days_ago: rng.gen_range(0.0..created.max(0.01)),
                stability: rng.gen_range(1.0f32..200.0),
            }
        })
        .collect()
}

/// Build a store at `config.storage.data_dir`. The embedding provider in
/// `config` must be an endpoint serving [`hash_embed`] (see
/// [`serve_embed_stub`]); the caller starts one.
pub async fn generate(
    mut config: crate::RecalldConfig,
    opts: &SyntheticOptions,
) -> anyhow::Result<SyntheticSummary> {
    let mut rng = StdRng::seed_from_u64(opts.seed);
    let specs = if opts.sample {
        sample_specs(&mut rng)
    } else {
        perf_specs(&mut rng, opts.memories)
    };
    config.decay.disable_sweep = true;
    if !opts.sample {
        // Links per stored memory ≈ edges / memories, split auto / entity.
        let per = (opts.edges / specs.len().max(1)).clamp(1, 30);
        config.graph.max_auto_links = per.div_ceil(2);
        config.graph.max_entity_links = per / 2;
        config.graph.max_temporal_links = 1;
    }

    let now = chrono::Utc::now().timestamp_millis();
    let mut order: Vec<usize> = (0..specs.len()).collect();
    order.sort_by(|&a, &b| {
        specs[b]
            .created_days_ago
            .total_cmp(&specs[a].created_days_ago)
    });
    let start = now - (specs[order[0]].created_days_ago * DAY as f64) as i64 - DAY;
    let clock = Arc::new(crate::time::ManualClock::new(start));
    let system = crate::Recalld::with_clock(config.clone(), clock.clone()).await?;
    let bridge = crate::mcp::bridge_adapters::direct_bridge(&system, "default".into());

    let mut namespaces: Vec<String> = specs.iter().map(|s| s.namespace.to_owned()).collect();
    namespaces.sort();
    namespaces.dedup();
    for ns in namespaces.iter().filter(|n| *n != "default") {
        let result = crate::mcp::tools::handle_create_namespace_bounded(
            &bridge,
            json!({ "name": ns }),
            Some(crate::serialization::catalog_api1::CatalogReplyAdmission::http()),
        )
        .await;
        let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
        let created: Value =
            serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone()));
        anyhow::ensure!(
            created.get("error").is_none() && !created.is_string(),
            "create_namespace {ns} failed: {created}"
        );
    }

    // One timeline: stores, a few early agent recalls (Timeline activity,
    // RIF) and each memory's last access as a targeted get (touches only it),
    // replayed in time order on the manual clock.
    enum Step {
        Store(usize),
        Recall(usize),
        Access(usize),
        Reinforce(usize),
    }
    let mut timeline: Vec<(i64, u8, Step)> = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        let s = &specs[i];
        let created = now - (s.created_days_ago * DAY as f64) as i64;
        timeline.push((created, 0, Step::Store(i)));
        if s.last_access_days_ago < s.created_days_ago - 0.001 {
            let at = now - (s.last_access_days_ago * DAY as f64) as i64;
            // Spaced reviews grow stability toward the spec's target, the way
            // FSRS actually grows it (the store ignores initialStability here).
            let reviews = ((s.stability / 3.7).ln() / 3.5f32.ln())
                .round()
                .clamp(0.0, 6.0) as i64;
            for r in 1..=reviews {
                let t = created + (at - created) * r / (reviews + 1);
                timeline.push((t, 1, Step::Reinforce(i)));
            }
            timeline.push((at, 2, Step::Access(i)));
            if k % 4 == 0 && s.created_days_ago - s.last_access_days_ago > 2.0 {
                timeline.push(((created + at) / 2, 1, Step::Recall(i)));
            }
        }
    }
    timeline.sort_by_key(|(at, rank, _)| (*at, *rank));
    let mut ids = vec![String::new(); specs.len()];
    for (at, _, step) in timeline {
        let _ = clock.advance_to(at.max(clock_now(&clock)));
        match step {
            Step::Store(i) => {
                let s = &specs[i];
                let stored = tool(
                    &bridge,
                    "store_memory",
                    json!({
                        "summary": s.summary, "fullText": s.full_text, "tags": s.tags,
                        "entities": s.entities, "namespace": s.namespace,
                        "initialStability": s.stability, "checkDuplicates": false,
                    }),
                )
                .await;
                ids[i] = match stored["id"].as_str() {
                    Some(id) => id.to_owned(),
                    None => anyhow::bail!("store_memory failed for {:?}: {stored}", s.summary),
                };
            }
            Step::Recall(i) => {
                let s = &specs[i];
                tool(
                    &bridge,
                    "recall_memories",
                    json!({ "query": s.summary, "namespace": s.namespace, "limit": 3 }),
                )
                .await;
            }
            Step::Reinforce(i) => {
                tool(
                    &bridge,
                    "reinforce_memory",
                    json!({ "id": ids[i], "quality": 4 }),
                )
                .await;
            }
            Step::Access(i) => {
                tool(
                    &bridge,
                    "get_memory",
                    json!({ "id": ids[i], "recordAccess": true }),
                )
                .await;
            }
        }
    }
    let _ = clock.advance_to(now.max(clock_now(&clock)));

    let probe = |label: &str| {
        use crate::storage::StorageEngine as _;
        if std::env::var_os("RECALLD_SYNTH_DEBUG").is_none() {
            return;
        }
        let s = system.storage().read().unwrap();
        for (_, r) in s.scan_all().unwrap().iter().take(4) {
            eprintln!(
                "{label}: created {} ago {:.1}d, accessed {:.1}d ago, S {:.1}, phase {:?}, history {}",
                r.summary,
                (now - r.created_at) as f64 / DAY as f64,
                (now - r.last_accessed_at) as f64 / DAY as f64,
                r.stability,
                r.phase,
                r.access_history.len()
            );
        }
    };
    probe("before sweep");
    // One real decay sweep at "now" settles every phase, through the same
    // path as `POST /decay/sweep`.
    {
        use crate::api::FsrsEngine as _;
        let decay = crate::api::adapters::FsrsEngineAdapter::new(
            system.storage().clone(),
            system.cache().clone(),
            system.graph().clone(),
            Arc::new(system.config().clone()),
            true,
        )
        .with_clock(system.clock().clone())
        .with_native_lifecycle_policy(system.query_engine().native_policy_inputs())
        .with_indexes(
            system.vector_index().clone(),
            system.fts_index().clone(),
            system.entity_index().clone(),
        );
        // A sweep moves a memory one phase at a time; repeat until settled.
        for _ in 0..4 {
            let swept = decay
                .trigger_sweep(None)
                .await
                .map_err(|e| anyhow::anyhow!("decay sweep failed: {e}"))?;
            eprintln!(
                "decay sweep: {} Full→Summary, {} Summary→Ghost, {} deleted",
                swept.full_to_summary, swept.summary_to_ghost, swept.deletions
            );
            if swept.full_to_summary + swept.summary_to_ghost + swept.deletions == 0 {
                break;
            }
        }
    }
    probe("after sweep");
    let edges = system.graph().read().await.edge_count();
    let memories = ids.iter().filter(|id| !id.is_empty()).count();
    system.shutdown().await?;
    Ok(SyntheticSummary {
        memories,
        edges,
        namespaces,
    })
}

fn clock_now(clock: &crate::time::ManualClock) -> i64 {
    use crate::time::Clock as _;
    clock.now_millis()
}

async fn tool(bridge: &crate::mcp::bridge::McpBridge, name: &str, args: Value) -> Value {
    let result = crate::mcp::tools::dispatch_tool(bridge, name, args).await;
    let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone()))
}

/// Agent-path recall latency against a daemon over the store in `config`:
/// `search` RPCs (what `recall_memories` sends), optionally with a UI event
/// subscriber attached. Used to show recall cost is unchanged without a UI.
pub async fn recall_latency(
    config: crate::RecalldConfig,
    queries: usize,
    subscribe: bool,
    dim: usize,
) -> anyhow::Result<Value> {
    let socket = std::path::PathBuf::from(&config.storage.data_dir).with_extension("bench.sock");
    let _ = std::fs::remove_file(&socket);
    let system = crate::Recalld::new(config).await?;
    let server = crate::daemon::DaemonServer::new(system);
    let run_socket = socket.clone();
    tokio::spawn(async move {
        let _ = server
            .run(&run_socket, Some(std::time::Duration::from_secs(3600)))
            .await;
    });
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    #[cfg(feature = "ui")]
    let _subscription = if subscribe {
        use crate::daemon::protocol::{
            DaemonRequest, MAX_MESSAGE_SIZE, read_frame, write_framed_request,
        };
        let stream = tokio::net::UnixStream::connect(&socket).await?;
        let (r, mut w) = stream.into_split();
        let mut r = tokio::io::BufReader::new(r);
        write_framed_request(
            &mut w,
            &DaemonRequest {
                jsonrpc: "2.0".into(),
                id: 1,
                method: "subscribe".into(),
                params: json!({}),
            },
            MAX_MESSAGE_SIZE,
        )
        .await?;
        read_frame(&mut r, MAX_MESSAGE_SIZE).await?;
        Some(tokio::spawn(async move {
            let _w = w;
            while read_frame(&mut r, MAX_MESSAGE_SIZE).await.is_ok() {}
        }))
    } else {
        None
    };
    #[cfg(not(feature = "ui"))]
    anyhow::ensure!(!subscribe, "--subscribe needs a build with --features ui");

    let client = crate::daemon::DaemonClient::connect(&socket).await?;
    client.ping().await?;
    let mut rng = StdRng::seed_from_u64(11);
    let syllables = ["ka", "lo", "mi", "ra", "tu", "ve", "zo", "ne", "pa", "si"];
    let mut times = Vec::with_capacity(queries);
    for i in 0..queries + 20 {
        let q: String = (0..3)
            .map(|_| {
                (0..3)
                    .map(|_| *syllables.choose(&mut rng).unwrap())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ");
        let started = std::time::Instant::now();
        client
            .call(
                "search",
                json!({"query": q, "namespace": "default", "limit": 10, "tags": [], "entities": [],
                       "topics": [], "emotions": [], "minStrength": null, "depth": 1,
                       "timeRangeStart": null, "timeRangeEnd": null}),
            )
            .await?;
        if i >= 20 {
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    times.sort_by(f64::total_cmp);
    let pct = |p: f64| times[((times.len() as f64 * p) as usize).min(times.len() - 1)];
    let _ = dim;
    let _ = std::fs::remove_file(&socket);
    Ok(json!({
        "queries": times.len(),
        "subscriber": subscribe,
        "uiFeature": cfg!(feature = "ui"),
        "p50Ms": pct(0.5),
        "p95Ms": pct(0.95),
        "meanMs": times.iter().sum::<f64>() / times.len() as f64,
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn hash_embed_is_unit_and_word_sensitive() {
        let a = super::hash_embed("decay sweep stability", 64);
        let b = super::hash_embed("Decay, sweep and stability", 64);
        let c = super::hash_embed("docker release image", 64);
        let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
        assert!((dot(&a, &a) - 1.0).abs() < 1e-5);
        assert!(dot(&a, &b) > dot(&a, &c));
    }
}
