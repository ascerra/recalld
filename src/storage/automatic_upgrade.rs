//! Automatic legacy upgrade. No serving engine escapes before durable commit.
//! Runtime must validate its configuration/provider prerequisites in `validate`.
use super::{
    RedbStorageEngine, StorageError,
    fsync::fsync_dir,
    ids::{IdSource, SystemIds},
    ownership::{StoreOwnership, lock_directory},
    schema_migration::{MigrationOptions, inspect_schema, migrate_schema},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreFormat {
    Missing,
    Legacy,
    Current,
    PendingUpgrade,
}

fn invalid(message: impl Into<String>) -> StorageError {
    StorageError::RecoveryRequired(format!("automatic upgrade: {}", message.into()))
}
pub(super) fn pending(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(".recalld-upgrade-pending");
    path.with_file_name(name)
}
/// Advisory classification; open rechecks while holding exclusive ownership.
/// Unknown/future/mixed formats are errors, never treated as empty or legacy.
pub(crate) fn classify(path: &Path) -> Result<StoreFormat, StorageError> {
    let owner = StoreOwnership::acquire_for_upgrade(path, false)?;
    if pending(&owner.path).is_symlink()
        || pending(&owner.path).try_exists()?
        || owner.pending.is_symlink()
        || owner.pending.try_exists()?
    {
        return Ok(StoreFormat::PendingUpgrade);
    }
    classify_owned(&owner.path)
}
fn classify_owned(path: &Path) -> Result<StoreFormat, StorageError> {
    if !path.try_exists()? {
        return Ok(StoreFormat::Missing);
    }
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(StorageError::InvalidPath);
    }
    if fs::read_dir(path)?.next().is_none() {
        return Ok(StoreFormat::Missing);
    }
    let (_copy, db) = super::validation::inspect_database_copy(&path.join("meta.db"), true)?;
    let read = db.begin_read()?;
    let control = super::control::validate_existing(&read)?;
    super::canonical_schema::validate_metadata_format(&read, control.as_ref())?;
    Ok(if control.is_some_and(|c| c.control_schema_version == 2) {
        StoreFormat::Current
    } else {
        StoreFormat::Legacy
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Seal {
    name: PathBuf,
    length: u64,
    hash: [u8; 32],
}
/// Includes derived files too: backup is a stopped-directory backup, not an export.
fn inventory(root: &Path) -> Result<Vec<Seal>, StorageError> {
    fn visit(root: &Path, path: &Path, out: &mut Vec<Seal>) -> Result<(), StorageError> {
        if !fs::symlink_metadata(path)?.is_dir() {
            return Err(StorageError::InvalidPath);
        }
        for entry in fs::read_dir(path)? {
            let p = entry?.path();
            let meta = fs::symlink_metadata(&p)?;
            if meta.is_dir() {
                visit(root, &p, out)?;
            } else if meta.is_file() {
                out.push(Seal {
                    name: p
                        .strip_prefix(root)
                        .map_err(|_| StorageError::InvalidPath)?
                        .into(),
                    length: meta.len(),
                    hash: super::restore_journal::hash_file(&p)?,
                });
            } else {
                return Err(invalid("backup contains symlink or special file"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
fn copy_tree(source: &Path, target: &Path) -> Result<(), StorageError> {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let source = entry?.path();
        let target = target.join(source.file_name().ok_or(StorageError::InvalidPath)?);
        let meta = fs::symlink_metadata(&source)?;
        if meta.is_dir() {
            copy_tree(&source, &target)?;
        } else if meta.is_file() {
            fs::copy(&source, &target)?;
            File::open(&target)?.sync_all()?;
        } else {
            return Err(invalid("copy contains symlink or special file"));
        }
    }
    fsync_dir(target)?;
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Phase {
    Preparing,
    BackedUp,
    Validated,
    OldMoved,
    OldRenameSynced,
    Installed,
    NewRenameSynced,
    Committed,
    RuntimeReady,
    Aborted,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    id: Uuid,
    target: PathBuf,
    phase: Phase,
    original: Vec<Seal>,
    candidate: Vec<Seal>,
}
impl Journal {
    fn root(&self) -> PathBuf {
        self.target
            .parent()
            .unwrap()
            .join(format!(".recalld-upgrade-{}", self.id))
    }
    fn backup(&self) -> PathBuf {
        self.root().join("backup")
    }
    fn stage(&self) -> PathBuf {
        self.root().join("candidate")
    }
    fn previous(&self) -> PathBuf {
        self.root().join("previous")
    }
    fn validate_paths(&self, target: &Path) -> Result<(), StorageError> {
        if self.version != 1 || self.id.is_nil() || self.target != target {
            return Err(invalid("invalid journal identity/version"));
        }
        for p in [
            self.root(),
            self.backup(),
            self.stage(),
            self.previous(),
            self.root().join("rollback"),
            self.root().join("failed-candidate"),
            self.target.clone(),
            pending(target),
        ] {
            if p.is_symlink() {
                return Err(invalid("symlink in upgrade paths"));
            }
        }
        Ok(())
    }
    fn save(&self) -> Result<(), StorageError> {
        self.validate_paths(&self.target)?;
        let parent = self.target.parent().unwrap();
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&serde_json::to_vec(self).map_err(|e| invalid(e.to_string()))?)?;
        file.as_file().sync_all()?;
        #[cfg(test)]
        save_fault(self.phase, false)?;
        file.persist(pending(&self.target))
            .map_err(|e| StorageError::Io(e.error))?;
        #[cfg(test)]
        save_fault(self.phase, true)?;
        fsync_dir(parent)?;
        Ok(())
    }
    fn clear(&self) -> Result<(), StorageError> {
        // Retain durable audit/backup metadata after removing the startup journal.
        let mut file = tempfile::NamedTempFile::new_in(self.root())?;
        file.write_all(&serde_json::to_vec(self).map_err(|e| invalid(e.to_string()))?)?;
        file.as_file().sync_all()?;
        file.persist(self.root().join("manifest.json"))
            .map_err(|e| StorageError::Io(e.error))?;
        fsync_dir(&self.root())?;
        fs::remove_file(pending(&self.target))?;
        fsync_dir(self.target.parent().unwrap())?;
        Ok(())
    }
}
fn read_journal(target: &Path) -> Result<Option<Journal>, StorageError> {
    let path = pending(target);
    if path.is_symlink() {
        return Err(invalid("symlink journal"));
    }
    if !path.try_exists()? {
        return Ok(None);
    }
    if fs::metadata(&path)?.len() > 16 * 1024 * 1024 {
        return Err(invalid("oversized journal"));
    }
    let j: Journal =
        serde_json::from_slice(&fs::read(path)?).map_err(|e| invalid(e.to_string()))?;
    j.validate_paths(target)?;
    Ok(Some(j))
}

/// A committed container layout is not evidence that a missing child is fresh.
/// Reconcile only an existing target-bound downstream journal, under the same
/// stable owner used by ordinary startup, without classification/bootstrap.
pub(super) fn recover_missing_container_child(path: &Path) -> Result<(), StorageError> {
    let owner = StoreOwnership::acquire_for_upgrade(path, false)?;
    if owner.path.try_exists()? {
        return Err(invalid(
            "container child appeared during recovery admission; retry startup",
        ));
    }
    if let Some(j) = read_journal(&owner.path)? {
        if j.phase == Phase::Committed {
            return Err(invalid(
                "committed upgrade child is missing; refusing rollback or bootstrap",
            ));
        }
        return recover(&owner);
    }
    if owner.pending.is_symlink() || !owner.pending.try_exists()? {
        return Err(invalid(
            "container child missing without a recoverable journal",
        ));
    }
    // This validates target, lineage and file seals before restoring/publishing.
    super::restore_journal::recover(&owner)?;
    if !owner.path.is_dir() {
        return Err(invalid("recovery did not restore container child"));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn container_cutover_crash(path: &Path) {
    let _ = prepare_at(
        path,
        1_800_000_000_000,
        Box::new(SystemIds),
        |_| Ok(()),
        |phase| {
            if phase == Phase::OldRenameSynced {
                std::process::exit(77);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("cutover boundary not reached");
}
/// Only uncommitted work can roll back. A committed store may have new user writes;
/// never compare it with the pre-write candidate seal or replace it with the backup.
fn recover(owner: &StoreOwnership) -> Result<(), StorageError> {
    let Some(j) = read_journal(&owner.path)? else {
        return Ok(());
    };
    let _target_lock = if owner.path.is_dir() {
        Some(lock_directory(&owner.path)?)
    } else {
        None
    };
    let _previous_lock = if j.previous().is_dir() {
        Some(lock_directory(&j.previous())?)
    } else {
        None
    };
    if j.phase == Phase::Committed {
        j.clear()?;
        return Ok(());
    }
    rollback(&j)?;
    Err(invalid(format!(
        "interrupted upgrade rolled back; startup stopped; {} at {}",
        if j.phase == Phase::Preparing {
            "incomplete backup artifacts retained"
        } else {
            "backup retained"
        },
        if j.phase == Phase::Preparing {
            j.root()
        } else {
            j.backup()
        }
        .display()
    )))
}
fn rollback(j: &Journal) -> Result<(), StorageError> {
    j.validate_paths(&j.target)?;
    // Before cutover the untouched original is itself the recovery source, even
    // when ENOSPC interrupted the backup. Do not require a complete partial copy.
    if j.target.try_exists()? && inventory(&j.target)? == j.original {
        let mut done = j.clone();
        done.phase = Phase::Aborted;
        return done.clear();
    }
    if j.phase == Phase::Preparing {
        return Err(invalid("original changed during incomplete backup"));
    }
    if !j.previous().try_exists()? {
        return Err(invalid(
            "original changed outside cutover; refusing to overwrite unknown writes",
        ));
    }
    if inventory(&j.backup())? != j.original {
        return Err(invalid(
            "backup seal mismatch; refusing destructive recovery",
        ));
    }
    let restore = j.root().join("rollback");
    if restore.try_exists()? {
        if inventory(&restore)? != j.original {
            fs::remove_dir_all(&restore)?;
            copy_tree(&j.backup(), &restore)?;
        }
    } else {
        copy_tree(&j.backup(), &restore)?;
    }
    if inventory(&restore)? != j.original {
        return Err(invalid("rollback copy mismatch"));
    }
    // Retain failed candidate evidence; never unlink an unrecognized directory.
    if j.target.try_exists()? {
        let failed = j.root().join("failed-candidate");
        if failed.try_exists()? {
            return Err(invalid("ambiguous rollback target"));
        }
        fs::rename(&j.target, &failed)?;
        fsync_dir(j.target.parent().unwrap())?;
        fsync_dir(&j.root())?;
    }
    fs::rename(&restore, &j.target)?;
    fsync_dir(j.target.parent().unwrap())?;
    fsync_dir(&j.root())?;
    let mut done = j.clone();
    done.phase = Phase::Aborted;
    done.clear()
}

/// Owns the startup transaction independently of runtime engine handles.
/// Dropping this guard is NOT commit or rollback: its durable journal remains.
/// Runtime must keep all listeners/writers stopped until `commit` succeeds.
pub(crate) struct UpgradeTransaction {
    owner: std::sync::Arc<StoreOwnership>,
    journal: Option<Journal>,
    _old_lock: Option<super::ownership::OwnedFileLock>,
}
impl UpgradeTransaction {
    pub(crate) fn backup_path(&self) -> Option<PathBuf> {
        self.journal.as_ref().map(Journal::backup)
    }
    pub(crate) fn commit(self) -> Result<(), StorageError> {
        self.commit_at(|_| Ok(()))
    }
    fn commit_at(
        mut self,
        mut boundary: impl FnMut(Phase) -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        let Some(j) = self.journal.as_mut() else {
            return Ok(());
        };
        boundary(Phase::RuntimeReady)?;
        j.phase = Phase::Committed;
        if let Err(error) = j.save() {
            // persist/parent-fsync may have failed after the atomic replacement.
            // Reconcile the actual marker, then explicitly establish durability.
            // If uncertain, leave it for startup recovery; NEVER blind rollback.
            let durable = read_journal(&self.owner.path)?;
            if !durable.is_some_and(|saved| saved.id == j.id && saved.phase == Phase::Committed) {
                return Err(invalid(format!(
                    "commit failed; recovery required, backup {}: {error}",
                    j.backup().display()
                )));
            }
            fsync_dir(j.target.parent().unwrap()).map_err(|e| {
                invalid(format!(
                    "commit durability uncertain; backup {}; startup recovery required: {e}",
                    j.backup().display()
                ))
            })?;
        }
        boundary(Phase::Committed)?;
        // Cleanup is not the commit point. Once committed, later user writes
        // must survive even if removing the pending journal fails.
        if let Err(error) = j.clear() {
            tracing::warn!(%error, backup = %j.backup().display(), "upgrade committed; journal cleanup deferred");
        }
        Ok(())
    }
    /// Caller must first drop the entire quiescent runtime, including index and
    /// storage handles. A retained worker prevents rollback instead of racing it.
    pub(crate) fn abort(self) -> Result<(), StorageError> {
        let Some(j) = &self.journal else {
            return Ok(());
        };
        if std::sync::Arc::strong_count(&self.owner) != 1 {
            return Err(StorageError::DatabaseLocked);
        }
        let _lock = lock_directory(&self.owner.path)?;
        let saved = read_journal(&self.owner.path)?
            .ok_or_else(|| invalid("upgrade journal disappeared before abort"))?;
        if saved.id != j.id || saved.phase == Phase::Committed {
            return Err(invalid("cannot abort changed or committed upgrade"));
        }
        rollback(&saved)
    }
}

/// Prepare an uncommitted final-location engine. Structural stage validation is
/// not runtime readiness: the caller MUST initialize quiescently, then commit.
pub(crate) fn prepare_upgrade(
    path: &Path,
    now: i64,
    ids: Box<dyn IdSource>,
) -> Result<(RedbStorageEngine, UpgradeTransaction), StorageError> {
    prepare_at(path, now, ids, |_| Ok(()), |_| Ok(()))
}
fn prepare_at(
    path: &Path,
    now: i64,
    ids: Box<dyn IdSource>,
    validate: impl Fn(&RedbStorageEngine) -> Result<(), StorageError>,
    mut boundary: impl FnMut(Phase) -> Result<(), StorageError>,
) -> Result<(RedbStorageEngine, UpgradeTransaction), StorageError> {
    let owner = std::sync::Arc::new(StoreOwnership::acquire_for_upgrade(path, true)?);
    recover(&owner)?;
    match classify_owned(&owner.path)? {
        StoreFormat::Missing => return Err(invalid("missing store requires native bootstrap")),
        StoreFormat::Current => {
            let engine = RedbStorageEngine::open_shared_owned_with_ids(owner.clone(), true, ids)?;
            validate(&engine)?;
            return Ok((
                engine,
                UpgradeTransaction {
                    owner,
                    journal: None,
                    _old_lock: None,
                },
            ));
        }
        StoreFormat::Legacy => {}
        StoreFormat::PendingUpgrade => unreachable!("owned classification excludes journals"),
    }
    let _old_lock = lock_directory(&owner.path)?;
    let mut j = Journal {
        version: 1,
        id: Uuid::new_v4(),
        target: owner.path.clone(),
        phase: Phase::Preparing,
        original: inventory(&owner.path)?,
        candidate: vec![],
    };
    fs::create_dir(j.root())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(j.root(), fs::Permissions::from_mode(0o700))?;
    }
    fsync_dir(j.target.parent().unwrap())?;
    let prepared = (|| {
        j.save()?;
        boundary(Phase::Preparing)?;
        copy_tree(&j.target, &j.backup())?;
        if inventory(&j.backup())? != j.original {
            return Err(invalid("backup verification mismatch"));
        }
        fsync_dir(&j.root())?;
        j.phase = Phase::BackedUp;
        j.save()?;
        boundary(j.phase)?;
        let working = j.root().join("working");
        copy_tree(&j.backup(), &working)?;
        // Recover only the disposable working copy. The backup is never opened
        // writable, including redb's automatic bookkeeping/recovery writes.
        {
            let engine = RedbStorageEngine::open(&working)?;
            drop(engine);
        }
        let selected = {
            let (_copy, db) = super::validation::inspect_database(&working.join("meta.db"))?;
            super::metadata::text_generation::read_selection(&db.begin_read()?)?.is_some()
        };
        let input = if selected {
            working
        } else {
            let text = j.root().join("text");
            super::text_migration::migrate_legacy_text(&working, &text)?;
            text
        };
        let inspection = inspect_schema(&input)?;
        migrate_schema(
            &input,
            &j.stage(),
            &MigrationOptions {
                migration_now: now,
                expected_source: inspection.control.generation_ref(),
                attestations: vec![],
            },
        )?;
        {
            let engine = RedbStorageEngine::open_native(&j.stage())?;
            validate(&engine)?;
        }
        j.candidate = inventory(&j.stage())?;
        if inventory(&j.backup())? != j.original || inventory(&j.target)? != j.original {
            return Err(invalid("source or backup changed during preparation"));
        }
        j.phase = Phase::Validated;
        j.save()?;
        boundary(j.phase)?;
        fs::rename(&j.target, j.previous())?;
        fsync_dir(j.target.parent().unwrap())?;
        fsync_dir(&j.root())?;
        boundary(Phase::OldRenameSynced)?;
        j.phase = Phase::OldMoved;
        j.save()?;
        boundary(j.phase)?;
        fs::rename(j.stage(), &j.target)?;
        fsync_dir(j.target.parent().unwrap())?;
        fsync_dir(&j.root())?;
        boundary(Phase::NewRenameSynced)?;
        j.phase = Phase::Installed;
        j.save()?;
        boundary(j.phase)?;
        if inventory(&j.target)? != j.candidate {
            return Err(invalid("installed candidate seal mismatch"));
        }
        Ok(())
    })();
    if let Err(error) = prepared {
        rollback(&j)?;
        return Err(invalid(format!(
            "{error}; original unchanged/restored; upgrade artifacts retained at {}",
            j.root().display()
        )));
    }
    match RedbStorageEngine::open_shared_owned_with_ids(owner.clone(), true, ids) {
        Ok(engine) => Ok((
            engine,
            UpgradeTransaction {
                owner,
                journal: Some(j),
                _old_lock: Some(_old_lock),
            },
        )),
        Err(error) => {
            rollback(&j)?;
            Err(error)
        }
    }
}

#[cfg(test)]
thread_local! {
    static SAVE_FAULT: std::cell::RefCell<Option<(Phase,bool)>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
fn save_fault(phase: Phase, after_replace: bool) -> Result<(), StorageError> {
    SAVE_FAULT.with(|fault| {
        if *fault.borrow() == Some((phase, after_replace)) {
            fault.borrow_mut().take();
            Err(StorageError::Io(std::io::Error::other(
                "injected journal I/O failure",
            )))
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
#[path = "automatic_upgrade_tests.rs"]
mod tests;
