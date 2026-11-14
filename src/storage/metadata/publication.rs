//! Bridge actual canonical transactions and the single owned view registry.
use super::*;
use crate::storage::publication::{DerivedView, PublicationState};

impl MetadataStore {
    pub(crate) fn publication_state(&self) -> Result<PublicationState, StorageError> {
        let _publication = self.publication.read().map_err(|_| {
            StorageError::RecoveryRequired("metadata publication poisoned".into())
        })?;
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        let table = read.open_table(control::CONTROL)?;
        let value = table.get("state")?
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
        let _publication = self.publication.read().map_err(|_| {
            StorageError::RecoveryRequired("metadata publication poisoned".into())
        })?;
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        let table = read.open_table(control::CONTROL)?;
        let value = table.get("state")?
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
        assert_eq!(after.applied, initial.applied.map(|_| Some(initial.canonical)));
        assert_ne!(after.canonical, initial.canonical);
        assert!(!after.matches_canonical());
        assert!(store.publish_derived_view(DerivedView::Graph, initial.canonical).is_err());
        store.publish_derived_view(DerivedView::Graph, after.canonical).unwrap();
        assert!(!store.publication_state().unwrap().matches_canonical());
        let foreign = control::GenerationRef { store_id: uuid::Uuid::new_v4(), ..after.canonical };
        assert!(store.publish_derived_view(DerivedView::Vectors, foreign).is_err());
    }

    #[test]
    fn actual_catalog_commit_does_not_bless_untouched_views() {
        let root = tempfile::tempdir().unwrap();
        let store = MetadataStore::open(&root.path().join("meta.db")).unwrap();
        let at = store.publication_state().unwrap().canonical;
        for view in REQUIRED_VIEWS {
            store.publish_derived_view(view, at).unwrap();
        }
        store.create_namespace(&NamespaceConfig::default_namespace(0)).unwrap();
        let after = store.publication_state().unwrap();
        assert_ne!(after.canonical, at);
        assert_eq!(after.applied, [Some(at); 4]);
        assert!(!after.matches_canonical());
    }
}
