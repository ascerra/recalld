# CanonicalVectorSnapshot handshake, 2026-11-14

Owner: Feynman (storage capture/current validation); Huygens (shared MCP/REST exact comparator, cursor/schema/routes). Fold private backend into intended consumer branch; no standalone helper PR or extra offline CLI. Native serving activation remains gated. This is not a whole-query ReadView.

Minimal backend parent: impl/canonical-writer-guards af20d73 (runtime 12a55a0), including canonical insertion/tombstone/maintenance, actual Observed58 and raw writer restrictions. Consumer can merge this backend commit into its actual prepared consumer parent. No broad integration ancestry. Native-owner storage/serving join is an explicit additional prerequisite before v2 endpoint activation.

## Frozen crate-private API

Module `storage::canonical_vector_snapshot` (pub(crate)); inherent methods on RedbStorageEngine, not a new engine or public StorageEngine escape. Types are Debug; binding/limits/reasons additionally Clone, PartialEq, Eq. Snapshot and row are owned, no guards or borrowed mmap bytes escape.

```rust
pub(crate) struct SnapshotLimits {
    pub max_records: usize,
    pub max_bytes: usize,
}
pub(crate) struct SnapshotBinding {
    pub at: crate::storage::control::GenerationRef,
    pub namespace_id: crate::model::NamespaceId,
    pub namespace_sha256: [u8; 32],
}
pub(crate) struct CanonicalVectorSnapshot {
    pub binding: SnapshotBinding,
    pub namespace: crate::model::NamespaceConfig,
    pub rows: Vec<CanonicalVectorRow>,
}
pub(crate) struct CanonicalVectorRow {
    pub id: crate::model::MemoryId,
    pub record: crate::model::DiskRecord,
    pub vector: SnapshotVector,
}
pub(crate) enum SnapshotVector {
    Available(Vec<f32>),
    Missing,
    Invalid(InvalidVectorReason),
}
pub(crate) enum InvalidVectorReason { DimensionMismatch, NonFinite, ZeroNorm }
pub(crate) enum SnapshotError {
    NamespaceNotFound,
    Stale,
    LimitExceeded,
    Unavailable(crate::storage::StorageError),
}
impl RedbStorageEngine {
    pub(crate) fn canonical_vector_snapshot(
        &self, namespace_name: &str, expected: Option<&SnapshotBinding>,
        limits: SnapshotLimits,
    ) -> Result<CanonicalVectorSnapshot, SnapshotError>;
    pub(crate) fn validate_vector_snapshot(
        &self, binding: &SnapshotBinding,
    ) -> Result<(), SnapshotError>;
}
```

Caller uses spawn_blocking and owns actual Arc<RwLock<RedbStorageEngine>> read guard for each synchronous call. Inside each call: acquire metadata publication read guard, recheck canonical availability, begin one redb read transaction. Capture control GenerationRef, canonical namespace bytes, active namespace status and matching canonical rows from that SAME transaction; read actual vector bytes while outer storage guard and metadata publication guard remain held. No provider invocation or std guard across await. No graph/cache/FTS/vector-index reads. Avoid helpers that independently open read transactions or recursively acquire publication locks.

Binding hashes exact persisted namespace bytes, including profile descriptor/provenance, lifecycle version/revision, policy, identity/name, required features and extensions. Expected binding must match current generation AND namespace ID AND bytes digest, before reading rows. validate_vector_snapshot repeats these checks after comparison; an absent/reincarnated namespace, foreign lineage, generation or policy change is Stale. Metadata unavailability/corruption is Unavailable, not a false empty page. Consumer converts Stale to CURSOR_STALE and discards computed page. No silent retry advancing a continuation. Any initial-request retry is bounded explicitly by consumer.

Rows include Full/Summary/Ghost/Tombstone and are ordered by UUID raw bytes, with key/record identity checked. Preserve f32 bits; do not normalize stored vectors. Canonical vector reference/namespace dimension/slot allocation ownership/digest must agree. A retained Ghost vector may be eligible. Tombstone cannot gain eligibility by vector remnants. Missing denotes legitimate absent retained vector; corrupt referenced payload/slot ownership/hash mismatch fails the whole capture, not a silently excluded row. Nonfinite/zero norm/dimension mismatch, when supported by retained legacy semantics, has a stable reason, never silently treated as dissimilar.

The snapshot never blesses UnknownLegacy with the current configured provider. It returns canonical NamespaceConfig; Huygens requires its Bound embedding profile for v2 exact eligibility. Legacy capture/ownership guard tests are permitted without enabling v2 serving. Actual native ownership/reference proof is a required integration check before live v2 activation.

Bounds are whole-snapshot errors, no truncation. Count every namespace row including excluded/terminal rows. max_bytes is a checked budget for retained encoded record bytes plus copied vector bytes; check lengths before decoding/copying each row and checked additions. Unknown/invalid limits or arithmetic overflow fail LimitExceeded. Hard server caps are consumer policy and advertised consistently. Memory overhead is bounded by row count in addition to encoded payload budget; this is not an allocator-exact accounting promise.

Consumer cursor owns version, threshold, scope, profile binding and pair position; expected SnapshotBinding must be re-created from validated cursor, and each computation ends with validate_vector_snapshot. Compare/serialize only owned values after releasing std locks. Consumer coverage remains exact-pair scope only; no whole-query readiness/ReadView claims.

## Required backend checks

Actual writers during postcapture comparison: insertion/tombstone, catalog/policy-only commit, observed lifecycle write, lineage change/reopen. Actual storage lock must prevent vector slot reuse during copy. No namespace ID/name rebinding. Overflow and malformed metadata/vector ownership fail closed. Pending canonical effects reject. Owned snapshot f32 bits and UUID ordering survive release. Ghost eligibility and tombstone exclusion tested through consumer. Native production serving stays gated until native owner joins participating writers.

## Whole-query follow-up, owned by Halley coordinator

A coherent query must pin one canonical transaction and matching graph/vector/FTS/entity publications, with exact GenerationRefs. Invalidation occurs inside canonical commit publication guard for catalog/policy/observed/batch bookkeeping and content/graph writers; effects_epoch is not a generation. Channel stamps advance only after actual publication or an audited no-change proof for that channel. No generic complete() blessing. Readiness/barrier must account accepted/transitive work and unresolved canonical effects, and fail closed rather than return stale-index empty success. This snapshot adds no duplicate registry.
