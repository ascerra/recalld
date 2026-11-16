//! Fresh-destination re-embedding. The source owner spans inspection and all
//! provider work; only a private copy is writable. No serving admission here.
use super::*;
use crate::embedding::{EmbeddingProvider, runtime::generate_native};
use crate::storage::metadata::{META_TABLE, NAMESPACE_TABLE};
use crate::storage::{offline_snapshot::OfflineSnapshot, ownership::StoreOwnership};
use redb::ReadableTable;
use std::path::{Path, PathBuf};

const MAX_CONTROL_BYTES: u64 = 512 * 1024 * 1024;

fn snapshot_owned(owner: StoreOwnership) -> Result<OfflineSnapshot, StorageError> {
    let mut bytes = 0_u64;
    for name in ["meta.db", "edges.db"] {
        let path = owner.path.join(name);
        crate::storage::validation::require_file(&path)?;
        bytes = bytes
            .checked_add(path.metadata()?.len())
            .ok_or_else(|| invalid("inspection size overflow"))?;
        if bytes > MAX_CONTROL_BYTES {
            return Err(invalid("certification control inspection exceeds 512 MiB"));
        }
    }
    OfflineSnapshot::open_vector_remedy(owner)
}

fn replay(
    snapshot: &OfflineSnapshot,
    request: &ReembedVectors,
) -> Result<WriteReceipt, StorageError> {
    // This write transaction belongs to the disposable inspection copy. It is
    // never committed and never opens the original canonical database writable.
    let tx = snapshot.metadata.begin_write()?;
    let stored = control::keyed_receipt(&tx, &request.context)?.ok_or_else(|| {
        invalid("existing destination/stage has no matching cutover; inspect it before retrying")
    })?;
    let control::MutationOutcome::VectorsReembedded { transition } = &stored.receipt.outcome else {
        return Err(StorageError::IdempotencyConflict);
    };
    let hash = request.hash(
        &transition.source_origins,
        transition.before_inventory_root,
        transition.documents_root()?,
    )?;
    control::replay_or_check(&tx, &request.context, hash)?
        .ok_or_else(|| invalid("cutover receipt disappeared"))
}

fn publish(stage: &Path, target: &Path, receipt: &WriteReceipt) -> Result<(), StorageError> {
    let result = (|| {
        crate::storage::catalog::boundary("certification_after_commit")?;
        for file in crate::storage::validation::vector_remedy_files(stage)? {
            std::fs::File::open(file)?.sync_all()?;
        }
        crate::storage::fsync::fsync_dir(stage)?;
        if target.try_exists()? {
            return Err(invalid("fresh destination appeared during cutover"));
        }
        std::fs::rename(stage, target)?;
        crate::storage::catalog::boundary("certification_published")?;
        crate::storage::fsync::fsync_dir(target.parent().ok_or(StorageError::InvalidPath)?)?;
        Ok(())
    })();
    result.map_err(|source| StorageError::CommittedMutation {
        receipt: Box::new(receipt.clone()),
        source: Box::new(source),
    })
}

