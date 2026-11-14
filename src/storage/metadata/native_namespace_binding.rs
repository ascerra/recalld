//! Canonical namespace evidence for replay under an existing checked guard.
use super::*;
use crate::storage::control::GenerationRef;
use crate::storage::native_purge::NativeNamespaceProof;

impl MetadataStore {
    /// Capture identity for the caller's original retry hash/assertions. This is
    /// neither fresh policy admission nor derived readiness. The caller already
    /// owns publication and must not drop its known receipt on a capture error.
    /// An existing unchanged write transaction must match the returned G before
    /// using this snapshot. No publication lock, clock or provider is reacquired.
    pub(crate) fn capture_namespace_binding(
        &self,
        guard: &MutationGuard<'_>,
        namespace: NamespaceId,
    ) -> Result<(GenerationRef, Option<NativeNamespaceProof>), StorageError> {
        guard.check(self)?;
        let read = self.db.begin_read()?;
        let control = self.native_control_at(&read)?;
        let table = read.open_table(NAMESPACE_TABLE)?;
        if let Some(value) = table.get(namespace.get())? {
            let config = NamespaceConfig::decode(value.value())?;
            if config.id != namespace {
                return Err(StorageError::InvalidControl(
                    "namespace binding table identity mismatch".into(),
                ));
            }
            // Strict native open validates live/retired agreement. Participating
            // writers preserve it; unknown outcomes poison this owning handle.
            return Ok((
                control.generation_ref(),
                Some(NativeNamespaceProof::capture(&config)?),
            ));
        }
        let progress = purge::validate_existing(&read, Some(&control))?;
        let bindings = crate::storage::native_purge::retired_namespace_bindings(&progress)?;
        let binding = bindings.get(&namespace.get()).map(|proof| (*proof).clone());
        Ok((control.generation_ref(), binding))
    }
}
