//! Standalone native selection uses canonical payloads, never cache/index guesses.
use crate::model::{DecayPhase, DiskRecord, EdgeType, MemoryId, NamespaceId};
use crate::storage::control::GenerationRef;
use crate::storage::native_graph::{MAX_NATIVE_GRAPH_EDGES, NativeEdgeInput, NativeGraphInput};
use crate::storage::{RedbStorageEngine, StorageEngine, StorageError};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Volatile automatic intent. Only unkeyed operations may select fresh neighbors.
/// This is an internal/core API, not a new transport contract or durable plan.
#[derive(Debug, Clone)]
pub enum NativeLinkSelection {
    Semantic {
        source: MemoryId,
        threshold: f32,
        max_links: usize,
    },
    Entity {
        source: MemoryId,
        max_links: usize,
    },
    Temporal {
        source: MemoryId,
        window_ms: u64,
        max_links: usize,
    },
}
impl NativeLinkSelection {
    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        let (source, cap) = match self {
            Self::Semantic {
                source,
                threshold,
                max_links,
            } => {
                if !threshold.is_finite() || !(0.0..=1.0).contains(threshold) {
                    return Err(StorageError::InvalidMutation(
                        "invalid semantic threshold".into(),
                    ));
                }
                (*source, *max_links)
            }
            Self::Entity { source, max_links }
            | Self::Temporal {
                source, max_links, ..
            } => (*source, *max_links),
        };
        if source == MemoryId::nil() || cap > MAX_NATIVE_GRAPH_EDGES {
            return Err(StorageError::InvalidMutation(
                "invalid native selection source or cap".into(),
            ));
        }
        Ok(())
    }
}
fn entities(row: &DiskRecord) -> Result<BTreeSet<String>, StorageError> {
    let schema = row
        .schema
        .as_ref()
        .ok_or_else(|| StorageError::InvalidControl("native graph record schema missing".into()))?;
    Ok(schema
        .labels
        .entities
        .iter()
        .flat_map(|s| crate::search::canonicalize_entity(s))
        .collect())
}
fn vector(
    engine: &RedbStorageEngine,
    ns: NamespaceId,
    row: &DiskRecord,
    width: u32,
) -> Result<Vec<f32>, StorageError> {
    let reference = row
        .schema
        .as_ref()
        .and_then(|s| s.vector.as_ref())
        .ok_or_else(|| {
            StorageError::OwnershipMismatch("native graph vector reference missing".into())
        })?;
    if reference.slot != row.vector_slot {
        return Err(StorageError::OwnershipMismatch(
            "native graph vector slot mismatch".into(),
        ));
    }
    #[cfg(test)]
    vector_probe::read(MemoryId::from_bytes(row.id));
    let values = engine
        .get_vector(ns, reference.slot)?
        .ok_or_else(|| StorageError::OwnershipMismatch("native graph vector missing".into()))?;
    if values.len() != width as usize {
        return Err(StorageError::OwnershipMismatch(
            "native graph vector width mismatch".into(),
        ));
    }
    crate::storage::canonical_schema::validate_bound_vector(&values)?;
    let mut digest = Sha256::new();
    for v in &values {
        digest.update(v.to_le_bytes());
    }
    if <[u8; 32]>::from(digest.finalize()) != reference.sha256 {
        return Err(StorageError::OwnershipMismatch(
            "native graph vector hash mismatch".into(),
        ));
    }
    Ok(values)
}

