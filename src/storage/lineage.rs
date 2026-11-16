//! Versioned restore provenance. Origin generations are compared only within lineage.
use super::{
    StorageError,
    control::{ControlState, GenerationRef},
    metadata::removal_ledger::LedgerState,
};
use redb::{
    MultimapTableHandle, ReadTransaction, ReadableTable, ReadableTableMetadata, TableDefinition,
    TableHandle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(crate) const LINEAGE: TableDefinition<&str, &[u8]> = TableDefinition::new("restore_lineage_v1");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RestoreMode {
    Serving,
    Historical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LedgerSource {
    pub source: GenerationRef,
    pub coverage: LedgerState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LineageState {
    pub version: u32,
    pub active_store_id: Uuid,
    pub operation_id: Uuid,
    pub mode: RestoreMode,
    pub source_snapshot: GenerationRef,
    pub captured_ledger_source: Option<GenerationRef>,
    pub archive_sha256: [u8; 32],
    pub ancestors: Vec<GenerationRef>,
    pub ledger_sources: Vec<LedgerSource>,
    /// Only validated private restore staging creates this anchor. Omission
    /// preserves all historical version1 bytes; explicit null is not absence.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::request_context::deserialize_present"
    )]
    pub native_text_anchor: Option<NativeTextAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NativeTextAnchor {
    version: u32,
    count: u64,
    sha256: [u8; 32],
}

/// A streaming commitment to original scoped keys and StoredReceipt bytes.
/// Callers traverse redb's canonical key order after the real receipt merge.
pub(crate) struct TextAnchorBuilder {
    hash: Sha256,
    count: u64,
}
impl TextAnchorBuilder {
    pub(crate) fn new() -> Self {
        let mut hash = Sha256::new();
        hash.update(b"restored-native-text-receipts-v1");
        Self { hash, count: 0 }
    }
    pub(crate) fn push(
        &mut self,
        active: Uuid,
        key: &[u8],
        bytes: &[u8],
    ) -> Result<(), StorageError> {
        let stored = super::control::validate_receipt(bytes)?;
        if matches!(
            stored.receipt.outcome,
            super::control::MutationOutcome::NativeTextGenerationSelected { .. }
        ) && stored.receipt.store_id != active
        {
            let expected = super::control::scoped_key(
                stored.receipt.store_id,
                super::control::MutationScope::Catalog,
                stored.receipt.operation_id.as_bytes(),
            );
            if key != expected {
                return Err(StorageError::InvalidControl(
                    "native text anchor scope mismatch".into(),
                ));
            }
            self.count = self.count.checked_add(1).ok_or_else(|| {
                StorageError::InvalidControl("native text anchor overflow".into())
            })?;
            for part in [key, bytes] {
                self.hash.update((part.len() as u64).to_be_bytes());
                self.hash.update(part);
            }
        }
        Ok(())
    }
    pub(crate) fn finish(self) -> Option<NativeTextAnchor> {
        (self.count > 0).then(|| NativeTextAnchor {
            version: 1,
            count: self.count,
            sha256: self.hash.finalize().into(),
        })
    }
}

/// Full strict readers call once, not once per historical permits() lookup.
pub(crate) fn validate_text_anchor(
    tx: &ReadTransaction,
    control: &ControlState,
    lineage: Option<&LineageState>,
) -> Result<(), StorageError> {
    let mut actual = TextAnchorBuilder::new();
    for row in tx.open_table(super::control::RECEIPTS)?.iter()? {
        let (key, value) = row?;
        actual.push(control.store_id, key.value(), value.value())?;
    }
    if actual.finish().as_ref() != lineage.and_then(|l| l.native_text_anchor.as_ref()) {
        return Err(StorageError::InvalidControl(
            "foreign native text receipt anchor mismatch".into(),
        ));
    }
    Ok(())
}

impl LineageState {
    pub(crate) fn permits(&self, origin: GenerationRef) -> bool {
        self.ancestors
            .iter()
            .any(|a| a.store_id == origin.store_id && a.generation >= origin.generation)
    }
    pub(crate) fn validate(&self, control: &ControlState) -> Result<(), StorageError> {
        let mut ids = std::collections::HashSet::new();
        if !matches!(
            (self.version, self.native_text_anchor.as_ref()),
            (1, None)
                | (
                    2,
                    Some(NativeTextAnchor {
                        version: 1,
                        count: 1..,
                        ..
                    })
                )
        ) || control.restore_lineage_version != Some(self.version)
            || self.active_store_id != control.store_id
            || self.operation_id.is_nil()
            || (self.mode == RestoreMode::Serving) != self.captured_ledger_source.is_some()
            || self.ancestors.is_empty()
            || self.ancestors.iter().any(|a| {
                a.store_id.is_nil() || a.store_id == control.store_id || !ids.insert(a.store_id)
            })
            || !self.permits(self.source_snapshot)
            || self
                .captured_ledger_source
                .is_some_and(|s| !self.permits(s))
            || self.ledger_sources.is_empty()
            || self.ledger_sources.iter().any(|s| {
                !self.permits(s.source)
                    || s.coverage.ledger_version != 1
                    || !self.permits(s.coverage.tracking_started_at)
            })
        {
            return Err(StorageError::InvalidControl(
                "invalid restore lineage/provenance".into(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn read_lineage(
    tx: &ReadTransaction,
    control: &ControlState,
) -> Result<Option<LineageState>, StorageError> {
    let present = tx.list_tables()?.any(|t| t.name() == LINEAGE.name());
    let wrong = tx
        .list_multimap_tables()?
        .any(|t| t.name() == LINEAGE.name());
    if !present && !wrong && control.restore_lineage_version.is_none() {
        return Ok(None);
    }
    if wrong || !present || !matches!(control.restore_lineage_version, Some(1 | 2)) {
        return Err(StorageError::InvalidControl(
            "missing/unmarked/unsupported restore lineage".into(),
        ));
    }
    let table = tx.open_table(LINEAGE)?;
    if table.len()? != 1 {
        return Err(StorageError::InvalidControl(
            "invalid lineage row count".into(),
        ));
    }
    let value = table
        .get("state")?
        .ok_or_else(|| StorageError::InvalidControl("missing lineage state".into()))?;
    let state: LineageState = serde_json::from_slice(value.value())
        .map_err(|e| StorageError::InvalidControl(e.to_string()))?;
    state.validate(control)?;
    Ok(Some(state))
}

/// Schema/export consumers use this to validate preserved historical provenance.
pub(crate) fn permits_origin(
    tx: &ReadTransaction,
    control: &ControlState,
    origin: GenerationRef,
) -> Result<bool, StorageError> {
    let lineage = read_lineage(tx, control)?;
    Ok(if origin.store_id == control.store_id {
        origin.generation <= control.committed_generation
    } else {
        lineage.is_some_and(|s| s.permits(origin))
    })
}

pub(crate) fn ensure_serving(
    tx: &ReadTransaction,
    control: &ControlState,
) -> Result<(), StorageError> {
    if read_lineage(tx, control)?.is_some_and(|s| s.mode == RestoreMode::Historical) {
        return Err(StorageError::RecoveryRequired(
            "historical restore is read-only and excluded from ordinary serving".into(),
        ));
    }
    Ok(())
}
