//! Cross-namespace similarity observations retain a separate source admission.
//! Every assertion is rechecked by the existing observation worker before writes.
use std::collections::BTreeMap;

use crate::model::{AccessKind, DecayPhase, MemoryId, NamespaceConfig, NamespaceId};
use crate::storage::control::{ExposureScope, GenerationRef};
use crate::storage::{RedbStorageEngine, StorageEngine as _, StorageError};

#[derive(Clone)]
pub struct SimilarityExposure {
    at: GenerationRef,
    source: MemoryId,
    source_namespace: NamespaceId,
    profile: crate::model::schema::EmbeddingProfile,
    namespaces: BTreeMap<u32, String>,
    selected: BTreeMap<MemoryId, u32>,
}

impl SimilarityExposure {
    pub(crate) fn selected(
        at: GenerationRef,
        source: MemoryId,
        source_namespace: &NamespaceConfig,
        profile: crate::model::schema::EmbeddingProfile,
        rows: impl IntoIterator<Item = (MemoryId, NamespaceConfig)>,
    ) -> Self {
        let mut namespaces = BTreeMap::new();
        namespaces.insert(source_namespace.id.get(), source_namespace.name.clone());
        let mut selected = BTreeMap::new();
        for (id, ns) in rows {
            namespaces.insert(ns.id.get(), ns.name);
            selected.insert(id, ns.id.get());
        }
        Self {
            at,
            source,
            source_namespace: source_namespace.id,
            profile,
            namespaces,
            selected,
        }
    }

    fn scopes(
        &self,
        storage: &RedbStorageEngine,
        groups: &BTreeMap<u32, Vec<(MemoryId, AccessKind)>>,
    ) -> Result<BTreeMap<u32, Option<ExposureScope>>, StorageError> {
        let invalid =
            || StorageError::InvalidMutation("similarity exposure selection changed".into());
        if storage.meta_store().control_state()?.store_id != self.at.store_id {
            return Err(invalid());
        }
        let source = storage.get_record(self.source)?.ok_or_else(invalid)?;
        if source.namespace_id != self.source_namespace.get()
            || matches!(source.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
        {
            return Err(invalid());
        }
        // Namespace policy is intentionally captured fresh by each writer; a
        // selection G is not a claim that exposure commits at that same G.
        for (&id, name) in &self.namespaces {
            let ns = storage
                .get_namespace(NamespaceId::new(id))?
                .ok_or_else(invalid)?;
            if &ns.name != name {
                return Err(invalid());
            }
            crate::embedding::profile::require_compatible(&ns, &self.profile)
                .map_err(|_| invalid())?;
        }
        let mut scopes = BTreeMap::new();
        let mut count = 0;
        for (&ns, batch) in groups {
            let name = self.namespaces.get(&ns).ok_or_else(invalid)?;
            for &(id, kind) in batch {
                if id == self.source
                    || kind != AccessKind::AssociativeRetrieval
                    || self.selected.get(&id) != Some(&ns)
                {
                    return Err(invalid());
                }
                let record = storage.get_record(id)?.ok_or_else(invalid)?;
                if record.namespace_id != ns
                    || matches!(record.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
                {
                    return Err(invalid());
                }
                count += 1;
            }
            scopes.insert(
                ns,
                Some(ExposureScope {
                    namespace: name.clone(),
                    // The foreign source is validated above, not smuggled through
                    // the writer's same-namespace source assertion.
                    source: (ns == self.source_namespace.get()).then_some(self.source),
                }),
            );
        }
        if count != self.selected.len() {
            return Err(invalid());
        }
        Ok(scopes)
    }
}

#[derive(Clone)]
pub(crate) enum ExposureScopes {
    Existing(Option<ExposureScope>),
    Similarity(SimilarityExposure),
}

impl ExposureScopes {
    pub(crate) fn validate(
        &self,
        storage: &RedbStorageEngine,
        groups: &BTreeMap<u32, Vec<(MemoryId, AccessKind)>>,
    ) -> Result<BTreeMap<u32, Option<ExposureScope>>, StorageError> {
        match self {
            Self::Existing(scope) => Ok(groups.keys().map(|&id| (id, scope.clone())).collect()),
            Self::Similarity(proof) => proof.scopes(storage, groups),
        }
    }
}
