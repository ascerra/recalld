//! Same-insertion native linking. This is an internal Rust intent, not a serving
//! activation or an extension of the historical two-edge insertion proof.
use serde::{Deserialize, Serialize};

use super::control::GenerationRef;
use super::native_records::{NativeInsert, NativeInsertIntent};
use crate::model::{EdgeType, MemoryId};

#[derive(Debug, Clone)]
pub struct NativeStoreInput {
    pub insert: NativeInsert,
    /// Caller/default expansion is part of the new operation's identity.
    pub check_duplicates: bool,
}

/// Only settings consumed by link selection. Pin before provider work; matching
/// retries use the original receipt's expansion, not mutable current defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeStoreSettings {
    pub autolink_enabled: bool,
    /// Original f64 bits, before the producer's f32 comparison conversion.
    pub auto_link_threshold_bits: u64,
    pub max_auto_links: u32,
    pub max_entity_links: u32,
    pub temporal_window_ms: u64,
    pub max_temporal_links: u32,
}

#[derive(Debug, Clone)]
pub struct NativeStoreIntent {
    pub(crate) insert: NativeInsertIntent,
    pub(crate) check_duplicates: bool,
    pub(crate) settings: NativeStoreSettings,
}
impl NativeStoreIntent {
    pub fn insert_intent(&self) -> &NativeInsertIntent {
        &self.insert
    }
    pub fn settings(&self) -> &NativeStoreSettings {
        &self.settings
    }
}

/// Producer-computed, ephemeral canonical selection. Core checks the capture G,
/// namespace/profile, live peers and temporal creation times before allocation.
/// Completed receipts contain final IDs/numeric outcomes, never source payloads.
#[derive(Debug, Clone)]
pub struct FrozenInsertLinks {
    pub generation: GenerationRef,
    pub profile_id: String,
    /// New record -> existing target; explicit parent/supersedes stay in input.
    pub static_links: Vec<(MemoryId, EdgeType, f32)>,
    /// All eligible temporal candidates, before one core-clock selection.
    pub temporal: Vec<(MemoryId, i64)>,
    /// None means disabled; Some(empty) means requested and checked.
    pub advisory: Option<Vec<(MemoryId, f32)>>,
}

#[derive(Debug, Clone)]
pub struct PreparedNativeStore {
    pub(crate) intent: NativeStoreIntent,
    pub(crate) links: FrozenInsertLinks,
}
