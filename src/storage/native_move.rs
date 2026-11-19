//! Atomic namespace relocation. Caller identity is independent of live catalog
//! resolution so a completed request remains replayable after later moves.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::StorageError;
use super::control::{self, MutationContext, MutationScope};
use super::control::{GenerationRef, WriteReceipt};
use super::native_lifecycle::invalid;
use super::native_purge::NativeNamespaceProof;
use super::native_records::NativeRecordChange;
use super::vector_certification::{DocumentEvidence, DocumentSource};
use crate::model::MemoryId;
use crate::model::{NamespaceId, schema::EmbeddingBinding};

pub(crate) const MAX_MOVE_PROOF_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_MOVE_TEXT_BYTES: usize = 16 * 1024 * 1024;

pub(crate) struct CapturedNativeMove {
    pub input: NativeMoveInput,
    pub at: GenerationRef,
    pub source: crate::model::NamespaceConfig,
    pub destination: crate::model::NamespaceConfig,
    pub records: Vec<crate::model::DiskRecord>,
    pub documents: Vec<super::vector_certification::RetainedDocument>,
}

#[derive(Default)]
pub(crate) enum MoveProgress {
    #[default]
    Preparing,
    Staging(uuid::Uuid),
    Committed(WriteReceipt),
}

pub(crate) type SharedMoveProgress = std::sync::Arc<std::sync::Mutex<MoveProgress>>;

/// Configuration and numeric/hash witnesses only; never retain text or vectors
/// in receipts. Original creation receipts remain immutable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeMoveWrite {
    pub ids: Vec<MemoryId>,
    pub source_namespace: String,
    pub destination_namespace: String,
    pub source_namespace_id: NamespaceId,
    pub destination_namespace_id: NamespaceId,
    pub destination_profile_id: String,
    pub capture: GenerationRef,
    pub source: NativeNamespaceProof,
    pub destination: NativeNamespaceProof,
    pub records: Vec<NativeMovedRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeMovedRecord {
    pub change: NativeRecordChange,
    pub document: DocumentEvidence,
}

impl NativeMoveWrite {
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        let source = self.source.namespace()?;
        let destination = self.destination.namespace()?;
        let EmbeddingBinding::Bound { profile_id, .. } = &self.destination.schema.embedding_binding
        else {
            return Err(invalid("move destination lacks explicit embedding binding"));
        };
        let certification = self
            .destination
            .schema
            .vector_certification
            .as_ref()
            .ok_or_else(|| invalid("move destination lacks vector certification"))?;
        certification.validate()?;
        if self.source_namespace_id != source.id
            || self.destination_namespace_id != destination.id
            || source.id == destination.id
            || self.source_namespace != source.name
            || self.destination_namespace != destination.name
            || &self.destination_profile_id != profile_id
            || self.capture.store_id != receipt.store_id
            || self.capture.generation.next()? != receipt.committed_generation
            || self.source.schema.lifecycle_version != self.destination.schema.lifecycle_version
            || self.source.schema.policy_revision != self.destination.schema.policy_revision
            || !(1..=100).contains(&self.ids.len())
            || self.records.len() != self.ids.len()
            || self.ids.iter().collect::<BTreeSet<_>>().len() != self.ids.len()
            || control::encode(self)?.len() > MAX_MOVE_PROOF_BYTES
            || (certification.origin.store_id == receipt.store_id
                && certification.origin.generation >= receipt.committed_generation)
        {
            return Err(invalid("invalid native move identity/binding"));
        }
        for (id, row) in self.ids.iter().zip(&self.records) {
            let change = &row.change;
            let before = change
                .before
                .as_ref()
                .ok_or_else(|| invalid("move source absent"))?;
            before.lifecycle.validate()?;
            row.document.validate_source(before)?;
            before.content.validate_namespace_profile(&source)?;
            change
                .after
                .content
                .validate_namespace_profile(&destination)?;
            let vector = change
                .after
                .content
                .vector
                .as_ref()
                .ok_or_else(|| invalid("move destination vector absent"))?;
            let mut expected = before.clone();
            expected.content.vector = Some(vector.clone());
            expected.content.profile_id = Some(profile_id.clone());
            expected.content.embedding_origin = None;
            expected.lifecycle.lifecycle.record_generation =
                before.lifecycle.lifecycle.record_generation.next()?;
            if *id == MemoryId::nil()
                || *id != change.memory_id
                || before.content.vector.is_none()
                || row.document.source != DocumentSource::RetainedCanonicalV1
                || before.lifecycle.lifecycle.lifecycle_version
                    != self.source.schema.lifecycle_version
                || before.lifecycle.lifecycle.policy_revision != self.source.schema.policy_revision
                || vector.allocation_epoch != receipt.committed_generation
                || expected != change.after
            {
                return Err(invalid(
                    "move changed unrelated record state or lacks execution proof",
                ));
            }
        }
        Ok(())
    }
}

