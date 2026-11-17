use super::data::*;
use crate::model::{AccessKind, MemoryId, NamespaceId};
use crate::search::{RetrievalSemantics, SimilarityScope, VersionMode};
use crate::serialization::contract::{CommitState, Scope, Status};
use crate::serialization::query::{QueryRequest, SimilarRequest};
use crate::serialization::query_response::QueryReply;
use crate::storage::control::{MutationOutcome, WriteReceipt};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

// Ranked rows do not expose complete channel provenance unless requested, so
// their exact direct/associative classification cannot be inferred here. IDs
// and namespaces must still match fully; similarity/neighbor kinds are known.
type Returned = BTreeMap<MemoryId, (NamespaceId, bool)>;

pub(super) struct Expected {
    pub similar: bool,
    pub access: bool,
    pub namespace: Option<String>,
    pub all: bool,
    pub limit: usize,
    pub embeddings: bool,
    pub explain: bool,
    pub history: bool,
    pub neighbors: bool,
}
impl Expected {
    pub(super) fn query(r: &QueryRequest) -> Self {
        Self {
            similar: false,
            access: r.query.record_access,
            namespace: Some(r.query.namespace.clone()),
            all: false,
            limit: r.query.limit,
            embeddings: r.include_embeddings,
            explain: r.explain,
            history: r.version_mode == VersionMode::History,
            neighbors: r.include_neighbors,
        }
    }
    pub(super) fn similar(r: &SimilarRequest) -> Self {
        Self {
            similar: true,
            access: r.options.record_access,
            namespace: r.namespace.clone(),
            all: r.options.scope == SimilarityScope::AllCompatible,
            limit: r.options.limit,
            embeddings: r.options.include_embeddings,
            explain: r.explain,
            history: matches!(
                r.options.semantics,
                RetrievalSemantics::V2(VersionMode::History)
            ),
            neighbors: false,
        }
    }
}

/// Typed decode plus exact canonical output equality closes serde's ignored
/// fields/defaults. Do not manufacture StoredReceipt request hashes here.
pub(crate) fn typed<T: DeserializeOwned + Serialize>(v: &Value) -> Result<T, ()> {
    let decoded: T = serde_json::from_value(v.clone()).map_err(|_| ())?;
    let bytes = serde_json::to_vec(&decoded).map_err(|_| ())?;
    let encoded: Value = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if encoded != *v {
        return Err(());
    }
    Ok(decoded)
}
fn require(value: bool) -> Result<(), ()> {
    value.then_some(()).ok_or(())
}
fn one(value: &str, allowed: &[&str]) -> Result<(), ()> {
    require(allowed.contains(&value))
}
fn reasons(map: &BTreeMap<String, u64>) -> Result<(), ()> {
    for key in map.keys() {
        one(
            key,
            &[
                "missingRecord",
                "missingStructure",
                "foreignNamespace",
                "branch",
                "cycle",
                "hopLimit",
                "workLimit",
                "ineligible",
            ],
        )?;
    }
    Ok(())
}
fn version(v: &Version) -> Result<(), ()> {
    if let Version::Unresolved { reason } = v {
        one(
            reason,
            &[
                "missingRecord",
                "missingStructure",
                "foreignNamespace",
                "branch",
                "cycle",
                "hopLimit",
                "workLimit",
                "ineligible",
            ],
        )?;
    }
    Ok(())
}
fn direct(d: &Direct) -> Result<(), ()> {
    one(&d.disposition, &["passed", "rejected", "auxiliary"])?;
    require((0.0..=1.0).contains(&d.min_score))
}
fn own(o: &Own) -> Result<(), ()> {
    one(&o.formula, &["direct", "graph", "entityRecall", "metadata"])?;
    direct(&o.direct)?;
    require(o.rif_multiplier == 1.0)
}
fn channel(c: &Channel) -> Result<(), ()> {
    one(&c.participation, &["notRequested", "completed"])?;
    if let Some(unit) = &c.examined_unit {
        one(
            unit,
            &[
                "vectorEntries",
                "entityPostings",
                "ftsRows",
                "scoredVectors",
                "canonicalRecords",
            ],
        )?;
    }
    Ok(())
}
fn hit<E>(
    hit: &Hit<E>,
    e: &Expected,
    seen: &mut BTreeSet<crate::model::MemoryId>,
) -> Result<(), ()> {
    require(
        hit.memory_id.as_bytes() != &[0; 16]
            && seen.insert(hit.memory_id)
            && !hit.namespace_id.is_unset(),
    )?;
    crate::model::validation::validate_namespace_name(&hit.namespace).map_err(|_| ())?;
    require(hit.stability > 0. && hit.phase != crate::model::DecayPhase::Tombstone)?;
    require(hit.embedding.is_none() || e.embeddings)?;
    if let Some(vector) = &hit.embedding {
        require(!vector.is_empty() && vector.len() <= 8192)?;
    }
    if let Some(labels) = &hit.labels {
        labels.validate().map_err(|_| ())?;
    }
    require(hit.explanation.is_some() == e.explain)?;
    if !e.similar {
        require(hit.version.is_some() && hit.graph_depth.is_some() && hit.related.is_some())?;
    } else {
        require(hit.version.is_none() && hit.graph_depth.is_none() && hit.related.is_none())?;
    }
    if let Some(Some(v)) = &hit.version {
        version(v)?;
    }
    Ok(())
}

