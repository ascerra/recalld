use super::*;

fn fixture(root: &Path) {
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "fixtures/released-v1.zip"
    )))
    .unwrap()
    .extract(root)
    .unwrap();
}

#[test]
fn relocation_preserves_canonical_bytes_and_unrelated_files_and_excludes_old_owner() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    let seals = inventory(root.path()).unwrap();
    fs::write(root.path().join("notes.txt"), b"not database data").unwrap();
    fs::write(root.path().join("default/notes.txt"), b"also unrelated").unwrap();
    let lease = prepare_container_layout(root.path()).unwrap();
    for seal in &seals {
        matches(&lease.path.join(&seal.name), seal).unwrap();
    }
    assert_eq!(
        fs::read(root.path().join("default/notes.txt")).unwrap(),
        b"also unrelated"
    );
    assert!(!lease.path.join("notes.txt").exists());
    assert!(lock_directory(root.path()).is_err());
    assert!(prepare_container_layout(root.path()).is_err());
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::container_layout::tests::layout_crash_child",
        ])
        .env("RECALLD_LAYOUT_TEST_ROOT", root.path())
        .env("RECALLD_LAYOUT_TEST_PHASE", "old-lock")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(77));
    drop(lease);
    let lease = resolve_container_layout(root.path()).unwrap();
    drop(lease);
    // Ready means subsequent canonical writes must survive, not match old seals.
    fs::write(root.path().join(STORE).join("additional-user-data"), b"new").unwrap();
    drop(prepare_container_layout(root.path()).unwrap());
    assert_eq!(
        fs::read(root.path().join(STORE).join("additional-user-data")).unwrap(),
        b"new"
    );
}

#[test]
fn fresh_partial_collision_and_custom_path_are_distinguished() {
    let root = tempfile::tempdir().unwrap();
    assert!(configured_layout(root.path(), &root.path().join("custom"), true).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    assert!(resolve_container_layout(root.path()).is_err());
    fs::write(root.path().join("edges.db"), b"partial").unwrap();
    assert!(prepare_container_layout(root.path()).is_err());
    assert!(!root.path().join(STORE).exists());
    fs::remove_file(root.path().join("edges.db")).unwrap();
    fs::create_dir(root.path().join(STORE)).unwrap();
    assert!(prepare_container_layout(root.path()).is_err());
    fs::remove_dir(root.path().join(STORE)).unwrap();
    drop(configured_layout(root.path(), &root.path().join(STORE), true).unwrap());
    assert!(
        read(&root.path().canonicalize().unwrap())
            .unwrap()
            .unwrap()
            .ready
    );
    assert!(root.path().join(STORE).is_dir());
}

#[test]
fn pending_recovery_rejects_changed_or_duplicate_source_without_overwrite() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    assert!(
        prepare_at(root.path(), |phase| if phase == "file" {
            Err(invalid("fault"))
        } else {
            Ok(())
        })
        .is_err()
    );
    assert!(resolve_container_layout(root.path()).is_err());
    let j = read(&root.path().canonicalize().unwrap()).unwrap().unwrap();
    // Explicit offline paths cannot bypass the pending layout fence, even
    // after the relocating process has released its OS locks.
    for path in [
        root.path().to_path_buf(),
        root.path().join(STORE),
        stage(root.path(), &j),
    ] {
        assert!(super::super::ownership::StoreOwnership::acquire(&path).is_err());
    }
    let moved = &j.files[0];
    let destination = stage(root.path(), &j).join(&moved.name);
    let before = fs::read(&destination).unwrap();
    fs::write(root.path().join(&moved.name), b"unrecognized new data").unwrap();
    assert!(prepare_container_layout(root.path()).is_err());
    assert_eq!(fs::read(&destination).unwrap(), before);
    assert_eq!(
        fs::read(root.path().join(&moved.name)).unwrap(),
        b"unrecognized new data"
    );
}