/// Only regeneration is supported. Width alone never establishes provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeMoveEmbeddingMode {
    Regenerate,
}

/// One all-or-nothing operation, not a reservation of independent child writes.
/// Preserve this entire ordered input when reconciling an uncertain outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeMoveInput {
    pub store_id: uuid::Uuid,
    pub idempotency_key: String,
    pub source_namespace: String,
    pub destination_namespace: String,
    pub ids: Vec<MemoryId>,
    pub embedding_mode: NativeMoveEmbeddingMode,
}

impl NativeMoveInput {
    pub(crate) fn context(&self) -> MutationContext {
        MutationContext {
            store_id: Some(self.store_id),
            scope: MutationScope::Catalog,
            idempotency_key: Some(self.idempotency_key.clone()),
            expected_generation: None,
            contract_version: 1,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        self.context().validate()?;
        for name in [&self.source_namespace, &self.destination_namespace] {
            crate::model::validation::validate_namespace_name(name)
                .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
        }
        if self.store_id.is_nil()
            || self
                .source_namespace
                .eq_ignore_ascii_case(&self.destination_namespace)
            || !(1..=100).contains(&self.ids.len())
            || self.ids.iter().any(|id| *id == MemoryId::nil())
            || self.ids.iter().collect::<BTreeSet<_>>().len() != self.ids.len()
        {
            return Err(StorageError::InvalidMutation(
                "move requires a store ID, distinct namespaces and 1–100 distinct non-nil memory IDs".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn request_hash(&self) -> Result<[u8; 32], StorageError> {
        self.validate()?;
        Ok(control::request_hash(
            &self.context(),
            "native_memories_move_v1",
            &[&control::encode(self)?],
        ))
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;

    fn input() -> NativeMoveInput {
        NativeMoveInput {
            store_id: uuid::Uuid::from_u128(1),
            idempotency_key: "move-1".into(),
            source_namespace: "default".into(),
            destination_namespace: "project-mtx".into(),
            ids: vec![MemoryId::from_uuid(uuid::Uuid::from_u128(2))],
            embedding_mode: NativeMoveEmbeddingMode::Regenerate,
        }
    }

    #[test]
    fn rejects_unbounded_ambiguous_or_unkeyed_requests() {
        for change in [
            |v: &mut NativeMoveInput| v.ids.clear(),
            |v: &mut NativeMoveInput| v.ids.push(v.ids[0]),
            |v: &mut NativeMoveInput| v.ids[0] = MemoryId::nil(),
            |v: &mut NativeMoveInput| v.store_id = uuid::Uuid::nil(),
            |v: &mut NativeMoveInput| v.idempotency_key.clear(),
            |v: &mut NativeMoveInput| v.destination_namespace = "DEFAULT".into(),
            |v: &mut NativeMoveInput| {
                v.ids = (1..=101)
                    .map(|id| MemoryId::from_uuid(uuid::Uuid::from_u128(id)))
                    .collect()
            },
        ] {
            let mut v = input();
            change(&mut v);
            assert!(v.validate().is_err());
        }
    }

    #[test]
    fn retry_identity_binds_order_and_explicit_names() {
        let mut original = input();
        original
            .ids
            .push(MemoryId::from_uuid(uuid::Uuid::from_u128(3)));
        let hash = original.request_hash().unwrap();
        assert_eq!(hash, original.clone().request_hash().unwrap());
        let mut changed = original.clone();
        changed.ids.reverse();
        assert_ne!(hash, changed.request_hash().unwrap());
        changed = original;
        changed.destination_namespace = "project-other".into();
        assert_ne!(hash, changed.request_hash().unwrap());
    }
}

#[cfg(test)]
#[path = "native_move_tests.rs"]
pub(crate) mod tests;
