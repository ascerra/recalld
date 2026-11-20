//! Read-only memory methods for the UI: Explore rows, neighborhoods, access
//! history and search with its score breakdown. None of them record access.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use super::daemon::UiService;
use super::events::{self, EventContext, RecallTrace};
use super::wire::*;
use crate::decay::fsrs::FsrsEngine;
use crate::health::report::decay_config_for_namespace;
use crate::mcp::bridge::BridgeError;
use crate::model::record::DiskRecord;
use crate::model::{AccessKind, DecayPhase, EdgeType, MemoryId, NamespaceConfig};
use crate::storage::StorageEngine as _;

const DAY: f64 = 86_400_000.0;
pub const MAX_PAGE: usize = 500;
const MAX_NEIGHBORS: usize = 150;

/// Sorted Explore rows for one filter set, valid for one effects stamp.
pub(super) struct RowCache {
    entries: VecDeque<(String, u64, Arc<Vec<MemoryRow>>)>,
}

impl RowCache {
    pub(super) fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }
}

fn internal(e: impl std::fmt::Display) -> BridgeError {
    BridgeError::Internal(e.to_string())
}

fn parse_id(s: &str) -> Result<MemoryId, BridgeError> {
    uuid::Uuid::parse_str(s)
        .map(MemoryId::from_uuid)
        .map_err(|e| BridgeError::InvalidInput(format!("invalid memory ID: {e}")))
}

/// R now and the next transition, exactly as the health report forecasts it.
fn decay_now(r: &DiskRecord, ns: Option<&NamespaceConfig>, now: i64) -> (f32, Option<NextChange>) {
    let dc = ns.map(decay_config_for_namespace).unwrap_or_default();
    let engine = FsrsEngine::new(&dc);
    let elapsed_days = ((now - r.last_accessed_at).max(0) as f64 / DAY) as f32;
    let current = if r.stability > 0.0 {
        engine.retrievability(elapsed_days, r.stability, 1.0)
    } else {
        r.strength
    };
    if r.is_permastore != 0 || r.stability <= 0.0 {
        return (current, None);
    }
    let (threshold, to) = match r.phase {
        DecayPhase::Full => (dc.phase_1_threshold, 2),
        DecayPhase::Summary => (dc.phase_2_threshold, 3),
        DecayPhase::Ghost => (dc.phase_3_threshold, 4),
        DecayPhase::Tombstone => return (current, None),
    };
    let at = if current <= threshold {
        now
    } else {
        let days = engine.days_until_threshold(r.stability, threshold) as f64;
        r.last_accessed_at + (days * DAY) as i64
    };
    (current, Some(NextChange { at, to }))
}

/// Tag/entity filters (AND) and the Explore sort, shared by both paths.
fn filter_sort(
    rows: Vec<MemoryRow>,
    tags: &[String],
    entities: &[String],
    sort: &str,
    desc: bool,
) -> Vec<MemoryRow> {
    let mut rows: Vec<MemoryRow> = rows
        .into_iter()
        .filter(|m| tags.iter().all(|t| m.tags.iter().any(|x| x == t)))
        .filter(|m| {
            entities
                .iter()
                .all(|e| m.entities.iter().any(|x| x.to_lowercase() == *e))
        })
        .collect();
    match sort {
        "strength" => rows.sort_by(|a, b| a.strength.total_cmp(&b.strength)),
        "created" => rows.sort_by_key(|m| m.created_at),
        "nextChange" => rows.sort_by_key(|m| m.next_change.as_ref().map_or(i64::MAX, |n| n.at)),
        "summary" => rows.sort_by(|a, b| {
            a.summary
                .as_deref()
                .unwrap_or("\u{10FFFF}")
                .to_lowercase()
                .cmp(&b.summary.as_deref().unwrap_or("\u{10FFFF}").to_lowercase())
        }),
        _ => rows.sort_by_key(|m| m.last_accessed_at),
    }
    if desc {
        rows.reverse();
    }
    rows
}

