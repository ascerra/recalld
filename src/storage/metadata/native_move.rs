use super::*;
use crate::storage::native_lifecycle::invalid;
use crate::storage::native_move::{CapturedNativeMove, MAX_MOVE_TEXT_BYTES, NativeMoveInput};
use crate::storage::native_purge::NativeNamespaceProof;

impl MetadataStore {
    pub(crate) fn commit_native_move(
        &self,
        guard: &MutationGuard<'_>,
        capture: &CapturedNativeMove,
        generated: &[crate::embedding::runtime::GeneratedEmbedding],
        progress: &crate::storage::native_move::SharedMoveProgress,
        stage: impl FnOnce(&mut [DiskRecord], &[Vec<f32>]) -> Result<(), StorageError>,
    ) -> Result<WriteReceipt, StorageError> {
        use crate::model::schema::VectorReference;
        use crate::storage::native_move::{MoveProgress, NativeMoveWrite, NativeMovedRecord};
        use crate::storage::native_records::NativeRecordState;
        use sha2::{Digest, Sha256};
        use std::sync::atomic::Ordering;
        guard.check(self)?;
        let input = &capture.input;
        let ctx = input.context();
        let hash = input.request_hash()?;
        let tx = self.db.begin_write()?;
        if let Some(receipt) = control::replay_or_check(&tx, &ctx, hash)? {
            return Ok(receipt);
        }
        let state = control::read_control(&tx)?;
        if state.generation_ref() != capture.at {
            return Err(StorageError::LifecycleConflict(
                "move capture generation changed".into(),
            ));
        }
        let source = namespace_state::require_namespace_active(&tx, capture.source.id)?;
        let destination = namespace_state::require_namespace_active(&tx, capture.destination.id)?;
        let source_proof = NativeNamespaceProof::capture(&source)?;
        let destination_proof = NativeNamespaceProof::capture(&destination)?;
        if source_proof != NativeNamespaceProof::capture(&capture.source)?
            || destination_proof != NativeNamespaceProof::capture(&capture.destination)?
        {
            return Err(StorageError::LifecycleConflict(
                "move namespace binding changed".into(),
            ));
        }
        self.require_certified_namespace(capture.at, &destination)?;
        if generated.len() != capture.records.len() || capture.documents.len() != generated.len() {
            return Err(invalid("move generated document count mismatch"));
        }
        let next = state.committed_generation.next()?;
        let mut after_records = Vec::new();
        let mut normalized = Vec::new();
        let mut records = Vec::new();
        for (((id, before), document), generated) in input
            .ids
            .iter()
            .zip(&capture.records)
            .zip(&capture.documents)
            .zip(generated)
        {
            let table = tx.open_table(META_TABLE)?;
            let current = table
                .get(id.as_bytes().as_slice())?
                .ok_or(StorageError::NotFound(id.into_inner()))?;
            // Compare physical state as well as tokens; text compaction cannot
            // replace a captured pointer without invalidating this operation.
            if current.value() != before.encode()?.as_slice() {
                return Err(StorageError::LifecycleConflict(
                    "move record capture changed".into(),
                ));
            }
            generated
                .verify_document(&document.text)
                .map_err(|e| StorageError::ProviderVectorUnavailable(e.to_string()))?;
            crate::embedding::profile::require_compatible(
                &destination,
                generated
                    .profile
                    .as_ref()
                    .ok_or(StorageError::ProviderConfigurationUnavailable)?,
            )
            .map_err(|e| StorageError::ProviderVectorUnavailable(e.to_string()))?;
            let mut values = generated.values.clone();
            crate::embedding::validation::normalize_vector(
                &mut values,
                destination.embedding_dim as usize,
            )
            .map_err(StorageError::InvalidMutation)?;
            let mut digest = Sha256::new();
            for value in &values {
                digest.update(value.to_le_bytes());
            }
            let mut after = before.clone();
            after.namespace_id = destination.id.get();
            // A maximal-width placeholder bounds the encoded proof before any
            // vector allocation. The actual slot is substituted after sync.
            after.vector_slot = u32::MAX;
            let schema = after
                .schema
                .as_mut()
                .ok_or_else(|| invalid("move schema missing"))?;
            schema.embedding_origin = None;
            schema.vector = Some(VectorReference {
                slot: u32::MAX,
                allocation_epoch: next,
                sha256: digest.finalize().into(),
            });
            schema.lifecycle.record_generation = schema.lifecycle.record_generation.next()?;
            crate::storage::native_move::follow_destination_policy(
                &mut schema.lifecycle,
                &mut after.last_accessed_at,
                destination_proof.schema.lifecycle_version,
                destination_proof.schema.policy_revision,
            );
            let text_hash = match document.evidence.full_text {
                crate::storage::vector_certification::RetainedTextInput::Absent => None,
                crate::storage::vector_certification::RetainedTextInput::Present { sha256 } => {
                    Some(sha256)
                }
            };
            records.push(NativeMovedRecord {
                change: crate::storage::native_records::NativeRecordChange {
                    memory_id: *id,
                    before: Some(NativeRecordState::capture(before, &source, text_hash)?),
                    after: NativeRecordState::capture(&after, &destination, text_hash)?,
                },
                document: document.evidence.clone(),
            });
            after_records.push(after);
            normalized.push(values);
        }
        let operation_id = self
            .id_source()
            .uuid(crate::storage::ids::IdKind::Operation)?;
        let mut transition = NativeMoveWrite {
            ids: input.ids.clone(),
            source_namespace: source.name.clone(),
            destination_namespace: destination.name.clone(),
            source_namespace_id: source.id,
            destination_namespace_id: destination.id,
            destination_profile_id: crate::storage::native_records::profile_id(&destination)
                .ok_or_else(|| invalid("move profile missing"))?,
            capture: capture.at,
            source: source_proof,
            destination: destination_proof,
            records,
        };
        let candidate = WriteReceipt {
            store_id: state.store_id,
            operation_id,
            committed_generation: next,
            receipt_version: 1,
            outcome: MutationOutcome::NativeMemoriesMoved {
                transition: transition.clone(),
            },
            replayed: false,
        };
        transition.validate(&candidate)?;
        *progress.lock().unwrap_or_else(|e| e.into_inner()) = MoveProgress::Staging(operation_id);
        stage(&mut after_records, &normalized)?;
        for (row, after) in transition.records.iter_mut().zip(&after_records) {
            row.change.after.content.vector = after.schema.as_ref().unwrap().vector.clone();
        }
        let receipt = control::record_reserved_receipt(
            &tx,
            &ctx,
            hash,
            MutationOutcome::NativeMemoriesMoved { transition },
            operation_id,
        )?;
        {
            let mut table = tx.open_table(META_TABLE)?;
            let mut index = tx.open_multimap_table(NAMESPACE_INDEX)?;
            for (id, record) in input.ids.iter().zip(&after_records) {
                table.insert(id.as_bytes().as_slice(), record.encode()?.as_slice())?;
                index.remove(source.id.get(), id.as_bytes().as_slice())?;
                index.insert(destination.id.get(), id.as_bytes().as_slice())?;
            }
        }
        let pending = effects::PendingEffects {
            version: 1,
            receipt: receipt.clone(),
            scope: MutationScope::Catalog,
            puts: vec![],
            deletes: vec![],
            removed_ids: vec![],
            peer_counts: vec![],
            invalidated_ids: input.ids.clone(),
            canonical_applied: false,
            releases: input
                .ids
                .iter()
                .zip(&capture.records)
                .map(|(id, record)| effects::VectorRelease {
                    namespace_id: source.id,
                    slot: record.vector_slot,
                    memory_id: *id,
                })
                .collect(),
        };
        effects::validate_native_move_effect(&pending)?;
        tx.open_table(effects::EFFECTS)?.insert(
            operation_id.as_bytes().as_slice(),
            control::encode(&pending)?.as_slice(),
        )?;
        let certification = self.prepare_vector_validation_commit(&tx, &receipt)?;
        let epoch = self
            .effects_epoch
            .load(Ordering::SeqCst)
            .checked_add(1)
            .ok_or(StorageError::GenerationExhausted)?;
        effects::crash_point("before_move_metadata_commit");
        self.derived_dirty.store(true, Ordering::SeqCst);
        self.canonical_pending.store(true, Ordering::SeqCst);
        self.effects_epoch.store(epoch, Ordering::SeqCst);
        tx.commit().map_err(|source| {
            self.mark_unavailable();
            StorageError::CommitOutcomeUnknown {
                operation_id,
                source,
            }
        })?;
        *progress.lock().unwrap_or_else(|e| e.into_inner()) =
            MoveProgress::Committed(receipt.clone());
        self.publish_vector_validation_commit(certification, &receipt)?;
        self.view_publication.canonical_committed();
        effects::crash_point("after_move_metadata_commit");
        let mut phases = self.phase_index.write().unwrap_or_else(|e| e.into_inner());
        for (before, after) in capture.records.iter().zip(&after_records) {
            phases.remove(before.namespace_id, before.vector_slot, before.phase);
            phases.transition(
                after.namespace_id,
                after.vector_slot,
                DecayPhase::Full,
                after.phase,
            );
        }
        Ok(receipt)
    }