#[test]
fn stale_or_truncated_journal_cannot_authorize_empty_bootstrap_or_move_locks() {
    for fault in ["empty", "omit", "lock", "root", "namespace", "text"] {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        let before = fs::read(root.path().join("meta.db")).unwrap();
        let mut j = Journal {
            version: 1,
            root: root.path().canonicalize().unwrap(),
            id: Uuid::new_v4(),
            ready: false,
            files: inventory(root.path()).unwrap(),
        };
        match fault {
            "empty" => j.files.clear(),
            "omit" => {
                j.files.retain(|s| s.name != Path::new("edges.db"));
            }
            "lock" => {
                j.files[0].name = "recalld.lock".into();
            }
            "root" => j.root = root.path().join("other"),
            "namespace" => {
                j.files
                    .iter_mut()
                    .find(|s| s.name.ends_with("vectors.dat"))
                    .unwrap()
                    .name = "invented/vectors.dat".into();
            }
            "text" => j.files.push(Seal {
                name: format!("fulltext.{}.dat", Uuid::new_v4()).into(),
                length: 1,
                hash: [0; 32],
            }),
            _ => unreachable!(),
        }
        save(root.path(), &j).unwrap();
        assert!(prepare_container_layout(root.path()).is_err(), "{fault}");
        assert_eq!(fs::read(root.path().join("meta.db")).unwrap(), before);
        assert!(!root.path().join(STORE).exists());
    }
}

#[test]
fn legitimate_store_namespace_is_not_the_reserved_hidden_child() {
    use crate::storage::StorageEngine;
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    {
        let mut engine = super::super::RedbStorageEngine::open(root.path()).unwrap();
        let mut ns = crate::model::NamespaceConfig::default_namespace(1000);
        ns.id = crate::model::NamespaceId::new(55);
        ns.name = "store".into();
        ns.embedding_dim = 4;
        engine.create_namespace(&ns).unwrap();
        engine.sync().unwrap();
    }
    let original = fs::read(root.path().join("store/vectors.dat")).unwrap();
    let lease = prepare_container_layout(root.path()).unwrap();
    assert_eq!(
        fs::read(lease.path.join("store/vectors.dat")).unwrap(),
        original
    );
    assert!(crate::model::validation::validate_namespace_name(STORE).is_err());
}

#[test]
fn process_exit_recovery_at_each_durable_boundary() {
    for phase in ["journal", "stage", "file", "installed", "ready"] {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        let seals = inventory(root.path()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::container_layout::tests::layout_crash_child",
                "--nocapture",
            ])
            .env("RECALLD_LAYOUT_TEST_ROOT", root.path())
            .env("RECALLD_LAYOUT_TEST_PHASE", phase)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(77), "{phase}");
        if phase != "ready" {
            assert!(resolve_container_layout(root.path()).is_err());
        }
        let lease = prepare_container_layout(root.path()).unwrap();
        for seal in &seals {
            matches(&lease.path.join(&seal.name), seal).unwrap();
        }
    }
}

#[test]
fn layout_crash_child() {
    let Some(root) = std::env::var_os("RECALLD_LAYOUT_TEST_ROOT") else {
        return;
    };
    let phase = std::env::var("RECALLD_LAYOUT_TEST_PHASE").unwrap();
    if phase == "old-lock" {
        assert!(lock_directory(Path::new(&root)).is_err());
        std::process::exit(77);
    }
    if phase == "schema" {
        let lease = prepare_container_layout(Path::new(&root)).unwrap();
        super::super::automatic_upgrade::container_cutover_crash(lease.store_path());
    }
    prepare_at(Path::new(&root), |at| {
        if at == phase {
            std::process::exit(77);
        }
        Ok(())
    })
    .unwrap();
    panic!("crash boundary not reached");
}

