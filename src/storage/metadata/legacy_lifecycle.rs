//! Format-independent scope capture on the already-owned publication boundary.
use super::*;

impl MetadataStore {
    pub(crate) fn lifecycle_guard_with_work(
        &self,
        parent: Option<&crate::storage::work::WorkTicket>,
    ) -> Result<MutationGuard<'_>, StorageError> {
        if self.uses_native_lifecycle() {
            self.native_guard_with_work(parent)
        } else {
            self.mutation_guard_with_work(parent)
        }
    }

    pub(crate) fn lifecycle_namespace_name(
        &self,
        guard: &MutationGuard<'_>,
        namespace: NamespaceId,
    ) -> Result<Option<String>, StorageError> {
        guard.check(self)?;
        if self.uses_native_lifecycle() {
            return self
                .capture_namespace_binding(guard, namespace)
                .map(|(_, binding)| binding.map(|b| b.name));
        }
        let read = self.db.begin_read()?;
        let table = read.open_table(NAMESPACE_TABLE)?;
        if let Some(value) = table.get(namespace.get())? {
            let config = NamespaceConfig::decode(value.value())?;
            if config.id != namespace || config.schema.is_some() {
                return Err(StorageError::InvalidControl(
                    "legacy namespace binding mismatch".into(),
                ));
            }
            return Ok(Some(config.name));
        }
        // Rare retired-name lookup validates its retained purge proof. Never
        // substitute a currently reused name or a connection default.
        let state = self.control_state()?;
        let progress = purge::validate_existing(&read, Some(&state))?;
        let mut names = std::collections::BTreeSet::new();
        for p in progress {
            if p.namespace_id == namespace && p.store_id == state.store_id {
                names.insert(p.name);
            }
        }
        if names.len() > 1 {
            return Err(StorageError::InvalidControl(
                "ambiguous retired namespace binding".into(),
            ));
        }
        Ok(names.into_iter().next())
    }
}
