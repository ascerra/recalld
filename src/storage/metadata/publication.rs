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
