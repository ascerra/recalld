//! Private canonical metadata/vector capture for the exact duplicate comparator.
//! This certifies no graph/FTS/entity publication or whole-query ReadView.
use crate::model::{DiskRecord, MemoryId, NamespaceConfig, NamespaceId};
use crate::storage::{RedbStorageEngine, StorageError, control::GenerationRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SnapshotLimits {
    pub max_records: usize,
    pub max_bytes: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnapshotBinding {
    pub at: GenerationRef,
    pub namespace_id: NamespaceId,
    pub namespace_sha256: [u8; 32],
}
#[derive(Debug)]
pub(crate) struct CanonicalVectorSnapshot {
    pub binding: SnapshotBinding,
    pub namespace: NamespaceConfig,
    pub rows: Vec<CanonicalVectorRow>,
    pub accounting: SnapshotAccounting,
}
/// Encoded/copy accounting, not physical I/O or allocator-exact memory usage.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SnapshotAccounting {
    pub record_encoded_bytes: usize,
    pub vector_examined_bytes: usize,
    pub vector_copied_bytes: usize,
}
#[derive(Debug)]
pub(crate) struct CanonicalVectorRow {
    pub id: MemoryId,
    pub record: DiskRecord,
    pub vector: SnapshotVector,
}
#[derive(Debug)]
pub(crate) enum SnapshotVector {
    Available(Vec<f32>),
    Missing,
    Invalid(InvalidVectorReason),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InvalidVectorReason {
    DimensionMismatch,
    NonFinite,
    ZeroNorm,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum SnapshotError {
    #[error("snapshot namespace not found")]
    NamespaceNotFound,
    #[error("CURSOR_STALE")]
    Stale,
    #[error("canonical vector snapshot limit exceeded")]
    LimitExceeded,
    #[error("canonical vector snapshot unavailable: {0}")]
    Unavailable(#[from] StorageError),
}
impl RedbStorageEngine {
    /// Caller holds the owning storage read guard throughout this synchronous call.
    pub(crate) fn canonical_vector_snapshot(
        &self,
        namespace_name: &str,
        expected: Option<&SnapshotBinding>,
        limits: SnapshotLimits,
    ) -> Result<CanonicalVectorSnapshot, SnapshotError> {
        self.meta_store().capture_canonical_vectors(
            self.vector_manager(),
            namespace_name,
            expected,
            limits,
        )
    }
    /// Recheck after comparing owned vectors; discard the entire page on failure.
    pub(crate) fn validate_vector_snapshot(
        &self,
        binding: &SnapshotBinding,
    ) -> Result<(), SnapshotError> {
        self.meta_store().validate_canonical_vectors(binding)
    }
}
