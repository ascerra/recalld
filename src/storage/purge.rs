//! Durable namespace deletion identity and payload-free progress.
use super::control::WriteReceipt;
use crate::model::{MemoryId, NamespaceId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Invocation-local physical observation, never persisted or a replay authority.
pub(crate) struct PurgeCleanupObservation {
    pub store_id: Uuid,
    pub namespace_id: NamespaceId,
    pub start_operation_id: Uuid,
    pub directory_removed: Option<bool>,
}
impl PurgeCleanupObservation {
    pub(crate) fn for_progress(progress: &PurgeProgress) -> Self {
        Self {
            store_id: progress.store_id,
            namespace_id: progress.namespace_id,
            start_operation_id: progress.start_receipt.operation_id,
            directory_removed: None,
        }
    }
    pub(crate) fn matches(&self, progress: &PurgeProgress) -> bool {
        self.store_id == progress.store_id
            && self.namespace_id == progress.namespace_id
            && self.start_operation_id == progress.start_receipt.operation_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PurgeStage {
    Deleting,
    /// Configuration is gone; the name remains leased until directory cleanup.
    Cleanup,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PurgeProgress {
    pub version: u32,
    pub store_id: Uuid,
    pub namespace_id: NamespaceId,
    pub name: String,
    pub start_receipt: WriteReceipt,
    pub last_receipt: WriteReceipt,
    pub sequence: u64,
    pub stage: PurgeStage,
    pub next_operation_id: Option<Uuid>,
    pub cursor: Option<MemoryId>,
    pub removed: u64,
    pub tombstones_removed: u64,
    pub edges_removed: u64,
}
