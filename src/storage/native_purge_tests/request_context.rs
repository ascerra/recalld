use super::*;
use redb::ReadableTable;
use std::collections::BTreeMap;

fn bytes(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            out.extend(bytes(&entry.path()));
        } else {
            out.insert(entry.path(), std::fs::read(entry.path()).unwrap());
        }
    }
    out
}
fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}
fn raw_start(path: &Path, p: &PurgeProgress) -> Vec<u8> {
    let (_copy, db) = validation::inspect_database_copy(&path.join("meta.db"), true).unwrap();
    let read = db.begin_read().unwrap();
    read.open_table(control::RECEIPTS)
        .unwrap()
        .get(
            control::scoped_key(
                p.store_id,
                MutationScope::Namespace(p.namespace_id),
                p.start_receipt.operation_id.as_bytes(),
            )
            .as_slice(),
        )
        .unwrap()
        .unwrap()
        .value()
        .to_vec()
}

#[test]
fn native_context_api1_purge_process_child() {
    let Some(path) = std::env::var_os("RECALLD_CONTEXT_PURGE_CHILD") else {
        return;
    };
    let mut s = open(Path::new(&path));
    let ns = s
        .engine
        .get_namespace_by_name("doomed")
        .unwrap()
        .unwrap()
        .id;
    let mut key = ctx(&s, ns, "api1-purge");
    key.contract_version = 1;
    s.engine.begin_namespace_purge(&key, true).unwrap();
    panic!("purge start crash did not fire");
}
fn crashed_start(path: &Path) {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::native_purge_tests::request_context::native_context_api1_purge_process_child",
            "--nocapture",
        ])
        .env("RECALLD_CONTEXT_PURGE_CHILD", path)
        .env("RECALLD_TEST_EFFECT_CRASH", "after_purge_commit")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(91),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn rewrite_requests(path: &Path, replacement: Option<&serde_json::Value>) -> usize {
    fn strip(value: &mut serde_json::Value, replacement: Option<&serde_json::Value>) -> usize {
        match value {
            serde_json::Value::Object(m) => {
                let mut count = usize::from(m.remove("request").is_some());
                if count != 0 {
                    if let Some(replacement) = replacement {
                        m.insert("request".into(), replacement.clone());
                    }
                    // Do not descend into the newly replaced request.
                }
                for (key, v) in m.iter_mut() {
                    if key != "request" {
                        count += strip(v, replacement);
                    }
                }
                count
            }
            serde_json::Value::Array(a) => a.iter_mut().map(|v| strip(v, replacement)).sum(),
            _ => 0,
        }
    }
    let db = redb::Database::open(path.join("meta.db")).unwrap();
    let tx = db.begin_write().unwrap();
    let mut count = 0;
    for table in [control::RECEIPTS, metadata::purge::PROGRESS] {
        let mut table = tx.open_table(table).unwrap();
        let rows: Vec<_> = table
            .iter()
            .unwrap()
            .map(|r| {
                let (k, v) = r.unwrap();
                (k.value().to_vec(), v.value().to_vec())
            })
            .collect();
        for (key, raw) in rows {
            let mut value: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            let n = strip(&mut value, replacement);
            count += n;
            if n > 0 {
                table
                    .insert(
                        key.as_slice(),
                        serde_json::to_vec(&value).unwrap().as_slice(),
                    )
                    .unwrap();
            }
        }
    }
    tx.commit().unwrap();
    count
}

#[test]
fn native_context_api1_start_all_copies_strip_before_first_child_rejected_immutable() {
    let root = tempfile::tempdir().unwrap();
    let (path, _, _) = fixture(root.path());
    crashed_start(&path);
    let intact = root.path().join("intact");
    copy_tree(&path, &intact);
    let (_copy, db) = validation::inspect_database_copy(&path.join("meta.db"), true).unwrap();
    let read = db.begin_read().unwrap();
    let state = control::validate_existing(&read).unwrap();
    let p = metadata::purge::validate_existing(&read, state.as_ref())
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(p.sequence, 0);
    assert_eq!(p.start_receipt, p.last_receipt);
    let before_hash = control::validate_receipt(&raw_start(&path, &p))
        .unwrap()
        .request_hash;
    drop(read);
    drop(db);
    assert_eq!(
        rewrite_requests(&path, None),
        3,
        "canonical + progress start + progress last; no children exist"
    );
    let after = raw_start(&path, &p);
    assert_eq!(
        control::validate_receipt(&after).unwrap().request_hash,
        before_hash
    );
    let frozen = bytes(&path);
    let error = validation::preflight_native(&path).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("historical native purge request hash"),
        "{error}"
    );
    assert_eq!(bytes(&path), frozen);
    assert!(RedbStorageEngine::open_native(&path).is_err());
    assert_eq!(bytes(&path), frozen);
    let s = open(&intact);
    assert!(s.engine.get_namespace_by_name("doomed").unwrap().is_none());
}

