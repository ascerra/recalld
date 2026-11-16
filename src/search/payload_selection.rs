//! Supplemental actual-edge metadata for explicit coherent semantics 2.
//! Ranked supersession/inheritance stays in the ordinary pipeline.
use std::collections::BTreeMap;

use super::read_view::CoherentRelation;
use super::semantics::Resolution;
use super::*;
use crate::graph::RelationshipGraph;
use crate::graph::query::{MAX_GRAPH_EDGES, MAX_GRAPH_STATES};
use crate::model::EdgeType;

#[derive(Debug, Default)]
pub struct SupplementalDiagnostics {
    pub neighbors_requested: bool,
    pub neighborhoods_examined: usize,
    pub edges_examined: usize,
    pub supersession_edges_examined: usize,
    pub supersession_work_limited: bool,
    pub neighborhoods_skipped_edge_budget: usize,
    pub neighborhoods_unexamined_state_budget: usize,
    pub depth_limited_sources: usize,
    pub ineligible_neighbors: usize,
    /// Proven historical adjacency omitted in current mode, not unresolved.
    pub historical_neighbors_omitted: usize,
    pub unresolved_neighbors: BTreeMap<UnresolvedReason, usize>,
    pub eligible_unique_neighbors: usize,
    pub neighbor_limit_applied: bool,
    /// Only admitted supplemental IDs, never omitted/predecessor IDs.
    pub versions: BTreeMap<MemoryId, VersionStatus>,
    pub depths: BTreeMap<MemoryId, u8>,
}

pub(super) struct SupplementalSelection {
    pub related: HashMap<MemoryId, Vec<CoherentRelation>>,
    pub neighbors: Vec<(MemoryId, (f32, String, MemoryId))>,
    pub diagnostics: SupplementalDiagnostics,
}

struct Link {
    id: MemoryId,
    weight: f32,
    kind: EdgeType,
    connected_to: MemoryId,
    source: MemoryId,
    target: MemoryId,
    depth: u8,
}

impl Link {
    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        other
            .weight
            .total_cmp(&self.weight)
            .then_with(|| self.id.cmp(&other.id))
            .then_with(|| self.kind.as_u8().cmp(&other.kind.as_u8()))
            .then_with(|| self.connected_to.cmp(&other.connected_to))
            .then_with(|| self.source.cmp(&other.source))
            .then_with(|| self.target.cmp(&other.target))
    }
}

