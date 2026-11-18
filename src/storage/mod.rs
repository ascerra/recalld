//! Storage layer for Recalld.
//!
//! Provides persistent storage for memory records, embeddings, text,
//! and graph edges. The public API is defined by the `StorageEngine`
//! trait; the concrete implementation is `RedbStorageEngine`.
//!
//! # Submodules
//!
//! - `vectors` — Memory-mapped vector storage (per-namespace .dat files)
//! - `metadata` — redb-backed metadata B-tree with secondary indexes
//! - `indexes` — Phase bitmap indexes (roaring)
//! - `text` — Append-only text log with CRC32 integrity
//! - `edges` — redb-backed graph edge persistence
//! - `error` — Unified `StorageError` type
//! - `fsync` — Filesystem sync helpers for crash safety

pub(crate) mod automatic_upgrade;
pub mod batch;
pub mod canonical_schema;
pub(crate) mod canonical_vector_snapshot;
pub mod catalog;
pub mod container_layout;
pub mod control;
pub mod edges;
pub mod engine;
pub mod error;
pub mod fsync;
pub(crate) mod ids;
pub mod indexes;
pub mod inherited_policy;
#[cfg(test)]
mod inherited_policy_tests;
pub mod insertion;
pub mod lifecycle_migration;
pub mod legacy_vector_repair;
pub(crate) mod lineage;
pub mod logical_export;
pub mod maintenance;
#[cfg(test)]
mod maintenance_tests;
pub mod metadata;
pub mod native_catalog;
pub mod native_graph;
pub mod native_lifecycle;
pub mod native_maintenance;
pub mod native_policy;
pub mod native_purge;
#[cfg(test)]
mod native_purge_tests;
pub mod native_records;
pub mod native_store;
pub mod native_text;
pub mod offline_snapshot;
pub(crate) mod ownership;
pub(crate) mod publication;
pub use publication::{DerivedReadiness, ObservedDerived, Readiness, ReadinessObservation};
pub mod purge;
#[cfg(test)]
mod purge_tests;
pub(crate) mod restore_journal;
pub mod schema_migration;
pub mod text;
pub mod text_migration;
pub mod tombstone;
#[cfg(test)]
mod tombstone_tests;
pub(crate) mod validation;
pub mod vector_certification;
pub mod vectors;
pub mod work;

// ── Re-exports ──────────────────────────────────────────────────────

pub use edges::{EdgeStore, PersistedEdge, cleanup_orphaned_edges};
pub use engine::{RedbStorageEngine, StorageEngine};
pub use error::StorageError;
pub use indexes::PhaseIndex;
pub use metadata::MetadataStore;
pub use text::{CompactionResult, TextRef, TextStore, recover_text_compaction};
pub use vectors::{VectorManager, VectorStore};

pub mod request_context;
