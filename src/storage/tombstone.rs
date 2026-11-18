//! Payload-free forget outcomes. Canonical commit and derived readiness are
//! separate; the transport must await the shared completion worker.
use serde::{Deserialize, Serialize};

use super::control::{self, MutationContext};
use crate::model::MemoryId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TombstoneStatus {
    Tombstoned,
    AlreadyTombstoned,
}

pub(crate) fn request_hash(ctx: &MutationContext, id: MemoryId) -> [u8; 32] {
    scoped_request_hash(ctx, id, None)
}

pub(crate) fn scoped_request_hash(
    ctx: &MutationContext,
    id: MemoryId,
    namespace: Option<&str>,
) -> [u8; 32] {
    // No generated timestamp, mutable policy, or removed content participates.
    // Preserve historical unasserted requests byte-for-byte in either API domain.
    match namespace {
        None => control::request_hash(ctx, "memory_tombstone_v1", &[id.as_bytes()]),
        Some(name) => control::request_hash(
            ctx,
            "memory_tombstone_v1",
            &[id.as_bytes(), name.as_bytes()],
        ),
    }
}
