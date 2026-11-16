//! One derived vector handle; each native namespace uses the unchanged flat kernel.
use super::{
    FlatVectorIndex, SearchFilter, VectorError, VectorIndex, VectorMetadata, VectorSearchResult,
};
use crate::model::{MemoryId, NamespaceConfig, NamespaceId};
use std::collections::{HashMap, HashSet};

pub struct VectorIndexOwner {
    layout: Layout,
}
enum Layout {
    Uniform(FlatVectorIndex),
    Namespaces {
        indexes: HashMap<NamespaceId, FlatVectorIndex>,
        locations: HashMap<MemoryId, NamespaceId>,
        total: usize,
    },
}
fn invalid(message: &str) -> VectorError {
    VectorError::CorruptIndex(message.into())
}
impl VectorIndexOwner {
    /// Fixed-width compatibility, including the original kernel's filter semantics.
    pub fn uniform(dimension: usize) -> Self {
        Self {
            layout: Layout::Uniform(FlatVectorIndex::new(dimension)),
        }
    }
    /// Layout only: canonical ownership/profile/certification are checked by callers.
    pub(crate) fn from_namespaces(namespaces: &[NamespaceConfig]) -> Result<Self, VectorError> {
        let mut indexes = HashMap::new();
        for ns in namespaces {
            if ns.id == NamespaceId::UNSET
                || !(1..=8192).contains(&ns.embedding_dim)
                || indexes.contains_key(&ns.id)
            {
                return Err(invalid("invalid or duplicate namespace vector layout"));
            }
            indexes.insert(ns.id, FlatVectorIndex::new(ns.embedding_dim as usize));
        }
        Ok(Self {
            layout: Layout::Namespaces {
                indexes,
                locations: HashMap::new(),
                total: 0,
            },
        })
    }
    pub(crate) fn empty_for_rebuild(
        &self,
        namespaces: &[NamespaceConfig],
    ) -> Result<Self, VectorError> {
        match &self.layout {
            Layout::Uniform(index) => Ok(Self::uniform(index.dimensions())),
            Layout::Namespaces { .. } => Self::from_namespaces(namespaces),
        }
    }
    /// Legacy advisory helpers cannot use this as native semantic admission.
    pub(crate) fn uniform_index(&self) -> Result<&FlatVectorIndex, VectorError> {
        match &self.layout {
            Layout::Uniform(index) => Ok(index),
            Layout::Namespaces { .. } => {
                Err(invalid("legacy vector helper requires uniform owner"))
            }
        }
    }
    pub fn len(&self) -> usize {
        match &self.layout {
            Layout::Uniform(index) => index.len(),
            Layout::Namespaces { total, .. } => *total,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(crate) fn reserve(&mut self, additional: usize) -> Result<(), VectorError> {
        // Native partitions grow only for actual rows; never reserve N in each namespace.
        if let Layout::Uniform(index) = &mut self.layout {
            additional
                .checked_mul(index.dimensions())
                .and_then(|n| n.checked_mul(4))
                .ok_or_else(|| invalid("vector allocation size overflow"))?;
            index.reserve(additional);
        }
        Ok(())
    }
    pub fn add(
        &mut self,
        id: MemoryId,
        vector: &[f32],
        metadata: VectorMetadata,
    ) -> Result<(), VectorError> {
        match &mut self.layout {
            Layout::Uniform(index) => index.add(id, vector, metadata),
            Layout::Namespaces {
                indexes,
                locations,
                total,
            } => {
                let ns = metadata.namespace_id;
                if locations.get(&id).is_some_and(|old| *old != ns) {
                    return Err(invalid("vector ID already belongs to another namespace"));
                }
                let added = !locations.contains_key(&id);
                let next = total
                    .checked_add(usize::from(added))
                    .ok_or_else(|| invalid("vector count overflow"))?;
                let index = indexes
                    .get_mut(&ns)
                    .ok_or_else(|| invalid("unknown vector namespace"))?;
                next.checked_mul(index.dimensions())
                    .and_then(|n| n.checked_mul(4))
                    .ok_or_else(|| invalid("vector allocation size overflow"))?;
                index.add(id, vector, metadata)?;
                locations.insert(id, ns);
                *total = next;
                Ok(())
            }
        }
    }
    pub fn remove(&mut self, id: MemoryId) -> Result<bool, VectorError> {
        match &mut self.layout {
            Layout::Uniform(index) => index.remove(id),
            Layout::Namespaces {
                indexes,
                locations,
                total,
            } => {
                let Some(ns) = locations.get(&id) else {
                    return Ok(false);
                };
                let next = total
                    .checked_sub(1)
                    .ok_or_else(|| invalid("vector count underflow"))?;
                if !indexes
                    .get_mut(ns)
                    .ok_or_else(|| invalid("vector location namespace missing"))?
                    .remove(id)?
                {
                    return Err(invalid("vector location entry missing"));
                }
                locations.remove(&id);
                *total = next;
                Ok(true)
            }
        }
    }
    pub fn get_vector(&self, id: MemoryId) -> Option<Vec<f32>> {
        match &self.layout {
            Layout::Uniform(index) => index.get_vector(id),
            Layout::Namespaces {
                indexes, locations, ..
            } => indexes.get(locations.get(&id)?)?.get_vector(id),
        }
    }
    pub fn update_metadata(
        &mut self,
        id: MemoryId,
        metadata: VectorMetadata,
    ) -> Result<bool, VectorError> {
        match &mut self.layout {
            Layout::Uniform(index) => index.update_metadata(id, metadata),
            Layout::Namespaces {
                indexes, locations, ..
            } => {
                let Some(ns) = locations.get(&id) else {
                    return Ok(false);
                };
                if *ns != metadata.namespace_id {
                    return Err(invalid("vector metadata cannot change namespace"));
                }
                indexes
                    .get_mut(ns)
                    .ok_or_else(|| invalid("vector namespace missing"))?
                    .update_metadata(id, metadata)
            }
        }
    }
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        filter: &SearchFilter,
    ) -> Result<Vec<VectorSearchResult>, VectorError> {
        self.search_eligible(query, k, filter, None)
    }
    pub fn search_eligible(
        &self,
        query: &[f32],
        k: usize,
        filter: &SearchFilter,
        eligible: Option<&HashSet<MemoryId>>,
    ) -> Result<Vec<VectorSearchResult>, VectorError> {
        match &self.layout {
            Layout::Uniform(index) => index.search_eligible(query, k, filter, eligible),
            Layout::Namespaces { indexes, .. } => {
                let ns = filter
                    .namespace_id
                    .ok_or_else(|| invalid("partitioned vector search requires namespace"))?;
                indexes
                    .get(&ns)
                    .ok_or_else(|| invalid("unknown vector namespace"))?
                    .search_eligible(query, k, filter, eligible)
            }
        }
    }
    /// Forward measured work from exactly the selected namespace's flat scan.
    pub(crate) fn search_scoped_measured(
        &self,
        namespace: NamespaceId,
        query: &[f32],
        k: usize,
        filter: &SearchFilter,
        eligible: Option<&HashSet<MemoryId>>,
    ) -> Result<super::SourceResults<VectorSearchResult>, VectorError> {
        if filter.namespace_id != Some(namespace) {
            return Err(invalid("vector search scope/filter mismatch"));
        }
        match &self.layout {
            Layout::Uniform(index) => index.search_eligible_measured(query, k, filter, eligible),
            Layout::Namespaces { indexes, .. } => indexes
                .get(&namespace)
                .ok_or_else(|| invalid("unknown vector namespace"))?
                .search_eligible_measured(query, k, filter, eligible),
        }
    }

    pub(crate) fn search_scoped(
        &self,
        namespace: NamespaceId,
        query: &[f32],
        k: usize,
        filter: &SearchFilter,
        eligible: Option<&HashSet<MemoryId>>,
    ) -> Result<Vec<VectorSearchResult>, VectorError> {
        if filter.namespace_id != Some(namespace) {
            return Err(invalid("vector search scope/filter mismatch"));
        }
        self.search_eligible(query, k, filter, eligible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(id: u32, width: u32) -> NamespaceConfig {
        let mut ns = NamespaceConfig::default_namespace(0);
        ns.id = NamespaceId::new(id);
        ns.embedding_dim = width;
        ns
    }
    fn metadata(ns: u32) -> VectorMetadata {
        VectorMetadata {
            namespace_id: NamespaceId::new(ns),
            decay_phase: 1,
            tags: vec!["kept".into()],
        }
    }
    fn scores(rows: Vec<VectorSearchResult>) -> Vec<(MemoryId, u32, u8)> {
        rows.into_iter()
            .map(|r| (r.id, r.score.to_bits(), r.decay_phase))
            .collect()
    }
    #[test]
    fn vector_owner_measured_scope_forwards_actual_selected_scan_and_errors() {
        let mut partitioned = VectorIndexOwner::from_namespaces(&[
            config(1, 4),
            config(2, 4),
            config(3, 5),
            config(4, 4),
        ])
        .unwrap();
        let mut uniform = VectorIndexOwner::uniform(4);
        for (n, ns, vector) in [
            (1, 1, [1., 0., 0., 0.]),
            (2, 1, [1., 0., 0., 0.]),
            (3, 1, [0., 1., 0., 0.]),
            (4, 2, [1., 0., 0., 0.]),
        ] {
            let id = MemoryId::from_uuid(uuid::Uuid::from_u128(n));
            partitioned.add(id, &vector, metadata(ns)).unwrap();
            uniform.add(id, &vector, metadata(ns)).unwrap();
        }
        let filter = SearchFilter {
            namespace_id: Some(NamespaceId::new(1)),
            min_score: Some(0.5),
            ..Default::default()
        };
        for (owner, examined) in [(&partitioned, 3), (&uniform, 4)] {
            for k in [0, 1, 2, 3] {
                let measured = owner
                    .search_scoped_measured(
                        NamespaceId::new(1),
                        &[1., 0., 0., 0.],
                        k,
                        &filter,
                        None,
                    )
                    .unwrap();
                assert_eq!(scores(measured.results), scores(owner.search_scoped(
                    NamespaceId::new(1), &[1., 0., 0., 0.], k, &filter, None,
                ).unwrap()));
                if k == 0 {
                    assert_eq!(measured.coverage, None);
                } else {
                    let coverage = measured.coverage.unwrap();
                    assert_eq!(
                        coverage.examined_unit,
                        super::super::ExaminationUnit::VectorEntries
                    );
                    assert_eq!(coverage.examined, examined);
                    assert_eq!(coverage.eligible, 2);
                    assert_eq!(coverage.candidate_limited, k < 2);
                    assert!(coverage.exhausted);
                    assert!(!coverage.budget_exhausted);
                }
                assert!(
                    owner
                        .search_scoped_measured(
                            NamespaceId::new(2),
                            &[1., 0., 0., 0.],
                            k,
                            &filter,
                            None,
                        )
                        .is_err()
                );
            }
        }
        for ns in [3, 99] {
            let filter = SearchFilter {
                namespace_id: Some(NamespaceId::new(ns)),
                ..Default::default()
            };
            for k in [0, 2] {
                assert!(
                    partitioned
                        .search_scoped_measured(
                            NamespaceId::new(ns),
                            &[1., 0., 0., 0.],
                            k,
                            &filter,
                            None,
                        )
                        .is_err()
                );
            }
        }
        let filter = SearchFilter {
            namespace_id: Some(NamespaceId::new(4)),
            ..Default::default()
        };
        let empty = partitioned
            .search_scoped_measured(NamespaceId::new(4), &[1., 0., 0., 0.], 0, &filter, None)
            .unwrap();
        let coverage = empty.coverage.unwrap();
        assert!(empty.results.is_empty() && coverage.exhausted);
        assert_eq!((coverage.examined, coverage.eligible), (0, 0));
    }
    #[tokio::test]
    async fn vector_owner_legacy_autolink_rejects_partitioned_even_at_zero_cap() {
        use crate::storage::StorageEngine as _;
        let fixture = crate::test_support::Fixture::new();
        let namespaces = fixture.storage.read().unwrap().list_namespaces().unwrap();
        let ns = namespaces[0].id;
        let vectors = std::sync::Arc::new(tokio::sync::RwLock::new(
            VectorIndexOwner::from_namespaces(&namespaces).unwrap(),
        ));
        let before = fixture
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
        let result = crate::graph::perform_autolink(
            MemoryId::new(),
            ns,
            &vec![0.; namespaces[0].embedding_dim as usize],
            &[],
            0.5,
            0,
            &vectors,
            &fixture.graph,
            &fixture.storage,
            &fixture.cache,
        )
        .await;
        assert!(matches!(
            result,
            Err(crate::graph::AutoLinkError::VectorSearch(_))
        ));
        assert_eq!(
            fixture
                .storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .generation_ref(),
            before
        );
    }
    #[test]
    fn vector_owner_partition_preserves_kernel_ties_filters_and_foreign_eligibility() {
        let mut owner =
            VectorIndexOwner::from_namespaces(&[config(1, 4), config(2, 4), config(3, 5)]).unwrap();
        let mut kernel = FlatVectorIndex::new(4);
        let a = MemoryId::new();
        let b = MemoryId::new();
        let foreign = MemoryId::new();
        for id in [a, b] {
            owner.add(id, &[1., 0., 0., 0.], metadata(1)).unwrap();
            kernel.add(id, &[1., 0., 0., 0.], metadata(1)).unwrap();
        }
        owner.add(foreign, &[1., 0., 0., 0.], metadata(2)).unwrap();
        for cap in [0, 1, 10] {
            for tags in [vec![], vec!["kept".into()], vec!["absent".into()]] {
                let filter = SearchFilter {
                    namespace_id: Some(NamespaceId::new(1)),
                    include_tags: tags,
                    ..Default::default()
                };
                let eligible = HashSet::from([a, b, foreign]);
                assert_eq!(
                    scores(
                        owner
                            .search_scoped(
                                NamespaceId::new(1),
                                &[1., 0., 0., 0.],
                                cap,
                                &filter,
                                Some(&eligible)
                            )
                            .unwrap()
                    ),
                    scores(
                        kernel
                            .search_eligible(&[1., 0., 0., 0.], cap, &filter, Some(&eligible))
                            .unwrap()
                    )
                );
                assert!(
                    owner
                        .search_scoped(NamespaceId::new(2), &[1., 0., 0., 0.], cap, &filter, None)
                        .is_err()
                );
            }
        }
        assert!(
            owner
                .search(&[1., 0., 0., 0.], 0, &SearchFilter::default())
                .is_err()
        );
        for ns in [3, 99] {
            assert!(
                owner
                    .search(
                        &[1., 0., 0., 0.],
                        0,
                        &SearchFilter {
                            namespace_id: Some(NamespaceId::new(ns)),
                            ..Default::default()
                        }
                    )
                    .is_err()
            );
        }
        assert!(
            owner
                .search(
                    &[1., 0., 0., 0., 0.],
                    10,
                    &SearchFilter {
                        namespace_id: Some(NamespaceId::new(3)),
                        ..Default::default()
                    }
                )
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn vector_owner_partition_failures_preserve_maps_counts_and_replacements() {
        let mut owner = VectorIndexOwner::from_namespaces(&[config(1, 4), config(2, 5)]).unwrap();
        let id = MemoryId::new();
        owner.add(id, &[1., 0., 0., 0.], metadata(1)).unwrap();
        assert!(owner.add(id, &[1., 0., 0., 0., 0.], metadata(2)).is_err());
        assert!(owner.add(id, &[1.], metadata(1)).is_err());
        assert!(
            owner
                .add(MemoryId::new(), &[1., 0., 0., 0.], metadata(99))
                .is_err()
        );
        assert!(owner.update_metadata(id, metadata(2)).is_err());
        assert_eq!(owner.len(), 1);
        assert_eq!(owner.get_vector(id), Some(vec![1., 0., 0., 0.]));
        owner.add(id, &[0., 1., 0., 0.], metadata(1)).unwrap();
        assert_eq!(owner.len(), 1);
        assert!(owner.remove(id).unwrap());
        assert!(!owner.remove(id).unwrap());
        assert!(owner.get_vector(id).is_none());
        owner.add(id, &[1., 0., 0., 0., 0.], metadata(2)).unwrap();
        assert_eq!(owner.len(), 1);
        let rebuilt = owner.empty_for_rebuild(&[config(1, 4)]).unwrap();
        assert_eq!(rebuilt.len(), 0);
        assert!(rebuilt.get_vector(id).is_none());
        assert!(
            rebuilt
                .search(
                    &[1., 0., 0., 0., 0.],
                    0,
                    &SearchFilter {
                        namespace_id: Some(NamespaceId::new(2)),
                        ..Default::default()
                    }
                )
                .is_err()
        );
        assert!(VectorIndexOwner::from_namespaces(&[config(1, 4), config(1, 5)]).is_err());
        assert!(VectorIndexOwner::from_namespaces(&[config(1, 8193)]).is_err());
        assert!(VectorIndexOwner::from_namespaces(&[config(0, 4)]).is_err());
    }
}
