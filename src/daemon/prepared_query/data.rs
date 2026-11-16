//! Typed prepared payloads used only to validate an incoming tree. The original
//! validated Value is returned, preserving its exact field/number representation.
use crate::model::schema::StructuredLabels;
use crate::model::{DecayPhase, MemoryId, NamespaceId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// These private structs mirror the finite prepared response schema. Exact
// reserialization equality rejects ignored/missing keys and explicit nulls in
// optional fields; no generic runtime JSON-schema evaluator is introduced.
macro_rules! wire {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub(super) struct $name { $(pub(super) $field: $ty),* }
    };
}
wire!(Namespace {
    namespace_id: NamespaceId,
    namespace: String
});
wire!(Related {
    id: MemoryId,
    edge_type: String,
    weight: f64
});
wire!(Direct { vector: Option<f64>, fts: Option<f64>, fused: Option<f64>, min_score: f64, disposition: String });
wire!(Own { formula: String, direct: Direct, entity_overlap: f64, entity_recall: f64,
    fts_signal: f64, raw_retrievability: f64, effective_retrievability: f64,
    activation: Option<f64>, activation_depth: Option<u64>, base_score: f64,
    temporal_factor: f64, rif_multiplier: f64, score: f64 });
wire!(Inherited {
    source: MemoryId,
    own: Own
});
wire!(RankedExplanation { ranking_profile: String, retrieval_semantics_version: u32,
    version: Option<Version>, raw_retrievability: f64, effective_retrievability: f64,
    own: Option<Own>, rejected_direct: Option<Direct>, inherited_floor: Option<f64>,
    inherited_sources: Vec<Inherited>, inherited_sources_omitted: u64, rif_multiplier: f64, final_score: f64 });
wire!(Origin {
    origin_id: MemoryId,
    cosine: f64
});
wire!(CosineExplanation { algorithm: String, own_cosine: Option<f64>, own_admission: String,
    min_score: Option<f64>, admitted_own: Option<f64>, inherited_floor: Option<f64>,
    origins: Vec<Origin>, omitted_origins: u64, final_similarity: f64, version: Option<Version> });

#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum Version {
    Current,
    Historical,
    Unresolved { reason: String },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Hit<E> {
    pub memory_id: MemoryId,
    pub created_at: i64,
    pub last_accessed_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fts_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composite_score: Option<f64>,
    pub retrievability: f64,
    #[serde(rename = "effectiveR")]
    pub effective_r: f64,
    pub phase: DecayPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_text: Option<String>,
    pub tags: Vec<String>,
    pub edge_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_score: Option<f64>,
    pub is_permastore: bool,
    pub stability: f64,
    pub namespace: String,
    pub namespace_id: NamespaceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<StructuredLabels>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<E>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub version: Option<Option<Version>>,
    // Ranked payloads require these two fields even when graphDepth is null;
    // the exact-shape check below treats them separately from similar payloads.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub graph_depth: Option<Option<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related: Option<Vec<Related>>,
}
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}
wire!(Neighbor { memory_id: MemoryId, namespace_id: NamespaceId, namespace: String,
    summary: String, full_text: Option<String>, labels: Option<StructuredLabels>, tags: Vec<String>,
    connected_to: MemoryId, edge_type: String, weight: f64 });
wire!(Channel { participation: String, offered: Option<u64>, admitted: Option<u64>,
    examined_unit: Option<String>, examined: Option<u64>, eligible: Option<u64>,
    exhausted: Option<bool>, candidate_limited: Option<bool>, budget_exhausted: Option<bool>,
    returned_own: u64, returned_inherited: u64 });
wire!(Channels {
    vector: Channel,
    fts: Channel,
    entity: Channel,
    metadata: Channel,
    graph: Channel
});
wire!(Eligibility { examined_unit: String, examined: u64, passed: u64, first_rejections: BTreeMap<String,u64> });
wire!(Graph {
    expanded_states: u64,
    examined_edges: u64,
    state_limited: bool,
    edge_limited: bool
});
wire!(Supersession { examined_edges: u64, unresolved: BTreeMap<String,u64>, work_limited: bool });
wire!(SimilarSupersession { examined_edges: u64, unresolved: BTreeMap<String,u64> });
wire!(Supplemental { requested: bool, neighborhoods_examined: u64, edges_examined: u64,
    supersession_edges_examined: u64, supersession_work_limited: bool,
    neighborhoods_skipped_edge_budget: u64, neighborhoods_unexamined_state_budget: u64,
    depth_limited_sources: u64, ineligible_neighbors: u64, historical_neighbors_omitted: u64,
    unresolved_neighbors: BTreeMap<String,u64>, eligible_unique_neighbors: u64, neighbor_limit_applied: bool });
wire!(Coverage { semantics_version: u32, version_mode: String, budget_profile: String,
    canonical_eligibility: Option<Eligibility>, channels: Channels, fts_availability: String,
    graph: Graph, supersession: Supersession, metadata_candidate_limited: bool,
    result_limited: bool, supplemental: Supplemental });
wire!(Excluded {
    namespace_id: NamespaceId,
    reason: String
});
wire!(SimilarCoverage { semantics_version: u32, version_mode: String, searched_namespace_ids: Vec<NamespaceId>,
    excluded_namespaces: Vec<Excluded>, examined_records: u64, eligible_records: u64, scored_records: u64,
    score_admitted: u64, resolved_candidates: u64, result_limited: bool, supersession: SimilarSupersession });
wire!(QueryData { results: Vec<Hit<RankedExplanation>>, neighbors: Vec<Neighbor>, total_matches: u64, coverage: Coverage });
wire!(SimilarData { results: Vec<Hit<CosineExplanation>>, total_matches: u64, source_namespace: Namespace, coverage: SimilarCoverage });
