//! Strict legacy-store inspection. Never initializes missing canonical files.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use redb::{ReadableTable, ReadableTableMetadata};

use super::StorageError;
use super::edges::{FORWARD_EDGES, PersistedEdge, REVERSE_EDGES};
use super::metadata::{META_TABLE, NAMESPACE_TABLE};
use super::text::{TextRef, TextStore, recover_text_compaction};
use super::vectors::VectorStore;
use crate::model::validation::validate_namespace_name;
use crate::model::{DecayPhase, DiskRecord, MemoryId, NamespaceConfig};

pub(crate) fn require_file(path: &Path) -> Result<(), StorageError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(StorageError::RecoveryRequired(format!(
            "missing, empty or nonregular canonical file {}",
            path.display()
        )));
    }
    Ok(())
}

// redb 2.x writes bookkeeping even on open. Inspect disposable copies and
// abort repair; source files remain byte-for-byte unchanged.
pub(crate) fn inspect_database(
    path: &Path,
) -> Result<(tempfile::TempDir, redb::Database), StorageError> {
    inspect_database_copy(path, false)
}

pub(crate) fn inspect_database_copy(
    path: &Path,
    recover_copy: bool,
) -> Result<(tempfile::TempDir, redb::Database), StorageError> {
    require_file(path)?;
    let temp = tempfile::tempdir()?;
    let copy = temp.path().join("inspect.db");
    std::fs::copy(path, &copy)?;
    let mut builder = redb::Builder::new();
    if !recover_copy {
        builder.set_repair_callback(|session| session.abort());
    }
    let mut db = builder.open(copy)?;
    if !db.check_integrity()? {
        return Err(StorageError::RecoveryRequired(format!(
            "database integrity failure: {}",
            path.display()
        )));
    }
    Ok((temp, db))
}

/// Choose the existing startup preflight under the caller's two ownership locks.
/// Header inspection is not admission: the selected full validator must run
/// before any writable canonical open. Do not add another lifetime history scan.
pub(crate) fn startup_native_format(path: &Path) -> Result<bool, StorageError> {
    use redb::{MultimapTableHandle, TableHandle};

    let (_copy, metadata) = inspect_database_copy(&path.join("meta.db"), true)?;
    let read = metadata.begin_read()?;
    let reserved = [
        super::control::CONTROL.name(),
        super::control::RECEIPTS.name(),
        super::control::KEYS.name(),
    ];
    if read.list_multimap_tables()?.any(|table| {
        reserved.contains(&table.name()) || table.name() == super::canonical_schema::SCHEMA.name()
    }) {
        return Err(StorageError::InvalidControl(
            "control table is a multimap".into(),
        ));
    }
    let mut present = [false; 3];
    let mut schema = false;
    for table in read.list_tables()? {
        for (index, name) in reserved.iter().enumerate() {
            present[index] |= table.name() == *name;
        }
        schema |= table.name() == super::canonical_schema::SCHEMA.name();
    }
    if !present.iter().any(|v| *v) {
        if schema {
            return Err(StorageError::InvalidControl(
                "schema without control lineage".into(),
            ));
        }
        return Ok(false);
    }
    if !present.iter().all(|v| *v) {
        return Err(StorageError::InvalidControl(
            "incomplete control tables".into(),
        ));
    }
    let control = read.open_table(super::control::CONTROL)?;
    if control.len()? != 1 {
        return Err(StorageError::InvalidControl(
            "control must contain exactly one state row".into(),
        ));
    }
    let state = super::control::ControlState::decode(
        control
            .get("state")?
            .ok_or_else(|| StorageError::InvalidControl("missing control state".into()))?
            .value(),
    )?;
    Ok(state.control_schema_version == 2)
}

