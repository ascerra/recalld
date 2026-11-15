//! Owned payload capture while the actual engine read guard pins file handles.
use super::*;
use crate::storage::control::GenerationRef;

#[derive(Debug, Clone)]
pub struct HydratedRecord {
    pub record: DiskRecord,
    pub full_text: Option<String>,
    pub embedding: Option<Vec<f32>>,
}

impl RedbStorageEngine {
    /// No awaits or escaped references. Generation mismatch rejects the entire
    /// page, including an empty page. Namespace/labels come from canonical bytes.
    pub(crate) fn hydrate_at(
        &self,
        at: GenerationRef,
        namespace: NamespaceId,
        ids: &[MemoryId],
        include_embeddings: bool,
    ) -> Result<Vec<HydratedRecord>, StorageError> {
        self.ensure_available()?;
        self.meta_store.with_generation_read_at(at, |view| {
            let mut result = Vec::with_capacity(ids.len());
            for &id in ids {
                let record = view
                    .disk_record(id)?
                    .ok_or(StorageError::NotFound(id.into_inner()))?;
                if record.namespace_id != namespace.get() {
                    return Err(StorageError::MutationNamespaceMismatch);
                }
                if matches!(record.phase, DecayPhase::Ghost | DecayPhase::Tombstone) {
                    return Err(StorageError::LifecycleContentUnavailable);
                }
                let full_text = if record.text_length == 0 {
                    None
                } else {
                    Some(self.text_store.read(TextRef {
                        file_offset: record.text_offset,
                        length: record.text_length,
                    })?)
                };
                let embedding = if include_embeddings {
                    let vectors = self
                        .vector_manager
                        .get(namespace)
                        .ok_or(StorageError::NamespaceNotFound(namespace.get()))?;
                    if vectors.is_free(record.vector_slot) {
                        return Err(StorageError::OwnershipMismatch(
                            "selected vector is free".into(),
                        ));
                    }
                    let vector = vectors.get_vector(record.vector_slot).ok_or_else(|| {
                        StorageError::OwnershipMismatch("selected vector is missing".into())
                    })?;
                    if let Some(schema) = &record.schema {
                        let reference = schema.vector.as_ref().ok_or_else(|| {
                            StorageError::OwnershipMismatch(
                                "canonical vector reference missing".into(),
                            )
                        })?;
                        let bytes: Vec<u8> = vector.iter().flat_map(|v| v.to_le_bytes()).collect();
                        if reference.slot != record.vector_slot
                            || reference.sha256
                                != crate::storage::native_records::hash_bytes(&bytes)
                        {
                            return Err(StorageError::OwnershipMismatch(
                                "canonical vector reference mismatch".into(),
                            ));
                        }
                    }
                    Some(vector.to_vec())
                } else {
                    None
                };
                result.push(HydratedRecord {
                    record,
                    full_text,
                    embedding,
                });
            }
            Ok(result)
        })
    }
}