struct UncommittedStage(Option<PathBuf>);
impl Drop for UncommittedStage {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

/// Re-embed actual retained Full/Summary documents into a fresh destination.
/// Existing matching destinations/stages replay before provider use or source
/// CAS. A private ambiguous stage is never overwritten or automatically erased.
pub async fn reembed_fresh(
    source: &Path,
    destination: &Path,
    request: &ReembedVectors,
    provider: &dyn EmbeddingProvider,
) -> Result<WriteReceipt, StorageError> {
    reembed_fresh_with_provider(source, destination, request, async { Ok(provider) }).await
}

/// The operator command defers configuration/credentials/provider construction
/// until after exact replay, while retaining the same source/destination owners.
pub(crate) async fn reembed_fresh_with_provider<'a, P, F>(
    source: &Path,
    destination: &Path,
    request: &ReembedVectors,
    provider: F,
) -> Result<WriteReceipt, StorageError>
where
    P: std::ops::Deref<Target = dyn EmbeddingProvider + 'a>,
    F: std::future::Future<Output = Result<P, StorageError>>,
{
    request.context.validate()?;
    if request.context.idempotency_key.is_none() {
        return Err(StorageError::InvalidMutationContext(
            "offline certification requires a stable idempotency key".into(),
        ));
    }
    // Validate original context even on an existing destination.
    request.hash(&[], [0; 32], [0; 32])?;
    let target_owner = StoreOwnership::acquire(destination)?;
    let target = &target_owner.path;
    let mut stage_name = target
        .file_name()
        .ok_or(StorageError::InvalidPath)?
        .to_os_string();
    stage_name.push(".recalld-vector-certification");
    let stage = target.with_file_name(stage_name);
    if target.try_exists()? {
        return replay(&snapshot_owned(target_owner)?, request);
    }
    let stage_owner = StoreOwnership::acquire(&stage)?;
    if stage.try_exists()? {
        let snapshot = snapshot_owned(stage_owner)?;
        let receipt = replay(&snapshot, request)?;
        let _stage_owner = snapshot.into_owner();
        publish(&stage, target, &receipt)?;
        return Ok(receipt);
    }
    let source = source.canonicalize()?;
    if target.starts_with(&source) || source.starts_with(target) {
        return Err(StorageError::InvalidPath);
    }
    let snapshot = snapshot_owned(StoreOwnership::acquire(&source)?)?;
    let state = snapshot.control();
    if state.generation_ref() != request.source {
        return Err(if state.store_id != request.source.store_id {
            StorageError::StoreLineageMismatch
        } else {
            StorageError::GenerationConflict {
                expected: request.source.generation.into(),
                actual: state.committed_generation.into(),
            }
        });
    }
    let namespace = snapshot
        .namespaces()
        .iter()
        .find(|n| n.id == request.namespace_id)
        .ok_or(StorageError::NamespaceNotFound(request.namespace_id.get()))?
        .clone();
    if namespace.name != request.namespace_name
        || super::super::native_records::profile_id(&namespace).as_ref()
            != Some(&request.profile_id)
    {
        return Err(StorageError::InvalidMutation(
            "re-embedding namespace/profile does not match original request".into(),
        ));
    }
    let schema = namespace
        .schema
        .as_ref()
        .ok_or_else(|| invalid("re-embedding requires native schema"))?;
    if schema.vector_certification.is_some() || !schema.required_features.is_empty() {
        return Err(StorageError::InvalidMutation(
            "namespace is already certified or has unsupported required features".into(),
        ));
    }
    let provider = provider.await?;
    let provider = &*provider;
    let active = provider
        .profile()
        .ok_or_else(|| invalid("re-embedding provider profile missing"))?;
    crate::embedding::profile::require_compatible(&namespace, &active)
        .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
    let mut origins = {
        let read = snapshot.metadata.begin_read()?;
        crate::storage::lineage::ensure_serving(&read, state)?;
        let mut origins = crate::storage::lineage::read_lineage(&read, state)?
            .map(|l| l.ancestors)
            .unwrap_or_default();
        origins.retain(|o| o.store_id != state.store_id);
        origins.push(state.generation_ref());
        origins
    };
    origins.sort_by_key(|o| o.store_id);
    if origins
        .windows(2)
        .any(|pair| pair[0].store_id == pair[1].store_id)
    {
        return Err(invalid("duplicate source ancestry"));
    }
    let mut retained = Vec::new();
    {
        let read = snapshot.metadata.begin_read()?;
        let mut count = 0_u64;
        let mut bytes = 0_u64;
        for row in read.open_table(META_TABLE)?.iter()? {
            let (id, value) = row?;
            count += 1;
            bytes = bytes
                .checked_add(value.value().len() as u64)
                .ok_or_else(|| invalid("inventory size overflow"))?;
            if count > MAX_ROWS || bytes > MAX_BYTES {
                return Err(invalid("certification inventory limit exceeded"));
            }
            let record = DiskRecord::from_bytes(value.value())?;
            if record.id.as_slice() != id.value() {
                return Err(invalid("inventory key mismatch"));
            }
            if record.namespace_id != namespace.id.get() {
                continue;
            }
            let Some(reference) = record.schema.as_ref().and_then(|s| s.vector.as_ref()) else {
                continue;
            };
            // Charge vector and text bytes BEFORE OfflineSnapshot materializes them.
            bytes = bytes
                .checked_add(u64::from(namespace.embedding_dim) * 4)
                .and_then(|n| n.checked_add(u64::from(record.text_length)))
                .ok_or_else(|| invalid("inventory payload size overflow"))?;
            if bytes > MAX_BYTES {
                return Err(invalid("certification inventory payload limit exceeded"));
            }
            let raw = snapshot
                .record(MemoryId::from_bytes(record.id))?
                .ok_or_else(|| invalid("retained record disappeared"))?;
            let vector = raw
                .embedding
                .as_ref()
                .ok_or_else(|| invalid("referenced vector missing"))?;
            let mut digest = Sha256::new();
            for value in vector {
                digest.update(value.to_le_bytes());
            }
            if reference.sha256 != <[u8; 32]>::from(digest.finalize()) {
                return Err(invalid("retained vector hash mismatch"));
            }
            let document = retained_document(&raw.record, raw.full_text.as_deref())?;
            let before = NativeRecordState::capture(
                &raw.record,
                &namespace,
                raw.full_text.as_deref().map(|t| hash_bytes(t.as_bytes())),
            )?;
            retained.push((raw.record, before, document));
        }
    }
    let before_root = inventory_root(
        &namespace,
        retained.iter().map(|(record, proof, _)| {
            (
                MemoryId::from_bytes(record.id),
                proof.lifecycle.lifecycle.record_generation,
                proof.content.vector.as_ref().expect("selected reference"),
            )
        }),
    )?;
    let mut documents = Sha256::new();
    documents.update(b"recalld-retained-documents-v1\0");
    for (record, _, document) in &retained {
        documents.update(control::encode(&(
            MemoryId::from_bytes(record.id),
            &document.evidence,
        ))?);
    }
    let hash = request.hash(&origins, before_root, documents.finalize().into())?;
    // Stable key conflicts in the source are resolved before provider or staging.
    if control::replay_or_check(&snapshot.metadata.begin_write()?, &request.context, hash)?
        .is_some()
    {
        return Err(invalid(
            "source already contains this cutover; use its original destination",
        ));
    }
    let next = state.committed_generation.next()?;
    std::fs::create_dir(&stage)?;
    let mut cleanup = UncommittedStage(Some(stage.clone()));
    // Already validated before this snapshot acquired the payload handles.
    // Reopening them for a second scan would contend with our own file locks.
    let files = &snapshot.canonical_files;
    let mut copy_bytes = 0_u64;
    for file in files {
        copy_bytes = copy_bytes
            .checked_add(file.metadata()?.len())
            .ok_or_else(|| invalid("staging copy size overflow"))?;
        if copy_bytes > MAX_CONTROL_BYTES {
            return Err(invalid("certification canonical copy exceeds 512 MiB"));
        }
    }
    for file in files {
        let copied = stage.join(
            file.strip_prefix(&source)
                .map_err(|_| StorageError::InvalidPath)?,
        );
        let parent = copied.parent().ok_or(StorageError::InvalidPath)?;
        std::fs::create_dir_all(parent)?;
        std::fs::copy(file, &copied)?;
        std::fs::File::open(&copied)?.sync_all()?;
        crate::storage::fsync::fsync_dir(parent)?;
    }
    let mut vectors = crate::storage::VectorStore::open(
        &stage.join(&namespace.name),
        namespace.embedding_dim as usize,
    )?;
    let mut replacements = Vec::new();
    let mut updated = Vec::new();
    for (mut record, before, document) in retained {
        let generated = generate_native(provider, &namespace, &document.text)
            .await
            .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
        generated
            .verify_document(&document.text)
            .map_err(|e| StorageError::InvalidMutation(e.to_string()))?;
        let mut values = generated.values;
        crate::storage::insertion::normalize_embedding(
            &mut values,
            namespace.embedding_dim,
            false,
        )?;
        let slot = vectors.insert_vector(&values)?;
        let mut digest = Sha256::new();
        for value in &values {
            digest.update(value.to_le_bytes());
        }
        record.vector_slot = slot;
        let schema = record
            .schema
            .as_mut()
            .ok_or_else(|| invalid("retained schema missing"))?;
        schema.vector = Some(VectorReference {
            slot,
            allocation_epoch: next,
            sha256: digest.finalize().into(),
        });
        schema.lifecycle.record_generation = schema.lifecycle.record_generation.next()?;
        let mut after = before.clone();
        after.content.vector = schema.vector.clone();
        after.lifecycle.lifecycle.record_generation = schema.lifecycle.record_generation;
        replacements.push(VectorReplacement {
            record: NativeRecordChange {
                memory_id: MemoryId::from_bytes(record.id),
                before: Some(before),
                after,
            },
            document: document.evidence,
        });
        updated.push(record);
    }
    vectors.sync()?;
    let operation_id = crate::storage::ids::IdSource::uuid(
        &crate::storage::ids::SystemIds,
        crate::storage::ids::IdKind::Operation,
    )?;
    let certification = VectorCertification {
        version: 1,
        origin: GenerationRef {
            store_id: state.store_id,
            generation: next,
        },
        operation_id,
        vector_count: replacements.len() as u64,
        inventory_root: inventory_root(
            &namespace,
            replacements.iter().map(|r| {
                (
                    r.record.memory_id,
                    r.record.after.lifecycle.lifecycle.record_generation,
                    r.record
                        .after
                        .content
                        .vector
                        .as_ref()
                        .expect("replacement reference"),
                )
            }),
        )?,
    };
    let transition = VectorsReembedded {
        version: 1,
        contract_version: request.context.contract_version,
        source: request.source,
        source_origins: origins,
        before_namespace: crate::storage::native_purge::NativeNamespaceProof::capture(&namespace)?,
        certification: certification.clone(),
        before_inventory_root: before_root,
        records: replacements,
    };
    if control::encode(&transition)?.len() as u64 > MAX_PROOF_BYTES {
        return Err(invalid("replacement proof exceeds 16 MiB"));
    }
    let db = redb::Database::open(stage.join("meta.db"))?;
    let tx = db.begin_write()?;
    control::replay_or_check(&tx, &request.context, hash)?;
    for record in updated {
        tx.open_table(META_TABLE)?
            .insert(record.id.as_slice(), record.to_bytes().as_slice())?;
    }
    let mut after_namespace = namespace;
    let schema = after_namespace
        .schema
        .as_mut()
        .ok_or_else(|| invalid("native namespace schema missing"))?;
    schema.vector_certification = Some(certification);
    schema.required_features = vec![REQUIRED_FEATURE.into()];
    tx.open_table(NAMESPACE_TABLE)?.insert(
        after_namespace.id.get(),
        control::encode(&after_namespace)?.as_slice(),
    )?;
    let receipt = control::record_reserved_receipt(
        &tx,
        &request.context,
        hash,
        control::MutationOutcome::VectorsReembedded { transition },
        operation_id,
    )?;
    let encoded = control::encode(&control::StoredReceipt {
        request_hash_version: 1,
        request_hash: hash,
        receipt: receipt.clone(),
    })?;
    control::validate_receipt(&encoded)?;
    crate::storage::catalog::boundary("certification_before_commit")?;
    // From this point neither cancellation nor uncertain acknowledgement may
    // delete the stage. Retry inspects the same canonical receipt first.
    cleanup.0 = None;
    let committed = tx.commit();
    #[cfg(test)]
    let committed = committed.and_then(|()| {
        if crate::storage::catalog::injected("certification_commit_unknown") {
            Err(redb::CommitError::Storage(redb::StorageError::Io(
                std::io::Error::other("injected uncertain certification acknowledgement"),
            )))
        } else {
            Ok(())
        }
    });
    committed.map_err(|source| StorageError::CommitOutcomeUnknown {
        operation_id,
        source,
    })?;
    drop(db);
    drop(vectors);
    publish(&stage, target, &receipt)?;
    Ok(receipt)
}