/// Canonical text preflight on a disposable metadata copy. This precedes any
/// writable canonical open; pending plans are checked without applying effects.
pub(crate) fn preflight_text(path: &Path) -> Result<(), StorageError> {
    // An unclean process exit may require redb free-list recovery. Perform it
    // only on the disposable copy before touching the original database.
    let (_copy, metadata) = inspect_database_copy(&path.join("meta.db"), true)?;
    let read = metadata.begin_read()?;
    let control = super::control::validate_existing(&read)?;
    if let Some(control) = &control {
        super::lineage::ensure_serving(&read, control)?;
    }
    super::metadata::removal_ledger::read_export(&read, control.as_ref())?;
    // Valid pending effects are recoverable at startup; only malformed evidence fails here.
    let effects = super::metadata::effects::validate_existing(&read)?;
    if control
        .as_ref()
        .is_some_and(|c| c.control_schema_version == 2)
    {
        return Err(StorageError::RecoveryRequired("schema v2 remains offline until participating writers are enabled; use schema inspect/export".into()));
    }
    let text = selected_text(path, &read)?;
    let records = read.open_table(META_TABLE)?;
    let mut decoded = Vec::new();
    for row in records.iter()? {
        let (_, bytes) = row?;
        let record = DiskRecord::from_bytes(bytes.value())?;
        decoded.push((MemoryId::from_bytes(record.id), record.clone()));
        if record.text_offset != 0 || record.text_length != 0 {
            text.read(TextRef {
                file_offset: record.text_offset,
                length: record.text_length,
            })?;
        }
    }
    for effect in effects.iter().filter(|effect| !effect.canonical_applied) {
        super::metadata::effects::validate_plan(effect, &decoded)?;
    }
    Ok(())
}

fn selected_text(path: &Path, read: &redb::ReadTransaction) -> Result<TextStore, StorageError> {
    match super::metadata::text_generation::read_selection(read)? {
        Some(descriptor) => {
            super::text::generation::validate_rejection_gate(path, descriptor.store_id)?;
            TextStore::open_generation(path, &descriptor, false)
        }
        None => TextStore::open_read_only(&path.join("fulltext.dat")),
    }
}

/// Caller owns/quiesces the store, or supplies an inaccessible staging copy.
/// Returns the required canonical inventory derived from validated metadata.
pub(crate) fn validate_existing(path: &Path) -> Result<HashSet<String>, StorageError> {
    validate_snapshot(path, InspectionPurpose::Ordinary)
}
/// Only the logical importer may inspect its unpublished, explicitly importing stage.
pub(crate) fn validate_import_stage(path: &Path) -> Result<HashSet<String>, StorageError> {
    validate_snapshot(path, InspectionPurpose::Import)
}
/// Strict private-native preflight admits only validated native recovery plans.
pub(crate) fn preflight_native(path: &Path) -> Result<(), StorageError> {
    let result = validate_snapshot(path, InspectionPurpose::NativeRecovery).map(|_| ());
    #[cfg(test)]
    NATIVE_PREFLIGHT_PROBE.with(|probe| {
        if let Some(check) = probe.borrow().as_ref() {
            check(path);
        }
    });
    result
}