impl QueryEngine {
    pub(super) async fn select_payload_v2(
        &self,
        graph: &RelationshipGraph,
        selection: &SearchSelectionContext,
        ids: &[MemoryId],
        include_neighbors: bool,
        mode: VersionMode,
        diagnostics: &mut SemanticsDiagnostics,
    ) -> Result<SupplementalSelection> {
        let selected: HashSet<_> = ids.iter().copied().collect();
        let mut ordered = ids.to_vec();
        ordered.sort();
        let mut result = SupplementalSelection {
            related: HashMap::new(),
            neighbors: Vec::new(),
            diagnostics: SupplementalDiagnostics {
                neighbors_requested: include_neighbors,
                ..Default::default()
            },
        };
        let mut candidates = BTreeMap::<MemoryId, Link>::new();
        let chain_edges_before = diagnostics.supersession_edges_examined;
        for (position, id) in ordered.iter().copied().enumerate() {
            let Some(record) = selection.eligible.get(&id) else {
                return Err(unavailable_payload("selected row lost hard eligibility"));
            };
            if matches!(record.phase, DecayPhase::Ghost | DecayPhase::Tombstone) {
                continue;
            }
            let node = graph
                .get_node(&id)
                .ok_or_else(|| unavailable_payload("missing selected graph node"))?;
            if node.namespace_id != selection.namespace.id {
                return Err(unavailable_payload("selected graph node scope differs"));
            }
            if diagnostics.graph.expanded_states >= MAX_GRAPH_STATES {
                diagnostics.graph.state_limit = true;
                result.diagnostics.neighborhoods_unexamined_state_budget = ordered.len() - position;
                break;
            }
            diagnostics.graph.expanded_states += 1;
            result.diagnostics.neighborhoods_examined += 1;
            let remaining = MAX_GRAPH_EDGES.saturating_sub(diagnostics.graph.examined_edges);
            // Stored vector lengths: no iterator count, partial collection, or
            // allocation proportional to a neighborhood we cannot examine.
            if node.degree() > remaining {
                diagnostics.graph.edge_limit = true;
                result.diagnostics.neighborhoods_skipped_edge_budget += 1;
                continue;
            }
            let depth = diagnostics.graph_depths.get(&id).copied();
            let can_expand =
                include_neighbors && depth.is_some_and(|d| d < selection.query.graph_depth);
            if include_neighbors && !can_expand {
                result.diagnostics.depth_limited_sources += 1;
            }
            let mut relations = Vec::new();
            for key in node.outgoing.iter().chain(&node.incoming) {
                diagnostics.graph.examined_edges += 1;
                result.diagnostics.edges_examined += 1;
                let edge = graph
                    .get_edge(*key)
                    .ok_or_else(|| unavailable_payload("missing supplemental edge"))?;
                let source = graph
                    .get_node_by_key(edge.source)
                    .ok_or_else(|| unavailable_payload("missing supplemental source"))?
                    .memory_id;
                let target = graph
                    .get_node_by_key(edge.target)
                    .ok_or_else(|| unavailable_payload("missing supplemental target"))?
                    .memory_id;
                if source != id && target != id {
                    return Err(unavailable_payload("invalid supplemental adjacency"));
                }
                if !edge.weight.is_finite() {
                    return Err(unavailable_payload("nonfinite supplemental edge"));
                }
                let other = if source == id { target } else { source };
                if other == id {
                    continue;
                }
                let link = Link {
                    id: other,
                    weight: if edge.weight == 0.0 { 0.0 } else { edge.weight },
                    kind: edge.edge_type,
                    connected_to: id,
                    source,
                    target,
                    depth: depth
                        .unwrap_or(selection.query.graph_depth)
                        .saturating_add(1),
                };
                if selected.contains(&other) {
                    if selection.eligible.get(&other).is_some_and(|r| {
                        !matches!(r.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
                    }) {
                        relations.push(link);
                    }
                } else if can_expand {
                    match candidates.entry(other) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(link);
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            let min_depth = entry.get().depth.min(link.depth);
                            if link.compare(entry.get()).is_lt() {
                                entry.insert(link);
                            }
                            entry.get_mut().depth = min_depth;
                        }
                    }
                }
            }
            relations.sort_by(Link::compare);
            relations
                .dedup_by(|a, b| a.source == b.source && a.target == b.target && a.kind == b.kind);
            result.related.insert(
                id,
                relations
                    .into_iter()
                    .map(|link| CoherentRelation {
                        id: link.id,
                        edge_type: format!("{:?}", link.kind).to_lowercase(),
                        weight: link.weight,
                    })
                    .collect(),
            );
        }
        let mut eligible = Vec::new();
        // UUID-ordered resolution makes the remaining chain budget deterministic.
        for (id, link) in candidates {
            let Some(record) = selection.eligible.get(&id).filter(|r| {
                !matches!(r.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
                    && !r.summary.is_empty()
            }) else {
                result.diagnostics.ineligible_neighbors += 1;
                continue;
            };
            let status = match self
                .resolve_path(
                    record.clone(),
                    selection.namespace.id,
                    diagnostics,
                    Some(&selection.strength),
                )
                .await?
            {
                Resolution::Current(_) => VersionStatus::Current,
                Resolution::Resolved { .. } => {
                    if mode == VersionMode::Current {
                        result.diagnostics.historical_neighbors_omitted += 1;
                        continue;
                    }
                    VersionStatus::Historical
                }
                Resolution::Unresolved(reason) => {
                    *result
                        .diagnostics
                        .unresolved_neighbors
                        .entry(reason)
                        .or_default() += 1;
                    result.diagnostics.supersession_work_limited |=
                        reason == UnresolvedReason::WorkLimit;
                    if mode == VersionMode::Current {
                        continue;
                    }
                    VersionStatus::Unresolved(reason)
                }
            };
            eligible.push((link, status));
        }
        result.diagnostics.eligible_unique_neighbors = eligible.len();
        result.diagnostics.supersession_edges_examined =
            diagnostics.supersession_edges_examined - chain_edges_before;
        result.diagnostics.neighbor_limit_applied = eligible.len() > 10;
        eligible.sort_by(|a, b| a.0.compare(&b.0));
        eligible.truncate(10);
        for (link, version) in eligible {
            result.diagnostics.versions.insert(link.id, version);
            result.diagnostics.depths.insert(link.id, link.depth);
            result.neighbors.push((
                link.id,
                (
                    link.weight,
                    format!("{:?}", link.kind).to_lowercase(),
                    link.connected_to,
                ),
            ));
        }
        Ok(result)
    }
}

fn unavailable_payload(message: &str) -> SearchError {
    SearchError::Unavailable(message.into())
}
