//! Unified error type for all storage modules.

use thiserror::Error;
use uuid::Uuid;

use crate::model::error::DecodeError;

/// Errors from storage operations (metadata, vectors, text, edges).
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("maintenance response evidence limit reached before commit")]
    ReceiptBudgetExceeded,
    #[error("maintenance publication depends on work outside this pass")]
    MaintenanceDependencyUnavailable,
    #[error("conflicting maintenance receipt evidence")]
    ReceiptEvidenceConflict,
    /// Existing item receipts remain retained. This is reservation evidence only,
    /// never an assertion that the batch committed atomically or became ready.
    /// Known fresh admission failure; no canonical operation was attempted.
    #[error("unsupported mutation: {0}")]
    UnsupportedMutation(String),

    #[error("RESPONSE_TOO_LARGE: complete native batch result exceeds transport budget")]
    BatchResponseTooLarge {
        reservation_receipt: Option<Box<crate::storage::control::WriteReceipt>>,
    },
    #[error("LIFECYCLE_CONFLICT: {0}")]
    LifecycleConflict(String),
    #[error("CONTENT_UNAVAILABLE")]
    LifecycleContentUnavailable,
    #[error(transparent)]
    Work(#[from] crate::storage::work::WorkError),

    /// Canonical mutation exists; retry publication, never retry as a new write.
    #[error("INDEX_NOT_READY commitState=committed ready=false receipt={} cause={source}", serde_json::to_string(receipt).unwrap_or_default())]
    CommittedMutation {
        receipt: Box<crate::storage::control::WriteReceipt>,
        #[source]
        source: Box<StorageError>,
    },

    /// Invalid lineage, key, version, or generation encoding.
    #[error("invalid mutation context: {0}")]
    InvalidMutationContext(String),

    #[error("STORE_LINEAGE_MISMATCH")]
    StoreLineageMismatch,

    #[error("GENERATION_CONFLICT: expected {expected}, actual {actual}")]
    GenerationConflict { expected: String, actual: String },

    #[error("IDEMPOTENCY_CONFLICT")]
    IdempotencyConflict,

    /// A well-formed caller selector conflicts with the canonical binding.
    #[error("EMBEDDING_PROFILE_MISMATCH")]
    EmbeddingProfileMismatch,

    #[error("EMBEDDING_PROFILE_REQUIRED")]
    EmbeddingProfileRequired,

    #[error("NAMESPACE_MISMATCH")]
    MutationNamespaceMismatch,

    #[error("canonical generation exhausted")]
    GenerationExhausted,

    #[error("unsupported or corrupt canonical control: {0}")]
    InvalidControl(String),

    /// The database may have committed; reconcile by operation ID after reopen.
    #[error("commit outcome unknown for operation {operation_id}: {source}")]
    CommitOutcomeUnknown {
        operation_id: Uuid,
        #[source]
        source: redb::CommitError,
    },

    /// A worker was lost without returning its canonical outcome. No operation
    /// ID or receipt can be invented; recover and reconcile the original intent.
    #[error("worker outcome unknown: {0}")]
    WorkerOutcomeUnknown(String),

    /// Canonical metadata is durable; materialization must resume after reopen.
    #[error("operation {operation_id} committed; canonical effects unavailable: {source}")]
    CommittedEffectsPending {
        operation_id: Uuid,
        #[source]
        source: Box<StorageError>,
    },

    // ── redb errors (from metadata.rs and edges.rs) ──────────────────
    /// A generic redb error.
    #[error("redb error: {0}")]
    Redb(#[from] redb::Error),

    /// A redb database-level error.
    #[error("redb database error: {0}")]
    RedbDatabase(#[from] redb::DatabaseError),

    /// A redb table operation error.
    #[error("redb table error: {0}")]
    RedbTable(#[from] redb::TableError),

    /// A redb transaction error.
    #[error("redb transaction error: {0}")]
    RedbTransaction(#[from] redb::TransactionError),

    /// A redb storage-level error.
    #[error("redb storage error: {0}")]
    RedbStorage(#[from] redb::StorageError),

    /// A redb commit error.
    #[error("redb commit error: {0}")]
    RedbCommit(#[from] redb::CommitError),

    /// Provider output failed vector validation; the store remains usable.
    #[error("EMBEDDING_UNAVAILABLE: invalid provider vector: {0}")]
    ProviderVectorUnavailable(String),

    #[error("active embedding provider configuration is unavailable or invalid")]
    ProviderConfigurationUnavailable,

    #[error("active embedding profile does not match {field}")]
    NamespaceEmbeddingProfileMismatch { field: &'static str },
    /// Exact owned canonical state proves the namespace has no certificate.
    /// This is a known precommit admission failure, never a commit outcome.
    #[error("namespace {namespace_id} vectors require explicit offline re-embedding certification")]
    VectorCertificationRequired { namespace_id: u32 },

    /// Caller-supplied vector failed the shared numeric boundary. Keep the
    /// legacy diagnostic text while allowing typed transport classification.
    #[error("invalid storage mutation: {0}")]
    CallerVectorInvalid(String),

    #[error("EDGE_CONFLICT: {0}")]
    EdgeConflict(String),

    #[error("invalid storage mutation: {0}")]
    InvalidMutation(String),

    #[error(
        "operation too large for this response transport or its bounded admission scan; no new purge was started"
    )]
    PurgeResponseTooLarge,

    #[error("storage recovery required: {0}")]
    RecoveryRequired(String),

    #[error("vector ownership mismatch: {0}")]
    OwnershipMismatch(String),

    // ── Metadata errors ──────────────────────────────────────────────
    /// A record with this ID already exists.
    #[error("duplicate memory ID: {0}")]
    DuplicateId(Uuid),

    /// No record exists for the given ID.
    #[error("memory not found: {0}")]
    NotFound(Uuid),

    /// A namespace with this name already exists.
    #[error("duplicate namespace name: {0}")]
    DuplicateName(String),

    /// A namespace name is not usable as an on-disk directory name.
    #[error("invalid namespace name '{name}': {reason}")]
    InvalidNamespaceName {
        /// The rejected name, echoed back for the caller's message.
        name: String,
        /// Why it was rejected.
        reason: String,
    },

    /// No namespace exists for the given ID.
    #[error("namespace not found: {0}")]
    NamespaceNotFound(u32),

    #[error("namespace not found: {0}")]
    NamespaceNameNotFound(String),

    /// An explicit namespace assertion does not match the record's immutable scope.
    #[error("NAMESPACE_MISMATCH: memory {id} does not belong to namespace '{namespace}'")]
    NamespaceMismatch {
        /// Memory being addressed.
        id: Uuid,
        /// Caller asserted namespace.
        namespace: String,
    },

    /// A secondary index is corrupt or inconsistent.
    #[error("corrupt index: {0}")]
    CorruptIndex(String),

    /// Failed to deserialize a stored value.
    #[error("deserialization error: {0}")]
    Deserialize(String),

    /// Failed to serialize a value for storage.
    #[error("serialization error: {0}")]
    Serialize(String),

    /// Failed to decode a record from its binary representation.
    #[error("decode error: {0}")]
    Decode(#[from] DecodeError),

    /// The database directory is locked by another process.
    #[error("database is locked by another process")]
    DatabaseLocked,

    /// An invalid decay phase discriminant was encountered.
    #[error("invalid decay phase: {0}")]
    InvalidPhase(u8),

    // ── I/O errors ───────────────────────────────────────────────────
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // ── Text storage errors ──────────────────────────────────────────
    /// The on-disk entry length does not match the expected length.
    #[error("stored entry length mismatch at offset {offset}: expected {expected}, found {found}")]
    TextLengthMismatch {
        /// Byte offset in fulltext.dat.
        offset: u64,
        /// Expected payload length from meta.db.
        expected: u32,
        /// Actual length stored in the entry header.
        found: u32,
    },

    /// The CRC32 checksum of the text payload does not match.
    #[error(
        "CRC32 mismatch at offset {offset}: expected {expected:#010x}, computed {computed:#010x}"
    )]
    TextCrcMismatch {
        /// Byte offset in fulltext.dat.
        offset: u64,
        /// CRC32 stored in the entry header.
        expected: u32,
        /// CRC32 computed from the payload bytes.
        computed: u32,
    },

    /// The text payload is not valid UTF-8.
    #[error("invalid UTF-8 at offset {offset}: {source}")]
    InvalidUtf8 {
        /// Byte offset in fulltext.dat.
        offset: u64,
        /// The underlying UTF-8 conversion error.
        source: std::string::FromUtf8Error,
    },

    /// The text payload exceeds the maximum allowed size.
    #[error("text payload too large: {size} bytes, max {max}")]
    TextTooLarge {
        /// Actual payload size in bytes.
        size: usize,
        /// Maximum allowed size in bytes.
        max: usize,
    },

    /// Attempted to read using the `TextRef::NONE` sentinel.
    #[error("attempted to read TextRef::NONE sentinel")]
    InvalidTextRef,

    /// Attempted to append an empty text payload.
    #[error("attempted to append empty text")]
    EmptyText,

    /// The file header could not be parsed.
    #[error("file header could not be parsed: {file}")]
    HeaderParseError {
        /// Name of the file with the unparseable header.
        file: &'static str,
    },

    /// The file has invalid magic bytes.
    #[error("invalid magic bytes in {file}: expected {expected:?}, found {found:?}")]
    InvalidMagic {
        /// Name of the file with invalid magic.
        file: &'static str,
        /// Expected magic bytes.
        expected: [u8; 4],
        /// Actual magic bytes found.
        found: [u8; 4],
    },

    /// The file format version is not supported.
    #[error("unsupported version in {file}: expected {expected}, found {found}")]
    UnsupportedVersion {
        /// Name of the file with the unsupported version.
        file: &'static str,
        /// Expected version number.
        expected: u16,
        /// Actual version number found.
        found: u16,
    },

    /// The parent directory path is missing or invalid.
    #[error("parent directory path is missing or invalid")]
    InvalidPath,

    // ── Edge storage errors ──────────────────────────────────────────
    /// An edge key has an unexpected byte length.
    #[error("corrupt edge key: expected {expected} bytes, found {found}")]
    CorruptEdgeKey {
        /// Expected key size in bytes.
        expected: usize,
        /// Actual key size in bytes.
        found: usize,
    },

    /// An edge value has an unexpected byte length.
    #[error("corrupt edge value: expected {expected} bytes, found {found}")]
    CorruptEdgeValue {
        /// Expected value size in bytes.
        expected: usize,
        /// Actual value size in bytes.
        found: usize,
    },

    /// An invalid edge type discriminant byte was encountered.
    #[error("invalid edge type discriminant: {0}")]
    InvalidEdgeType(u8),

    // ── Vector storage errors ───────────────────────────────────────
    /// An error from the vector storage subsystem.
    #[error("vector storage error: {0}")]
    Vector(#[from] crate::storage::vectors::VectorError),
}