#[cfg(test)]
thread_local! {
    pub(crate) static NATIVE_PREFLIGHT_PROBE:
        std::cell::RefCell<Option<Box<dyn Fn(&Path)>>> = const { std::cell::RefCell::new(None) };
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectionPurpose {
    Ordinary,
    Import,
    NativeRecovery,
    VectorRemedy,
}

/// Only explicit fresh-destination vector remediation may preserve validated
/// pending native batches. No repair, effects application or ordinary export.
pub(crate) fn vector_remedy_files(path: &Path) -> Result<Vec<PathBuf>, StorageError> {
    let mut files: Vec<_> = validate_snapshot(path, InspectionPurpose::VectorRemedy)?
        .into_iter()
        .map(|name| path.join(name))
        .collect();
    files.sort();
    Ok(files)
}

fn validate_snapshot(
    path: &Path,
    purpose: InspectionPurpose,
) -> Result<HashSet<String>, StorageError> {
    let importing = purpose == InspectionPurpose::Import;
    let native_recovery = purpose == InspectionPurpose::NativeRecovery;
    if !importing && std::fs::symlink_metadata(path.join(".recalld-importing")).is_ok() {
        return Err(StorageError::RecoveryRequired(
            "incomplete logical import; resume using its original export".into(),
        ));
    }
    recover_text_compaction(path)?;
    for name in ["meta.db", "edges.db", "fulltext.dat"] {
        require_file(&path.join(name))?;
    }
    // Only native startup permits redb recovery, and only on disposable copies.
    // All canonical validation still precedes writable opens of the originals.
    let (_meta_copy, meta) = inspect_database_copy(&path.join("meta.db"), native_recovery)?;
    let read = meta.begin_read()?;
    let control = super::control::validate_existing(&read)?;
    if native_recovery {
        let state = control
            .as_ref()
            .ok_or_else(|| StorageError::InvalidControl("native control missing".into()))?;
        if state.control_schema_version != 2
            || super::metadata::text_generation::read_selection(&read)?.is_none()
        {
            return Err(StorageError::InvalidControl(
                "native entry requires gated schema 2".into(),
            ));
        }
        super::lineage::ensure_serving(&read, state)?;
        super::metadata::restoration::validate_inventory(&read)?;
        super::canonical_schema::validate_metadata_format(&read, Some(state))?;
    }
    let batches = super::metadata::batch::validate_existing(&read, control.as_ref())?;
    if batches.iter().any(|p| {
        p.unresolved() != 0
            && (!(native_recovery || purpose == InspectionPurpose::VectorRemedy)
                || !matches!(p.version, 2 | 3))
    }) {
        return Err(StorageError::RecoveryRequired(
            "unfinished batch needs matching caller resubmission; offline inventory is unavailable"
                .into(),
        ));
    }

    if super::metadata::purge::validate_existing(&read, control.as_ref())?
        .iter()
        .any(|p| {
            p.stage != super::purge::PurgeStage::Complete && (!native_recovery || p.version != 2)
        })
    {
        return Err(StorageError::RecoveryRequired(
            "unfinished namespace purge; reopen and finish before offline inventory".into(),
        ));
    }
    super::metadata::removal_ledger::read_export(&read, control.as_ref())?;
    let native_creations = if control
        .as_ref()
        .is_some_and(|c| c.control_schema_version == 2)
    {
        super::native_records::creation_proofs(&read)?
    } else {
        HashMap::new()
    };
    let schema_state = control
        .as_ref()
        .map(|c| super::canonical_schema::read_state(&read, c))
        .transpose()?
        .flatten();
    if control.is_some() {
        read.open_table(super::metadata::NAMESPACE_COUNTER)?;
    }
    let effects = super::metadata::effects::validate_existing(&read)?;
    if effects.iter().any(|e| {
        !e.canonical_applied
            && (!native_recovery
                || !matches!(
                    e.receipt.outcome,
                    super::control::MutationOutcome::NativeNamespacePurgeAdvanced { .. }
                        | super::control::MutationOutcome::NativePhaseMaterialized { .. }
                        | super::control::MutationOutcome::NativeMemoryHardDeleted { .. }
                        | super::control::MutationOutcome::NativeMemoryInsertedWithLinks { .. }
                        | super::control::MutationOutcome::NativeMemoryInserted { .. }
                        | super::control::MutationOutcome::NativeMemoryTombstoned { .. }
                        | super::control::MutationOutcome::NativeGraphChanged { .. }
                        | super::control::MutationOutcome::NativeGraphUnchanged { .. }
                ))
    }) {
        return Err(StorageError::RecoveryRequired(
            "pending canonical effects; reopen before backup".into(),
        ));
    }
    let namespaces = read.open_table(NAMESPACE_TABLE)?;
    let mut inventory: HashSet<String> = ["meta.db", "edges.db", "fulltext.dat"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let mut vectors = HashMap::new();
    let mut configs = HashMap::new();
    let mut names = HashSet::new();
    for item in namespaces.iter()? {
        let (key, value) = item?;
        let config = NamespaceConfig::decode(value.value())?;
        validate_namespace_name(&config.name).map_err(|reason| {
            StorageError::InvalidNamespaceName {
                name: config.name.clone(),
                reason: reason.to_string(),
            }
        })?;
        if key.value() != config.id.get()
            || !names.insert(config.name.to_ascii_lowercase())
            || !(1..=8192).contains(&config.embedding_dim)
        {
            return Err(StorageError::InvalidMutation(
                "invalid namespace identity/dimension".into(),
            ));
        }
        if schema_state.is_some() != config.schema.is_some() {
            return Err(StorageError::InvalidControl(
                "mixed namespace schema".into(),
            ));
        }
        configs.insert(config.id.get(), config.clone());
        let dir = path.join(&config.name);
        if !std::fs::symlink_metadata(&dir)?.is_dir() {
            return Err(StorageError::InvalidPath);
        }
        require_file(&dir.join("vectors.dat"))?;
        inventory.insert(format!("{}/vectors.dat", config.name));
        // VectorStore opens an existing, nonempty file without modifying it.
        vectors.insert(
            config.id.get(),
            VectorStore::open(&dir, config.embedding_dim as usize)?,
        );
    }
    if let Some(descriptor) = super::metadata::text_generation::read_selection(&read)? {
        inventory.insert(descriptor.filename);
    }
    let text = selected_text(path, &read)?;
    let certified_text = if configs.values().any(|ns| {
        ns.schema
            .as_ref()
            .is_some_and(|s| s.vector_certification.is_some())
    }) {
        super::vector_certification::retained_text_proofs(&read)?
    } else {
        HashMap::new()
    };
    let records = read.open_table(META_TABLE)?;
    let mut ids = HashSet::new();
    let mut owners = HashSet::new();
    let mut decoded = Vec::new();
    for item in records.iter()? {
        let (key, value) = item?;
        let record = DiskRecord::from_bytes(value.value())?;
        if key.value() != record.id {
            return Err(StorageError::InvalidMutation(
                "record key/ID mismatch".into(),
            ));
        }
        let id = MemoryId::from_bytes(record.id);
        decoded.push((id, record.clone()));
        ids.insert(id);
        let namespace = configs
            .get(&record.namespace_id)
            .ok_or(StorageError::NamespaceNotFound(record.namespace_id))?;
        super::canonical_schema::validate_record_namespace(
            &record,
            namespace,
            schema_state.as_ref(),
        )?;
        let store = vectors
            .get(&record.namespace_id)
            .ok_or(StorageError::NamespaceNotFound(record.namespace_id))?;
        if record.phase != DecayPhase::Tombstone
            && (!owners.insert((record.namespace_id, record.vector_slot))
                || store.is_free(record.vector_slot)
                || store
                    .get_vector(record.vector_slot)
                    .is_none_or(|v| v.iter().any(|f| !f.is_finite())))
        {
            return Err(StorageError::OwnershipMismatch(format!(
                "invalid vector owner {id}"
            )));
        }
        if let Some(reference) = record
            .schema
            .as_ref()
            .and_then(|schema| schema.vector.as_ref())
        {
            use sha2::{Digest, Sha256};
            let vector = store
                .get_vector(record.vector_slot)
                .ok_or_else(|| StorageError::OwnershipMismatch("missing v2 vector".into()))?;
            let mut hash = Sha256::new();
            for value in vector {
                hash.update(value.to_le_bytes());
            }
            if <[u8; 32]>::from(hash.finalize()) != reference.sha256 {
                return Err(StorageError::OwnershipMismatch(
                    "v2 vector hash mismatch".into(),
                ));
            }
            if matches!(
                &namespace.schema.as_ref().unwrap().embedding_binding,
                crate::model::schema::EmbeddingBinding::Bound { .. }
            ) {
                super::canonical_schema::validate_bound_vector(vector)?;
            }
        }
        if record.text_offset != 0 || record.text_length != 0 {
            let retained = text.read(TextRef {
                file_offset: record.text_offset,
                length: record.text_length,
            })?;
            if let Some(expected) = certified_text.get(&id) {
                use super::vector_certification::RetainedTextInput;
                if *expected
                    != (RetainedTextInput::Present {
                        sha256: super::native_records::hash_bytes(retained.as_bytes()),
                    })
                {
                    return Err(StorageError::InvalidMutation(
                        "retained cutover text differs from actual document commitment".into(),
                    ));
                }
            }
            super::native_records::validate_retained_text(
                &record,
                native_creations.get(&id),
                &retained,
            )?;
        }
    }
    for effect in effects.iter().filter(|e| !e.canonical_applied) {
        super::metadata::effects::validate_plan(effect, &decoded)?;
    }
    let (_edges_copy, edges) = inspect_database_copy(&path.join("edges.db"), native_recovery)?;
    let read = edges.begin_read()?;
    let forward = read.open_table(FORWARD_EDGES)?;
    let reverse = read.open_table(REVERSE_EDGES)?;
    if forward.len()? != reverse.len()? {
        return Err(StorageError::CorruptIndex(
            "edge index counts disagree".into(),
        ));
    }
    let mut decoded_edges = Vec::new();
    for item in forward.iter()? {
        let (key, value) = item?;
        let edge = PersistedEdge::from_forward(key.value(), value.value())?;
        let mut reverse_key = [0u8; 33];
        reverse_key[..16].copy_from_slice(edge.target.as_bytes());
        reverse_key[16] = edge.edge_type.as_u8();
        reverse_key[17..].copy_from_slice(edge.source.as_bytes());
        // A committed native hard deletion can precede its separate edge-DB
        // replay. Admit only the exact, already validated pending deletion;
        // unrelated dangling edges and missing reverse entries still fail.
        let pending_delete = native_recovery
            && effects.iter().any(|effect| {
                let permitted_deleted = |id: &MemoryId| match &effect.receipt.outcome {
                    super::control::MutationOutcome::NativeMemoryHardDeleted { transition: t } => {
                        *id == t.memory_id
                    }
                    super::control::MutationOutcome::NativeNamespacePurgeAdvanced {
                        transition: t,
                    } => t
                        .chunk
                        .as_ref()
                        .is_some_and(|c| c.deletions.iter().any(|d| d.memory_id == *id)),
                    _ => false,
                };
                !effect.canonical_applied
                    && effect.deletes.iter().any(|deleted| {
                        super::native_records::NativeEdgeProof::from(deleted)
                            == super::native_records::NativeEdgeProof::from(&edge)
                    })
                    && [edge.source, edge.target]
                        .iter()
                        .all(|id| permitted_deleted(id) || ids.contains(id))
            });
        if ((!ids.contains(&edge.source) || !ids.contains(&edge.target)) && !pending_delete)
            || reverse.get(reverse_key.as_slice())?.is_none()
        {
            return Err(StorageError::CorruptIndex(
                "dangling or inconsistent edge".into(),
            ));
        }
        if schema_state.is_some() {
            decoded_edges.push(edge);
        }
    }
    if let (Some(state), Some(schema)) = (control.as_ref(), schema_state.as_ref()) {
        let baseline = if schema.source.store_id == state.store_id {
            schema.publication_generation
        } else {
            super::control::Generation::default().next()?
        };
        super::native_graph::validate_edge_history(
            &meta.begin_read()?,
            state,
            baseline,
            &decoded_edges,
            &effects,
        )?;
    }
    if purpose == InspectionPurpose::VectorRemedy {
        let state = control.as_ref().ok_or_else(|| {
            StorageError::InvalidControl("vector remedy requires canonical control".into())
        })?;
        let metadata_read = meta.begin_read()?;
        let (_, bindings) =
            super::vector_certification::validate_current_with_pending(&metadata_read, state)?;
        super::vector_certification::validate_pending_bindings(
            &metadata_read,
            state,
            &bindings,
            &batches,
        )?;
    }
    Ok(inventory)
}

pub(crate) fn canonical_files(path: &Path) -> Result<Vec<PathBuf>, StorageError> {
    let mut files: Vec<_> = validate_existing(path)?
        .into_iter()
        .map(|name| path.join(name))
        .collect();
    files.sort();
    Ok(files)
}
