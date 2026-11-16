//! Format selection uses the existing validators and retains both owner locks.
use super::*;
use crate::model::schema::ProviderKind;
use crate::storage::{control, ownership, schema_migration, validation};
use redb::ReadableTable;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn detected(path: &Path) -> Result<RedbStorageEngine, StorageError> {
    RedbStorageEngine::open_detected_with_ids(path, Box::new(crate::storage::ids::SystemIds))
}

fn native_fixture(root: &Path) -> PathBuf {
    let source = root.join("legacy");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let mut config = NamespaceConfig::default_namespace(1000);
    config.embedding_dim = 4;
    config.decay_rate_multiplier = Some(1.);
    let namespace_id = engine.create_namespace(&config).unwrap();
    engine.sync().unwrap();
    drop(engine);
    let destination = root.join("native");
    schema_migration::migrate_schema(
        &source,
        &destination,
        &schema_migration::MigrationOptions {
            expected_source: schema_migration::inspect_schema(&source)
                .unwrap()
                .control
                .generation_ref(),
            migration_now: 2000,
            attestations: vec![schema_migration::ProfileAttestationInput {
                namespace_id,
                namespace_name: "default".into(),
                descriptor: crate::embedding::runtime::native_profile(
                    ProviderKind::Ollama,
                    "format-open-fixture",
                    4,
                ),
            }],
        },
    )
    .unwrap();
    destination
}

fn files(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else if path.file_name().unwrap() != "recalld.lock" {
                out.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    Sha256::digest(std::fs::read(path).unwrap()).to_vec(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(path, path, &mut out);
    out
}

struct Probe;
impl Drop for Probe {
    fn drop(&mut self) {
        validation::NATIVE_PREFLIGHT_PROBE.with(|probe| {
            probe.borrow_mut().take();
        });
    }
}
fn probe(path: &Path) -> (Probe, Arc<AtomicUsize>) {
    let before = files(path);
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    validation::NATIVE_PREFLIGHT_PROBE.with(|probe| {
        assert!(probe.borrow().is_none());
        *probe.borrow_mut() = Some(Box::new(move |path| {
            assert!(matches!(
                ownership::StoreOwnership::acquire(path),
                Err(StorageError::DatabaseLocked)
            ));
            assert!(ownership::lock_directory(path).is_err());
            assert_eq!(files(path), before, "source changed before writable open");
            observed.fetch_add(1, Ordering::SeqCst);
        }));
    });
    (Probe, count)
}

#[test]
fn detected_format_keeps_fresh_and_existing_legacy_defaults() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("store");
    let engine = detected(&path).unwrap();
    let before = engine.control_state().unwrap();
    assert_eq!(before.control_schema_version, 1);
    drop(engine);
    let engine = detected(&path).unwrap();
    assert_eq!(engine.control_state().unwrap(), before);
    drop(engine);
    assert_eq!(
        RedbStorageEngine::open(&path)
            .unwrap()
            .control_state()
            .unwrap(),
        before
    );
}

#[test]
fn detected_native_runs_strict_preflight_with_same_owners_without_opening_ordinary_gate() {
    let root = tempfile::tempdir().unwrap();
    let path = native_fixture(root.path());
    let before = schema_migration::inspect_schema(&path).unwrap();
    let hashes = files(&path);
    assert!(
        RedbStorageEngine::open(&path).is_err(),
        "ordinary legacy entry remains gated"
    );
    assert_eq!(files(&path), hashes);
    let (_probe, count) = probe(&path);
    let engine = detected(&path).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(engine.control_state().unwrap(), before.control);
    assert_eq!(
        serde_json::to_value(engine.list_namespaces().unwrap()).unwrap(),
        serde_json::to_value(before.namespaces).unwrap()
    );
    assert!(matches!(
        ownership::StoreOwnership::acquire(&path),
        Err(StorageError::DatabaseLocked)
    ));
    assert!(ownership::lock_directory(&path).is_err());
    drop(engine);
    assert!(ownership::StoreOwnership::acquire(&path).is_ok());
    assert!(ownership::lock_directory(&path).is_ok());
}

#[test]
fn detected_format_rejects_unknown_partial_and_malformed_schema_without_source_changes() {
    for fault in ["version", "partial", "schema"] {
        let root = tempfile::tempdir().unwrap();
        let path = native_fixture(root.path());
        {
            let db = redb::Database::open(path.join("meta.db")).unwrap();
            let tx = db.begin_write().unwrap();
            match fault {
                "version" => {
                    let mut table = tx.open_table(control::CONTROL).unwrap();
                    let mut value: serde_json::Value =
                        serde_json::from_slice(table.get("state").unwrap().unwrap().value())
                            .unwrap();
                    value["controlSchemaVersion"] = serde_json::json!(99);
                    table
                        .insert("state", serde_json::to_vec(&value).unwrap().as_slice())
                        .unwrap();
                }
                "partial" => {
                    tx.delete_table(control::KEYS).unwrap();
                }
                "schema" => {
                    tx.open_table(crate::storage::canonical_schema::SCHEMA)
                        .unwrap()
                        .insert("state", b"{}".as_slice())
                        .unwrap();
                }
                _ => unreachable!(),
            }
            tx.commit().unwrap();
        }
        let before = files(&path);
        let (_probe, count) = probe(&path);
        assert!(detected(&path).is_err(), "{fault}");
        assert_eq!(files(&path), before, "{fault}");
        assert_eq!(count.load(Ordering::SeqCst), usize::from(fault == "schema"));
        assert!(ownership::StoreOwnership::acquire(&path).is_ok());
        assert!(ownership::lock_directory(&path).is_ok());
    }
}

#[test]
fn detected_nonempty_root_cannot_reinitialize_missing_metadata() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("retained-payload"), b"do not initialize").unwrap();
    let before = files(root.path());
    assert!(detected(root.path()).is_err());
    assert_eq!(files(root.path()), before);
}
