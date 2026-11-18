use super::*;
use crate::model::{MemoryId, NamespaceId, schema::EmbeddingBinding};
use crate::storage::{StorageEngine, TextRef};

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("db");
    fs::create_dir(&path).unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "fixtures/released-v1.zip"
    )))
    .unwrap()
    .extract(&path)
    .unwrap();
    (root, path)
}
fn open(path: &Path) -> Result<RedbStorageEngine, StorageError> {
    open_with_upgrade(path, 1795025815000, Box::new(SystemIds), |_| Ok(()))
}
// Test convenience only: production runtime must explicitly prove readiness.
fn open_with_upgrade(
    path: &Path,
    now: i64,
    ids: Box<dyn IdSource>,
    validate: impl Fn(&RedbStorageEngine) -> Result<(), StorageError>,
) -> Result<RedbStorageEngine, StorageError> {
    open_at(path, now, ids, validate, |_| Ok(()))
}
fn open_at(
    path: &Path,
    now: i64,
    ids: Box<dyn IdSource>,
    validate: impl Fn(&RedbStorageEngine) -> Result<(), StorageError>,
    mut boundary: impl FnMut(Phase) -> Result<(), StorageError>,
) -> Result<RedbStorageEngine, StorageError> {
    let (engine, transaction) = prepare_at(path, now, ids, validate, &mut boundary)?;
    transaction.commit_at(boundary)?;
    Ok(engine)
}
fn assert_release_data(engine: &RedbStorageEngine) {
    let expected: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/released-v1.json")).unwrap();
    for (i, created) in expected["created"].as_array().unwrap().iter().enumerate() {
        let memory = &created["data"];
        let id: MemoryId = serde_json::from_value(memory["id"].clone()).unwrap();
        let record = engine.get_record(id).unwrap().unwrap();
        assert_eq!(record.version, 2);
        assert_eq!(
            record.tags.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
            memory["tags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        if i == 0 {
            assert_eq!(
                record.schema.as_ref().unwrap().labels.entities,
                ["fixture-entity"]
            );
            assert_eq!(
                record.schema.as_ref().unwrap().labels.topics,
                ["compatibility"]
            );
        }
        assert_eq!(record.summary, memory["summary"].as_str().unwrap());
        assert_eq!(
            engine
                .get_text(TextRef {
                    file_offset: record.text_offset,
                    length: record.text_length
                })
                .unwrap()
                .unwrap(),
            memory["fullText"].as_str().unwrap()
        );
        let ns = engine
            .get_namespace_by_name(memory["namespace"].as_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(record.namespace_id, ns.id.get());
        assert_eq!(
            u64::from(ns.id.get()),
            expected["namespaceIds"][memory["namespace"].as_str().unwrap()]
                .as_u64()
                .unwrap()
        );
        assert_eq!(
            ns.schema.unwrap().embedding_binding,
            EmbeddingBinding::UnknownLegacy
        );
        let expected_vector = [[1., 0., 0., 0.], [0.8, 0.6, 0., 0.], [0., 0., 1., 0.]][i];
        assert_eq!(
            engine
                .get_vector(NamespaceId::new(record.namespace_id), record.vector_slot)
                .unwrap()
                .unwrap(),
            expected_vector
        );
    }
    let edges = engine.edge_store().load_all_edges().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(
        edges[0].source.to_string(),
        expected["created"][0]["data"]["id"].as_str().unwrap()
    );
    assert_eq!(
        edges[0].target.to_string(),
        expected["created"][1]["data"]["id"].as_str().unwrap()
    );
    let ns = engine.meta_store().list_namespaces().unwrap();
    assert_eq!(ns.len(), 2);
}
#[test]
fn released_binary_fixture_upgrade_backup_restart_and_no_downgrade() {
    let (_root, path) = fixture();
    assert_eq!(classify(&path).unwrap(), StoreFormat::Legacy);
    let before = inventory(&path).unwrap();
    let engine = open(&path).unwrap();
    assert_release_data(&engine);
    drop(engine);
    assert_eq!(classify(&path).unwrap(), StoreFormat::Current);
    let operation = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".recalld-upgrade-")
                .then_some(p)
        })
        .next()
        .unwrap();
    assert_eq!(inventory(&operation.join("backup")).unwrap(), before);
    assert!(RedbStorageEngine::open(&path).is_err());
    let engine = open(&path).unwrap();
    assert_release_data(&engine);
    assert_eq!(inventory(&operation.join("backup")).unwrap(), before);
}
#[test]
fn runtime_rejection_keeps_original_and_durable_backup() {
    let (_root, path) = fixture();
    let before = inventory(&path).unwrap();
    let result = open_with_upgrade(&path, 1795025815000, Box::new(SystemIds), |engine| {
        assert_release_data(engine);
        Err(invalid("provider cannot serve UnknownLegacy"))
    });
    assert!(result.is_err());
    assert_eq!(inventory(&path).unwrap(), before);
    assert_eq!(classify(&path).unwrap(), StoreFormat::Legacy);
    let backups: Vec<_> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().path().join("backup"))
        .filter(|p| p.is_dir())
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(inventory(&backups[0]).unwrap(), before);
}
#[test]
fn every_precommit_failure_restores_original() {
    for phase in [
        Phase::Preparing,
        Phase::BackedUp,
        Phase::Validated,
        Phase::OldMoved,
        Phase::OldRenameSynced,
        Phase::Installed,
        Phase::NewRenameSynced,
    ] {
        let (_root, path) = fixture();
        let before = inventory(&path).unwrap();
        let result = open_at(
            &path,
            1795025815000,
            Box::new(SystemIds),
            |_| Ok(()),
            |at| {
                if at == phase {
                    Err(invalid("injected I/O failure"))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err(), "{phase:?}");
        assert_eq!(inventory(&path).unwrap(), before, "{phase:?}");
        assert_eq!(classify(&path).unwrap(), StoreFormat::Legacy);
    }
}
#[test]
fn upgrade_crash_child() {
    let Ok(path) = std::env::var("RECALLD_UPGRADE_TEST_PATH") else {
        return;
    };
    let phase: Phase =
        serde_json::from_str(&std::env::var("RECALLD_UPGRADE_TEST_PHASE").unwrap()).unwrap();
    let _ = open_at(
        Path::new(&path),
        1795025815000,
        Box::new(SystemIds),
        |_| Ok(()),
        |at| {
            if at == phase {
                std::process::exit(73);
            }
            Ok(())
        },
    );
    panic!("crash boundary not reached");
}
#[test]
fn process_crashes_recover_without_serving_legacy() {
    for phase in [
        Phase::Preparing,
        Phase::BackedUp,
        Phase::Validated,
        Phase::OldMoved,
        Phase::OldRenameSynced,
        Phase::Installed,
        Phase::NewRenameSynced,
        Phase::RuntimeReady,
        Phase::Committed,
    ] {
        let (_root, path) = fixture();
        let before = inventory(&path).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::automatic_upgrade::tests::upgrade_crash_child",
                "--nocapture",
            ])
            .env("RECALLD_UPGRADE_TEST_PATH", &path)
            .env(
                "RECALLD_UPGRADE_TEST_PHASE",
                serde_json::to_string(&phase).unwrap(),
            )
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73), "{phase:?}");
        assert_eq!(classify(&path).unwrap(), StoreFormat::PendingUpgrade);
        let recovered = open(&path);
        if phase == Phase::Committed {
            assert_release_data(&recovered.unwrap());
        } else {
            assert!(recovered.is_err(), "recovery must fail this startup");
            assert_eq!(inventory(&path).unwrap(), before, "{phase:?}");
            assert_eq!(classify(&path).unwrap(), StoreFormat::Legacy);
        }
    }
}
#[test]
fn committed_journal_never_rolls_back_later_canonical_write() {
    let (_root, path) = fixture();
    let mut engine = open(&path).unwrap();
    let operation = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.join("manifest.json").is_file())
        .unwrap();
    let journal: Journal =
        serde_json::from_slice(&fs::read(operation.join("manifest.json")).unwrap()).unwrap();
    let state = engine.control_state().unwrap();
    let ctx = super::super::control::MutationContext {
        store_id: Some(state.store_id),
        idempotency_key: None,
        scope: super::super::control::MutationScope::Catalog,
        expected_generation: Some(state.committed_generation),
        contract_version: 2,
    };
    engine
        .create_native_namespace(
            &ctx,
            &super::super::native_catalog::NativeNamespaceInput {
                name: "post-upgrade".into(),
                embedding_dim: Some(4),
                embedding_profile_id: None,
                initial_stability: None,
                desired_retention: None,
                decay_rate_multiplier: None,
            },
            &crate::embedding::runtime::tests::profile(),
            &super::super::native_lifecycle::NativePolicyInputs::default(),
            &crate::time::SystemClock,
            None,
        )
        .unwrap();
    let after = engine.control_state().unwrap();
    drop(engine);
    journal.save().unwrap(); // Simulate committed marker surviving cleanup.
    assert!(RedbStorageEngine::open_native(&path).is_err());
    let engine = open(&path).unwrap();
    assert!(
        engine
            .get_namespace_by_name("post-upgrade")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        engine.control_state().unwrap().committed_generation,
        after.committed_generation
    );
    drop(engine);
    assert!(RedbStorageEngine::open_native(&path).is_ok());
}
#[test]
fn invalid_or_symlink_source_fails_closed() {
    let (root, path) = fixture();
    fs::write(path.join("meta.db"), b"invalid database").unwrap();
    let before = inventory(&path).unwrap();
    assert!(classify(&path).is_err());
    assert!(open(&path).is_err());
    assert_eq!(inventory(&path).unwrap(), before);
    #[cfg(unix)]
    {
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        assert!(open(&alias).is_err());
    }
}

