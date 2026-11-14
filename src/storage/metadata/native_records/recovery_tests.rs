//! Real nonbatch process exits. Test-only corruption is applied only after a
//! valid native insertion, to prove repaired copies still undergo strict checks.
use super::*;
use crate::storage::{ownership, validation};
use std::io::{Seek, SeekFrom, Write};

fn files(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for item in std::fs::read_dir(at).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().into(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = Default::default();
    walk(path, path, &mut out);
    out
}

fn child(d: &Data, id: MemoryId, action: &str, crash: Option<&str>) -> std::process::Output {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "storage::metadata::native_records::tests::recovery::native_preflight_process_child",
        "--nocapture",
    ])
    .env("RECALLD_NATIVE_RECOVERY_PATH", &d.path)
    .env("RECALLD_NATIVE_RECOVERY_PARENT", d.parent.to_string())
    .env("RECALLD_NATIVE_RECOVERY_NAMESPACE", d.ns.get().to_string())
    .env("RECALLD_NATIVE_RECOVERY_ID", id.to_string())
    .env("RECALLD_NATIVE_RECOVERY_ACTION", action)
    .env_remove("RECALLD_TEST_EFFECT_CRASH");
    if let Some(point) = crash {
        cmd.env("RECALLD_TEST_EFFECT_CRASH", point);
    }
    cmd.output().unwrap()
}

#[test]
fn native_preflight_process_child() {
    let Ok(path) = std::env::var("RECALLD_NATIVE_RECOVERY_PATH") else {
        return;
    };
    let d = Data {
        path: path.into(),
        parent: MemoryId::from_uuid(
            std::env::var("RECALLD_NATIVE_RECOVERY_PARENT")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        ns: NamespaceId::new(
            std::env::var("RECALLD_NATIVE_RECOVERY_NAMESPACE")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        profile: crate::embedding::runtime::tests::profile(),
    };
    let id = MemoryId::from_uuid(
        std::env::var("RECALLD_NATIVE_RECOVERY_ID")
            .unwrap()
            .parse()
            .unwrap(),
    );
    let action = std::env::var("RECALLD_NATIVE_RECOVERY_ACTION").unwrap();
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    if action == "locked" {
        assert!(matches!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock),
            Err(StorageError::DatabaseLocked)
        ));
        return;
    }
    let mut session = open(&d, clock);
    let mut original = input(&d);
    original.memory.id = Some(id);
    let intent = session.capture_insert(scope(&d), &original).unwrap();
    let receipt = session
        .insert_prepared(
            &ctx(&session, &d, "native-crash-insert"),
            &intent,
            &generated(),
        )
        .unwrap();
    assert_ne!(action, "crash", "requested crash hook did not exit");
    let record = session.engine.get_record(id).unwrap().unwrap();
    session.engine.sync().unwrap();
    drop(session);

    // Negative fixture corruption only, under actual store/directory ownership.
    // Leave both redb handles open at exit so their copies need recovery.
    let _owner = ownership::StoreOwnership::acquire(&d.path).unwrap();
    let _directory = ownership::lock_directory(&d.path).unwrap();
    let meta = redb::Database::open(d.path.join("meta.db")).unwrap();
    let edges = redb::Database::open(d.path.join("edges.db")).unwrap();
    meta.begin_write().unwrap().commit().unwrap();
    edges.begin_write().unwrap().commit().unwrap();
    match action.as_str() {
        "schema" => {
            let tx = meta.begin_write().unwrap();
            {
                let mut table = tx.open_table(NAMESPACE_TABLE).unwrap();
                let mut ns =
                    NamespaceConfig::decode(table.get(d.ns.get()).unwrap().unwrap().value())
                        .unwrap();
                ns.schema.as_mut().unwrap().namespace_schema_version = 99;
                table
                    .insert(d.ns.get(), control::encode(&ns).unwrap().as_slice())
                    .unwrap();
            }
            tx.commit().unwrap();
        }
        "effects" => {
            let tx = meta.begin_write().unwrap();
            {
                let mut table = tx.open_table(super::super::effects::EFFECTS).unwrap();
                let key = receipt.operation_id.as_bytes().as_slice();
                let mut effect: super::super::effects::PendingEffects =
                    control::decode(table.get(key).unwrap().unwrap().value()).unwrap();
                effect.version = 99;
                table
                    .insert(key, control::encode(&effect).unwrap().as_slice())
                    .unwrap();
            }
            tx.commit().unwrap();
        }
        "text" => {
            let selection = crate::storage::metadata::text_generation::read_selection(
                &meta.begin_read().unwrap(),
            )
            .unwrap()
            .unwrap();
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(d.path.join(selection.filename))
                .unwrap();
            file.seek(SeekFrom::End(-1)).unwrap();
            file.write_all(&[0xff]).unwrap();
            file.sync_all().unwrap();
        }
        "vector" => {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(d.path.join("native/vectors.dat"))
                .unwrap();
            file.seek(SeekFrom::Start(
                crate::storage::vectors::VectorFileHeader::data_offset(record.vector_slot, 16),
            ))
            .unwrap();
            file.write_all(&f32::NAN.to_le_bytes()).unwrap();
            file.sync_all().unwrap();
        }
        "edges" => {
            let tx = edges.begin_write().unwrap();
            {
                let mut table = tx.open_table(crate::storage::edges::FORWARD_EDGES).unwrap();
                let key = table
                    .iter()
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .0
                    .value()
                    .to_vec();
                table.remove(key.as_slice()).unwrap();
            }
            tx.commit().unwrap();
        }
        _ => panic!("unknown corruption case"),
    }
    std::process::exit(94);
}

