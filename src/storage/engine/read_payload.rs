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
    /// Direct fetch may return a legacy marker (the caller preserves its projection).
    /// No marker text is copied. Keep this separate
    /// from search hydration, whose entire selected page must remain eligible.
    /// The caller retains engine read ownership through this synchronous copy.
    pub(crate) fn hydrate_get_at(
        &self,
        at: GenerationRef,
        id: MemoryId,
        asserted_namespace: Option<&str>,
    ) -> Result<Option<(NamespaceConfig, HydratedRecord)>, StorageError> {
        self.ensure_available()?;
        self.meta_store.with_generation_read_at(at, |view| {
            // The metadata generation guard supplies the canonical view. Point
            // reads also require completed canonical effects, not unrelated index
            // stamps; ranking/projection callers retain their own pinned views.
            self.meta_store.effects_stamp()?;

            if let Some(namespace) = asserted_namespace {
                validate_namespace_name(namespace).map_err(|e| {
                    StorageError::InvalidNamespaceName {
                        name: namespace.to_owned(),
                        reason: e.to_string(),
                    }
                })?;
            }
            let Some(record) = view.disk_record(id)? else {
                return if asserted_namespace.is_some() {
                    Err(StorageError::NotFound(id.into_inner()))
                } else {
                    Ok(None)
                };
            };
            let namespace = view
                .namespaces()?
                .into_iter()
                .find(|namespace| namespace.id.get() == record.namespace_id)
                .ok_or(StorageError::NamespaceNotFound(record.namespace_id))?;
            if let Some(asserted) = asserted_namespace {
                if !namespace.name.eq_ignore_ascii_case(asserted) {
                    return Err(StorageError::NamespaceMismatch {
                        id: id.into_inner(),
                        namespace: asserted.to_owned(),
                    });
                }
            }
            let full_text = if matches!(record.phase, DecayPhase::Full | DecayPhase::Summary)
                && record.text_length != 0
            {
                Some(self.text_store.read(TextRef {
                    file_offset: record.text_offset,
                    length: record.text_length,
                })?)
            } else {
                None
            };
            Ok(Some((
                namespace,
                HydratedRecord {
                    record,
                    full_text,
                    embedding: None,
                },
            )))
        })
    }

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