    pub(crate) fn lookup_native_move(
        &self,
        input: &NativeMoveInput,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let hash = input.request_hash()?;
        let _guard = self.native_guard_with_work(parent)?;
        control::replay_or_check(&self.db.begin_write()?, &input.context(), hash)
    }

    pub(crate) fn capture_native_move(
        &self,
        input: NativeMoveInput,
        parent: Option<&WorkTicket>,
    ) -> Result<CapturedNativeMove, StorageError> {
        input.validate()?;
        let _guard = self.native_guard_with_work(parent)?;
        let read = self.db.begin_read()?;
        let state = canonical_scope::current(&read)?;
        if state.store_id != input.store_id {
            return Err(StorageError::StoreLineageMismatch);
        }
        let namespaces: Vec<NamespaceConfig> = read
            .open_table(NAMESPACE_TABLE)?
            .iter()?
            .map(|row| Ok(NamespaceConfig::decode(row?.1.value())?))
            .collect::<Result<_, StorageError>>()?;
        let resolve = |name: &str| -> Result<NamespaceConfig, StorageError> {
            let selected: Vec<_> = namespaces
                .iter()
                .filter(|n| n.name.eq_ignore_ascii_case(name))
                .collect();
            if selected.len() != 1 {
                return Err(StorageError::InvalidMutation(
                    "move namespace absent or ambiguous".into(),
                ));
            }
            Ok(selected[0].clone())
        };
        let source = resolve(&input.source_namespace)?;
        let destination = resolve(&input.destination_namespace)?;
        // Both bindings are captured in full, including mutable policy revision.
        let from = NativeNamespaceProof::capture(&source)?;
        let to = NativeNamespaceProof::capture(&destination)?;
        if source.id == destination.id
            || !matches!(
                to.schema.embedding_binding,
                crate::model::schema::EmbeddingBinding::Bound { .. }
            )
        {
            return Err(StorageError::InvalidMutation(
                "move requires distinct namespaces and a bound destination".into(),
            ));
        }
        self.require_certified_namespace(state.generation_ref(), &destination)?;
        let table = read.open_table(META_TABLE)?;
        let mut records = Vec::with_capacity(input.ids.len());
        let mut text_bytes = 0usize;
        for id in &input.ids {
            let bytes = table
                .get(id.as_bytes().as_slice())?
                .ok_or(StorageError::NotFound(id.into_inner()))?;
            let record = DiskRecord::from_bytes(bytes.value())?;
            let schema = record
                .schema
                .as_ref()
                .ok_or_else(|| invalid("move needs canonical record schema"))?;
            if record.namespace_id != source.id.get() {
                return Err(StorageError::MutationNamespaceMismatch);
            }
            if !matches!(record.phase, DecayPhase::Full | DecayPhase::Summary)
                || record.summary.is_empty()
            {
                return Err(StorageError::LifecycleContentUnavailable);
            }
            if schema.lifecycle.lifecycle_version != from.schema.lifecycle_version
                || schema.lifecycle.policy_revision != from.schema.policy_revision
                || schema.vector.is_none()
            {
                return Err(invalid("move record policy/vector differs from source"));
            }
            schema.lifecycle.record_generation.next()?;
            text_bytes = text_bytes
                .checked_add(record.text_length as usize)
                .ok_or_else(|| invalid("move text size overflow"))?;
            if text_bytes > MAX_MOVE_TEXT_BYTES {
                return Err(StorageError::InvalidMutation(
                    "move retained text exceeds 16 MiB; use a smaller batch".into(),
                ));
            }
            records.push(record);
        }
        Ok(CapturedNativeMove {
            input,
            at: state.generation_ref(),
            source,
            destination,
            records,
            documents: Vec::new(),
        })
    }
}