fn require_disposable_repair(path: &Path) {
    let before = files(path);
    let mut needs_repair = Vec::new();
    for name in ["meta.db", "edges.db"] {
        match validation::inspect_database(&path.join(name)) {
            Err(StorageError::RedbDatabase(redb::DatabaseError::RepairAborted)) => {
                needs_repair.push(name)
            }
            Ok(_) => {}
            Err(error) => panic!("{name}: unexpected inspection error: {error:?}"),
        }
        let (_copy, _db) = validation::inspect_database_copy(&path.join(name), true).unwrap();
    }
    eprintln!("unclean redb files requiring copy repair: {needs_repair:?}");
    assert!(
        !needs_repair.is_empty(),
        "process exit did not require redb repair"
    );
    assert_eq!(files(path), before, "copy repair changed source bytes");
    assert!(crate::storage::offline_snapshot::OfflineSnapshot::open(path).is_err());
    assert_eq!(
        files(path),
        before,
        "offline inspection changed source bytes"
    );
}

struct ProbeReset;
impl Drop for ProbeReset {
    fn drop(&mut self) {
        validation::NATIVE_PREFLIGHT_PROBE.with(|probe| {
            probe.borrow_mut().take();
        });
    }
}
fn pre_writable_probe(path: &Path) -> (ProbeReset, Arc<AtomicUsize>) {
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
    (ProbeReset, count)
}

#[test]
fn native_preflight_recovers_actual_nonbatch_process_exits_and_retries_once() {
    for point in ["before_metadata_commit", "after_metadata_commit"] {
        eprintln!("native recovery crash point: {point}");
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let id = MemoryId::new();
        let initial = inspect_schema(&d.path).unwrap().control;
        let output = child(&d, id, "crash", Some(point));
        assert_eq!(
            output.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        require_disposable_repair(&d.path);
        let before = files(&d.path);
        {
            let _owner = ownership::StoreOwnership::acquire(&d.path).unwrap();
            let _directory = ownership::lock_directory(&d.path).unwrap();
            let probe = child(&d, id, "locked", None);
            assert!(
                probe.status.success(),
                "{}",
                String::from_utf8_lossy(&probe.stderr)
            );
            assert_eq!(files(&d.path), before);
        }
        let (probe, checked) = pre_writable_probe(&d.path);
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session =
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
                .unwrap_or_else(|e| panic!("{point}: actual native reopen failed: {e:?}"));
        assert_eq!(checked.load(Ordering::SeqCst), 1);
        drop(probe);
        assert!(matches!(
            ownership::StoreOwnership::acquire(&d.path),
            Err(StorageError::DatabaseLocked)
        ));
        assert!(ownership::lock_directory(&d.path).is_err());
        let committed = point == "after_metadata_commit";
        assert_eq!(session.engine.get_record(id).unwrap().is_some(), committed);
        let mut original = input(&d);
        original.memory.id = Some(id);
        let intent = session.capture_insert(scope(&d), &original).unwrap();
        let key = ctx(&session, &d, "native-crash-insert");
        let receipt = session
            .insert_prepared(&key, &intent, &generated())
            .unwrap();
        assert_eq!(receipt.replayed, committed);
        assert_eq!(
            receipt.committed_generation,
            initial.committed_generation.next().unwrap()
        );
        assert_eq!(clock.0.load(Ordering::SeqCst), usize::from(!committed));
        let replay = session
            .insert_prepared(&key, &intent, &generated())
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.operation_id, receipt.operation_id);
        assert_eq!(
            session
                .engine
                .get_record(d.parent)
                .unwrap()
                .unwrap()
                .edge_count,
            1
        );
        assert_eq!(session.engine.scan_all().unwrap().len(), 2);
        drop(session);
        let reopened = open(&d, clock);
        assert!(reopened.engine.get_record(id).unwrap().is_some());
        drop(reopened);
        inspect_schema(&d.path).unwrap();
    }
}

#[test]
fn native_preflight_rejects_malformed_repaired_copies_without_original_writes() {
    // No restore journal is pending: journal recovery is a separate owned path.
    for kind in ["schema", "effects", "text", "vector", "edges"] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let output = child(&d, MemoryId::new(), kind, None);
        assert_eq!(
            output.status.code(),
            Some(94),
            "{kind}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        require_disposable_repair(&d.path);
        let before = files(&d.path);
        let (probe, checked) = pre_writable_probe(&d.path);
        let result = NativeLifecycleSession::open(
            &d.path,
            NativePolicyInputs::default(),
            Arc::new(FixedClock(AtomicUsize::new(0))),
        );
        let error = result.err().expect("malformed store opened");
        eprintln!("rejected malformed {kind} after disposable repair: {error:?}");
        assert!(
            !matches!(
                error,
                StorageError::RedbDatabase(redb::DatabaseError::RepairAborted)
            ),
            "{kind}: copy repair was not reached"
        );
        assert_eq!(checked.load(Ordering::SeqCst), 1);
        drop(probe);
        assert_eq!(
            files(&d.path),
            before,
            "{kind}: rejected open changed source"
        );
    }
}
