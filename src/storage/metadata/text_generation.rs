//! Atomic text selection, offsets, generation and receipt. Payloads are synced first.
use super::*;
use crate::storage::control::Generation;
use crate::storage::text::{RefMapping, TextRef};
use redb::{MultimapTableHandle, TableHandle};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(test)]
thread_local! {
    pub(crate) static FAIL_AFTER_TEXT_COMMIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) const CATALOG: TableDefinition<&str, &[u8]> =
    TableDefinition::new("text_generations_v1");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TextGeneration {
    pub id: Uuid,
    pub store_id: Uuid,
    pub filename: String,
    /// 1 is the offset-preserving, recoverable legacy-upgrade intermediate.
    pub format_version: u16,
    pub sealed_length: u64,
    pub sealed_sha256: [u8; 32],
}

pub(crate) fn generation_filename(id: Uuid) -> String {
    format!("fulltext.{id}.dat")
}

pub(crate) fn is_generation_filename(name: &str) -> bool {
    name.strip_prefix("fulltext.")
        .and_then(|s| s.strip_suffix(".dat"))
        .and_then(|s| Uuid::parse_str(s).ok())
        .is_some_and(|id| name == generation_filename(id))
}

impl TextGeneration {
    pub(crate) fn validate(&self, store_id: Uuid) -> Result<(), StorageError> {
        let header = match self.format_version {
            1 => 16,
            2 => 48,
            _ => 0,
        };
        if header == 0
            || self.id.is_nil()
            || self.store_id != store_id
            || self.filename != generation_filename(self.id)
            || self.sealed_length < header
        {
            return Err(StorageError::InvalidControl(
                "invalid text generation descriptor".into(),
            ));
        }
        Ok(())
    }
}

/// Absence means legacy, never 'choose the newest file'. Wrong table types fail.
pub(crate) fn read_selection(
    read: &redb::ReadTransaction,
) -> Result<Option<TextGeneration>, StorageError> {
    let present = read.list_tables()?.any(|t| t.name() == CATALOG.name());
    let wrong_type = read
        .list_multimap_tables()?
        .any(|t| t.name() == CATALOG.name());
    if wrong_type {
        return Err(StorageError::InvalidControl("multimap text catalog".into()));
    }
    if !present {
        return Ok(None);
    }
    let state = control::validate_existing(read)?
        .ok_or_else(|| StorageError::InvalidControl("text catalog without control".into()))?;
    let table = read.open_table(CATALOG)?;
    let active = table
        .get("active")?
        .ok_or_else(|| StorageError::InvalidControl("text catalog without selection".into()))?;
    let active: Uuid = control::decode(active.value())?;
    let mut selected = None;
    for entry in table.iter()? {
        let (key, value) = entry?;
        if key.value() == "active" {
            continue;
        }
        let descriptor: TextGeneration = control::decode(value.value())?;
        descriptor.validate(state.store_id)?;
        if key.value() != descriptor.id.to_string() {
            return Err(StorageError::InvalidControl(
                "text descriptor key mismatch".into(),
            ));
        }
        if descriptor.id == active {
            selected = Some(descriptor);
        }
    }
    let selected = selected
        .ok_or_else(|| StorageError::InvalidControl("selected text descriptor missing".into()))?;
    // A same-identity physical generation must match its committed descriptor.
    // Import/restore may legitimately repack to a different generation UUID.
    for row in read.open_table(control::RECEIPTS)?.iter()? {
        let (_, value) = row?;
        let stored = control::validate_receipt(value.value())?;
        if let MutationOutcome::NativeTextGenerationSelected { transition } =
            &stored.receipt.outcome
            && transition.after.id == selected.id
        {
            if transition.after != selected || stored.receipt.store_id != state.store_id {
                return Err(StorageError::InvalidControl(
                    "selected descriptor differs from native relocation".into(),
                ));
            }
            let before = table
                .get(transition.before.id.to_string().as_str())?
                .ok_or_else(|| {
                    StorageError::InvalidControl("native relocation before catalog missing".into())
                })?;
            if control::decode::<TextGeneration>(before.value())? != transition.before {
                return Err(StorageError::InvalidControl(
                    "native relocation before descriptor mismatch".into(),
                ));
            }
            if stored.receipt.committed_generation == state.committed_generation {
                let refs = read
                    .open_table(META_TABLE)?
                    .iter()?
                    .filter_map(|row| {
                        let row = row.map_err(StorageError::from).and_then(|(_, v)| {
                            DiskRecord::from_bytes(v.value()).map_err(StorageError::from)
                        });
                        match row {
                            Err(e) => Some(Err(e)),
                            Ok(r) if r.text_offset != 0 || r.text_length != 0 => Some(Ok((
                                MemoryId::from_bytes(r.id),
                                TextRef {
                                    file_offset: r.text_offset,
                                    length: r.text_length,
                                },
                            ))),
                            Ok(_) => None,
                        }
                    })
                    .collect::<Result<RefMapping, StorageError>>()?;
                if refs.len() as u64 != transition.retained_count
                    || crate::storage::native_text::reference_digest(&refs)?
                        != transition.after_refs_sha256
                {
                    return Err(StorageError::InvalidControl(
                        "current native relocation references mismatch".into(),
                    ));
                }
            }
        }
    }
    Ok(Some(selected))
}