fn row(
    id: MemoryId,
    r: &DiskRecord,
    ns: &HashMap<u32, NamespaceConfig>,
    degree: u32,
    now: i64,
) -> MemoryRow {
    let config = ns.get(&r.namespace_id);
    let (strength, next_change) = decay_now(r, config, now);
    let labels = r.schema.as_ref().map(|s| s.labels.clone());
    let meta = crate::model::structured_metadata(&r.tags, labels.as_ref());
    let derived = |t: &str| {
        ["entity/", "topic/", "emotion/"]
            .iter()
            .any(|p| t.starts_with(p))
    };
    MemoryRow {
        id: id.to_string(),
        namespace: config.map(|n| n.name.clone()).unwrap_or_default(),
        summary: (!matches!(r.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
            && !r.summary.is_empty())
        .then(|| r.summary.clone()),
        phase: r.phase as u8,
        strength,
        stability: r.stability,
        created_at: r.created_at,
        last_accessed_at: r.last_accessed_at,
        permastore: r.is_permastore != 0,
        degree,
        next_change,
        tags: r
            .tags
            .iter()
            .map(|t| t.as_str().to_owned())
            .filter(|t| !derived(t))
            .collect(),
        entities: meta.entities,
    }
}

impl UiService {
    fn namespace_map(&self) -> Result<HashMap<u32, NamespaceConfig>, BridgeError> {
        Ok(self
            .storage
            .read()
            .map_err(internal)?
            .list_namespaces()
            .map_err(internal)?
            .into_iter()
            .map(|n| (n.id.get(), n))
            .collect())
    }

    async fn degrees(&self) -> HashMap<MemoryId, u32> {
        let graph = self.graph.read().await;
        graph
            .iter_nodes()
            .map(|(_, n)| (n.memory_id, n.degree() as u32))
            .collect()
    }

    pub async fn memory_page(&self, p: MemoryPageParams) -> Result<MemoryPage, BridgeError> {
        let limit = p.limit.unwrap_or(200).clamp(1, MAX_PAGE);
        let now = events::now_millis();
        let ns = self.namespace_map()?;
        let ns_filter = match &p.namespace {
            Some(name) => Some(
                ns.values()
                    .find(|n| &n.name == name)
                    .map(|n| n.id.get())
                    .ok_or_else(|| BridgeError::NotFound(format!("namespace '{name}'")))?,
            ),
            None => None,
        };
        let phases: HashSet<u8> = if p.phases.is_empty() {
            [1, 2, 3].into()
        } else {
            p.phases.iter().copied().collect()
        };
        let degrees = self.degrees().await;

        let mut tags = p.tags.clone();
        tags.sort();
        let mut entities: Vec<String> = p.entities.iter().map(|e| e.to_lowercase()).collect();
        entities.sort();
        let sort = p.sort.clone().unwrap_or_else(|| "lastAccess".into());
        let desc = p
            .desc
            .unwrap_or(matches!(sort.as_str(), "lastAccess" | "created"));
        // Explicit ids: a handful of point reads, never cached.
        if !p.ids.is_empty() {
            let ids = p
                .ids
                .iter()
                .take(MAX_PAGE)
                .map(|s| parse_id(s))
                .collect::<Result<Vec<_>, _>>()?;
            let storage = self.storage.clone();
            let records = tokio::task::spawn_blocking(move || {
                let s = storage.read().map_err(internal)?;
                ids.into_iter()
                    .map(|id| Ok((id, s.get_record(id).map_err(internal)?)))
                    .collect::<Result<Vec<_>, BridgeError>>()
            })
            .await
            .map_err(internal)??;
            let rows: Vec<MemoryRow> = records
                .into_iter()
                .filter_map(|(id, r)| r.map(|r| (id, r)))
                .filter(|(_, r)| ns_filter.is_none_or(|n| n == r.namespace_id))
                .filter(|(_, r)| p.phases.is_empty() || phases.contains(&(r.phase as u8)))
                .map(|(id, r)| row(id, &r, &ns, degrees.get(&id).copied().unwrap_or(0), now))
                .collect();
            let rows = filter_sort(rows, &tags, &entities, &sort, desc);
            return Ok(MemoryPage {
                total: rows.len(),
                offset: p.offset,
                rows: rows.into_iter().skip(p.offset).take(limit).collect(),
            });
        }

        let mut phase_key: Vec<_> = phases.iter().copied().collect();
        phase_key.sort();
        let key = format!("{ns_filter:?}|{phase_key:?}|{tags:?}|{entities:?}|{sort}|{desc}");
        let stamp = self
            .api_storage
            .effects_stamp()
            .await
            .map_err(|e| internal(format!("{e:?}")))?;

        let cached = {
            let cache = self.rows.lock().unwrap_or_else(|e| e.into_inner());
            cache
                .entries
                .iter()
                .find(|(k, s, _)| k == &key && *s == stamp)
                .map(|(_, _, rows)| rows.clone())
        };
        let rows = match cached {
            Some(rows) => rows,
            None => {
                let storage = self.storage.clone();
                let all = tokio::task::spawn_blocking(move || {
                    storage
                        .read()
                        .map_err(internal)?
                        .scan_all()
                        .map_err(internal)
                })
                .await
                .map_err(internal)??;
                let rows: Vec<MemoryRow> = all
                    .iter()
                    .filter(|(_, r)| ns_filter.is_none_or(|n| n == r.namespace_id))
                    .filter(|(_, r)| phases.contains(&(r.phase as u8)))
                    .map(|(id, r)| row(*id, r, &ns, degrees.get(id).copied().unwrap_or(0), now))
                    .collect();
                let rows = filter_sort(rows, &tags, &entities, &sort, desc);
                let rows = Arc::new(rows);
                let mut cache = self.rows.lock().unwrap_or_else(|e| e.into_inner());
                cache.entries.retain(|(_, s, _)| *s == stamp);
                cache.entries.push_front((key, stamp, rows.clone()));
                cache.entries.truncate(4);
                rows
            }
        };
        Ok(MemoryPage {
            total: rows.len(),
            offset: p.offset,
            rows: rows.iter().skip(p.offset).take(limit).cloned().collect(),
        })
    }

    pub async fn memory_neighbors(&self, p: NeighborsParams) -> Result<Neighborhood, BridgeError> {
        let center = parse_id(&p.id)?;
        let depth = p.depth.unwrap_or(1).clamp(1, 2);
        let (hops, edges, truncated) = {
            let graph = self.graph.read().await;
            if !graph.contains(&center) {
                return Err(BridgeError::NotFound(format!("memory {center}")));
            }
            let mut hops: HashMap<MemoryId, u8> = HashMap::from([(center, 0)]);
            let mut frontier = vec![center];
            let mut truncated = false;
            for hop in 1..=depth {
                let mut next = Vec::new();
                for id in &frontier {
                    for e in graph.edges_for(id) {
                        let (Some(s), Some(t)) =
                            (graph.nodes.get(e.source), graph.nodes.get(e.target))
                        else {
                            continue;
                        };
                        let other = if s.memory_id == *id {
                            t.memory_id
                        } else {
                            s.memory_id
                        };
                        if hops.contains_key(&other) {
                            continue;
                        }
                        if hops.len() >= MAX_NEIGHBORS {
                            truncated = true;
                            break;
                        }
                        hops.insert(other, hop);
                        next.push(other);
                    }
                }
                frontier = next;
            }
            let mut edges = Vec::new();
            for (_, e) in graph.iter_edges() {
                let (Some(s), Some(t)) = (graph.nodes.get(e.source), graph.nodes.get(e.target))
                else {
                    continue;
                };
                if hops.contains_key(&s.memory_id) && hops.contains_key(&t.memory_id) {
                    edges.push(NeighborEdge {
                        src: s.memory_id.to_string(),
                        dst: t.memory_id.to_string(),
                        kind: e.edge_type as u8,
                        weight: e.weight,
                    });
                }
            }
            (hops, edges, truncated)
        };
        let storage = self.storage.clone();
        let ids: Vec<(MemoryId, u8)> = hops.into_iter().collect();
        let nodes = tokio::task::spawn_blocking(move || {
            let s = storage.read().map_err(internal)?;
            let mut nodes = Vec::with_capacity(ids.len());
            for (id, hop) in ids {
                if let Some(r) = s.get_record(id).map_err(internal)? {
                    nodes.push(NeighborNode {
                        id: id.to_string(),
                        namespace_id: r.namespace_id,
                        summary: (!matches!(r.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
                            && !r.summary.is_empty())
                        .then_some(r.summary),
                        phase: r.phase as u8,
                        hop,
                    });
                }
            }
            nodes.sort_by_key(|n| n.hop);
            Ok::<_, BridgeError>(nodes)
        })
        .await
        .map_err(internal)??;
        Ok(Neighborhood {
            center: center.to_string(),
            nodes,
            edges,
            truncated,
        })
    }

    pub async fn access_history(&self, p: HistoryParams) -> Result<AccessHistory, BridgeError> {
        let id = parse_id(&p.id)?;
        let limit = p.limit.unwrap_or(20).clamp(1, 100);
        let storage = self.storage.clone();
        let record = tokio::task::spawn_blocking(move || {
            storage
                .read()
                .map_err(internal)?
                .get_record(id)
                .map_err(internal)
        })
        .await
        .map_err(internal)??
        .ok_or_else(|| BridgeError::NotFound(format!("memory {id}")))?;
        let total = record.access_history.len();
        let events = record
            .access_history
            .iter()
            .rev()
            .take(limit)
            .map(|e| {
                let note = match e.kind {
                    AccessKind::DecaySweep => None,
                    kind => super::events::attribute(
                        &p.id,
                        e.timestamp,
                        kind == AccessKind::ManualReinforcement,
                    ),
                };
                AccessEventWire {
                    at: e.timestamp,
                    kind: match e.kind {
                        AccessKind::DirectRetrieval => "direct",
                        AccessKind::AssociativeRetrieval => "associative",
                        AccessKind::DecaySweep => "sweep",
                        AccessKind::ManualReinforcement => "reinforce",
                    }
                    .into(),
                    agent: note.as_ref().and_then(|n| n.agent.clone()),
                    query: note.and_then(|n| n.query.clone()),
                }
            })
            .collect();
        Ok(AccessHistory {
            id: id.to_string(),
            events,
            total,
        })
    }

    /// recalld's hybrid search, read-only, with the pipeline's own score
    /// breakdown and the activation path that reached linked results.
    pub async fn search_explain(&self, p: ExplainParams) -> Result<SearchExplain, BridgeError> {
        let ns = self
            .storage
            .read()
            .map_err(internal)?
            .get_namespace_by_name(&p.namespace)
            .map_err(internal)?
            .ok_or_else(|| BridgeError::NotFound(format!("namespace '{}'", p.namespace)))?;
        let profile = match ns.schema.as_ref().map(|s| &s.embedding_binding) {
            Some(crate::model::schema::EmbeddingBinding::Bound { profile_id, .. }) => {
                profile_id.clone()
            }
            _ => format!("legacy:{}", ns.embedding_dim),
        };
        // Built exactly like the MCP adapter's query (no filters), but with
        // recordAccess forced off.
        let query = crate::search::SearchQuery {
            record_access: false,
            query_embedding: None,
            text: Some(p.query),
            fts_query: None,
            namespace: p.namespace.clone(),
            filter: Default::default(),
            limit: p.limit.unwrap_or(10).clamp(1, 50),
            min_score: 0.0,
            include_ghosts: false,
            query_mode: crate::search::QueryMode::default(),
            graph_depth: p.depth.unwrap_or(1).min(3),
            time_range_start: None,
            time_range_end: None,
            entities: vec![],
        };
        let ctx = Arc::new(EventContext {
            agent: None,
            recall: Some(Mutex::new(RecallTrace::default())),
        });
        let response = events::scope(ctx.clone(), self.query_engine.search(query))
            .await
            .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let trace = std::mem::take(&mut *ctx.recall.as_ref().unwrap().lock().unwrap());

        let summaries: HashMap<MemoryId, Option<String>> = response
            .results
            .iter()
            .map(|r| (r.memory_id, r.summary.clone()))
            .collect();
        let seeds: HashSet<MemoryId> = trace.seeds.iter().map(|(id, _)| *id).collect();
        // Strongest incoming spread per node, to walk a path back to a seed.
        let mut best: HashMap<MemoryId, &crate::graph::activation::SpreadStep> = HashMap::new();
        for step in &trace.steps {
            let e = best.entry(step.to).or_insert(step);
            if step.amount > e.amount {
                *e = step;
            }
        }
        let path_to_seed = |id: MemoryId| -> Option<(MemoryId, EdgeType, u8)> {
            let first = best.get(&id)?;
            let (mut cur, mut hops) = (first.from, 1u8);
            while !seeds.contains(&cur) && hops < 8 {
                cur = best.get(&cur)?.from;
                hops += 1;
            }
            Some((cur, first.edge_type, hops))
        };
        let ids: HashSet<MemoryId> = response.results.iter().map(|r| r.memory_id).collect();
        let contradicts: HashMap<MemoryId, MemoryId> = {
            let graph = self.graph.read().await;
            let mut out = HashMap::new();
            for id in &ids {
                for e in graph.edges_for(id) {
                    if e.edge_type != EdgeType::Contradicts {
                        continue;
                    }
                    let (Some(s), Some(t)) = (graph.nodes.get(e.source), graph.nodes.get(e.target))
                    else {
                        continue;
                    };
                    let other = if s.memory_id == *id {
                        t.memory_id
                    } else {
                        s.memory_id
                    };
                    if ids.contains(&other) {
                        out.insert(*id, other);
                    }
                }
            }
            out
        };
        let mut missing: Vec<MemoryId> = Vec::new();
        let mut hits: Vec<UiSearchHit> = response
            .results
            .iter()
            .map(|r| {
                let path = r.activation_score.and_then(|_| path_to_seed(r.memory_id));
                if let Some((seed, _, _)) = path
                    && !summaries.contains_key(&seed)
                {
                    missing.push(seed);
                }
                UiSearchHit {
                    id: r.memory_id.to_string(),
                    namespace: r.namespace.clone(),
                    summary: r.summary.clone(),
                    phase: r.phase as u8,
                    score: r.composite_score.or(r.score).unwrap_or(0.0),
                    vector: r.score,
                    text: r.fts_score,
                    activation: r.activation_score,
                    via: path.map(|(seed, _, _)| seed.to_string()),
                    via_summary: path
                        .and_then(|(seed, _, _)| summaries.get(&seed).cloned().flatten()),
                    via_edge_type: path.map(|(_, e, _)| e as u8),
                    hops: path.map(|(_, _, h)| h),
                    contradicts: contradicts
                        .get(&r.memory_id)
                        .and_then(|o| summaries.get(o).cloned().flatten()),
                }
            })
            .collect();
        if !missing.is_empty() {
            let storage = self.storage.clone();
            let found = tokio::task::spawn_blocking(move || {
                let s = storage.read().ok()?;
                Some(
                    missing
                        .into_iter()
                        .filter_map(|id| Some((id.to_string(), s.get_record(id).ok()??.summary)))
                        .collect::<HashMap<_, _>>(),
                )
            })
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
            for h in &mut hits {
                if h.via_summary.is_none() {
                    h.via_summary = h.via.as_ref().and_then(|v| found.get(v).cloned());
                }
            }
        }
        Ok(SearchExplain {
            namespace: p.namespace,
            embedding_profile: profile,
            hits,
        })
    }
}

impl UiService {
    /// Timeline data from persisted records: creations bucketed by today's
    /// phase, recall accesses per bucket, and the activity stream.
    pub async fn activity(&self, p: ActivityParams) -> Result<Activity, BridgeError> {
        let ns = self.namespace_map()?;
        let ns_filter = match &p.namespace {
            Some(name) => Some(
                ns.values()
                    .find(|n| &n.name == name)
                    .map(|n| n.id.get())
                    .ok_or_else(|| BridgeError::NotFound(format!("namespace '{name}'")))?,
            ),
            None => None,
        };
        let storage = self.storage.clone();
        let all = tokio::task::spawn_blocking(move || {
            storage
                .read()
                .map_err(internal)?
                .scan_all()
                .map_err(internal)
        })
        .await
        .map_err(internal)??;
        let degrees = self.degrees().await;
        let now = events::now_millis();
        let records: Vec<_> = all
            .iter()
            .filter(|(_, r)| ns_filter.is_none_or(|n| n == r.namespace_id))
            .collect();
        let oldest = records
            .iter()
            .map(|(_, r)| r.created_at)
            .min()
            .unwrap_or(now);
        let since = p.since.unwrap_or(oldest).min(now);
        let week = 7 * 86_400_000i64;
        let bucket_ms = if now - since > 730 * 86_400_000 {
            30 * 86_400_000
        } else {
            week
        };
        let start = since - since.rem_euclid(bucket_ms);
        let n = ((now - start) / bucket_ms + 1).max(1) as usize;
        let mut buckets: Vec<ActivityBucket> = (0..n)
            .map(|i| ActivityBucket {
                start: start + i as i64 * bucket_ms,
                full: 0,
                summary: 0,
                ghost: 0,
                recalls: 0,
            })
            .collect();
        let slot = |t: i64| ((t - start) / bucket_ms) as usize;
        // Grouped stream: accesses sharing a timestamp are one recall or sweep.
        let mut groups: HashMap<(i64, &'static str), (MemoryId, u32)> = HashMap::new();
        let mut rows = Vec::new();
        let name = |id: u32| ns.get(&id).map(|n| n.name.clone()).unwrap_or_default();
        for (id, r) in &records {
            if r.created_at >= start {
                if let Some(b) = buckets.get_mut(slot(r.created_at)) {
                    match r.phase {
                        DecayPhase::Full => b.full += 1,
                        DecayPhase::Summary => b.summary += 1,
                        DecayPhase::Ghost => b.ghost += 1,
                        DecayPhase::Tombstone => {}
                    }
                }
                if r.phase != DecayPhase::Tombstone {
                    rows.push((
                        r.created_at,
                        "store",
                        *id,
                        degrees.get(id).copied().unwrap_or(0),
                    ));
                }
            }
            for e in &r.access_history {
                if e.timestamp < start {
                    continue;
                }
                let kind = match e.kind {
                    AccessKind::DirectRetrieval | AccessKind::AssociativeRetrieval => "recall",
                    AccessKind::ManualReinforcement => "reinforce",
                    AccessKind::DecaySweep => "sweep",
                };
                if kind == "recall"
                    && let Some(b) = buckets.get_mut(slot(e.timestamp))
                {
                    b.recalls += 1;
                }
                let entry = groups.entry((e.timestamp, kind)).or_insert((*id, 0));
                entry.1 += 1;
                if kind == "recall" && e.kind == AccessKind::DirectRetrieval {
                    entry.0 = *id;
                }
            }
        }
        rows.extend(
            groups
                .into_iter()
                .map(|((at, kind), (id, count))| (at, kind, id, count)),
        );
        rows.sort_by(|a, b| b.0.cmp(&a.0));
        rows.truncate(p.limit.unwrap_or(100).clamp(1, 500));
        let lookup: HashMap<MemoryId, &DiskRecord> =
            records.iter().map(|(id, r)| (*id, r)).collect();
        let rows = rows
            .into_iter()
            .map(|(at, kind, id, count)| {
                let r = lookup.get(&id);
                ActivityRow {
                    at,
                    kind: kind.into(),
                    id: id.to_string(),
                    summary: r
                        .filter(|r| !matches!(r.phase, DecayPhase::Ghost | DecayPhase::Tombstone))
                        .map(|r| r.summary.clone())
                        .filter(|s| !s.is_empty()),
                    namespace: r.map(|r| name(r.namespace_id)).unwrap_or_default(),
                    count,
                }
            })
            .collect();
        Ok(Activity {
            bucket_ms,
            buckets,
            rows,
        })
    }
}

/// Merge per-namespace searches: one ranking when every namespace shares an
/// embedding profile (scores are comparable), otherwise grouped by namespace
/// in the order given.
pub fn merge(per_namespace: Vec<SearchExplain>, query: String, limit: usize) -> UiSearch {
    let merged = per_namespace
        .windows(2)
        .all(|w| w[0].embedding_profile == w[1].embedding_profile);
    let mut results: Vec<UiSearchHit> = Vec::new();
    if merged {
        results = per_namespace.into_iter().flat_map(|s| s.hits).collect();
        results.sort_by(|a, b| b.score.total_cmp(&a.score));
        // Direct matches first, then linked results, each by score.
        results.sort_by_key(|h| h.activation.is_some());
        results.truncate(limit);
    } else {
        for s in per_namespace {
            results.extend(s.hits.into_iter().take(limit));
        }
    }
    UiSearch {
        query,
        merged,
        results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(id: &str, ns: &str, score: f32, linked: bool) -> UiSearchHit {
        UiSearchHit {
            id: id.into(),
            namespace: ns.into(),
            summary: Some(id.into()),
            phase: 1,
            score,
            vector: (!linked).then_some(score),
            text: None,
            activation: linked.then_some(score),
            via: None,
            via_summary: None,
            via_edge_type: None,
            hops: None,
            contradicts: None,
        }
    }
    fn explain(ns: &str, profile: &str, hits: Vec<UiSearchHit>) -> SearchExplain {
        SearchExplain {
            namespace: ns.into(),
            embedding_profile: profile.into(),
            hits,
        }
    }

    #[test]
    fn merges_by_score_when_profiles_match() {
        let out = merge(
            vec![
                explain(
                    "a",
                    "p",
                    vec![hit("a1", "a", 0.9, false), hit("a2", "a", 0.3, true)],
                ),
                explain(
                    "b",
                    "p",
                    vec![hit("b1", "b", 0.95, false), hit("b2", "b", 0.5, false)],
                ),
            ],
            "q".into(),
            3,
        );
        assert!(out.merged);
        let ids: Vec<_> = out.results.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["b1", "a1", "b2"]);
    }

    #[test]
    fn groups_by_namespace_when_profiles_differ() {
        let out = merge(
            vec![
                explain("a", "p1", vec![hit("a1", "a", 0.2, false)]),
                explain("b", "p2", vec![hit("b1", "b", 0.9, false)]),
            ],
            "q".into(),
            10,
        );
        assert!(!out.merged);
        let ids: Vec<_> = out.results.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["a1", "b1"]);
    }
}
