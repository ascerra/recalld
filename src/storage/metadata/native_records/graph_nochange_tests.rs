use super::*;

#[test]
fn native_graph_nochange_empty_cas_and_unkeyed_immutability() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let empty = NativeGraphInput { edges: vec![] };
    let before = session.metadata().control_state().unwrap();
    let mut key = ctx(&session, &d, "empty");
    key.expected_generation = Some(before.committed_generation);
    let calls = clock.0.load(Ordering::SeqCst);
    let receipt = session.put_graph(&key, scope(&d), &empty).unwrap().unwrap();
    assert_eq!(
        receipt.committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    let MutationOutcome::NativeGraphUnchanged { transition } = &receipt.outcome else {
        panic!()
    };
    assert!(transition.edges.is_empty() && transition.records.is_empty());
    assert!(
        session
            .put_graph(&key, scope(&d), &empty)
            .unwrap()
            .unwrap()
            .replayed
    );
    let mut stale = key.clone();
    stale.idempotency_key = Some("stale".into());
    assert!(session.put_graph(&stale, scope(&d), &empty).is_err());
    let mut unkeyed = ctx(&session, &d, "none");
    unkeyed.idempotency_key = None;
    let files = bytes(&d.path);
    assert!(
        session
            .put_graph(&unkeyed, scope(&d), &empty)
            .unwrap()
            .is_none()
    );
    assert_eq!(files, bytes(&d.path));
    drop(session);
    inspect_schema(&d.path).unwrap();
    let mut session = open(&d, clock);
    assert!(
        session
            .put_graph(&key, scope(&d), &empty)
            .unwrap()
            .unwrap()
            .replayed
    );
}

#[test]
fn native_graph_nochange_failures_reconcile_without_peer_or_clock_changes() {
    for unknown in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock.clone());
        let id = add(&mut session, &d, "source");
        let input = plan(id, d.parent);
        session
            .put_graph(&ctx(&session, &d, "first"), scope(&d), &input)
            .unwrap();
        let prior = session.engine.get_record(id).unwrap().unwrap().to_bytes();
        let edges = session.engine.load_all_edges().unwrap();
        let calls = clock.0.load(Ordering::SeqCst);
        let key = ctx(&session, &d, "noop-fault");
        let before = session.metadata().control_state().unwrap();
        if unknown {
            effects::FAIL_AFTER_RECORD_COMMIT.with(|v| v.set(true));
        } else {
            session
                .metadata()
                .fail_effect_application
                .store(true, Ordering::SeqCst);
        }
        let error = session.put_graph(&key, scope(&d), &input).unwrap_err();
        let operation = match error {
            StorageError::CommitOutcomeUnknown { operation_id, .. } if unknown => operation_id,
            StorageError::CommittedMutation { receipt, .. } if !unknown => {
                assert!(matches!(
                    receipt.outcome,
                    MutationOutcome::NativeGraphUnchanged { .. }
                ));
                receipt.operation_id
            }
            other => panic!("{other:?}"),
        };
        drop(session);
        let mut session = open(&d, clock.clone());
        let replay = session.put_graph(&key, scope(&d), &input).unwrap().unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.operation_id, operation);
        assert_eq!(
            replay.committed_generation,
            before.committed_generation.next().unwrap()
        );
        assert_eq!(
            session.engine.get_record(id).unwrap().unwrap().to_bytes(),
            prior
        );
        assert_eq!(session.engine.load_all_edges().unwrap(), edges);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    }
}

