//! Opt-in container volume layout. Layout commit precedes schema upgrade:
//! after it, schema rollback restores the original database at `root/.recalld-store`.
//! Never rename the mount root or place recovery evidence outside that volume.
use super::{
    StorageError,
    fsync::fsync_dir,
    ownership::{OwnedFileLock, lock_directory},
};
use redb::ReadableTable;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

const JOURNAL: &str = ".recalld-container-layout.json";
const STORE: &str = ".recalld-store";

/// Shared ownership fence for explicit offline paths too. A pending container
/// layout may not be treated as an empty root, child, or owned staging store.
pub(super) fn check_store_path(path: &Path) -> Result<(), StorageError> {
    if read(path)?.is_some() {
        return Err(invalid(
            "volume root is a container layout, not a store; use its committed child",
        ));
    }
    if let Some(parent) = path.parent() {
        if let Some(j) = read(parent)? {
            if path == parent.join(STORE) || path == stage(parent, &j) {
                if !j.ready || path != parent.join(STORE) {
                    return Err(invalid(
                        "pending or staging container layout is not an offline store",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Keep this lease alive through serving (or the offline operation). The old
/// root lock excludes binaries which know nothing about the new child layout.
pub struct ContainerLayout {
    path: PathBuf,
    _lock: OwnedFileLock,
}
impl ContainerLayout {
    pub fn store_path(&self) -> &Path {
        &self.path
    }
}

/// Explicit container opt-in must never silently replace a custom data path.
pub fn configured_layout(
    root: &Path,
    configured: &Path,
    serving: bool,
) -> Result<ContainerLayout, StorageError> {
    if root.as_os_str().is_empty()
        || !root.is_absolute()
        || (configured != root && configured != root.join(STORE))
    {
        return Err(invalid(
            "container root requires configured data directory root or root/.recalld-store",
        ));
    }
    if serving {
        prepare_container_layout(root)
    } else {
        resolve_container_layout(root)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    root: PathBuf,
    id: Uuid,
    ready: bool,
    files: Vec<Seal>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Seal {
    name: PathBuf,
    length: u64,
    hash: [u8; 32],
}

fn invalid(message: impl std::fmt::Display) -> StorageError {
    StorageError::RecoveryRequired(format!("container layout: {message}"))
}
fn exists(path: &Path) -> Result<bool, StorageError> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_symlink() => Err(invalid(format!("symlink: {}", path.display()))),
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
fn directory(path: &Path) -> Result<(), StorageError> {
    if !exists(path)? || !fs::symlink_metadata(path)?.is_dir() {
        return Err(invalid(format!("not a directory: {}", path.display())));
    }
    Ok(())
}
fn stage(root: &Path, j: &Journal) -> PathBuf {
    root.join(format!(".recalld-container-stage-{}", j.id))
}
fn save(root: &Path, j: &Journal) -> Result<(), StorageError> {
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    file.write_all(&serde_json::to_vec(j).map_err(invalid)?)?;
    file.as_file().sync_all()?;
    file.persist(root.join(JOURNAL))
        .map_err(|e| StorageError::Io(e.error))?;
    fsync_dir(root)?;
    Ok(())
}
fn read(root: &Path) -> Result<Option<Journal>, StorageError> {
    if !exists(&root.join(JOURNAL))? {
        return Ok(None);
    }
    let j: Journal = serde_json::from_slice(&fs::read(root.join(JOURNAL))?).map_err(invalid)?;
    let mut names = std::collections::HashSet::new();
    if j.version != 1 || j.root != root || j.id.is_nil() {
        return Err(invalid("invalid journal identity"));
    }
    for s in &j.files {
        if s.name.components().count() == 0
            || s.name.components().count() > 2
            || s.name
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || s.name.starts_with(STORE)
            || s.name.starts_with(JOURNAL)
            || !names.insert(&s.name)
        {
            return Err(invalid("invalid journal file path"));
        }
        let valid = match s.name.components().count() {
            1 => {
                s.name == Path::new("meta.db")
                    || s.name == Path::new("edges.db")
                    || s.name == Path::new("fulltext.dat")
                    || super::metadata::text_generation::is_generation_filename(
                        s.name.to_str().unwrap_or(""),
                    )
            }
            2 => {
                s.name.file_name().is_some_and(|n| n == "vectors.dat")
                    && crate::model::validation::validate_namespace_name(
                        s.name.parent().unwrap().to_str().unwrap_or(""),
                    )
                    .is_ok()
            }
            _ => false,
        };
        if !valid {
            return Err(invalid("journal contains noncanonical path"));
        }
    }
    Ok(Some(j))
}
fn matches(path: &Path, seal: &Seal) -> Result<(), StorageError> {
    // Check parents too: a namespace directory must never redirect recovery.
    directory(path.parent().ok_or(StorageError::InvalidPath)?)?;
    if !exists(path)?
        || !fs::symlink_metadata(path)?.is_file()
        || fs::metadata(path)?.len() != seal.length
        || super::restore_journal::hash_file(path)? != seal.hash
    {
        return Err(invalid(format!(
            "file changed or missing: {}",
            path.display()
        )));
    }
    Ok(())
}

fn verify_candidate(root: &Path, j: &Journal) -> Result<(), StorageError> {
    let mut expected: std::collections::BTreeSet<PathBuf> =
        j.files.iter().map(|s| s.name.clone()).collect();
    for s in &j.files {
        matches(&root.join(&s.name), s)?;
        if let Some(parent) = s.name.parent().filter(|p| !p.as_os_str().is_empty()) {
            expected.insert(parent.into());
        }
    }
    fn visit(
        base: &Path,
        dir: &Path,
        expected: &mut std::collections::BTreeSet<PathBuf>,
    ) -> Result<(), StorageError> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if !expected.remove(path.strip_prefix(base).map_err(invalid)?) {
                return Err(invalid("unowned candidate entry"));
            }
            if !exists(&path)? {
                return Err(invalid("candidate disappeared"));
            }
            if path.is_dir() {
                visit(base, &path, expected)?;
            }
        }
        Ok(())
    }
    visit(root, root, &mut expected)?;
    if !expected.is_empty() {
        return Err(invalid("incomplete candidate"));
    }
    Ok(())
}

/// Select only canonical files named by the metadata, never arbitrary volume
/// contents or whole namespace directories containing user files.
fn inventory(root: &Path) -> Result<Vec<Seal>, StorageError> {
    if exists(&root.join(".recalld-importing"))? {
        return Err(invalid(
            "incomplete logical import must be recovered before relocation",
        ));
    }
    super::text::recover_text_compaction(root)?;
    for name in ["meta.db", "edges.db", "fulltext.dat"] {
        super::validation::require_file(&root.join(name))?;
    }
    let paths = canonical_names(&root.join("meta.db"))?;
    paths
        .into_iter()
        .map(|name| {
            let path = root.join(&name);
            directory(path.parent().unwrap())?;
            super::validation::require_file(&path)?;
            File::open(&path)?.sync_all()?;
            Ok(Seal {
                name,
                length: fs::metadata(&path)?.len(),
                hash: super::restore_journal::hash_file(&path)?,
            })
        })
        .collect()
}

fn canonical_names(metadata: &Path) -> Result<Vec<PathBuf>, StorageError> {
    let (_copy, db) = super::validation::inspect_database_copy(metadata, true)?;
    let tx = db.begin_read()?;
    let control = super::control::validate_existing(&tx)?;
    super::canonical_schema::validate_metadata_format(&tx, control.as_ref())?;
    let mut paths = vec![
        PathBuf::from("meta.db"),
        "edges.db".into(),
        "fulltext.dat".into(),
    ];
    for row in tx.open_table(super::metadata::NAMESPACE_TABLE)?.iter()? {
        let (_, value) = row?;
        let ns = crate::model::NamespaceConfig::decode(value.value())?;
        crate::model::validation::validate_namespace_name(&ns.name).map_err(invalid)?;
        if ns.name == STORE {
            return Err(invalid("root namespace collides with store directory"));
        }
        paths.push(PathBuf::from(ns.name).join("vectors.dat"));
    }
    if let Some(text) = super::metadata::text_generation::read_selection(&tx)? {
        paths.push(text.filename.into());
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}
fn root_has_canonical_files(root: &Path) -> Result<bool, StorageError> {
    for name in [
        "meta.db",
        "edges.db",
        "fulltext.dat",
        ".recalld-importing",
        ".compacting",
        ".compaction-meta-committed",
        "fulltext.dat.new",
    ] {
        if exists(&root.join(name))? {
            return Ok(true);
        }
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if super::metadata::text_generation::is_generation_filename(
            &entry.file_name().to_string_lossy(),
        ) {
            return Ok(true);
        }
        if entry.file_type()?.is_dir()
            && entry.file_name() != STORE
            && exists(&entry.path().join("vectors.dat"))?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Complete/resume relocation before canonical startup. Errors leave the journal
/// and original bytes in their recorded root/staging locations; restart resumes.
/// No provider calls, schema mutations, or accepted writes happen here.
pub fn prepare_container_layout(root: &Path) -> Result<ContainerLayout, StorageError> {
    prepare_at(root, |_| Ok(()))
}
fn prepare_at(
    root: &Path,
    mut boundary: impl FnMut(&str) -> Result<(), StorageError>,
) -> Result<ContainerLayout, StorageError> {
    directory(root)?;
    let root = root.canonicalize()?;
    let lock = lock_directory(&root)?;
    let target = root.join(STORE);
    let mut j = match read(&root)? {
        Some(j) => j,
        None => {
            if exists(&target)? {
                return Err(invalid("unowned store directory collision"));
            }
            // An orphan stage without its journal is not proof of an empty store.
            for entry in fs::read_dir(&root)? {
                if entry?
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".recalld-container-stage-")
                {
                    return Err(invalid("stage exists without journal"));
                }
            }
            let files = if exists(&root.join("meta.db"))? {
                inventory(&root)?
            } else if root_has_canonical_files(&root)? {
                return Err(invalid("partial root store without metadata"));
            } else {
                Vec::new()
            };
            let j = Journal {
                version: 1,
                root: root.clone(),
                id: Uuid::new_v4(),
                ready: false,
                files,
            };
            save(&root, &j)?;
            boundary("journal")?;
            j
        }
    };
    let staging = stage(&root, &j);
    if j.ready {
        if exists(&root.join("meta.db"))? || exists(&staging)? {
            return Err(invalid(
                "committed layout has conflicting root or staging data",
            ));
        }
        if !exists(&target)? {
            // The schema/restore transaction can temporarily remove the child.
            // Only its authenticated recovery may restore it; never bootstrap.
            super::automatic_upgrade::recover_missing_container_child(&target)?;
        }
        directory(&target)?;
        return Ok(ContainerLayout {
            path: target,
            _lock: lock,
        });
    }
    let _child_owner = super::ownership::StoreOwnership::acquire_for_layout(&target)?;
    // A journal is recovery evidence, not permission to move arbitrary files
    // or omit canonical data. Reconstruct its exact file set from the sealed
    // metadata wherever the interrupted per-file move left that metadata.
    if j.files.is_empty() {
        if root_has_canonical_files(&root)? {
            return Err(invalid(
                "empty relocation journal conflicts with canonical data",
            ));
        }
    } else {
        let metadata = j
            .files
            .iter()
            .find(|s| s.name == Path::new("meta.db"))
            .ok_or_else(|| invalid("journal omits metadata"))?;
        let mut locations = Vec::new();
        for base in [&root, &staging, &target] {
            if exists(&base.join("meta.db"))? {
                locations.push(base.join("meta.db"));
            }
        }
        if locations.len() != 1 {
            return Err(invalid("ambiguous metadata location"));
        }
        matches(&locations[0], metadata)?;
        let expected = canonical_names(&locations[0])?;
        let mut recorded: Vec<_> = j.files.iter().map(|s| s.name.clone()).collect();
        recorded.sort();
        if recorded != expected {
            return Err(invalid("journal inventory differs from canonical metadata"));
        }
    }
    if exists(&target)? {
        // Rename may have completed before the ready marker was durable. Verify
        // the exact pre-serving candidate; never compare seals after ready.
        if exists(&staging)? {
            return Err(invalid("both target and stage exist"));
        }
        directory(&target)?;
        for seal in &j.files {
            if exists(&root.join(&seal.name))? {
                return Err(invalid("source and installed file both exist"));
            }
        }
        verify_candidate(&target, &j)?;
    } else {
        if !exists(&staging)? {
            fs::create_dir(&staging)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
            }
            fsync_dir(&root)?;
        }
        directory(&staging)?;
        boundary("stage")?;
        for seal in &j.files {
            let source = root.join(&seal.name);
            let destination = staging.join(&seal.name);
            let parent = destination.parent().unwrap();
            if !exists(parent)? {
                fs::create_dir(parent)?;
                fsync_dir(&staging)?;
            }
            directory(parent)?;
            if exists(&source)? {
                if exists(&destination)? {
                    return Err(invalid("source and staged file both exist"));
                }
                matches(&source, seal)?;
                fs::rename(&source, &destination)?;
                fsync_dir(source.parent().unwrap())?;
                fsync_dir(parent)?;
            } else {
                matches(&destination, seal)?;
            }
            boundary("file")?;
        }
        verify_candidate(&staging, &j)?;
        fs::rename(&staging, &target)?;
        fsync_dir(&root)?;
        boundary("installed")?;
    }
    j.ready = true;
    save(&root, &j)?;
    boundary("ready")?;
    Ok(ContainerLayout {
        path: target,
        _lock: lock,
    })
}

/// Offline commands may use only a committed layout. They never perform moves
/// or recover pending relocation. Explicit source/destination paths are separate.
pub fn resolve_container_layout(root: &Path) -> Result<ContainerLayout, StorageError> {
    directory(root)?;
    let root = root.canonicalize()?;
    let lock = lock_directory(&root)?;
    let j = read(&root)?.ok_or_else(|| {
        invalid("layout not initialized; start runtime first or use an explicit offline path")
    })?;
    if !j.ready {
        return Err(invalid("pending relocation; start runtime to recover"));
    }
    let target = root.join(STORE);
    directory(&target)?;
    if exists(&root.join("meta.db"))? || exists(&stage(&root, &j))? {
        return Err(invalid("conflicting layout data"));
    }
    Ok(ContainerLayout {
        path: target,
        _lock: lock,
    })
}

#[cfg(test)]
#[path = "container_layout_tests.rs"]
mod tests;
