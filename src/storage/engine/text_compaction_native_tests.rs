// BEFORE probes run unchanged on the actual PR138 runtime. The guard-only test
// is explicitly a fixture bypass, not a supported native mutation path.
fn native_compaction_fixture(root: &Path) -> (RedbStorageEngine, Vec<(MemoryId, String)>) {
    use crate::storage::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
    let source = root.join("legacy");
    let (mut engine, kept) = generation_fixture(&source);
    engine.sync().unwrap();
    drop(engine);
    let destination = root.join("native");
    migrate_schema(&source, &destination, &MigrationOptions {
        migration_now: 2000,
        expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
        attestations: vec![],
    }).unwrap();
    (RedbStorageEngine::open_with_ids(&destination, true,
        Box::new(crate::storage::ids::SystemIds)).unwrap(), kept)
}

#[test]
fn compaction_required_native_guard_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, kept) = native_compaction_fixture(dir.path());
    engine.compact_text_log().expect("native owner must support proved relocation");
    drop(engine);
    let engine = RedbStorageEngine::open_with_ids(dir.path().join("native"), true,
        Box::new(crate::storage::ids::SystemIds)).unwrap();
    assert_texts(&engine, &kept);
}

#[test]
fn compaction_required_guard_only_is_insufficient_before_probe() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _) = native_compaction_fixture(dir.path());
    let state = engine.control_state().unwrap();
    let refs: Vec<_> = engine.scan_all().unwrap().into_iter()
        .filter(|(_, r)| r.text_offset != 0 || r.text_length != 0)
        .map(|(id, r)| (id, TextRef { file_offset: r.text_offset, length: r.text_length }))
        .collect();
    let (mut prepared, id) = TextStore::create_generation(&engine.db_path, state.store_id).unwrap();
    let relocated: Vec<_> = refs.iter().map(|(id, reference)| {
        (*id, prepared.append(&engine.text_store.read(*reference).unwrap()).unwrap())
    }).collect();
    prepared.sync_all().unwrap();
    fsync_dir(&engine.db_path).unwrap();
    let descriptor = prepared.seal(state.store_id, id, 2).unwrap();
    let guard = engine.meta_store.native_guard_with_work(None).unwrap();
    // Deliberately call the old helper under a native guard, as a negative
    // implementation candidate. This MUST NOT be confused with supported API1.
    engine.meta_store.select_text_generation_guarded(&guard,
        state.committed_generation, &descriptor, &relocated).unwrap();
    drop(guard);
    drop(prepared);
    drop(engine);
    crate::storage::validation::validate_existing(&dir.path().join("native"))
        .expect("guard-only candidate must also satisfy strict native history");
}

fn required_compaction_receipt(stage: Stage) {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, kept) = generation_fixture(dir.path());
    let before = engine.control_state().unwrap();
    let error = engine.compact_text_at(|at| if at == stage {
        Err(std::io::Error::other("injected postcommit failure").into())
    } else { Ok(()) }).unwrap_err();
    drop(engine);
    let engine = RedbStorageEngine::open(dir.path()).unwrap();
    assert_eq!(engine.control_state().unwrap().committed_generation,
        before.committed_generation.next().unwrap());
    assert_texts(&engine, &kept);
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("durable {stage:?} failure lost full receipt: {error}");
    };
    assert_eq!(engine.lookup_operation(receipt.store_id, MutationScope::Catalog,
        receipt.operation_id).unwrap().as_ref(), Some(receipt.as_ref()));
}

#[test]
fn compaction_required_selected_error_receipt() {
    required_compaction_receipt(Stage::Selected);
}

#[test]
fn compaction_required_published_error_receipt() {
    required_compaction_receipt(Stage::Published);
}

#[test]
fn compaction_genuine_before_fixture() {
    let Ok(path) = std::env::var("RECALLD_COMPACTION_BEFORE_OUTPUT") else { return };
    let path = std::path::PathBuf::from(path);
    assert!(!path.exists());
    let (mut engine, _) = generation_fixture(&path);
    engine.compact_text_log().unwrap();
    engine.sync().unwrap();
    drop(engine);
    crate::storage::validation::validate_existing(&path).unwrap();
}
