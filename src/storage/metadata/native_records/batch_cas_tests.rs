use super::*;

#[test]
fn batch_cas_genuine_before106_none_bytes_hashes_reopen_and_replay() {
    let artifacts =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native-batch-before106");
    let root = tempfile::tempdir().unwrap();
    for version in [1, 2, 3] {
        let manifest = std::fs::read(artifacts.join(format!("v{version}-manifest.json"))).unwrap();
        let p: batch::BatchProgress = control::decode(&manifest).unwrap();
        assert!(p.reservation_precondition.is_none());
        assert_eq!(control::encode(&p).unwrap(), manifest);
        let raw_receipt =
            std::fs::read(artifacts.join(format!("v{version}-reservation.json"))).unwrap();
        let retained = control::validate_receipt(&raw_receipt).unwrap();
        assert_eq!(control::encode(&retained).unwrap(), raw_receipt);
        let request_bytes =
            std::fs::read(artifacts.join(format!("v{version}-request.json"))).unwrap();
        let r: BatchInput = serde_json::from_slice(&request_bytes).unwrap();
        assert!(r.expected_generation.is_none());
        assert_eq!(serde_json::to_vec(&r).unwrap(), request_bytes);
        assert_eq!(
            retained.request_hash,
            control::request_hash(
                &r.context(),
                batch::reservation_domain(version).unwrap(),
                &[&p.caller_hash]
            )
        );
        let destination = root.path().join(format!("store-{version}"));
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let native;
        let legacy;
        let meta = if version == 1 {
            let mut archive =
                zip::ZipArchive::new(std::fs::File::open(artifacts.join("v1-store.zip")).unwrap())
                    .unwrap();
            archive.extract(&destination).unwrap();
            legacy = RedbStorageEngine::open(&destination).unwrap();
            legacy.meta_store()
        } else {
            crate::storage::logical_export::import_store(
                &artifacts.join(format!("v{version}.jsonl")),
                &destination,
            )
            .unwrap();
            native = NativeLifecycleSession::open(
                &destination,
                NativePolicyInputs::default(),
                clock.clone(),
            )
            .unwrap();
            native.metadata()
        };
        let before = readers::bytes(&destination);
        let prepared = if version == 3 {
            plans(&r)
        } else {
            crate::mcp::batches::prepare(&r)
                .unwrap()
                .into_iter()
                .map(|p| p.plan)
                .collect()
        };
        let actual = if version == 3 {
            meta.reserve_native_store_batch_with_work(&r.context(), &prepared, None, None, || {
                panic!("historical replay sampled settings")
            })
            .unwrap()
        } else {
            meta.reserve_batch(&r.context(), r.kind, &prepared).unwrap()
        };
        assert!(actual.reservation_receipt.replayed);
        let mut expected = p.clone();
        expected.reservation_receipt.replayed = true;
        assert_eq!(actual, expected);
        assert_eq!(readers::bytes(&destination), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    }
}

fn cas_request(s: &NativeLifecycleSession, d: &Data) -> BatchInput {
    let state = s.metadata().control_state().unwrap();
    let mut r = request(d, state.store_id);
    r.expected_generation = Some(state.committed_generation);
    r
}

#[test]
fn batch_cas_reservation_replay_resume_and_conflict_preserve_original_precondition() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let r = cas_request(&s, &d);
    let original = plans(&r);
    let before = readers::bytes(&d.path);
    let mut stale = r.clone();
    stale.expected_generation = Some(control::Generation::default());
    assert!(matches!(
        s.metadata().reserve_native_store_batch_with_work(
            &stale.context(),
            &plans(&stale),
            None,
            None,
            || panic!("stale CAS sampled settings")
        ),
        Err(StorageError::GenerationConflict { .. })
    ));
    assert_eq!(readers::bytes(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    let p = reserve(&s, &r, &original);
    assert_eq!(
        p.reservation_expected_generation().unwrap(),
        r.expected_generation
    );
    assert_eq!(
        p.reservation_receipt.committed_generation,
        r.expected_generation.unwrap().next().unwrap()
    );
    assert!(p.child(0).unwrap().context().expected_generation.is_none());
    let receipt = commit(&mut s, &d, &p, native(&original[0]));
    let before = readers::bytes(&d.path);
    let calls = clock.0.load(Ordering::SeqCst);
    for by_operation in [false, true] {
        let mut retry = r.clone();
        if by_operation {
            retry.idempotency_key = None;
            retry.batch_operation_id = Some(p.reservation_receipt.operation_id);
        }
        let replay = s
            .metadata()
            .reserve_native_store_batch_with_work(
                &retry.context(),
                &plans(&retry),
                retry.batch_operation_id,
                None,
                || panic!("matching replay sampled current settings"),
            )
            .unwrap();
        assert!(replay.reservation_receipt.replayed);
        assert_eq!(replay.reservation_precondition, p.reservation_precondition);
        assert_eq!(
            replay.items[0].state,
            BatchItemState::Ok {
                receipt: receipt.clone()
            }
        );
        for expected in [None, Some(receipt.committed_generation)] {
            let mut changed = retry.clone();
            changed.expected_generation = expected;
            assert!(matches!(
                s.metadata().reserve_native_store_batch_with_work(
                    &changed.context(),
                    &plans(&changed),
                    changed.batch_operation_id,
                    None,
                    || panic!("different request sampled settings")
                ),
                Err(StorageError::IdempotencyConflict)
            ));
        }
    }
    assert_eq!(readers::bytes(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    drop(s);
    let s = open(&d, clock);
    let replay = reserve(&s, &r, &original);
    assert!(replay.reservation_receipt.replayed);
    assert_eq!(replay.reservation_precondition, p.reservation_precondition);
}

#[test]
fn batch_cas_competing_fresh_reservations_commit_once() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let r = cas_request(&s, &d);
    let start = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let jobs = (0..2)
            .map(|i| {
                let mut r = r.clone();
                r.idempotency_key = Some(format!("cas-contender-{i}"));
                let start = &start;
                let meta = s.metadata();
                scope.spawn(move || {
                    let prepared = plans(&r);
                    start.wait();
                    meta.reserve_native_store_batch_with_work(
                        &r.context(),
                        &prepared,
                        None,
                        None,
                        || Ok(settings()),
                    )
                })
            })
            .collect::<Vec<_>>();
        jobs.into_iter()
            .map(|j| j.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(StorageError::GenerationConflict { .. })))
            .count(),
        1
    );
    assert_eq!(
        s.metadata().control_state().unwrap().committed_generation,
        r.expected_generation.unwrap().next().unwrap()
    );
}

#[test]
fn batch_cas_corrupt_or_stripped_proof_fails_open_without_source_mutation() {
    for corruption in 0..7 {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let s = open(&d, clock.clone());
        let r = cas_request(&s, &d);
        let p = reserve(&s, &r, &plans(&r));
        let mut value = serde_json::to_value(&p).unwrap();
        match corruption {
            0 => {
                value
                    .as_object_mut()
                    .unwrap()
                    .remove("reservationPrecondition");
            }
            1 => value["reservationPrecondition"] = serde_json::Value::Null,
            2 => value["reservationPrecondition"]["proofVersion"] = json!(2),
            3 => value["reservationPrecondition"]["expectedGeneration"] = json!("0"),
            4 => {
                value["reservationPrecondition"]["expectedGeneration"] =
                    json!("9223372036854775807")
            }
            5 => value["reservationPrecondition"]["unknown"] = json!(true),
            _ => value["version"] = json!(2),
        }
        let tx = s.metadata().db.begin_write().unwrap();
        tx.open_table(crate::storage::metadata::batch::BATCHES)
            .unwrap()
            .insert(
                p.reservation_receipt.operation_id.as_bytes().as_slice(),
                serde_json::to_vec(&value).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
        drop(s);
        let before = readers::bytes(&d.path);
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
            "case {corruption}"
        );
        assert_eq!(readers::bytes(&d.path), before, "case {corruption}");
    }
}

#[test]
fn batch_cas_process_exit_reservation_reconciles_original_proof_once() {
    for point in ["before_batch_commit", "after_batch_commit"] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let s = open(&d, clock.clone());
        let r = cas_request(&s, &d);
        drop(s);
        let path = root.path().join("cas-request.json");
        std::fs::write(&path, serde_json::to_vec(&r).unwrap()).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "storage::metadata::native_records::tests::store_batches::linked_batch_process_child", "--nocapture"])
            .env("RECALLD_LINKED_BATCH_PATH", &d.path)
            .env("RECALLD_LINKED_BATCH_REQUEST", path)
            .env("RECALLD_TEST_EFFECT_CRASH", point).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut s = open(&d, clock.clone());
        let expected = r.expected_generation.unwrap();
        assert_eq!(
            s.metadata().control_state().unwrap().committed_generation,
            if point == "before_batch_commit" {
                expected
            } else {
                expected.next().unwrap()
            }
        );
        let prepared = plans(&r);
        let p = reserve(&s, &r, &prepared);
        assert_eq!(
            p.reservation_receipt.replayed,
            point == "after_batch_commit"
        );
        assert_eq!(p.reservation_expected_generation().unwrap(), Some(expected));
        let receipt = commit(&mut s, &d, &p, native(&prepared[0]));
        assert_eq!(
            receipt.committed_generation,
            expected.next().unwrap().next().unwrap()
        );
        let replay = reserve(&s, &r, &prepared);
        assert!(replay.reservation_receipt.replayed);
        assert_eq!(replay.reservation_precondition, p.reservation_precondition);
        assert_eq!(replay.items[0].state, BatchItemState::Ok { receipt });
    }
}
