//! Actual offline operator commands: synthetic store, empty HOME, no daemon.
use recalld::model::NamespaceConfig;
use recalld::storage::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
use recalld::storage::{RedbStorageEngine, StorageEngine};
use std::process::Command;
#[test]
fn cli_inherited_preview_adopt_replay_and_rejects_are_real_and_offline() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    let native = root.path().join("native");
    let home = root.path().join("empty-home");
    std::fs::create_dir(&home).unwrap();
    let scratch = root.path().join("inspector-scratch");
    std::fs::create_dir(&scratch).unwrap();
    let clean = || {
        assert_eq!(
            std::fs::read_dir(&scratch).unwrap().count(),
            0,
            "disposable inspector data leaked"
        )
    };
    let mut e = RedbStorageEngine::open(&old).unwrap();
    let mut ns = NamespaceConfig::default_namespace(0);
    ns.embedding_dim = 4;
    let ns = e.create_namespace(&ns).unwrap();
    e.sync().unwrap();
    drop(e);
    migrate_schema(
        &old,
        &native,
        &MigrationOptions {
            migration_now: 10,
            expected_source: inspect_schema(&old).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let source = inspect_schema(&native).unwrap().control.generation_ref();
    let before = std::fs::read(native.join("meta.db")).unwrap();
    let base = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_recalld-cli"));
        c.env("HOME", &home)
            .env("TMPDIR", &scratch)
            .env_remove("RECALLD_URL")
            .args(["--server", "not-an-http-url", "policy"]);
        c
    };
    let preview_command = || {
        base()
            .args(["preview-inherited", "--store"])
            .arg(&native)
            .args([
                "--namespace-id",
                &ns.get().to_string(),
                "--multiplier",
                "1",
                "--expected-store-id",
                &source.store_id.to_string(),
                "--expected-generation",
                &source.generation.get().to_string(),
            ])
            .output()
            .unwrap()
    };
    let out = preview_command();
    clean();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let preview: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(preview["prior"].is_null());
    assert_eq!(std::fs::read(native.join("meta.db")).unwrap(), before);
    let path = root.path().join("request.json");
    let request = serde_json::json!({"context":{"storeId":source.store_id,"scope":{"kind":"namespace","namespaceId":ns.get()},"idempotencyKey":"operator","expectedGeneration":source.generation,"contractVersion":1},"preview":preview});
    std::fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
    let run = || {
        base()
            .args(["adopt-inherited", "--store"])
            .arg(&native)
            .arg("--request")
            .arg(&path)
            .output()
            .unwrap()
    };
    let out = run();
    clean();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let first: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(first["readiness"], "reopen_required");
    let out = run();
    clean();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let replay: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        replay["receipt"]["operationId"],
        first["receipt"]["operationId"]
    );
    assert_eq!(replay["receipt"]["replayed"], true);
    let committed = std::fs::read(native.join("meta.db")).unwrap();
    let mut stale = request.clone();
    stale["context"]["idempotencyKey"] = "fresh-stale-key".into();
    std::fs::write(&path, serde_json::to_vec(&stale).unwrap()).unwrap();
    assert!(!run().status.success());
    clean();
    assert_eq!(std::fs::read(native.join("meta.db")).unwrap(), committed);
    let mut missing = request.clone();
    missing["preview"].as_object_mut().unwrap().remove("prior");
    std::fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
    assert!(!run().status.success());
    clean();
    assert_eq!(std::fs::read(native.join("meta.db")).unwrap(), committed);
    std::fs::write(&path, vec![b' '; 16 * 1024 + 1]).unwrap();
    assert!(!run().status.success());
    clean();
    assert_eq!(std::fs::read(native.join("meta.db")).unwrap(), committed);
    // Oversized control source is rejected before copying; sparse synthetic tail,
    // not a large fixture clone or a byte scan of arbitrary user data.
    let metadata = std::fs::OpenOptions::new()
        .write(true)
        .open(native.join("meta.db"))
        .unwrap();
    metadata.set_len(512 * 1024 * 1024 + 1).unwrap();
    let rejected = preview_command();
    assert!(!rejected.status.success());
    clean();
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("copy byte budget"));
    metadata.set_len(committed.len() as u64).unwrap();
    drop(metadata);
    assert_eq!(std::fs::read(native.join("meta.db")).unwrap(), committed);
}