#[test]
fn async_runtime_failure_restores_original_after_handles_drop() {
    let (_root, path) = fixture();
    let before = inventory(&path).unwrap();
    let (engine, transaction) = prepare_upgrade(&path, 1795025815000, Box::new(SystemIds)).unwrap();
    let backup = transaction.backup_path().unwrap();
    assert!(pending(&path).is_file());
    assert_release_data(&engine);
    // Represents any later async initialization failure, before background start.
    drop(engine);
    transaction.abort().unwrap();
    assert_eq!(inventory(&path).unwrap(), before);
    assert_eq!(inventory(&backup).unwrap(), before);
}
#[test]
fn abort_rejects_live_runtime_handle_and_drop_leaves_recovery_journal() {
    let (_root, path) = fixture();
    let before = inventory(&path).unwrap();
    let (engine, transaction) = prepare_upgrade(&path, 1795025815000, Box::new(SystemIds)).unwrap();
    assert!(transaction.abort().is_err());
    assert!(StoreOwnership::acquire(&path).is_err());
    drop(engine);
    assert!(open(&path).is_err());
    assert_eq!(inventory(&path).unwrap(), before);
}

#[test]
fn commit_io_failure_reconciles_marker_without_blind_rollback() {
    for after_replace in [false, true] {
        let (_root, path) = fixture();
        let original = inventory(&path).unwrap();
        let (engine, transaction) =
            prepare_upgrade(&path, 1795025815000, Box::new(SystemIds)).unwrap();
        SAVE_FAULT.with(|f| *f.borrow_mut() = Some((Phase::Committed, after_replace)));
        let committed = transaction.commit();
        drop(engine);
        if after_replace {
            assert!(committed.is_ok());
            assert_release_data(&open(&path).unwrap());
        } else {
            assert!(committed.is_err());
            assert!(open(&path).is_err());
            assert_eq!(inventory(&path).unwrap(), original);
        }
    }
}
#[test]
fn journal_io_failures_at_cutover_restore_original() {
    for phase in [
        Phase::BackedUp,
        Phase::Validated,
        Phase::OldMoved,
        Phase::Installed,
    ] {
        for after_replace in [false, true] {
            let (_root, path) = fixture();
            let original = inventory(&path).unwrap();
            SAVE_FAULT.with(|f| *f.borrow_mut() = Some((phase, after_replace)));
            assert!(prepare_upgrade(&path, 1795025815000, Box::new(SystemIds)).is_err());
            assert_eq!(
                inventory(&path).unwrap(),
                original,
                "{phase:?}/{after_replace}"
            );
        }
    }
}
#[test]
fn future_control_version_never_migrates_or_initializes() {
    let (_root, path) = fixture();
    let engine = open(&path).unwrap();
    let mut control = engine.control_state().unwrap();
    drop(engine);
    control.control_schema_version = 999;
    let db = redb::Database::open(path.join("meta.db")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.open_table(super::super::control::CONTROL)
        .unwrap()
        .insert(
            "state",
            super::super::control::encode(&control).unwrap().as_slice(),
        )
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    let original = inventory(&path).unwrap();
    assert!(classify(&path).is_err());
    assert!(prepare_upgrade(&path, 1795025815000, Box::new(SystemIds)).is_err());
    assert_eq!(inventory(&path).unwrap(), original);
}

#[test]
fn changed_original_before_cutover_is_never_overwritten() {
    let (_root, path) = fixture();
    let result = prepare_at(
        &path,
        1795025815000,
        Box::new(SystemIds),
        |_| Ok(()),
        |phase| {
            if phase == Phase::BackedUp {
                fs::write(path.join("external-write"), b"preserve unrecognized writes")?;
                return Err(invalid("stop before cutover"));
            }
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(
        fs::read(path.join("external-write")).unwrap(),
        b"preserve unrecognized writes"
    );
    assert!(open(&path).is_err());
    assert_eq!(
        fs::read(path.join("external-write")).unwrap(),
        b"preserve unrecognized writes"
    );
}

#[test]
fn crashed_pending_upgrade_blocks_offline_and_native_writes() {
    let (root, path) = fixture();
    let original = inventory(&path).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::automatic_upgrade::tests::upgrade_crash_child",
        ])
        .env("RECALLD_UPGRADE_TEST_PATH", &path)
        .env(
            "RECALLD_UPGRADE_TEST_PHASE",
            serde_json::to_string(&Phase::Installed).unwrap(),
        )
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    let installed = inventory(&path).unwrap();
    assert!(StoreOwnership::acquire(&path).is_err());
    assert!(StoreOwnership::acquire_recovering(&path).is_err());
    assert!(RedbStorageEngine::open_native(&path).is_err());
    assert!(inspect_schema(&path).is_err());
    let archive = root.path().join("restore.zip");
    fs::write(&archive, include_bytes!("fixtures/released-v1.zip")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let error = runtime
        .block_on(crate::backup::restore::restore_from_backup(
            crate::backup::restore::RestoreOptions {
                backup_path: archive,
                data_dir: path.clone(),
                force: true,
                no_stop_daemon: true,
                historical: false,
            },
        ))
        .unwrap_err();
    assert!(
        error.to_string().contains("pending automatic upgrade"),
        "{error}"
    );
    assert!(
        super::super::text_migration::migrate_legacy_text(&path, &root.path().join("offline-copy"))
            .is_err()
    );
    assert_eq!(inventory(&path).unwrap(), installed);
    assert!(open(&path).is_err());
    assert_eq!(inventory(&path).unwrap(), original);
    assert_eq!(classify(&path).unwrap(), StoreFormat::Legacy);
}
#[test]
fn upgrade_only_acquisition_cannot_bypass_lock_or_journal_identity() {
    let (_root, path) = fixture();
    let (engine, transaction) = prepare_upgrade(&path, 1795025815000, Box::new(SystemIds)).unwrap();
    assert!(StoreOwnership::acquire_for_upgrade(&path, true).is_err());
    drop(engine);
    drop(transaction);
    let original = inventory(&path).unwrap();
    let mut journal = read_journal(&path).unwrap().unwrap();
    journal.target = path.parent().unwrap().join("unrelated");
    fs::write(pending(&path), serde_json::to_vec(&journal).unwrap()).unwrap();
    assert!(prepare_upgrade(&path, 1795025815000, Box::new(SystemIds)).is_err());
    assert_eq!(inventory(&path).unwrap(), original);
    assert!(!path.parent().unwrap().join("unrelated").exists());
}
