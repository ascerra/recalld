//! Bridge actual canonical transactions and the single owned view registry.
use super::*;
use crate::model::CachedRecord;
use crate::storage::publication::{DerivedView, PublicationState};
use std::sync::{Arc, RwLock};

impl MetadataStore {
    pub(crate) fn publication_state(&self) -> Result<PublicationState, StorageError> {
        let _publication = self
            .publication
            .read()
            .map_err(|_| StorageError::RecoveryRequired("metadata publication poisoned".into()))?;
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        let table = read.open_table(control::CONTROL)?;
        let value = table
            .get("state")?
            .ok_or_else(|| StorageError::InvalidControl("missing control state".into()))?;
        let state = ControlState::decode(value.value())?;
        Ok(self.view_publication.state(state.generation_ref()))
    }

    pub(crate) fn invalidate_derived_view(&self, view: DerivedView) {
        self.view_publication.invalidate(view);
    }

    /// Only actual startup/rebuild publishers call this after completing `view`.
    /// No generic acknowledgement advances another component's generation.
    pub(crate) fn publish_derived_view(
        &self,
        view: DerivedView,
        at: control::GenerationRef,
    ) -> Result<(), StorageError> {
        let _publication = self
            .publication
            .read()
            .map_err(|_| StorageError::RecoveryRequired("metadata publication poisoned".into()))?;
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        let table = read.open_table(control::CONTROL)?;
        let value = table
            .get("state")?
            .ok_or_else(|| StorageError::InvalidControl("missing control state".into()))?;
        let state = ControlState::decode(value.value())?;
        if state.generation_ref() != at {
            return Err(StorageError::RecoveryRequired(
                "canonical generation changed during derived publication".into(),
            ));
        }
        self.view_publication.published(view, at);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::publication::REQUIRED_VIEWS;

    #[test]
    fn actual_canonical_read_detects_commit_without_notification_hook() {
        let root = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(&root.path().join("meta.db")).unwrap();
        let initial = store.publication_state().unwrap();
        assert!(!initial.matches_canonical());
        for view in REQUIRED_VIEWS {
            store.publish_derived_view(view, initial.canonical).unwrap();
        }
        assert!(store.publication_state().unwrap().matches_canonical());
        // Deliberately omit the in-memory notification. The actual transaction
        // still changes canonical G, so old component stamps cannot pass.
        {
            let _guard = store.mutation_guard().unwrap();
            let tx = store.db.begin_write().unwrap();
            control::advance_generation(&tx).unwrap();
            tx.commit().unwrap();
        }
        let after = store.publication_state().unwrap();
        assert_eq!(
            after.applied,
            initial.applied.map(|_| Some(initial.canonical))
        );
        assert_ne!(after.canonical, initial.canonical);
        assert!(!after.matches_canonical());
        assert!(
            store
                .publish_derived_view(DerivedView::Graph, initial.canonical)
                .is_err()
        );
        store
            .publish_derived_view(DerivedView::Graph, after.canonical)
            .unwrap();
        assert!(!store.publication_state().unwrap().matches_canonical());
        let foreign = control::GenerationRef {
            store_id: uuid::Uuid::new_v4(),
            ..after.canonical
        };
        assert!(
            store
                .publish_derived_view(DerivedView::Vectors, foreign)
                .is_err()
        );
    }

    #[test]
    fn actual_catalog_commit_does_not_bless_untouched_views() {
        let root = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(&root.path().join("meta.db")).unwrap();
        let at = store.publication_state().unwrap().canonical;
        for view in REQUIRED_VIEWS {
            store.publish_derived_view(view, at).unwrap();
        }
        store
            .create_namespace(&NamespaceConfig::default_namespace(0))
            .unwrap();
        let after = store.publication_state().unwrap();
        assert_ne!(after.canonical, at);
        assert_eq!(after.applied, [Some(at); 4]);
        assert!(!after.matches_canonical());
    }
}

/// Actual canonical read transaction retained for the lifetime of a query.
/// Derived guards are acquired by the coordinator before this transaction opens.
pub(crate) struct CanonicalReadView {
    read: redb::ReadTransaction,
    pub at: control::GenerationRef,
    native_owned: bool,
    vector_validation: Arc<RwLock<Option<super::certification::OwnedVectorValidation>>>,
}

/// Borrowed authority for one synchronous canonical point copy. Only the owner
/// constructs this facade; neither its transaction nor owner can be substituted.
pub(crate) struct PointRead<'a> {
    owner: &'a MetadataStore,
    view: &'a CanonicalReadView,
    graph_applied: Option<control::GenerationRef>,
}

