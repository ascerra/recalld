// BEFORE probes run unchanged on the actual PR138 runtime. The guard-only test
// is explicitly a fixture bypass, not a supported native mutation path.
fn native_compaction_fixture(root: &Path) -> (RedbStorageEngine, Vec<(MemoryId, String)>) {
    use crate::storage::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
    let source = root.join("legacy");
    let (mut engine, kept) = generation_fixture(&source);
    engine.sync().unwrap();
    drop(engine);
    let destination = root.join("native");
    migrate_schema(
        &source,
        &destination,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    (
        RedbStorageEngine::open_with_ids(
            &destination,
            true,
            Box::new(crate::storage::ids::SystemIds),
        )
        .unwrap(),
        kept,
    )
}

#[test]
fn compaction_required_native_guard_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, kept) = native_compaction_fixture(dir.path());
    engine
        .compact_text_log()
        .expect("native owner must support proved relocation");
    drop(engine);
    let engine = RedbStorageEngine::open_with_ids(
        dir.path().join("native"),
        true,
        Box::new(crate::storage::ids::SystemIds),
    )
    .unwrap();
    assert_texts(&engine, &kept);
}

#[test]
fn compaction_required_guard_only_is_insufficient_before_probe() {
    let dir = tempfile::tempdir().unwrap();
    let (engine, _) = native_compaction_fixture(dir.path());
    let state = engine.control_state().unwrap();
    let refs: Vec<_> = engine
        .scan_all()
        .unwrap()
        .into_iter()
        .filter(|(_, r)| r.text_offset != 0 || r.text_length != 0)
        .map(|(id, r)| {
            (
                id,
                TextRef {
                    file_offset: r.text_offset,
                    length: r.text_length,
                },
            )
        })
        .collect();
    let (mut prepared, id) = TextStore::create_generation(&engine.db_path, state.store_id).unwrap();
    let relocated: Vec<_> = refs
        .iter()
        .map(|(id, reference)| {
            (
                *id,
                prepared
                    .append(&engine.text_store.read(*reference).unwrap())
                    .unwrap(),
            )
        })
        .collect();
    prepared.sync_all().unwrap();
    fsync_dir(&engine.db_path).unwrap();
    let descriptor = prepared.seal(state.store_id, id, 2).unwrap();
    let guard = engine.meta_store.native_guard_with_work(None).unwrap();
    // Deliberately call the old helper under a native guard, as a negative
    // implementation candidate. This MUST NOT be confused with supported API1.
    assert!(
        engine
            .meta_store
            .select_text_generation_guarded(
                &guard,
                state.committed_generation,
                &descriptor,
                &relocated
            )
            .is_err()
    );
    drop(guard);
    drop(prepared);
    drop(engine);
    crate::storage::validation::validate_existing(&dir.path().join("native"))
        .expect("rejected guard-only candidate must leave valid native history");
}