fn data(
    v: &Value,
    e: &Expected,
    scope_ids: &[NamespaceId],
    scope_name: Option<&str>,
) -> Result<Returned, ()> {
    let mut ids = BTreeSet::new();
    let mut returned = BTreeMap::new();
    let wanted_version = if e.history { "history" } else { "current" };
    if e.similar {
        let data: SimilarData = typed(v)?;
        require(data.results.len() <= e.limit && data.total_matches >= data.results.len() as u64)?;
        let c = &data.coverage;
        require(c.semantics_version == 2 && c.version_mode == wanted_version)?;
        require(!data.source_namespace.namespace_id.is_unset())?;
        require(
            e.namespace
                .as_ref()
                .is_none_or(|n| n.eq_ignore_ascii_case(&data.source_namespace.namespace)),
        )?;
        let searched: BTreeSet<_> = c.searched_namespace_ids.iter().copied().collect();
        require(
            searched.len() == c.searched_namespace_ids.len()
                && searched.iter().all(|n| !n.is_unset()),
        )?;
        require(searched.contains(&data.source_namespace.namespace_id))?;
        require(c.searched_namespace_ids == scope_ids)?;
        if !e.all {
            require(
                scope_name.is_some_and(|name| {
                    name.eq_ignore_ascii_case(&data.source_namespace.namespace)
                }),
            )?;
        }
        let mut excluded = BTreeSet::new();
        for item in &c.excluded_namespaces {
            require(
                !item.namespace_id.is_unset()
                    && !searched.contains(&item.namespace_id)
                    && excluded.insert(item.namespace_id),
            )?;
            one(
                &item.reason,
                &["unknownProfile", "differentProfile", "differentDimension"],
            )?;
        }
        reasons(&c.supersession.unresolved)?;
        for row in &data.results {
            hit(row, e, &mut ids)?;
            require(searched.contains(&row.namespace_id))?;
            if row.namespace_id == data.source_namespace.namespace_id {
                require(
                    row.namespace
                        .eq_ignore_ascii_case(&data.source_namespace.namespace),
                )?;
            }
            returned.insert(row.memory_id, (row.namespace_id, true));
            if let Some(x) = &row.explanation {
                require(x.algorithm == "cosineMax" && x.origins.len() <= 64)?;
                one(
                    &x.own_admission,
                    &["missing", "rejectedByMinScore", "notAdmitted", "admitted"],
                )?;
                require(x.min_score.is_none_or(|n| (0.0..=1.0).contains(&n)))?;
                require(row.composite_score == Some(x.final_similarity))?;
                if let Some(v) = &x.version {
                    version(v)?;
                }
            }
        }
    } else {
        let data: QueryData = typed(v)?;
        require(data.results.len() <= e.limit && data.total_matches >= data.results.len() as u64)?;
        let c = &data.coverage;
        require(
            c.semantics_version == 2
                && c.version_mode == wanted_version
                && c.budget_profile == "retrieval-v2-default",
        )?;
        one(
            &c.fts_availability,
            &["notRequested", "available", "unavailable"],
        )?;
        for c in [
            &c.channels.vector,
            &c.channels.fts,
            &c.channels.entity,
            &c.channels.metadata,
            &c.channels.graph,
        ] {
            channel(c)?;
        }
        if let Some(c) = &c.canonical_eligibility {
            require(c.examined_unit == "canonicalRecords" && c.passed <= c.examined)?;
            for key in c.first_rejections.keys() {
                one(
                    key,
                    &[
                        "namespace",
                        "tombstone",
                        "ghost",
                        "phase",
                        "strength",
                        "permastore",
                        "labels",
                        "requiredTag",
                        "excludedTag",
                    ],
                )?;
            }
        }
        reasons(&c.supersession.unresolved)?;
        reasons(&c.supplemental.unresolved_neighbors)?;
        for row in &data.results {
            hit(row, e, &mut ids)?;
            require(
                scope_ids == [row.namespace_id]
                    && scope_name.is_some_and(|name| name.eq_ignore_ascii_case(&row.namespace)),
            )?;
            returned.insert(row.memory_id, (row.namespace_id, false));
            if let Some(x) = &row.explanation {
                require(
                    x.ranking_profile == "baseline-v1"
                        && x.retrieval_semantics_version == 2
                        && x.rif_multiplier == 1.,
                )?;
                require(x.inherited_sources.len() <= crate::search::MAX_EXPLANATION_SOURCES)?;
                if let Some(v) = &x.version {
                    version(v)?;
                }
                if let Some(o) = &x.own {
                    own(o)?;
                }
                if let Some(d) = &x.rejected_direct {
                    direct(d)?;
                }
                for s in &x.inherited_sources {
                    own(&s.own)?;
                }
            }
        }
        require(e.neighbors || data.neighbors.is_empty())?;
        let ranked = ids.clone();
        for n in &data.neighbors {
            require(
                n.memory_id.as_bytes() != &[0; 16]
                    && ids.insert(n.memory_id)
                    && ranked.contains(&n.connected_to)
                    && scope_ids == [n.namespace_id]
                    && scope_name.is_some_and(|name| name.eq_ignore_ascii_case(&n.namespace)),
            )?;
            if let Some(labels) = &n.labels {
                labels.validate().map_err(|_| ())?;
            }
            returned.insert(n.memory_id, (n.namespace_id, true));
        }
    }
    Ok(returned)
}