#[tokio::test]
async fn native_context_api1_purge_replay_export_fork_and_current_reader() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let mut key = ctx(&s, id, "api1-purge");
    key.contract_version = 1;
    let first = s.engine.begin_namespace_purge(&key, true).unwrap();
    let p = done(&mut s, first.clone());
    let state = s.engine.control_state().unwrap();
    let mut other = key.clone();
    other.contract_version = 2;
    assert!(matches!(
        s.engine.begin_namespace_purge(&other, true),
        Err(StorageError::IdempotencyConflict)
    ));
    let replay = s.engine.begin_namespace_purge(&key, true).unwrap();
    assert_eq!(
        replay.start_receipt.operation_id,
        first.start_receipt.operation_id
    );
    assert!(replay.start_receipt.replayed);
    assert_eq!(s.engine.control_state().unwrap(), state);
    drop(s);
    let original = raw_start(&path, &p);
    let stream = root.path().join("logical.jsonl");
    let imported = root.path().join("imported");
    assert_eq!(
        logical_export::export_store(&path, &stream).unwrap(),
        logical_export::import_store(&stream, &imported).unwrap()
    );
    let archive = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("backup.zip"),
        Some(&imported),
        false,
    )
    .await
    .unwrap();
    crate::backup::restore_from_backup(crate::backup::RestoreOptions {
        backup_path: archive,
        data_dir: imported.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    assert_eq!(raw_start(&imported, &p), original);
    let s = open(&imported);
    assert_ne!(s.engine.control_state().unwrap().store_id, p.store_id);
    let f = crate::test_support::Fixture::from_open_engine(root, s.engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    };
    publisher.rebuild_all().await.unwrap();
    let g = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        publisher
            .readiness_barrier(
                g,
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
            .await
            .unwrap()
            .ready
    );
    drop(publisher);
    let _root = f.close();
    assert_eq!(rewrite_requests(&imported, None), 2); // completed last is an advanced receipt
    let frozen = bytes(&imported);
    assert!(validation::canonical_files(&imported).is_err());
    assert!(validation::preflight_native(&imported).is_err());
    assert_eq!(bytes(&imported), frozen);
}

#[test]
fn native_context_genuine_before_four_cases_reopen_fork_unchanged_receipts() {
    let Some(source) = std::env::var_os("RECALLD_CONTEXT_BEFORE_FIXTURES") else {
        return;
    };
    for force in [false, true] {
        for cas in [false, true] {
            let case = Path::new(&source).join(format!("force-{force}-cas-{cas}"));
            let raw = std::fs::read(case.join("original-start-receipt.json")).unwrap();
            assert_eq!(
                control::encode(&control::validate_receipt(&raw).unwrap()).unwrap(),
                raw,
                "old absent proof encoding must stay byte-identical"
            );
            let data: serde_json::Value =
                serde_json::from_slice(&std::fs::read(case.join("case.json")).unwrap()).unwrap();
            let p: PurgeProgress = serde_json::from_value(data["progress"].clone()).unwrap();
            for stage in ["started", "native", "forked"] {
                let frozen = bytes(&case.join(stage));
                validation::preflight_native(&case.join(stage)).unwrap();
                assert_eq!(bytes(&case.join(stage)), frozen);
                let root = tempfile::tempdir().unwrap();
                let copy = root.path().join("store");
                copy_tree(&case.join(stage), &copy);
                let s = open(&copy);
                drop(s);
                assert_eq!(raw_start(&copy, &p), raw);
                validation::canonical_files(&copy).unwrap();
            }
        }
    }
}

#[test]
fn native_context_api1_purge_malformed_request_proofs_fail_closed_immutable() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let mut key = ctx(&s, id, "api1-malformed");
    key.contract_version = 1;
    let p = s.engine.begin_namespace_purge(&key, true).unwrap();
    drop(s);
    if let Some(binary) = std::env::var_os("RECALLD_CONTEXT_OLD_TEST_BINARY") {
        let frozen = bytes(&path);
        let out = std::process::Command::new(binary)
            .args([
                "--exact",
                "storage::native_purge_tests::native_purge_process_child",
                "--nocapture",
            ])
            .env("RECALLD_NATIVE_PURGE_TEST_STORE", &path)
            .env("RECALLD_NATIVE_PURGE_TEST_NS", id.get().to_string())
            .output()
            .unwrap();
        assert!(!out.status.success());
        let message = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            message.contains("unknown field") && message.contains("request"),
            "{message}"
        );
        assert_eq!(
            bytes(&path),
            frozen,
            "genuine old binary must reject before modifying source"
        );
    }
    let value: serde_json::Value = serde_json::from_slice(&raw_start(&path, &p)).unwrap();
    let request = value["receipt"]["outcome"]["transition"]["request"].clone();
    assert!(request.is_object());
    let mut bad = vec![serde_json::Value::Null];
    for version in [0, 2, 3] {
        let mut r = request.clone();
        r["context"]["contractVersion"] = version.into();
        bad.push(r);
    }
    let mut r = request.clone();
    r["context"]["proofVersion"] = 2.into();
    bad.push(r);
    let mut r = request.clone();
    r["context"] = serde_json::Value::Null;
    bad.push(r);
    let mut r = request.clone();
    r["context"]["unknown"] = true.into();
    bad.push(r);
    let mut r = request.clone();
    r["force"] = false.into();
    bad.push(r);
    let mut r = request.clone();
    r["expectedGeneration"] = serde_json::Value::Null;
    bad.push(r);
    let mut r = request.clone();
    r["expectedGeneration"] = "0".into();
    bad.push(r);
    for (index, r) in bad.iter().enumerate() {
        let copy = root.path().join(format!("bad-{index}"));
        copy_tree(&path, &copy);
        assert_eq!(rewrite_requests(&copy, Some(r)), 3);
        let frozen = bytes(&copy);
        assert!(
            validation::preflight_native(&copy).is_err(),
            "accepted case{index}"
        );
        assert_eq!(bytes(&copy), frozen);
        assert!(
            RedbStorageEngine::open_native(&copy).is_err(),
            "opened case{index}"
        );
        assert_eq!(bytes(&copy), frozen);
    }
}