impl PointRead<'_> {
    pub(crate) fn view(&self) -> &CanonicalReadView {
        self.view
    }

    /// Caller retains the actual graph read lease. A lease alone is not a stamp.
    pub(crate) fn require_graph(&self) -> Result<(), StorageError> {
        if self.graph_applied != Some(self.view.at) {
            return Err(StorageError::RecoveryRequired(
                "point projection graph generation is unavailable".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn capture_exposure(
        &self,
        scope: crate::storage::native_lifecycle::LifecycleScope,
        id: MemoryId,
        policy: &crate::storage::native_lifecycle::NativePolicyInputs,
    ) -> Result<crate::storage::native_lifecycle::ExposureIntent, StorageError> {
        self.owner.capture_exposure_at(
            &self.view.read,
            scope,
            &[(id, crate::model::AccessKind::DirectRetrieval)],
            policy,
        )
    }
}

impl MetadataStore {
    /// Caller holds engine read ownership through this synchronous copy, and
    /// the actual graph lease when projecting graph-dependent strength.
    pub(crate) fn with_point_read<T>(
        &self,
        copy: impl for<'a> FnOnce(PointRead<'a>) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let _publication = self.publication.read().map_err(|_| {
            StorageError::RecoveryRequired("metadata publication poisoned".into())
        })?;
        // Existing owned-mode/lineage admission rejects canonical_pending and
        // active purge, but unrelated stale derived views do not gate a point.
        let view = self.open_canonical_read()?;
        let graph_applied = self.view_publication.state(view.at).applied[DerivedView::Graph as usize];
        copy(PointRead { owner: self, view: &view, graph_applied })
    }

    pub(crate) fn capture_read_view(&self) -> Result<CanonicalReadView, StorageError> {
        let _publication = self
            .publication
            .read()
            .map_err(|_| StorageError::RecoveryRequired("metadata publication poisoned".into()))?;
        let view = self.open_canonical_read()?;
        let at = view.at;
        if self.needs_effects_rebuild() || !self.view_publication.state(at).matches_canonical() {
            return Err(StorageError::RecoveryRequired(
                "canonical and derived generations are not coherent".into(),
            ));
        }
        Ok(view)
    }

    // Caller holds metadata publication. The same transaction supplies admission,
    // lineage, namespace/record bytes and G, with no derived readiness assertion.
    fn open_canonical_read(&self) -> Result<CanonicalReadView, StorageError> {
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        let state = if self.native_lifecycle_only {
            // Same transaction and already-owned native gate as the native
            // writer/reader join; no ordinary schema-2 startup bypass.
            self.native_control_at(&read)?
        } else {
            self.ensure_legacy_operation()?;
            let table = read.open_table(control::CONTROL)?;
            let value = table
                .get("state")?
                .ok_or_else(|| StorageError::InvalidControl("missing control state".into()))?;
            ControlState::decode(value.value())?
        };
        crate::storage::lineage::ensure_serving(&read, &state)?;
        Ok(CanonicalReadView {
            read,
            at: state.generation_ref(),
            native_owned: self.native_lifecycle_only,
            vector_validation: self.vector_validation.clone(),
        })
    }

    /// Keep canonical writers excluded until the caller has copied all payload.
    /// The caller must additionally retain the actual engine read guard, which
    /// pins selected text handles and excludes vector slot reuse/compaction.
    pub(crate) fn with_generation_read_at<T>(
        &self,
        expected: control::GenerationRef,
        copy: impl FnOnce(&CanonicalReadView) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let _publication = self
            .publication
            .read()
            .map_err(|_| StorageError::RecoveryRequired("metadata publication poisoned".into()))?;
        let view = self.open_canonical_read()?;
        if view.at.store_id != expected.store_id {
            return Err(StorageError::StoreLineageMismatch);
        }
        if view.at != expected {
            return Err(StorageError::GenerationConflict {
                expected: expected.generation.get().to_string(),
                actual: view.at.generation.get().to_string(),
            });
        }
        copy(&view)
    }
}

impl CanonicalReadView {
    /// Constructor-validated owner format, never a request or profile inference.
    pub(crate) fn is_native_owned(&self) -> bool {
        self.native_owned
    }

    /// Semantic inventory admission on this exact owned canonical snapshot.
    /// Supplied query vectors need this same scope/certification proof without
    /// claiming that an active provider produced the caller's vector.
    pub(crate) fn check_native_vector_namespace(
        &self,
        namespace: &NamespaceConfig,
    ) -> Result<(), StorageError> {
        if !self.native_owned {
            return Err(StorageError::RecoveryRequired(
                "native query requires an owned native read view".into(),
            ));
        }
        let actual = self
            .namespaces()?
            .into_iter()
            .find(|ns| ns.id == namespace.id)
            .ok_or(StorageError::NamespaceNotFound(namespace.id.get()))?;
        if control::encode(&actual)? != control::encode(namespace)? {
            return Err(StorageError::RecoveryRequired(
                "native query canonical scope mismatch".into(),
            ));
        }
        self.vector_validation
            .read()
            .map_err(|_| StorageError::RecoveryRequired("vector validation poisoned".into()))?
            .as_ref()
            .ok_or_else(|| {
                StorageError::RecoveryRequired("owned vector validation missing".into())
            })?
            .require_namespace(self.at, &actual)
    }

    /// Generated queries additionally require the active provider's identity.
    /// A namespace descriptor or provider alone cannot grant native serving.
    pub(crate) fn check_native_query_profile(
        &self,
        namespace: &NamespaceConfig,
        active: Option<&crate::model::schema::EmbeddingProfile>,
    ) -> Result<(), StorageError> {
        self.check_native_vector_namespace(namespace)?;
        let active = active.ok_or_else(|| {
            StorageError::RecoveryRequired("native query provider identity missing".into())
        })?;
        crate::embedding::profile::require_compatible(namespace, active)
            .map_err(|e| StorageError::RecoveryRequired(e.to_string()))?;
        Ok(())
    }

    pub(crate) fn get(&self, id: MemoryId) -> Result<Option<CachedRecord>, StorageError> {
        Ok(self.disk_record(id)?.as_ref().map(CachedRecord::from))
    }

    pub(crate) fn disk_record(&self, id: MemoryId) -> Result<Option<DiskRecord>, StorageError> {
        let table = self.read.open_table(META_TABLE)?;
        table
            .get(id.as_bytes().as_slice())?
            .map(|value| {
                let record = DiskRecord::from_bytes(value.value())?;
                if record.id != *id.as_bytes() {
                    return Err(StorageError::CorruptIndex(
                        "record identity mismatch".into(),
                    ));
                }
                Ok(record)
            })
            .transpose()
    }

    pub(crate) fn namespaces(&self) -> Result<Vec<NamespaceConfig>, StorageError> {
        let table = self.read.open_table(NAMESPACE_TABLE)?;
        table
            .iter()?
            .map(|entry| {
                let (key, value) = entry?;
                let config: NamespaceConfig = serde_json::from_slice(value.value())
                    .map_err(|e| StorageError::Deserialize(e.to_string()))?;
                if config.id.get() != key.value() {
                    return Err(StorageError::CorruptIndex(
                        "namespace identity mismatch".into(),
                    ));
                }
                Ok(config)
            })
            .collect()
    }

    pub(crate) fn records(
        &self,
        namespace: NamespaceId,
    ) -> Result<Vec<CachedRecord>, StorageError> {
        let namespaces = self.read.open_multimap_table(NAMESPACE_INDEX)?;
        let mut result = Vec::new();
        for entry in namespaces.get(namespace.get())? {
            let entry = entry?;
            let bytes: [u8; 16] = entry
                .value()
                .try_into()
                .map_err(|_| StorageError::CorruptIndex("invalid namespace member UUID".into()))?;
            if let Some(record) = self.get(MemoryId::from_bytes(bytes))? {
                if record.namespace_id != namespace {
                    return Err(StorageError::CorruptIndex(
                        "namespace member mismatch".into(),
                    ));
                }
                result.push(record);
            }
        }
        Ok(result)
    }
}

/// Current durable work on a handle whose full startup proof already passed.
/// Reads real tables, never infers zero pending from an in-memory dirty flag.
/// Deadline checks bound cooperative row processing, not blocking OS I/O.
pub(super) fn check_current_pending(
    read: &redb::ReadTransaction,
    state: &ControlState,
    check: impl FnMut() -> Result<(), StorageError>,
) -> Result<(), StorageError> {
    let (effects, purges) = current_pending_counts(read, state, check)?;
    if effects != 0 || purges != 0 {
        return Err(StorageError::RecoveryRequired(format!(
            "durable work pending: {effects} effects, {purges} purges"
        )));
    }
    Ok(())
}

fn current_pending_counts(
    read: &redb::ReadTransaction,
    state: &ControlState,
    mut check: impl FnMut() -> Result<(), StorageError>,
) -> Result<(u64, usize), StorageError> {
    let mut effects_count = 0;
    let mut purges_count = 0usize;
    use super::{effects, purge};
    use redb::{MultimapTableHandle, TableHandle};
    let reserved = [
        effects::SCHEMA.name(),
        effects::EFFECTS.name(),
        purge::SCHEMA.name(),
        purge::PROGRESS.name(),
    ];
    check()?;
    for table in read.list_multimap_tables()? {
        check()?;
        if reserved.contains(&table.name()) {
            return Err(StorageError::InvalidControl(
                "pending table is a multimap".into(),
            ));
        }
    }
    let mut names = Vec::new();
    for table in read.list_tables()? {
        check()?;
        names.push(table.name().to_owned());
    }
    for (schema, records) in [
        (effects::SCHEMA, effects::EFFECTS),
        (purge::SCHEMA, purge::PROGRESS),
    ] {
        check()?;
        let present = [schema.name(), records.name()].map(|name| names.iter().any(|v| v == name));
        if present == [false, false] {
            continue;
        }
        if present != [true, true] {
            return Err(StorageError::InvalidControl(
                "partial current pending schema".into(),
            ));
        }
        let table = read.open_table(schema)?;
        if table.len()? != 1 || table.get("version")?.is_none_or(|v| v.value() != 1) {
            return Err(StorageError::InvalidControl(
                "unsupported current pending schema".into(),
            ));
        }
        if records.name() == effects::EFFECTS.name() {
            effects_count = read.open_table(records)?.len()?;
        } else {
            for row in read.open_table(records)?.iter()? {
                check()?;
                let (key, value) = row?;
                let progress: crate::storage::purge::PurgeProgress =
                    control::decode(value.value())?;
                if !matches!(progress.version, 1 | 2)
                    || (progress.version == 2 && state.control_schema_version != 2)
                    || !purge::start_matches(&progress)
                    || key.value() != progress.start_receipt.operation_id.as_bytes()
                {
                    return Err(StorageError::InvalidControl(
                        "invalid current purge identity".into(),
                    ));
                }
                if progress.version == 2
                    && progress.stage == crate::storage::purge::PurgeStage::Complete
                {
                    purge::check_completed_native_progress(read, &progress)?;
                }
                // Completed restored ancestry was verified at open and is not
                // forbidden just because it belongs to an earlier lineage.
                if progress.stage != crate::storage::purge::PurgeStage::Complete {
                    if progress.store_id != state.store_id {
                        return Err(StorageError::InvalidControl(
                            "active purge has foreign lineage".into(),
                        ));
                    }
                    purges_count = purges_count
                        .checked_add(1)
                        .ok_or(StorageError::GenerationExhausted)?;
                }
            }
        }
    }
    check()?;
    Ok((effects_count, purges_count))
}

impl MetadataStore {
    /// Caller retains the actual engine read guard and publication coordinator
    /// read lease. Never closes admission or waits on the caller's own root.
    pub(crate) fn observe_readiness(
        &self,
        deadline: std::time::Instant,
    ) -> Result<crate::storage::ReadinessObservation, StorageError> {
        let check = || {
            if std::time::Instant::now() >= deadline {
                return Err(StorageError::RecoveryRequired(
                    "readiness observation deadline expired".into(),
                ));
            }
            Ok(())
        };
        check()?;
        let _publication = self.publication.try_read().map_err(|_| {
            StorageError::RecoveryRequired("metadata publication busy or poisoned".into())
        })?;
        self.ensure_handle_available()?;
        let read = self.db.begin_read()?;
        let state = if self.native_lifecycle_only {
            self.native_control_at(&read)?
        } else {
            self.ensure_legacy_operation()?;
            let table = read.open_table(control::CONTROL)?;
            let bytes = table
                .get("state")?
                .ok_or_else(|| StorageError::InvalidControl("missing control state".into()))?;
            ControlState::decode(bytes.value())?
        };
        crate::storage::lineage::ensure_serving(&read, &state)?;
        let (pending_effects, pending_purges) = current_pending_counts(&read, &state, check)?;
        let pending_batch_items = Self::pending_batch_items_at(&read, check)?;
        let state = self.view_publication.state(state.generation_ref());
        let work = self.work_tracker.status();
        check()?;
        Ok(crate::storage::ReadinessObservation {
            canonical: state.canonical,
            derived: ["graph", "vectors", "fts", "entities"]
                .into_iter()
                .zip(state.applied)
                .map(|(name, applied)| crate::storage::ObservedDerived {
                    name,
                    applied,
                    matches_canonical: applied == Some(state.canonical),
                })
                .collect(),
            accepted_work: work.pending,
            unresolved_work: work.unresolved,
            admission_open: work.admission_open,
            pending_effects,
            pending_purges,
            pending_batch_items,
        })
    }
}
