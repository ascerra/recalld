//! Retained-freeze canonical staging. An unpublished stage is not an A6 snapshot.
use super::*;
use crate::storage::control::GenerationRef;
use crate::storage::metadata::MetadataFreeze;
use std::io::{Read, Write};
use std::time::Instant;

pub(crate) struct StagedCanonical {
    pub(crate) directory: tempfile::TempDir,
    pub(crate) generation: GenerationRef,
    pub(crate) logical_sha256: [u8; 32],
}

impl StagedCanonical {
    pub(crate) fn path(&self) -> PathBuf {
        self.directory.path().join("canonical")
    }
}

fn check(deadline: Instant) -> Result<(), StorageError> {
    if Instant::now() >= deadline {
        return Err(StorageError::RecoveryRequired(
            "canonical capture deadline expired".into(),
        ));
    }
    Ok(())
}

fn copy_file(source: &Path, target: &Path, deadline: Instant) -> Result<(), StorageError> {
    crate::storage::validation::require_file(source)?;
    let mut input = std::fs::File::open(source)?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    let mut buffer = [0; 64 * 1024];
    loop {
        check(deadline)?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n])?;
    }
    output.sync_all()?;
    check(deadline)
}

impl RedbStorageEngine {
    /// Caller retains the actual engine guard and this owner's metadata freeze.
    /// Copies only canonical inventory selected by metadata, never a directory
    /// walk. All original ownership locks and view leases remain with the caller.
    /// Cooperative byte-loop checks do not bound OS I/O or redb integrity scans.
    pub(crate) fn stage_frozen_canonical(
        &self,
        frozen: &MetadataFreeze<'_>,
        deadline: Instant,
    ) -> Result<StagedCanonical, StorageError> {
        check(deadline)?;
        self.meta_store
            .frozen_publication_state(frozen)?
            .ready_report()?;
        let namespaces = self.meta_store.list_namespaces()?;
        let selected = self.meta_store.text_selection()?;
        // Commits make redb durable; payload synchronization is separate and is
        // never treated as derived readiness or an admission barrier.
        self.text_store.sync_all()?;
        for (_, store) in self.vector_manager.iter() {
            check(deadline)?;
            store.sync()?;
        }
        let directory = tempfile::Builder::new()
            .prefix("recalld-frozen-canonical-")
            .tempdir_in(self.db_path.parent().ok_or(StorageError::InvalidPath)?)?;
        // Keep OfflineSnapshot's sibling owner lock inside this private stage,
        // so timeout/error cleanup removes the entire disposable capture.
        let stage = directory.path().join("canonical");
        std::fs::create_dir(&stage)?;
        for name in ["meta.db", "edges.db"] {
            check(deadline)?;
            // A live owned redb file can have an open/unclean header. Recover and
            // inspect only its disposable copy, then close it for a clean file.
            // Ordinary offline inspection still aborts repair of its inputs.
            let (copy, database) =
                crate::storage::validation::inspect_database_copy(&self.db_path.join(name), true)?;
            drop(database);
            copy_file(&copy.path().join("inspect.db"), &stage.join(name), deadline)?;
        }
        copy_file(
            &self.db_path.join("fulltext.dat"),
            &stage.join("fulltext.dat"),
            deadline,
        )?;
        if let Some(selected) = selected {
            let path = Path::new(&selected.filename);
            if path.components().count() != 1
                || !matches!(path.components().next(), Some(Component::Normal(_)))
            {
                return Err(StorageError::InvalidPath);
            }
            copy_file(&self.db_path.join(path), &stage.join(path), deadline)?;
        }
        for ns in namespaces {
            check(deadline)?;
            validate_namespace_name(&ns.name).map_err(|_| StorageError::InvalidPath)?;
            let path = Path::new(&ns.name).join("vectors.dat");
            std::fs::create_dir(stage.join(&ns.name))?;
            // The vector file includes its actual ownership/free bitmap.
            copy_file(&self.db_path.join(&path), &stage.join(path), deadline)?;
        }
        // Strict whole-copy validation, including typed receipts, retained
        // payloads, selected text, ledger and pending manifests. No promotion of
        // unchecked copied files and no repair of the original source.
        let snapshot = crate::storage::offline_snapshot::OfflineSnapshot::open(&stage)?;
        let generation = snapshot.control().generation_ref();
        if generation != frozen.generation() {
            return Err(StorageError::RecoveryRequired(
                "staged canonical generation mismatch".into(),
            ));
        }
        let logical_sha256 = match snapshot.control().control_schema_version {
            1 => crate::storage::logical_export::isolated_legacy_hash(&snapshot, deadline)?,
            2 => crate::storage::logical_export::isolated_native_hash(&snapshot, deadline)?,
            _ => {
                return Err(StorageError::RecoveryRequired(
                    "unsupported checkpoint schema".into(),
                ));
            }
        };
        drop(snapshot);
        check(deadline)?;
        self.meta_store
            .frozen_publication_state(frozen)?
            .ready_report()?;
        Ok(StagedCanonical {
            directory,
            generation,
            logical_sha256,
        })
    }
}