pub(crate) fn receipt(v: &Value) -> Result<WriteReceipt, ()> {
    receipt_identity(typed(v)?)
}

/// Caller must run the bounded borrowed structural preflight first. Exact typed
/// bytes preserve f32 identity without the query Value decoder's f64 hop.
pub(crate) fn typed_raw<T: DeserializeOwned + Serialize>(raw: &str) -> Result<T, ()> {
    let decoded: T = serde_json::from_str(raw).map_err(|_| ())?;
    require(serde_json::to_string(&decoded).map_err(|_| ())? == raw)?;
    Ok(decoded)
}
pub(crate) fn receipt_raw(raw: &str) -> Result<WriteReceipt, ()> {
    receipt_identity(typed_raw(raw)?)
}
fn receipt_identity(receipt: WriteReceipt) -> Result<WriteReceipt, ()> {
    require(
        !receipt.store_id.is_nil()
            && !receipt.operation_id.is_nil()
            && receipt.receipt_version == 1
            && receipt.committed_generation.get() > 0,
    )?;
    Ok(receipt)
}

pub(super) fn query(v: &Value, status: u16, e: &Expected) -> Result<QueryReply, ()> {
    require(v.get("contractVersion").and_then(Value::as_u64) == Some(2))?;
    require(v.get("receivedEvidence").is_none())?;
    let mut reply: QueryReply = typed(v)?;
    reply.http_status = status;
    require(matches!(status, 200 | 400 | 404 | 409 | 422 | 503))?;
    let ok = reply.envelope.status == Status::Ok;
    require(ok == (status == 200))?;
    require(if ok {
        reply.envelope.error.is_none()
            && reply.envelope.scope.is_some()
            && reply.envelope.read_generation.is_some()
    } else {
        reply.envelope.status == Status::Error
            && reply.envelope.error.is_some()
            && reply.envelope.data.is_none()
    })?;
    let mut scope_name = None;
    let scope_ids = match &reply.envelope.scope {
        Some(Scope::Namespace {
            namespace_id,
            namespace,
        }) => {
            require(!namespace_id.is_unset() && !e.all)?;
            crate::model::validation::validate_namespace_name(namespace).map_err(|_| ())?;
            require(
                e.namespace
                    .as_ref()
                    .is_none_or(|n| n.eq_ignore_ascii_case(namespace)),
            )?;
            scope_name = Some(namespace.as_str());
            vec![*namespace_id]
        }
        Some(Scope::Namespaces { namespace_ids }) => {
            require(e.similar && e.all && !namespace_ids.is_empty())?;
            require(namespace_ids.iter().all(|n| !n.is_unset()))?;
            require(namespace_ids.iter().collect::<BTreeSet<_>>().len() == namespace_ids.len())?;
            namespace_ids.clone()
        }
        Some(Scope::Catalog { .. }) => return Err(()),
        None => vec![],
    };
    let returned = if ok {
        Some(data(
            reply.envelope.data.as_ref().ok_or(())?,
            e,
            &scope_ids,
            scope_name,
        )?)
    } else {
        None
    };
    require(
        e.access
            || (reply.observation_receipts.is_empty()
                && reply.envelope.receipt.is_none()
                && reply.envelope.readiness.is_none()),
    )?;
    require(
        reply.envelope.receipt.as_ref()
            == if reply.observation_receipts.len() == 1 {
                reply.observation_receipts.first()
            } else {
                None
            },
    )?;
    let mut seen = BTreeSet::new();
    let mut exposed = BTreeSet::new();
    let mut previous_generation = reply.envelope.read_generation.map(|g| g.generation);
    let mut lineage = reply.envelope.read_generation.map(|g| g.store_id);
    if let Some(id) = lineage {
        require(!id.is_nil())?;
    }
    for r in &reply.observation_receipts {
        require(
            !r.store_id.is_nil()
                && !r.operation_id.is_nil()
                && r.receipt_version == 1
                && !r.replayed
                && r.committed_generation.get() > 0
                && seen.insert(r.operation_id),
        )?;
        if let Some(previous) = previous_generation {
            require(r.committed_generation > previous)?;
        }
        previous_generation = Some(r.committed_generation);
        if let Some(id) = lineage {
            require(id == r.store_id)?;
        } else {
            lineage = Some(r.store_id);
        }
        match &r.outcome {
            MutationOutcome::LifecycleExposureRecorded { transition } => {
                transition.validate(r, false).map_err(|_| ())?;
                require(scope_ids.contains(&transition.namespace_id))?;
                for step in &transition.records {
                    require(exposed.insert(step.memory_id))?;
                    if let Some(rows) = &returned {
                        let (ns, associative) = rows.get(&step.memory_id).ok_or(())?;
                        require(
                            *ns == transition.namespace_id
                                && (!associative || step.kind == AccessKind::AssociativeRetrieval),
                        )?;
                    }
                }
            }
            MutationOutcome::ExposuresRecorded { accesses } => {
                require(!accesses.is_empty())?;
                let mut seen = BTreeSet::new();
                for a in accesses {
                    require(
                        a.memory_id.as_bytes() != &[0; 16]
                            && seen.insert(a.memory_id)
                            && exposed.insert(a.memory_id),
                    )?;
                    require(matches!(
                        a.kind,
                        crate::model::AccessKind::DirectRetrieval
                            | crate::model::AccessKind::AssociativeRetrieval
                    ))?;
                    require(chrono::DateTime::from_timestamp_millis(a.accessed_at).is_some())?;
                    if let Some(rows) = &returned {
                        let (_, associative) = rows.get(&a.memory_id).ok_or(())?;
                        require(!associative || a.kind == AccessKind::AssociativeRetrieval)?;
                    }
                }
            }
            _ => return Err(()),
        }
    }
    if ok && e.access {
        require(
            returned
                .as_ref()
                .unwrap()
                .keys()
                .copied()
                .collect::<BTreeSet<_>>()
                == exposed,
        )?;
    }
    if let Some(error) = &reply.envelope.error {
        require(!error.retryable)?;
        require(e.access || error.commit_state == CommitState::NotCommitted)?;
        require(
            error.commit_state != CommitState::NotCommitted
                || reply.observation_receipts.is_empty(),
        )?;
        require(
            error.commit_state != CommitState::Committed || !reply.observation_receipts.is_empty(),
        )?;
        require(error.operation_id.is_none_or(|id| !id.is_nil()))?;
    }
    if let Some(r) = &reply.envelope.readiness {
        require(
            ok && !reply.observation_receipts.is_empty()
                && Some(r.store_id) == lineage
                && r.ready
                && r.pending_work == 0,
        )?;
        require(
            r.pending_work_scope.is_none() && r.observed_work.is_none() && r.derived.len() == 4,
        )?;
        let mut names = BTreeSet::new();
        for d in &r.derived {
            one(&d.name, &["graph", "vectors", "fts", "entities"])?;
            require(
                names.insert(d.name.as_str())
                    && d.ready
                    && d.applied_generation == Some(r.committed_generation),
            )?;
        }
        require(
            reply
                .observation_receipts
                .iter()
                .all(|v| v.committed_generation <= r.committed_generation),
        )?;
    } else {
        require(!ok || reply.observation_receipts.is_empty())?;
    }
    Ok(reply)
}
