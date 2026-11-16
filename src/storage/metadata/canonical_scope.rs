//! Shared exact-first namespace resolution in the caller's canonical transaction.
use super::*;
use crate::storage::canonical_vector_snapshot::{SnapshotBinding, SnapshotError};
use sha2::{Digest, Sha256};
fn unavailable(message: &str) -> SnapshotError {
    StorageError::OwnershipMismatch(message.into()).into()
}
pub(super) fn current(tx: &redb::ReadTransaction) -> Result<ControlState, StorageError> {
    let table = tx.open_table(control::CONTROL)?;
    let value = table
        .get("state")?
        .ok_or_else(|| StorageError::InvalidControl("missing control".into()))?;
    let state = ControlState::decode(value.value())?;
    Ok(state)
}
pub(super) fn binding(
    state: &ControlState,
    namespace: NamespaceId,
    bytes: &[u8],
) -> SnapshotBinding {
    SnapshotBinding {
        at: state.generation_ref(),
        namespace_id: namespace,
        namespace_sha256: Sha256::digest(bytes).into(),
    }
}

pub(super) fn resolve_namespace(
    tx: &redb::ReadTransaction,
    state: &ControlState,
    name: &str,
    expected: Option<&SnapshotBinding>,
    max_bytes: usize,
) -> Result<(SnapshotBinding, NamespaceConfig), SnapshotError> {
    let namespaces = tx.open_table(NAMESPACE_TABLE).map_err(StorageError::from)?;
    let mut selected = None;
    let mut folded = None;
    let mut ambiguous = false;
    for row in namespaces.iter().map_err(StorageError::from)? {
        let (key, value) = row.map_err(StorageError::from)?;
        if value.value().len() > max_bytes {
            return Err(SnapshotError::LimitExceeded);
        }
        let config = NamespaceConfig::decode(value.value()).map_err(StorageError::from)?;
        if config.id.get() != key.value() {
            return Err(unavailable("namespace key/ID mismatch"));
        }
        if config.name == name {
            if selected.is_some() {
                return Err(unavailable("duplicate namespace name"));
            }
            selected = Some((binding(&state, config.id, value.value()), config));
        } else if config.name.eq_ignore_ascii_case(name) {
            ambiguous |= folded.is_some();
            folded = Some((binding(&state, config.id, value.value()), config));
        }
    }
    // Preserve canonical exact-first lookup without a separate resolution
    // transaction. Corrupt/legacy ambiguous aliases never choose arbitrarily.
    if selected.is_none() && ambiguous {
        return Err(unavailable("ambiguous namespace alias"));
    }
    let Some((binding, namespace)) = selected.or(folded) else {
        return Err(if expected.is_some() {
            SnapshotError::Stale
        } else {
            SnapshotError::NamespaceNotFound
        });
    };
    if expected.is_some_and(|value| value != &binding) {
        return Err(SnapshotError::Stale);
    }
    Ok((binding, namespace))
}