fn required_compaction_receipt(stage: Stage) {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, kept) = generation_fixture(dir.path());
    let before = engine.control_state().unwrap();
    let error = engine
        .compact_text_at(|at| {
            if at == stage {
                Err(std::io::Error::other("injected postcommit failure").into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    drop(engine);
    let engine = RedbStorageEngine::open(dir.path()).unwrap();
    assert_eq!(
        engine.control_state().unwrap().committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert_texts(&engine, &kept);
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("durable {stage:?} failure lost full receipt: {error}");
    };
    assert_eq!(
        engine
            .lookup_operation(
                receipt.store_id,
                MutationScope::Catalog,
                receipt.operation_id
            )
            .unwrap()
            .as_ref(),
        Some(receipt.as_ref())
    );
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
    let Ok(path) = std::env::var("RECALLD_COMPACTION_BEFORE_OUTPUT") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    assert!(!path.exists());
    let (mut engine, _) = generation_fixture(&path);
    engine.compact_text_log().unwrap();
    engine.sync().unwrap();
    drop(engine);
    crate::storage::validation::validate_existing(&path).unwrap();
}

#[test]
fn compaction_genuine_legacy_before_receipt_bytes_survive_open_and_new_selection() {
    fn copy(source: &Path, target: &Path) {
        std::fs::create_dir_all(target).unwrap();
        for item in std::fs::read_dir(source).unwrap() {
            let item = item.unwrap();
            let dest = target.join(item.file_name());
            if item.path().is_dir() {
                copy(&item.path(), &dest)
            } else {
                std::fs::copy(item.path(), dest).unwrap();
            }
        }
    }
    fn receipts(path: &Path) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
        use redb::ReadableTable;
        let (_copy, db) =
            crate::storage::validation::inspect_database(&path.join("meta.db")).unwrap();
        let read = db.begin_read().unwrap();
        read.open_table(crate::storage::control::RECEIPTS)
            .unwrap()
            .iter()
            .unwrap()
            .map(|row| {
                let (k, v) = row.unwrap();
                (k.value().to_vec(), v.value().to_vec())
            })
            .collect()
    }
    let root = tempfile::tempdir().unwrap();
    if let Ok(source) = std::env::var("RECALLD_COMPACTION_BEFORE_INPUT") {
        copy(Path::new(&source), root.path());
    } else {
        // Genuine PR138 runtime bytes, generated by the frozen BEFORE binary.
        // The compact synthetic fixture makes this compatibility check run in CI.
        let input = std::io::Cursor::new(include_bytes!("fixtures/text-compaction-before.zip"));
        zip::ZipArchive::new(input)
            .unwrap()
            .extract(root.path())
            .unwrap();
    }
    let before = receipts(root.path());
    let mut engine = RedbStorageEngine::open(root.path()).unwrap();
    let result = engine.compact_text_log().unwrap();
    assert!(matches!(
        result.receipt.outcome,
        crate::storage::control::MutationOutcome::TextGenerationSelected { .. }
    ));
    drop(engine);
    let after = receipts(root.path());
    assert_eq!(after.len(), before.len() + 1);
    for (key, bytes) in before {
        assert_eq!(after.get(&key), Some(&bytes));
    }
}

#[test]
fn compaction_native_process_child() {
    let Ok(path) = std::env::var("RECALLD_NATIVE_TEXT_CRASH_PATH") else {
        return;
    };
    let stop = std::env::var("RECALLD_NATIVE_TEXT_CRASH_STAGE").unwrap();
    let mut engine =
        RedbStorageEngine::open_with_ids(path, true, Box::new(crate::storage::ids::SystemIds))
            .unwrap();
    engine
        .compact_text_at(|stage| {
            if format!("{stage:?}") == stop {
                std::process::exit(73)
            }
            Ok(())
        })
        .unwrap();
    panic!("crash boundary not reached");
}

#[test]
fn compaction_native_process_crashes_reopen_twice_at_exact_selection() {
    for stage in [
        Stage::EntriesWritten,
        Stage::FileSynced,
        Stage::DirectorySynced,
        Stage::Selected,
        Stage::Published,
        Stage::Cleanup,
        Stage::Unlinked,
    ] {
        let root = tempfile::tempdir().unwrap();
        let (engine, kept) = native_compaction_fixture(root.path());
        let path = root.path().join("native");
        let before = engine.control_state().unwrap();
        let selected = engine.meta_store.text_selection().unwrap().unwrap();
        drop(engine);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::engine::text_compaction::tests::compaction_native_process_child",
                "--nocapture",
            ])
            .env("RECALLD_NATIVE_TEXT_CRASH_PATH", &path)
            .env("RECALLD_NATIVE_TEXT_CRASH_STAGE", format!("{stage:?}"))
            .output()
            .unwrap();
        assert_eq!(
            child.status.code(),
            Some(73),
            "{stage:?}: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        for _ in 0..2 {
            let engine = RedbStorageEngine::open_with_ids(
                &path,
                true,
                Box::new(crate::storage::ids::SystemIds),
            )
            .unwrap();
            let committed = matches!(
                stage,
                Stage::Selected | Stage::Published | Stage::Cleanup | Stage::Unlinked
            );
            assert_eq!(
                engine.control_state().unwrap().committed_generation,
                if committed {
                    before.committed_generation.next().unwrap()
                } else {
                    before.committed_generation
                }
            );
            assert_eq!(
                engine.meta_store.text_selection().unwrap().unwrap().id == selected.id,
                !committed
            );
            assert_texts(&engine, &kept);
        }
    }
}

#[test]
fn compaction_native_unknown_commit_reopens_without_guessed_receipt() {
    let root = tempfile::tempdir().unwrap();
    let (mut engine, kept) = native_compaction_fixture(root.path());
    let before = engine.control_state().unwrap();
    crate::storage::metadata::text_generation::FAIL_AFTER_TEXT_COMMIT.with(|flag| flag.set(true));
    let error = engine.compact_text_log().unwrap_err();
    let StorageError::CommitOutcomeUnknown { operation_id, .. } = error else {
        panic!("{error}")
    };
    assert!(engine.compact_text_log().is_err());
    drop(engine);
    let engine = RedbStorageEngine::open_with_ids(
        root.path().join("native"),
        true,
        Box::new(crate::storage::ids::SystemIds),
    )
    .unwrap();
    let receipt = engine
        .lookup_operation(before.store_id, MutationScope::Catalog, operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt.committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert!(matches!(
        receipt.outcome,
        crate::storage::control::MutationOutcome::NativeTextGenerationSelected { .. }
    ));
    assert_texts(&engine, &kept);
}
