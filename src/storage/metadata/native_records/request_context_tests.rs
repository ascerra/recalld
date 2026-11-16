//! Actual native record consumers retain the originating API in request identity.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn files(root: &Path) -> BTreeMap<PathBuf, [u8; 32]> {
    fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, [u8; 32]>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    Sha256::digest(std::fs::read(path).unwrap()).into(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn receipts(s: &NativeLifecycleSession) -> Vec<(Vec<u8>, Vec<u8>)> {
    s.metadata()
        .db
        .begin_read()
        .unwrap()
        .open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|r| {
            let (k, v) = r.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}
fn replay(r: &WriteReceipt) -> WriteReceipt {
    let mut r = r.clone();
    r.replayed = true;
    r
}
fn key(s: &NativeLifecycleSession, d: &Data, name: &str, api: u32) -> MutationContext {
    let mut k = ctx(s, d, name);
    k.contract_version = api;
    k.expected_generation = Some(s.metadata().control_state().unwrap().committed_generation);
    k
}

#[tokio::test]
async fn native_record_api1_insert_original_contract_replay_and_admission() {
    for api in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut s = open(&d, clock.clone());
        let mut original = input(&d);
        original.memory.embedding = None;
        original.embedding_profile = None;
        let k = key(&s, &d, "origin-insert", api);
        let provider = Provider {
            profile: d.profile.clone(),
            calls: AtomicUsize::new(0),
            fail_if_called: false,
        };
        let r = s
            .insert_generated(&k, scope(&d), &original, &provider)
            .await
            .unwrap();
        assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
        assert_eq!(
            r.committed_generation,
            k.expected_generation.unwrap().next().unwrap()
        );
        assert!(matches!(
            r.outcome,
            MutationOutcome::NativeMemoryInserted { .. }
        ));
        let state = s.metadata().control_state().unwrap();
        let rows = receipts(&s);
        let before = files(&d.path);
        let calls = clock.0.load(Ordering::SeqCst);
        let unavailable = Provider {
            profile: d.profile.clone(),
            calls: AtomicUsize::new(0),
            fail_if_called: true,
        };
        assert_eq!(
            s.insert_generated(&k, scope(&d), &original, &unavailable)
                .await
                .unwrap(),
            replay(&r)
        );
        for case in 0..6 {
            let mut changed = k.clone();
            let mut body = original.clone();
            match case {
                0 => changed.contract_version = 3 - api,
                1 => body.memory.summary.push_str(" changed"),
                2 => changed.expected_generation = None,
                3 => changed.contract_version = 0,
                4 => changed.contract_version = 3,
                _ => changed.store_id = Some(uuid::Uuid::new_v4()),
            }
            let error = s
                .insert_generated(&changed, scope(&d), &body, &unavailable)
                .await
                .unwrap_err();
            match case {
                0..=2 => assert!(matches!(error, StorageError::IdempotencyConflict)),
                3 | 4 => assert!(matches!(error, StorageError::InvalidMutationContext(_))),
                _ => assert!(matches!(error, StorageError::StoreLineageMismatch)),
            }
            assert_eq!(s.metadata().control_state().unwrap(), state);
            assert_eq!(receipts(&s), rows);
            assert_eq!(files(&d.path), before);
        }
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
        drop(s);
        let mut s = open(&d, clock.clone());
        let before = files(&d.path);
        assert_eq!(
            s.insert_generated(&k, scope(&d), &original, &unavailable)
                .await
                .unwrap(),
            replay(&r)
        );
        assert_eq!(receipts(&s), rows);
        assert_eq!(files(&d.path), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        // An unkeyed API1 call remains unkeyed, using the same native owner.
        let intent = s.capture_insert(scope(&d), &input(&d)).unwrap();
        let unkeyed = s
            .insert_prepared(&MutationContext::unkeyed(d.ns), &intent, &generated())
            .unwrap();
        assert!(!unkeyed.replayed);
        assert_ne!(inserted(&unkeyed), inserted(&r));
        println!(
            "native-record origin_api={api} generated_provider_calls=3 replay_provider_calls=0 replay_clock_delta=0 unchanged_files={}",
            before.len()
        );
    }
}

#[test]
fn native_record_api1_forget_original_contract_replay_and_roundtrip() {
    for api in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let mut d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut s = open(&d, clock.clone());
        let intent = s.capture_tombstone(d.parent, None).unwrap();
        let k = key(&s, &d, "origin-forget", api);
        let r = s.tombstone(&k, &intent).unwrap();
        assert_eq!(
            r.committed_generation,
            k.expected_generation.unwrap().next().unwrap()
        );
        assert!(matches!(
            r.outcome,
            MutationOutcome::NativeMemoryTombstoned { .. }
        ));
        assert_eq!(
            s.engine.get_record(d.parent).unwrap().unwrap().phase,
            DecayPhase::Tombstone
        );
        let rows = receipts(&s);
        let state = s.metadata().control_state().unwrap();
        let before = files(&d.path);
        let calls = clock.0.load(Ordering::SeqCst);
        assert_eq!(s.tombstone(&k, &intent).unwrap(), replay(&r));
        let mut other = k.clone();
        other.contract_version = 3 - api;
        assert!(matches!(
            s.tombstone(&other, &intent),
            Err(StorageError::IdempotencyConflict)
        ));
        other = k.clone();
        other.expected_generation = None;
        assert!(matches!(
            s.tombstone(&other, &intent),
            Err(StorageError::IdempotencyConflict)
        ));
        assert_eq!(s.metadata().control_state().unwrap(), state);
        assert_eq!(receipts(&s), rows);
        assert_eq!(files(&d.path), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        drop(s);
        let stream = root.path().join("record-api.jsonl");
        let copy = root.path().join("copy");
        assert_eq!(
            crate::storage::logical_export::export_store(&d.path, &stream).unwrap(),
            crate::storage::logical_export::import_store(&stream, &copy).unwrap()
        );
        d.path = copy;
        let mut s = open(&d, clock.clone());
        let before = files(&d.path);
        assert_eq!(receipts(&s), rows);
        assert_eq!(s.tombstone(&k, &intent).unwrap(), replay(&r));
        assert!(matches!(
            s.tombstone(&other, &intent),
            Err(StorageError::IdempotencyConflict)
        ));
        assert_eq!(receipts(&s), rows);
        assert_eq!(files(&d.path), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        println!(
            "native-record forget_api={api} replay_clock_delta=0 retained_receipts={} unchanged_files={}",
            rows.len(),
            before.len()
        );
    }
}

#[test]
fn native_record_api1_preserves_scope_and_native_owner_guards() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let intent = s.capture_insert(scope(&d), &input(&d)).unwrap();
    let terminal = s.capture_tombstone(d.parent, None).unwrap();
    let before = files(&d.path);
    let state = s.metadata().control_state().unwrap();
    for scope in [
        MutationScope::Catalog,
        MutationScope::Namespace(NamespaceId::new(u32::MAX)),
    ] {
        let mut k = key(&s, &d, "wrong-scope", 1);
        k.scope = scope;
        assert!(matches!(
            s.insert_prepared(&k, &intent, &generated()),
            Err(StorageError::MutationNamespaceMismatch)
        ));
        assert!(matches!(
            s.tombstone(&k, &terminal),
            Err(StorageError::MutationNamespaceMismatch)
        ));
    }
    assert_eq!(s.metadata().control_state().unwrap(), state);
    assert_eq!(files(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    let legacy_path = root.path().join("ordinary-owner");
    let mut legacy = RedbStorageEngine::open(&legacy_path).unwrap();
    let before = files(&legacy_path);
    let state = legacy.control_state().unwrap();
    let k = MutationContext::unkeyed(d.ns);
    assert!(matches!(
        legacy.insert_native_record(
            &k,
            &intent,
            &generated(),
            &NativePolicyInputs::default(),
            clock.as_ref(),
            None
        ),
        Err(StorageError::RecoveryRequired(_))
    ));
    assert!(matches!(
        legacy.tombstone_native_record(
            &k,
            &terminal,
            &NativePolicyInputs::default(),
            clock.as_ref(),
            None
        ),
        Err(StorageError::RecoveryRequired(_))
    ));
    assert_eq!(legacy.control_state().unwrap(), state);
    assert_eq!(files(&legacy_path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
}

#[test]
fn native_record_context_before_fixture() {
    let Ok(output) = std::env::var("RECALLD_WRITE_RECORD_CONTEXT_BEFORE") else {
        return;
    };
    let output = PathBuf::from(output);
    std::fs::create_dir_all(&output).unwrap();
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock);
    let original = input(&d);
    let intent = s.capture_insert(scope(&d), &original).unwrap();
    let insert_key = key(&s, &d, "old2-insert", 2);
    let insertion = s
        .insert_prepared(&insert_key, &intent, &generated())
        .unwrap();
    let terminal = s.capture_tombstone(inserted(&insertion), None).unwrap();
    let forget_key = key(&s, &d, "old2-forget", 2);
    let forget = s.tombstone(&forget_key, &terminal).unwrap();
    let rows = receipts(&s);
    drop(s);
    crate::storage::logical_export::export_store(&d.path, &output.join("store.jsonl")).unwrap();
    std::fs::write(output.join("manifest.json"),serde_json::to_vec(&serde_json::json!({"namespace":d.ns.get(),"parent":d.parent,"profile":d.profile,"insert":insertion,"forget":forget,"insertExpected":insert_key.expected_generation,"forgetExpected":forget_key.expected_generation,"rows":rows})).unwrap()).unwrap();
}

#[tokio::test]
async fn native_record_context_replays_genuine_before_api2_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = match std::env::var("RECALLD_READ_RECORD_CONTEXT_BEFORE") {
        Ok(source) => PathBuf::from(source),
        Err(_) => {
            // Genuine PR121/API2 writer output, captured before the correction.
            // Normal CI exercises it without a local experiment environment.
            let source = root.path().join("historical");
            std::fs::create_dir(&source).unwrap();
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
                include_bytes!("fixtures/request-context-api2.zip").as_slice(),
            ))
            .unwrap();
            for name in ["store.jsonl", "manifest.json"] {
                std::io::copy(
                    &mut archive.by_name(name).unwrap(),
                    &mut std::fs::File::create(source.join(name)).unwrap(),
                )
                .unwrap();
            }
            source
        }
    };
    let path = root.path().join("copy");
    crate::storage::logical_export::import_store(&source.join("store.jsonl"), &path).unwrap();
    let m: serde_json::Value =
        serde_json::from_slice(&std::fs::read(source.join("manifest.json")).unwrap()).unwrap();
    let d = Data {
        path,
        ns: NamespaceId::new(m["namespace"].as_u64().unwrap() as u32),
        parent: serde_json::from_value(m["parent"].clone()).unwrap(),
        profile: serde_json::from_value(m["profile"].clone()).unwrap(),
    };
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let insertion: WriteReceipt = serde_json::from_value(m["insert"].clone()).unwrap();
    let forget: WriteReceipt = serde_json::from_value(m["forget"].clone()).unwrap();
    let old_rows: Vec<(Vec<u8>, Vec<u8>)> = serde_json::from_value(m["rows"].clone()).unwrap();
    assert_eq!(receipts(&s), old_rows);
    let before = files(&d.path);
    let state = s.metadata().control_state().unwrap();
    let mut insert_key = ctx(&s, &d, "old2-insert");
    insert_key.expected_generation = serde_json::from_value(m["insertExpected"].clone()).unwrap();
    // Genuine old history remains unadopted. Replay uses the actual accepted
    // consumer before any fresh-policy capture or provider call.
    assert!(
        s.engine
            .get_namespace(d.ns)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .inherited_decay
            .is_none()
    );
    let provider = Provider {
        profile: d.profile.clone(),
        calls: AtomicUsize::new(0),
        fail_if_called: true,
    };
    assert_eq!(
        s.insert_generated(&insert_key, scope(&d), &input(&d), &provider)
            .await
            .unwrap(),
        replay(&insertion)
    );
    let mut forget_key = ctx(&s, &d, "old2-forget");
    forget_key.expected_generation = serde_json::from_value(m["forgetExpected"].clone()).unwrap();
    let terminal = s.capture_tombstone(inserted(&insertion), None).unwrap();
    assert_eq!(
        s.tombstone(&forget_key, &terminal).unwrap(),
        replay(&forget)
    );
    insert_key.contract_version = 1;
    forget_key.contract_version = 1;
    assert!(matches!(
        s.insert_generated(&insert_key, scope(&d), &input(&d), &provider)
            .await,
        Err(StorageError::IdempotencyConflict)
    ));
    assert!(matches!(
        s.tombstone(&forget_key, &terminal),
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(receipts(&s), old_rows);
    assert_eq!(s.metadata().control_state().unwrap(), state);
    assert_eq!(files(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(
        s.engine
            .get_namespace(d.ns)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .inherited_decay
            .is_none()
    );
}