// Run explicitly in two different containers over one synthetic persistent
// mount. No external fixture path or personal data is accepted by normal tests.
#[test]
#[ignore = "isolated mounted-volume acceptance; requires synthetic test root"]
fn mounted_volume_reopen() {
    let root = PathBuf::from(std::env::var_os("RECALLD_LAYOUT_TEST_ROOT").unwrap());
    let mode = std::env::var("RECALLD_LAYOUT_TEST_MODE").unwrap();
    if mode == "crash" {
        assert!(!root.join("meta.db").exists());
        fixture(&root);
        if std::env::var("RECALLD_LAYOUT_TEST_PHASE").unwrap() == "schema" {
            let lease = prepare_container_layout(&root).unwrap();
            super::super::automatic_upgrade::container_cutover_crash(lease.store_path());
        }
        prepare_at(&root, |phase| {
            if phase == std::env::var("RECALLD_LAYOUT_TEST_PHASE").unwrap() {
                std::process::exit(77);
            }
            Ok(())
        })
        .unwrap();
        unreachable!();
    }
    assert_eq!(mode, "recover");
    if std::env::var("RECALLD_LAYOUT_TEST_PHASE").unwrap() == "schema" {
        assert!(!root.join(STORE).exists());
        assert!(prepare_container_layout(&root).is_err());
        assert!(root.join(STORE).join("meta.db").exists());
    }
    let lease = prepare_container_layout(&root).unwrap();
    let j = read(&root).unwrap().unwrap();
    assert!(j.ready);
    for seal in &j.files {
        matches(&lease.path.join(&seal.name), seal).unwrap();
    }
    let (engine, transaction) = super::super::automatic_upgrade::prepare_upgrade(
        lease.store_path(),
        1_800_000_000_000,
        Box::new(super::super::ids::SystemIds),
    )
    .unwrap();
    assert!(transaction.backup_path().unwrap().starts_with(&root));
    drop(engine);
    transaction.abort().unwrap();
    for seal in &j.files {
        matches(&lease.path.join(&seal.name), seal).unwrap();
    }
}

#[test]
fn committed_layout_missing_child_requires_authenticated_recovery_not_bootstrap() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    let seals = inventory(root.path()).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::container_layout::tests::layout_crash_child",
            "--nocapture",
        ])
        .env("RECALLD_LAYOUT_TEST_ROOT", root.path())
        .env("RECALLD_LAYOUT_TEST_PHASE", "schema")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(77));
    assert!(!root.path().join(STORE).exists());
    assert!(resolve_container_layout(root.path()).is_err());
    let error = prepare_container_layout(root.path()).err().unwrap();
    assert!(
        error
            .to_string()
            .contains("interrupted upgrade rolled back"),
        "{error}"
    );
    let lease = prepare_container_layout(root.path()).unwrap();
    for seal in &seals {
        matches(&lease.path.join(&seal.name), seal).unwrap();
    }
    drop(lease);
    fs::rename(root.path().join(STORE), root.path().join("unrecognized")).unwrap();
    assert!(prepare_container_layout(root.path()).is_err());
    assert!(!root.path().join(STORE).exists());
    fs::write(
        root.path().join(format!("{STORE}.recalld-restore-pending")),
        b"{}",
    )
    .unwrap();
    assert!(prepare_container_layout(root.path()).is_err());
    assert!(!root.path().join(STORE).exists());
}

#[tokio::test]
async fn schema_init_failure_restores_original_at_committed_child_not_root() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    let seals = inventory(root.path()).unwrap();
    let lease = prepare_container_layout(root.path()).unwrap();
    let mut config = crate::config::RecalldConfig::default();
    config.storage.data_dir = lease.path.to_str().unwrap().into();
    config.embedding.provider = crate::config::EmbeddingProvider::Ollama;
    config.embedding.model_name = "isolated-layout".into();
    config.embedding.dimensions = 4;
    config.embedding.base_url = "http://127.0.0.1:9".into();
    config.embedding.api_key_env = "RECALLD_LAYOUT_TEST_NO_CREDENTIAL".into();
    config.decay.sweep_interval_hours = f64::NAN;
    assert!(crate::Recalld::new(config).await.is_err());
    assert!(!root.path().join("meta.db").exists());
    for seal in &seals {
        matches(&lease.path.join(&seal.name), seal).unwrap();
    }
    assert!(
        read(&root.path().canonicalize().unwrap())
            .unwrap()
            .unwrap()
            .ready
    );
}

#[cfg(unix)]
#[test]
fn symlink_namespace_cannot_redirect_relocation() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    let elsewhere = tempfile::tempdir().unwrap();
    fs::rename(
        root.path().join("default"),
        elsewhere.path().join("original"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        elsewhere.path().join("original"),
        root.path().join("default"),
    )
    .unwrap();
    assert!(prepare_container_layout(root.path()).is_err());
    assert!(!root.path().join(JOURNAL).exists());
}
