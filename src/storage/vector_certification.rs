//! Explicit document-recipe evidence. A profile identity alone does not prove
//! how an existing vector was produced. Old bytes retain absent evidence.
use super::control::{
    self, Generation, GenerationRef, MutationContext, MutationScope, WriteReceipt,
};
use super::native_lifecycle::invalid;
use super::native_records::{NativeInsert, NativeRecordChange, NativeRecordState, hash_bytes};
use super::{StorageError, insertion::InsertMemory};
use crate::model::schema::{EmbeddingBinding, LabelProvenance, VectorReference};
use crate::model::{DecayPhase, DiskRecord, MemoryId, NamespaceConfig, NamespaceId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

mod history;
mod offline;
mod pending;
pub(crate) use history::validate_current_with_pending;
pub(crate) use history::{
    CertificationSummary, retained_text_proofs, validate_current, validate_restore_pair,
};
pub use offline::reembed_fresh;
pub(crate) use offline::reembed_fresh_with_provider;
pub(crate) use pending::PendingBindings;
pub(crate) use pending::validate_pending_bindings;

pub const REQUIRED_FEATURE: &str = "vector-recipe-certification-v1";
pub(crate) const MAX_ROWS: u64 = 100_000;
pub(crate) const MAX_BYTES: u64 = 256 * 1024 * 1024;
pub(crate) const MAX_PROOF_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_HISTORY_ROWS: u64 = 1_000_000;
pub(crate) const MAX_HISTORY_BYTES: u64 = 512 * 1024 * 1024;

/// Immutable original cutover identity. A restore must preserve, not retarget it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VectorCertification {
    pub version: u32,
    pub origin: GenerationRef,
    pub operation_id: Uuid,
    pub inventory_root: [u8; 32],
    pub vector_count: u64,
}
impl VectorCertification {
    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        if self.version != 1
            || self.origin.store_id.is_nil()
            || self.operation_id.is_nil()
            || self.origin.generation == Generation::default()
            || self.vector_count > MAX_ROWS
        {
            return Err(invalid("invalid vector certification binding"));
        }
        Ok(())
    }
    pub(crate) fn matches(&self, receipt: &WriteReceipt) -> bool {
        self.origin.store_id == receipt.store_id
            && self.origin.generation == receipt.committed_generation
            && self.operation_id == receipt.operation_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum RetainedTextInput {
    Absent,
    Present { sha256: [u8; 32] },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentRecipe {
    DocumentV1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentSource {
    RetainedCanonicalV1,
    CallerDocumentV1,
}

/// No payload or vector bytes. Explicit absence differs from unknown old text
/// provenance in ContentProof. Component hashes preserve the stored labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentEvidence {
    pub recipe: DocumentRecipe,
    pub source: DocumentSource,
    pub source_record_generation: Generation,
    pub phase: DecayPhase,
    pub summary_sha256: [u8; 32],
    pub tags_sha256: [u8; 32],
    pub labels_sha256: [u8; 32],
    pub label_provenance: LabelProvenance,
    pub full_text: RetainedTextInput,
    pub document_sha256: [u8; 32],
}

pub(crate) struct RetainedDocument {
    pub text: String,
    pub evidence: DocumentEvidence,
}

pub(crate) fn retained_document(
    record: &DiskRecord,
    full_text: Option<&str>,
) -> Result<RetainedDocument, StorageError> {
    let schema = record
        .schema
        .as_ref()
        .ok_or_else(|| invalid("document schema missing"))?;
    if !matches!(record.phase, DecayPhase::Full | DecayPhase::Summary)
        || record.summary.is_empty()
        || (record.text_length != 0) != full_text.is_some()
        || full_text.is_some_and(|text| text.len() != record.text_length as usize)
    {
        return Err(StorageError::InvalidMutation(
            "record has no complete retained document for re-embedding".into(),
        ));
    }
    // This value is used ONLY by the shared composition function. In particular
    // labels() is not called: it would manufacture NativeV2 label provenance.
    let input = NativeInsert {
        memory: InsertMemory {
            id: Some(MemoryId::from_bytes(record.id)),
            summary: record.summary.clone(),
            full_text: full_text.map(str::to_owned),
            tags: record.tags.clone(),
            initial_stability: None,
            created_at: None,
            embedding: None,
            parent_id: None,
            supersedes: None,
        },
        observed_at: None,
        entities: schema.labels.entities.clone(),
        topics: schema.labels.topics.clone(),
        emotions: schema.labels.emotions.clone(),
        embedding_profile: None,
    };
    let text = input.document_v1();
    let evidence = DocumentEvidence {
        recipe: DocumentRecipe::DocumentV1,
        source: DocumentSource::RetainedCanonicalV1,
        source_record_generation: schema.lifecycle.record_generation,
        phase: record.phase,
        summary_sha256: hash_bytes(record.summary.as_bytes()),
        tags_sha256: hash_bytes(&control::encode(&record.tags)?),
        labels_sha256: hash_bytes(&control::encode(&schema.labels)?),
        label_provenance: schema.labels.provenance,
        full_text: full_text.map_or(RetainedTextInput::Absent, |text| {
            RetainedTextInput::Present {
                sha256: hash_bytes(text.as_bytes()),
            }
        }),
        document_sha256: hash_bytes(text.as_bytes()),
    };
    Ok(RetainedDocument { text, evidence })
}

impl DocumentEvidence {
    pub(crate) fn validate_source(&self, before: &NativeRecordState) -> Result<(), StorageError> {
        let content = &before.content;
        let text_matches = match self.full_text {
            RetainedTextInput::Absent => !before.lifecycle.has_text,
            RetainedTextInput::Present { sha256 } => {
                before.lifecycle.has_text
                    && content
                        .text_sha256
                        .is_none_or(|expected| expected == sha256)
            }
        };
        if self.source_record_generation != before.lifecycle.lifecycle.record_generation
            || self.phase != before.lifecycle.phase
            || !matches!(self.phase, DecayPhase::Full | DecayPhase::Summary)
            || !before.lifecycle.has_summary
            || !text_matches
            || self.summary_sha256 != content.summary_sha256
            || self.tags_sha256 != content.tags_sha256
            || self.labels_sha256 != content.labels_sha256
            || self.label_provenance != content.label_provenance
        {
            return Err(invalid(
                "vector document evidence differs from retained source",
            ));
        }
        Ok(())
    }
}

/// The request commits to original source CAS and the exact immutable profile;
/// provider output is execution evidence, never normalized request identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReembedVectors {
    pub context: MutationContext,
    pub source: GenerationRef,
    pub namespace_id: NamespaceId,
    pub namespace_name: String,
    pub profile_id: String,
}
impl ReembedVectors {
    pub(crate) fn hash(
        &self,
        source_origins: &[GenerationRef],
        before_root: [u8; 32],
        documents_root: [u8; 32],
    ) -> Result<[u8; 32], StorageError> {
        self.context.validate()?;
        if self.context.scope != MutationScope::Namespace(self.namespace_id)
            || self.context.store_id != Some(self.source.store_id)
            || self.context.expected_generation != Some(self.source.generation)
        {
            return Err(StorageError::InvalidMutationContext(
                "vector re-embedding requires exact original source identity and generation".into(),
            ));
        }
        Ok(control::request_hash(
            &self.context,
            "reembed_retained_vectors_v1",
            &[&control::encode(&(
                self.source,
                self.namespace_id,
                &self.namespace_name,
                &self.profile_id,
                source_origins,
                before_root,
                documents_root,
            ))?],
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VectorReplacement {
    pub record: NativeRecordChange,
    pub document: DocumentEvidence,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VectorsReembedded {
    pub version: u32,
    pub contract_version: u32,
    pub source: GenerationRef,
    /// Exact sorted source ancestry cutoffs captured before this operation.
    /// A later restore cannot expand these to bless post-cutover old proofs.
    pub source_origins: Vec<GenerationRef>,
    pub before_namespace: super::native_purge::NativeNamespaceProof,
    pub certification: VectorCertification,
    pub before_inventory_root: [u8; 32],
    pub records: Vec<VectorReplacement>,
}

pub(crate) fn inventory_root<'a>(
    namespace: &NamespaceConfig,
    rows: impl IntoIterator<Item = (MemoryId, Generation, &'a VectorReference)>,
) -> Result<[u8; 32], StorageError> {
    let mut hash = Sha256::new();
    hash.update(b"recalld-vector-certification-inventory-v1\0");
    hash.update(control::encode(&(
        namespace.id,
        namespace.embedding_dim,
        super::native_records::profile_id(namespace),
    ))?);
    let mut previous = None;
    for (id, token, vector) in rows {
        if previous.is_some_and(|previous| previous >= id) {
            return Err(invalid("vector inventory is not strictly ordered"));
        }
        previous = Some(id);
        hash.update(control::encode(&(id, token, vector))?);
    }
    Ok(hash.finalize().into())
}

impl VectorsReembedded {
    pub(crate) fn documents_root(&self) -> Result<[u8; 32], StorageError> {
        let mut hash = Sha256::new();
        hash.update(b"recalld-retained-documents-v1\0");
        for row in &self.records {
            hash.update(control::encode(&(row.record.memory_id, &row.document))?);
        }
        Ok(hash.finalize().into())
    }
    pub(crate) fn precedes_cutover(&self, receipt: GenerationRef) -> bool {
        self.source_origins.iter().any(|origin| {
            origin.store_id == receipt.store_id && receipt.generation <= origin.generation
        })
    }
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        self.certification.validate()?;
        let namespace = self.before_namespace.namespace()?;
        let schema = namespace
            .schema
            .as_ref()
            .ok_or_else(|| invalid("cutover schema missing"))?;
        if self.version != 1
            || !matches!(self.contract_version, 1 | 2)
            || !self.certification.matches(receipt)
            || self.source.store_id != receipt.store_id
            || self.source.generation.next()? != receipt.committed_generation
            || !self.source_origins.contains(&self.source)
            || self.source_origins.iter().any(|o| o.store_id.is_nil())
            || self
                .source_origins
                .windows(2)
                .any(|pair| pair[0].store_id >= pair[1].store_id)
            || self.records.len() as u64 != self.certification.vector_count
            || !matches!(schema.embedding_binding, EmbeddingBinding::Bound { .. })
            || !schema.required_features.is_empty()
            || control::encode(self)?.len() as u64 > MAX_PROOF_BYTES
        {
            return Err(invalid("invalid vector re-embedding cutover"));
        }
        for replacement in &self.records {
            let change = &replacement.record;
            let before = change
                .before
                .as_ref()
                .ok_or_else(|| invalid("cutover source absent"))?;
            replacement.document.validate_source(before)?;
            if replacement.document.source != DocumentSource::RetainedCanonicalV1 {
                return Err(invalid(
                    "cutover requires actual retained-document evidence",
                ));
            }
            before.content.validate_namespace_profile(&namespace)?;
            change
                .after
                .content
                .validate_namespace_profile(&namespace)?;
            let before_vector = before
                .content
                .vector
                .as_ref()
                .ok_or_else(|| invalid("source vector absent"))?;
            let after_vector = change
                .after
                .content
                .vector
                .as_ref()
                .ok_or_else(|| invalid("output vector absent"))?;
            let mut expected = before.clone();
            expected.content.vector = Some(after_vector.clone());
            expected.lifecycle.lifecycle.record_generation =
                before.lifecycle.lifecycle.record_generation.next()?;
            if expected != change.after
                || before_vector.slot == after_vector.slot
                || after_vector.allocation_epoch != receipt.committed_generation
            {
                return Err(invalid(
                    "vector replacement changed unrelated canonical state",
                ));
            }
        }
        let before = inventory_root(
            &namespace,
            self.records.iter().map(|r| {
                let state = r.record.before.as_ref().expect("validated source");
                (
                    r.record.memory_id,
                    state.lifecycle.lifecycle.record_generation,
                    state
                        .content
                        .vector
                        .as_ref()
                        .expect("validated source vector"),
                )
            }),
        )?;
        let after = inventory_root(
            &namespace,
            self.records.iter().map(|r| {
                (
                    r.record.memory_id,
                    r.record.after.lifecycle.lifecycle.record_generation,
                    r.record
                        .after
                        .content
                        .vector
                        .as_ref()
                        .expect("validated output vector"),
                )
            }),
        )?;
        if before != self.before_inventory_root || after != self.certification.inventory_root {
            return Err(invalid("vector cutover inventory commitment mismatch"));
        }
        Ok(())
    }
}

/// Mandatory for a fresh vector in a certified namespace. Absence on genuine
/// older writes is permitted only inside the cutover's original ancestry bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VectorExecution {
    pub certification: VectorCertification,
    pub source: VectorSource,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum VectorSource {
    Generated { document: DocumentEvidence },
    Supplied { profile_id: String },
}
impl VectorExecution {
    pub(crate) fn for_insert(
        namespace: &NamespaceConfig,
        record: &DiskRecord,
        input: &NativeInsert,
        generated: &crate::embedding::runtime::GeneratedEmbedding,
    ) -> Result<Option<Self>, StorageError> {
        let Some(certification) = namespace
            .schema
            .as_ref()
            .and_then(|s| s.vector_certification.clone())
        else {
            return Ok(None);
        };
        let source = if input.memory.embedding.is_some() {
            let profile = input
                .embedding_profile
                .as_ref()
                .ok_or_else(|| invalid("supplied profile absent"))?;
            VectorSource::Supplied {
                profile_id: profile
                    .profile_id()
                    .map_err(StorageError::InvalidMutation)?,
            }
        } else {
            generated
                .verify_document(&input.document_v1())
                .map_err(|e| StorageError::ProviderVectorUnavailable(e.to_string()))?;
            let mut document =
                retained_document(record, input.memory.full_text.as_deref())?.evidence;
            document.source = DocumentSource::CallerDocumentV1;
            VectorSource::Generated { document }
        };
        Ok(Some(Self {
            certification,
            source,
        }))
    }
    pub(crate) fn validate(
        &self,
        primary: &NativeRecordState,
        receipt: &WriteReceipt,
    ) -> Result<(), StorageError> {
        self.certification.validate()?;
        if primary.content.vector.is_none()
            || (self.certification.origin.store_id == receipt.store_id
                && self.certification.origin.generation >= receipt.committed_generation)
        {
            return Err(invalid("vector execution does not follow certification"));
        }
        match &self.source {
            VectorSource::Generated { document } => {
                if document.source != DocumentSource::CallerDocumentV1 {
                    return Err(invalid(
                        "fresh vector evidence is not caller-document execution",
                    ));
                }
                document.validate_source(primary)?;
            }
            VectorSource::Supplied { profile_id } => {
                if primary.content.profile_id.as_ref() != Some(profile_id) {
                    return Err(invalid(
                        "supplied vector assertion differs from canonical profile",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