#[test]
fn native_graph_nochange_process_exit_recovers_key_once() {
    for point in [
        "before_metadata_commit",
        "after_metadata_commit",
        "after_canonical_ack",
    ] {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock.clone());
        let id = add(&mut session, &d, "source");
        let input = plan(id, d.parent);
        session
            .put_graph(&ctx(&session, &d, "first"), scope(&d), &input)
            .unwrap();
        let prior = session.engine.get_record(id).unwrap().unwrap().to_bytes();
        let edges = session.engine.load_all_edges().unwrap();
        let key = ctx(&session, &d, "crash");
        let before = session.metadata().control_state().unwrap();
        drop(session);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::metadata::native_records::tests::graph::native_graph_process_child",
                "--nocapture",
            ])
            .env("RECALLD_NATIVE_GRAPH_PATH", &d.path)
            .env("RECALLD_NATIVE_GRAPH_NS", d.ns.get().to_string())
            .env("RECALLD_NATIVE_GRAPH_TARGET", d.parent.to_string())
            .env("RECALLD_NATIVE_GRAPH_SOURCE", id.to_string())
            .env("RECALLD_TEST_EFFECT_CRASH", point)
            .output()
            .unwrap();
        assert_eq!(
            child.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        let mut session = open(&d, clock.clone());
        let receipt = session.put_graph(&key, scope(&d), &input).unwrap().unwrap();
        assert_eq!(receipt.replayed, point != "before_metadata_commit");
        assert!(matches!(
            receipt.outcome,
            MutationOutcome::NativeGraphUnchanged { .. }
        ));
        assert_eq!(
            receipt.committed_generation,
            before.committed_generation.next().unwrap()
        );
        assert_eq!(
            session.engine.get_record(id).unwrap().unwrap().to_bytes(),
            prior
        );
        assert_eq!(session.engine.load_all_edges().unwrap(), edges);
        drop(session);
        inspect_schema(&d.path).unwrap();
    }
}

#[test]
fn native_graph_nochange_rejects_changed_witness_action_or_missing_key_immutably() {
    for case in 0..6 {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock.clone());
        let id = add(&mut session, &d, "source");
        let input = plan(id, d.parent);
        session
            .put_graph(&ctx(&session, &d, "first"), scope(&d), &input)
            .unwrap();
        let receipt = session
            .put_graph(&ctx(&session, &d, "noop"), scope(&d), &input)
            .unwrap()
            .unwrap();
        drop(session);
        {
            let _owner = crate::storage::ownership::StoreOwnership::acquire(&d.path).unwrap();
            let _directory = crate::storage::ownership::lock_directory(&d.path).unwrap();
            let db = redb::Database::open(d.path.join("meta.db")).unwrap();
            let tx = db.begin_write().unwrap();
            let key = control::scoped_key(
                receipt.store_id,
                MutationScope::Namespace(d.ns),
                receipt.operation_id.as_bytes(),
            );
            let mut stored: control::StoredReceipt = control::decode(
                tx.open_table(control::RECEIPTS)
                    .unwrap()
                    .get(key.as_slice())
                    .unwrap()
                    .unwrap()
                    .value(),
            )
            .unwrap();
            let MutationOutcome::NativeGraphUnchanged { transition } = &mut stored.receipt.outcome
            else {
                panic!()
            };
            match case {
                0 => transition.records[0].after.edge_count += 1,
                1 => transition.edges[0].weight_bits = 0.125f32.to_bits(),
                2 => {
                    for row in &mut transition.records {
                        row.before
                            .as_mut()
                            .unwrap()
                            .lifecycle
                            .lifecycle
                            .policy_revision = Generation::default();
                        row.after.lifecycle.lifecycle.policy_revision = Generation::default();
                    }
                }
                3 => {
                    tx.open_table(control::KEYS)
                        .unwrap()
                        .remove(
                            control::scoped_key(
                                receipt.store_id,
                                MutationScope::Namespace(d.ns),
                                b"noop",
                            )
                            .as_slice(),
                        )
                        .unwrap();
                }
                _ => {}
            }
            let mut effect: effects::PendingEffects = control::decode(
                tx.open_table(effects::EFFECTS)
                    .unwrap()
                    .get(receipt.operation_id.as_bytes().as_slice())
                    .unwrap()
                    .unwrap()
                    .value(),
            )
            .unwrap();
            effect.receipt = stored.receipt.clone();
            if case == 4 {
                effect.invalidated_ids.push(id);
            }
            if case == 5 {
                effect.canonical_applied = false;
                effect.peer_counts.push((id, 3));
            }
            tx.open_table(control::RECEIPTS)
                .unwrap()
                .insert(key.as_slice(), control::encode(&stored).unwrap().as_slice())
                .unwrap();
            tx.open_table(effects::EFFECTS)
                .unwrap()
                .insert(
                    receipt.operation_id.as_bytes().as_slice(),
                    control::encode(&effect).unwrap().as_slice(),
                )
                .unwrap();
            tx.commit().unwrap();
        }
        let files = bytes(&d.path);
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
            "case {case}"
        );
        assert_eq!(files, bytes(&d.path), "case {case} changed canonical files");
    }
}

#[tokio::test]
async fn native_graph_nochange_foreign_profile_requires_actual_namespace_binding() {
    reject_foreign_profile_commitment(format!("ep1:{}", "0".repeat(64)), true, true).await;
}