/// ponytail: scan canonical edges once and namespace payloads under the owner.
/// Semantic work is O(namespace * vector-width), with indexed edge exclusion.
/// Optimize only with a coherent canonical/derived selection, never cache fallbacks.
pub(crate) fn select(
    engine: &RedbStorageEngine,
    namespace: NamespaceId,
    selection: &NativeLinkSelection,
) -> Result<(NativeGraphInput, GenerationRef), StorageError> {
    selection.validate()?;
    let generation = engine.control_state()?.generation_ref();
    let (source, kind, cap) = match selection {
        NativeLinkSelection::Semantic {
            source, max_links, ..
        } => (*source, EdgeType::Associative, *max_links),
        NativeLinkSelection::Entity { source, max_links } => {
            (*source, EdgeType::Entity, *max_links)
        }
        NativeLinkSelection::Temporal {
            source, max_links, ..
        } => (*source, EdgeType::Temporal, *max_links),
    };
    let row = engine
        .get_record(source)?
        .ok_or(StorageError::NotFound(source.into_inner()))?;
    if row.namespace_id != namespace.get() {
        return Err(StorageError::MutationNamespaceMismatch);
    }
    if !matches!(row.phase, DecayPhase::Full | DecayPhase::Summary) {
        return Err(StorageError::LifecycleContentUnavailable);
    }
    let ns = engine
        .get_namespace(namespace)?
        .ok_or(StorageError::NamespaceNotFound(namespace.get()))?;
    ns.schema.as_ref().ok_or_else(|| {
        StorageError::InvalidControl("native graph namespace schema missing".into())
    })?;
    let query = if matches!(selection, NativeLinkSelection::Semantic { .. }) {
        engine
            .meta_store()
            .with_generation_read_at(generation, |view| view.check_native_vector_namespace(&ns))?;
        Some(vector(engine, namespace, &row, ns.embedding_dim)?)
    } else {
        None
    };
    let source_entities = entities(&row)?;
    let connected: BTreeSet<_> = engine
        .load_all_edges()?
        .into_iter()
        .filter(|edge| edge.edge_type == kind)
        .filter_map(|edge| {
            if edge.source == source {
                Some(edge.target)
            } else if edge.target == source {
                Some(edge.source)
            } else {
                None
            }
        })
        .collect();
    let mut candidates = Vec::new();
    for other in engine.meta_store().records_in_namespace(namespace)? {
        let id = MemoryId::from_bytes(other.id);
        if id == source || !matches!(other.phase, DecayPhase::Full | DecayPhase::Summary) {
            continue;
        }
        if connected.contains(&id) {
            continue;
        }
        let (weight, rank) = match selection {
            NativeLinkSelection::Semantic { threshold, .. } => {
                let v = vector(engine, namespace, &other, ns.embedding_dim)?;
                let score =
                    crate::search::dot_product_simd(query.as_ref().unwrap(), &v).clamp(-1., 1.);
                let floor = super::autolink::THRESHOLD_HARD_FLOOR;
                let shared = row.tags.iter().any(|t| other.tags.contains(t));
                let threshold = threshold.max(floor)
                    - if shared {
                        super::autolink::TAG_THRESHOLD_ADJUSTMENT
                    } else {
                        0.
                    };
                if score < threshold.max(floor) {
                    continue;
                }
                (score, 0u64)
            }
            NativeLinkSelection::Entity { .. } => {
                let labels = entities(&other)?;
                let shared = source_entities.intersection(&labels).count();
                if shared == 0 {
                    continue;
                }
                (
                    shared as f32 / source_entities.union(&labels).count() as f32,
                    shared as u64,
                )
            }
            NativeLinkSelection::Temporal { window_ms, .. } => {
                let delta = row.created_at.abs_diff(other.created_at);
                if *window_ms == 0 || delta > *window_ms {
                    continue;
                }
                (
                    (1. - delta as f64 / *window_ms as f64).max(0.05) as f32,
                    u64::MAX - delta,
                )
            }
        };
        candidates.push((id, weight, rank));
    }
    candidates.sort_by(|a, b| {
        let rank = if matches!(selection, NativeLinkSelection::Semantic { .. }) {
            b.1.total_cmp(&a.1)
        } else {
            b.2.cmp(&a.2)
        };
        rank.then(a.0.cmp(&b.0))
    });
    candidates.truncate(cap);
    if engine.control_state()?.generation_ref() != generation {
        return Err(StorageError::LifecycleConflict(
            "native graph candidates changed during selection".into(),
        ));
    }
    Ok((
        NativeGraphInput {
            edges: candidates
                .into_iter()
                .map(|(target, weight, _)| NativeEdgeInput {
                    source,
                    target,
                    edge_type: kind,
                    weight,
                    auto_created: true,
                })
                .collect(),
        },
        generation,
    ))
}

#[cfg(test)]
pub(crate) mod vector_probe {
    use crate::model::MemoryId;
    use std::collections::BTreeMap;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    static COUNTERS: Mutex<BTreeMap<MemoryId, Arc<AtomicUsize>>> = Mutex::new(BTreeMap::new());
    pub(crate) struct Probe {
        ids: Vec<MemoryId>,
        count: Arc<AtomicUsize>,
    }
    impl Probe {
        pub(crate) fn new(ids: Vec<MemoryId>) -> Self {
            let count = Arc::new(AtomicUsize::new(0));
            let mut map = COUNTERS.lock().unwrap();
            for id in &ids {
                assert!(map.insert(*id, count.clone()).is_none());
            }
            Self { ids, count }
        }
        pub(crate) fn reads(&self) -> usize {
            self.count.load(Ordering::SeqCst)
        }
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            let mut map = COUNTERS.lock().unwrap();
            for id in &self.ids {
                map.remove(id);
            }
        }
    }
    pub(super) fn read(id: MemoryId) {
        if let Some(count) = COUNTERS.lock().unwrap().get(&id) {
            count.fetch_add(1, Ordering::SeqCst);
        }
    }
}