impl MetadataStore {
    pub(crate) fn text_compaction_guard_with_work(
        &self,
        parent: Option<&WorkTicket>,
    ) -> Result<MutationGuard<'_>, StorageError> {
        if self.native_lifecycle_only {
            self.native_guard_with_work(parent)
        } else {
            self.mutation_guard_with_work(parent)
        }
    }

    pub(crate) fn text_selection(&self) -> Result<Option<TextGeneration>, StorageError> {
        self.ensure_available()?;
        self.text_selection_for_recovery()
    }

    /// Constructor-only read before canonical effects replay; the engine has not escaped.
    pub(crate) fn text_selection_for_recovery(
        &self,
    ) -> Result<Option<TextGeneration>, StorageError> {
        read_selection(&self.db.begin_read()?)
    }

    /// The caller holds the exclusive engine gate through file and handle publication.
    pub(crate) fn select_text_generation(
        &self,
        expected: Generation,
        descriptor: &TextGeneration,
        refs: &RefMapping,
    ) -> Result<WriteReceipt, StorageError> {
        let guard = self.mutation_guard()?;
        self.select_text_generation_guarded(&guard, expected, descriptor, refs)
    }

    pub(crate) fn select_text_generation_guarded(
        &self,
        guard: &MutationGuard<'_>,
        expected: Generation,
        descriptor: &TextGeneration,
        refs: &RefMapping,
    ) -> Result<WriteReceipt, StorageError> {
        self.select_text_generation_proved_guarded(
            guard,
            expected,
            descriptor,
            refs,
            None,
            &Default::default(),
        )
    }

    pub(crate) fn select_text_generation_proved_guarded(
        &self,
        guard: &MutationGuard<'_>,
        expected: Generation,
        descriptor: &TextGeneration,
        refs: &RefMapping,
        proof: Option<&crate::storage::native_text::NativeTextRelocation>,
        text_hashes: &std::collections::BTreeMap<MemoryId, [u8; 32]>,
    ) -> Result<WriteReceipt, StorageError> {
        use crate::storage::native_text;
        guard.check(self)?;
        let tx = self.db.begin_write()?;
        let state = control::read_control(&tx)?;
        if (state.control_schema_version == 2) != self.native_lifecycle_only
            || self.native_lifecycle_only != proof.is_some()
        {
            return Err(StorageError::InvalidControl(
                "text relocation proof/owner mismatch".into(),
            ));
        }
        purge::require_no_active_write(&tx)?;
        let capture = || -> Result<_, StorageError> {
            let rows = tx
                .open_table(META_TABLE)?
                .iter()?
                .map(|row| {
                    let (_, value) = row?;
                    let r = DiskRecord::from_bytes(value.value())?;
                    Ok((MemoryId::from_bytes(r.id), r))
                })
                .collect::<Result<Vec<_>, StorageError>>()?;
            let namespaces = tx
                .open_table(NAMESPACE_TABLE)?
                .iter()?
                .map(|row| {
                    let (_, value) = row?;
                    NamespaceConfig::decode(value.value()).map_err(StorageError::from)
                })
                .collect::<Result<Vec<_>, StorageError>>()?;
            Ok((rows, namespaces))
        };
        if let Some(proof) = proof {
            if proof.source != state.generation_ref()
                || &proof.after != descriptor
                || native_text::reference_digest(refs)? != proof.after_refs_sha256
                || native_text::copy_digest(refs, text_hashes)? != proof.copied_text_sha256
                || proof.retained_count != refs.len() as u64
            {
                return Err(StorageError::InvalidControl(
                    "text relocation selection mismatch".into(),
                ));
            }
            let catalog = tx.open_table(CATALOG)?;
            let active: Uuid = control::decode(
                catalog
                    .get("active")?
                    .ok_or(StorageError::InvalidTextRef)?
                    .value(),
            )?;
            let before: TextGeneration = control::decode(
                catalog
                    .get(active.to_string().as_str())?
                    .ok_or(StorageError::InvalidTextRef)?
                    .value(),
            )?;
            if before != proof.before {
                return Err(StorageError::InvalidTextRef);
            }
            let (rows, namespaces) = capture()?;
            let old_refs = rows
                .iter()
                .filter(|(_, r)| r.text_offset != 0 || r.text_length != 0)
                .map(|(id, r)| {
                    (
                        *id,
                        TextRef {
                            file_offset: r.text_offset,
                            length: r.text_length,
                        },
                    )
                })
                .collect();
            if native_text::reference_digest(&old_refs)? != proof.before_refs_sha256
                || native_text::record_digest(&rows, &namespaces, text_hashes)?
                    != (proof.record_count, proof.records_sha256)
            {
                return Err(StorageError::InvalidControl(
                    "text relocation before-state mismatch".into(),
                ));
            }
        }
        descriptor.validate(state.store_id)?;
        let ctx = MutationContext {
            store_id: Some(state.store_id),
            scope: MutationScope::Catalog,
            idempotency_key: None,
            expected_generation: Some(expected),
            contract_version: 1,
        };
        let bytes = control::encode(descriptor)?;
        // File UUID and seal are generated results, not semantic request identity.
        let operation = if descriptor.format_version == 1 {
            "registerLegacyText"
        } else {
            "compactText"
        };
        let hash = control::request_hash(&ctx, operation, &[]);
        control::replay_or_check(&tx, &ctx, hash)?;
        {
            let mut records = tx.open_table(META_TABLE)?;
            // Every retained extent must participate, including non-Full records.
            let retained =
                records
                    .iter()?
                    .try_fold(0usize, |n, row| -> Result<_, StorageError> {
                        let (_, value) = row?;
                        let r = DiskRecord::from_bytes(value.value())?;
                        Ok(n + usize::from(r.text_offset != 0 || r.text_length != 0))
                    })?;
            let mut seen = std::collections::HashSet::new();
            if retained != refs.len() {
                return Err(StorageError::InvalidMutation(
                    "incomplete text relocation".into(),
                ));
            }
            for (id, reference) in refs {
                let header_size = if descriptor.format_version == 1 {
                    16
                } else {
                    48
                };
                if !seen.insert(*id)
                    || !reference.is_some()
                    || reference.file_offset < header_size
                    || reference
                        .file_offset
                        .checked_add(8 + u64::from(reference.length))
                        .is_none_or(|end| end > descriptor.sealed_length)
                {
                    return Err(StorageError::InvalidTextRef);
                }
                let mut record = {
                    let old = records
                        .get(id.as_bytes().as_slice())?
                        .ok_or(StorageError::NotFound(**id))?;
                    DiskRecord::from_bytes(old.value())?
                };
                if !(TextRef {
                    file_offset: record.text_offset,
                    length: record.text_length,
                })
                .is_some()
                {
                    return Err(StorageError::InvalidTextRef);
                }
                if record.text_length != reference.length {
                    return Err(StorageError::InvalidTextRef);
                }
                record.text_offset = reference.file_offset;
                record.text_length = reference.length;
                records.insert(id.as_bytes().as_slice(), record.encode()?.as_slice())?;
            }
        }
        {
            let mut catalog = tx.open_table(CATALOG)?;
            let key = descriptor.id.to_string();
            if catalog.get(key.as_str())?.is_some() {
                return Err(StorageError::InvalidMutation(
                    "text generation reused".into(),
                ));
            }
            catalog.insert(key.as_str(), bytes.as_slice())?;
            catalog.insert("active", control::encode(&descriptor.id)?.as_slice())?;
        }
        let outcome = if let Some(proof) = proof {
            let (rows, namespaces) = capture()?;
            if native_text::record_digest(&rows, &namespaces, text_hashes)?
                != (proof.record_count, proof.records_sha256)
            {
                return Err(StorageError::InvalidControl(
                    "text relocation changed logical content".into(),
                ));
            }
            MutationOutcome::NativeTextGenerationSelected {
                transition: proof.clone(),
            }
        } else {
            MutationOutcome::TextGenerationSelected {
                generation_id: descriptor.id,
                format_version: descriptor.format_version,
                entries_kept: refs.len() as u64,
            }
        };
        let receipt = self.record_receipt(&tx, &ctx, hash, outcome)?;
        if let Some(proof) = proof {
            proof.validate(&receipt)?;
        }

        let committed = tx.commit();
        // Model an I/O error whose commit actually became durable. Tests must
        // reconcile the receipt after reopen, not reclaim its selected payload.
        #[cfg(test)]
        let committed = committed.and_then(|()| {
            if FAIL_AFTER_TEXT_COMMIT.with(|flag| flag.replace(false)) {
                Err(redb::CommitError::Storage(redb::StorageError::Io(
                    std::io::Error::other("injected unknown text commit"),
                )))
            } else {
                Ok(())
            }
        });
        committed.map_err(|e| {
            self.mark_unavailable();
            StorageError::CommitOutcomeUnknown {
                operation_id: receipt.operation_id,
                source: e,
            }
        })?;
        self.view_publication.canonical_committed();
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(store_id: Uuid) -> TextGeneration {
        let id = Uuid::new_v4();
        TextGeneration {
            id,
            store_id,
            filename: generation_filename(id),
            format_version: 2,
            sealed_length: 48,
            sealed_sha256: [0; 32],
        }
    }

    #[test]
    fn text_selection_receipt_reopens_and_stale_generation_cannot_replace_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.db");
        let store = MetadataStore::open(&path).unwrap();
        let before = store.control_state().unwrap();
        let first = descriptor(before.store_id);
        let receipt = store
            .select_text_generation(before.committed_generation, &first, &vec![])
            .unwrap();
        assert!(matches!(
            store.select_text_generation(
                before.committed_generation,
                &descriptor(before.store_id),
                &vec![]
            ),
            Err(StorageError::GenerationConflict { .. })
        ));
        assert_eq!(store.text_selection().unwrap(), Some(first.clone()));
        assert_eq!(
            store.control_state().unwrap().committed_generation,
            receipt.committed_generation
        );
        drop(store);
        let store = MetadataStore::open(&path).unwrap();
        assert_eq!(store.text_selection().unwrap(), Some(first));
        assert_eq!(
            store
                .lookup_operation(
                    before.store_id,
                    MutationScope::Catalog,
                    receipt.operation_id
                )
                .unwrap(),
            Some(receipt)
        );
    }

    #[test]
    fn text_catalog_partial_unknown_descriptor_and_unsafe_filename_fail_closed() {
        for fault in [
            "empty",
            "unknown-format",
            "unsafe-name",
            "missing-descriptor",
            "wrong-owner",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("meta.db");
            let store = MetadataStore::open(&path).unwrap();
            let mut d = descriptor(store.control_state().unwrap().store_id);
            match fault {
                "unknown-format" => d.format_version = 999,
                "unsafe-name" => d.filename = "../escape".into(),
                "wrong-owner" => d.store_id = Uuid::new_v4(),
                _ => {}
            }
            {
                let tx = store.db.begin_write().unwrap();
                {
                    let mut table = tx.open_table(CATALOG).unwrap();
                    if fault != "empty" {
                        table
                            .insert("active", control::encode(&d.id).unwrap().as_slice())
                            .unwrap();
                    }
                    if fault != "missing-descriptor" && fault != "empty" {
                        table
                            .insert(
                                d.id.to_string().as_str(),
                                control::encode(&d).unwrap().as_slice(),
                            )
                            .unwrap();
                    }
                }
                tx.commit().unwrap();
            }
            assert!(store.text_selection().is_err(), "{fault}");
            drop(store);
            assert!(MetadataStore::open(&path).is_err(), "{fault}");
        }
    }
}
