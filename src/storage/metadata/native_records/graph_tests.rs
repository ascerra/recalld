//! Real native owner/receipts/outbox, using the actual migration fixture.
use super::*;
use crate::model::EdgeType;
use crate::storage::native_graph::*;

fn add(session: &mut NativeLifecycleSession, d: &Data, key: &str) -> MemoryId {
    let mut original = input(d);
    original.memory.parent_id = None;
    let intent = session.capture_insert(scope(d), &original).unwrap();
    inserted(
        &session
            .insert_prepared(&ctx(session, d, key), &intent, &generated())
            .unwrap(),
    )
}
fn plan(source: MemoryId, target: MemoryId) -> NativeGraphInput {
    NativeGraphInput {
        edges: [EdgeType::Associative, EdgeType::Entity, EdgeType::Temporal]
            .into_iter()
            .map(|edge_type| NativeEdgeInput {
                source,
                target,
                edge_type,
                weight: 0.75,
                auto_created: true,
            })
            .collect(),
    }
}
fn token(row: &DiskRecord) -> Generation {
    row.schema.as_ref().unwrap().lifecycle.record_generation
}

#[test]
fn native_graph_rejects_real_foreign_endpoint_and_preserves_unknown_profile() {
    let root = tempfile::tempdir().unwrap();
    let mut foreign = MemoryId::nil();
    let mut foreign_target = MemoryId::nil();
    let d = fixture_with_seed(root.path(), |engine| {
        let mut ns = NamespaceConfig::default_namespace(0);
        ns.name = "foreign".into();
        ns.embedding_dim = 4;
        let ns = engine.create_namespace(&ns).unwrap();
        let input = crate::storage::insertion::InsertMemory {
            id: None,
            summary: "foreign".into(),
            full_text: None,
            tags: vec![],
            initial_stability: None,
            created_at: Some(1000),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        foreign = crate::storage::insertion::inserted_id(
            &engine
                .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &input, &[], 1000)
                .unwrap(),
        )
        .unwrap();
        foreign_target = crate::storage::insertion::inserted_id(
            &engine
                .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &input, &[], 1001)
                .unwrap(),
        )
        .unwrap();
    });
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let id = add(&mut session, &d, "source");
    let before = session.metadata().control_state().unwrap();
    assert!(matches!(
        session.put_graph(&ctx(&session, &d, "foreign"), scope(&d), &plan(id, foreign)),
        Err(StorageError::MutationNamespaceMismatch)
    ));
    assert_eq!(session.metadata().control_state().unwrap(), before);
    assert!(session.engine.load_all_edges().unwrap().is_empty());
    let foreign_row = session.engine.get_record(foreign).unwrap().unwrap();
    let cfg = session
        .engine
        .get_namespace(NamespaceId::new(foreign_row.namespace_id))
        .unwrap()
        .unwrap();
    assert!(matches!(
        cfg.schema.as_ref().unwrap().embedding_binding,
        crate::model::schema::EmbeddingBinding::UnknownLegacy
    ));
    let receipt = session
        .put_graph(
            &MutationContext::unkeyed(cfg.id),
            LifecycleScope {
                namespace_id: cfg.id,
                namespace_name: cfg.name.clone(),
                source: None,
            },
            &plan(foreign, foreign_target),
        )
        .unwrap()
        .unwrap();
    let MutationOutcome::NativeGraphChanged { transition } = receipt.outcome else {
        panic!()
    };
    assert!(
        transition
            .records
            .iter()
            .all(|r| r.after.content.profile_id.is_none())
    );
    drop(session);
    inspect_schema(&d.path).unwrap();
    let _session = open(&d, clock);
}

#[test]
fn native_graph_committed_and_unknown_failures_reconcile_without_duplicate_tokens() {
    for unknown in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock.clone());
        let id = add(&mut session, &d, "source");
        let input = plan(id, d.parent);
        let key = ctx(&session, &d, "fault");
        let prior = session.engine.get_record(id).unwrap().unwrap();
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
                    MutationOutcome::NativeGraphChanged { .. }
                ));
                receipt.operation_id
            }
            other => panic!("unexpected failure {other:?}"),
        };
        drop(session);
        let mut session = open(&d, clock);
        let receipt = session.put_graph(&key, scope(&d), &input).unwrap().unwrap();
        assert!(receipt.replayed);
        assert_eq!(receipt.operation_id, operation);
        let row = session.engine.get_record(id).unwrap().unwrap();
        assert_eq!(token(&row), token(&prior).next().unwrap());
        assert_eq!(row.edge_count, 3);
        assert_eq!(session.engine.load_all_edges().unwrap().len(), 3);
    }
}

#[test]
fn native_graph_process_child() {
    let Ok(path) = std::env::var("RECALLD_NATIVE_GRAPH_PATH") else {
        return;
    };
    let d = Data {
        path: path.into(),
        ns: NamespaceId::new(
            std::env::var("RECALLD_NATIVE_GRAPH_NS")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        parent: MemoryId::from_uuid(
            std::env::var("RECALLD_NATIVE_GRAPH_TARGET")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        profile: crate::embedding::runtime::tests::profile(),
    };
    let id = MemoryId::from_uuid(
        std::env::var("RECALLD_NATIVE_GRAPH_SOURCE")
            .unwrap()
            .parse()
            .unwrap(),
    );
    let mut session = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    session
        .put_graph(&ctx(&session, &d, "crash"), scope(&d), &plan(id, d.parent))
        .unwrap();
    panic!("crash point not reached");
}

#[test]
fn native_graph_review_preserves_signed_zero_weight_bits() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let id = add(&mut session, &d, "source");
    let mut input = plan(id, d.parent);
    input.edges.truncate(1);
    input.edges[0].weight = 0.0;
    session
        .put_graph(&ctx(&session, &d, "positive-zero"), scope(&d), &input)
        .unwrap()
        .unwrap();
    let prior = session.engine.get_record(id).unwrap().unwrap();
    input.edges[0].weight = -0.0;
    session
        .put_graph(&ctx(&session, &d, "negative-zero"), scope(&d), &input)
        .unwrap()
        .unwrap();
    assert_eq!(
        session.engine.load_all_edges().unwrap()[0].weight.to_bits(),
        (-0.0f32).to_bits()
    );
    assert_eq!(
        token(&session.engine.get_record(id).unwrap().unwrap()),
        token(&prior).next().unwrap()
    );
    drop(session);
    inspect_schema(&d.path).unwrap();
    let session = open(&d, clock);
    assert_eq!(
        session.engine.load_all_edges().unwrap()[0].weight.to_bits(),
        (-0.0f32).to_bits()
    );
}

#[test]
fn native_graph_keyed_noop_never_reports_unbound_success_then_reexecutes() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let id = add(&mut session, &d, "source");
    let a = plan(id, d.parent);
    let committed_key = ctx(&session, &d, "committed-a");
    let original = session
        .put_graph(&committed_key, scope(&d), &a)
        .unwrap()
        .unwrap();
    let noop_key = ctx(&session, &d, "fresh-key-noop");
    let before = session.metadata().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    let noop = session.put_graph(&noop_key, scope(&d), &a);
    let after_noop = session.metadata().control_state().unwrap();
    let after_calls = clock.0.load(Ordering::SeqCst);
    let mut b = a.clone();
    b.edges[0].weight = 0.25;
    session
        .put_graph(&ctx(&session, &d, "writer-b"), scope(&d), &b)
        .unwrap()
        .unwrap();
    let after_b = session.metadata().control_state().unwrap();
    let edges_b = session.engine.load_all_edges().unwrap();
    if noop.is_ok() {
        // Any successful keyed no-op must be durably replayable. This exposes
        // the original bug after an intervening real writer, not just its None.
        let retry = session.put_graph(&noop_key, scope(&d), &a).unwrap();
        assert_eq!(
            session.metadata().control_state().unwrap(),
            after_b,
            "successful keyed no-op was not bound; retry performed another mutation"
        );
        assert!(retry.is_some_and(|r| r.replayed));
        assert!(matches!(
            session.put_graph(&noop_key, scope(&d), &b),
            Err(StorageError::IdempotencyConflict)
        ));
    } else {
        // This bounded API explicitly rejects a fresh keyed no-op. It promises
        // no successful outcome/key reservation, and does not sample the clock.
        assert!(matches!(noop, Err(StorageError::InvalidMutation(_))));
        assert_eq!(after_noop, before);
        assert_eq!(after_calls, calls);
    }
    let replay = session
        .put_graph(&committed_key, scope(&d), &a)
        .unwrap()
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, original.operation_id);
    assert_eq!(session.metadata().control_state().unwrap(), after_b);
    assert_eq!(session.engine.load_all_edges().unwrap(), edges_b);
    assert!(matches!(
        session.put_graph(&committed_key, scope(&d), &b),
        Err(StorageError::IdempotencyConflict)
    ));
}

#[test]
fn native_graph_ghost_cannot_activate_but_retained_parent_can_gain_structural_edge() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture_with_seed(root.path(), |engine| {
        let (id, row) = engine.scan_all().unwrap().remove(0);
        let ns = engine
            .get_namespace(NamespaceId::new(row.namespace_id))
            .unwrap()
            .unwrap();
        assert!(
            engine
                .apply_legacy_decay(id, &row, &ns, DecayPhase::Summary, 0.2, 0.2)
                .unwrap()
        );
        let row = engine.get_record(id).unwrap().unwrap();
        assert!(
            engine
                .apply_legacy_decay(id, &row, &ns, DecayPhase::Ghost, 0.1, 0.1)
                .unwrap()
        );
    });
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let id = add(&mut session, &d, "live");
    for (source, target) in [(id, d.parent), (d.parent, id)] {
        let before = session.metadata().control_state().unwrap();
        assert!(matches!(
            session.put_graph(
                &ctx(&session, &d, "ghost"),
                scope(&d),
                &plan(source, target)
            ),
            Err(StorageError::LifecycleContentUnavailable)
        ));
        assert_eq!(session.metadata().control_state().unwrap(), before);
    }
    let before = session.engine.get_record(d.parent).unwrap().unwrap();
    let mut structural = plan(d.parent, id);
    structural.edges.truncate(1);
    structural.edges[0].edge_type = EdgeType::ParentChild;
    session
        .put_graph(&ctx(&session, &d, "parent"), scope(&d), &structural)
        .unwrap()
        .unwrap();
    let mut expected = before.clone();
    expected.edge_count += 1;
    expected
        .schema
        .as_mut()
        .unwrap()
        .lifecycle
        .record_generation = token(&before).next().unwrap();
    assert_eq!(
        session
            .engine
            .get_record(d.parent)
            .unwrap()
            .unwrap()
            .to_bytes(),
        expected.to_bytes()
    );
    drop(session);
    inspect_schema(&d.path).unwrap();
    let _session = open(&d, clock);
}

#[tokio::test]
async fn native_graph_review_rejects_malformed_foreign_profile_commitment() {
    reject_foreign_profile_commitment("not-an-ep1-commitment".into(), false).await;
}

#[tokio::test]
async fn native_graph_review_rejects_wrong_foreign_profile_binding() {
    reject_foreign_profile_commitment(format!("ep1:{}", "0".repeat(64)), true).await;
}

async fn reject_foreign_profile_commitment(profile: String, well_formed: bool) {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let id = add(&mut session, &d, "source");
    let receipt = session
        .put_graph(&ctx(&session, &d, "edges"), scope(&d), &plan(id, d.parent))
        .unwrap()
        .unwrap();
    drop(session);
    let archive = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("profile.zip"),
        Some(&d.path),
        false,
    )
    .await
    .unwrap();
    restore_from_backup(RestoreOptions {
        backup_path: archive,
        data_dir: d.path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    assert_ne!(
        inspect_schema(&d.path).unwrap().control.store_id,
        receipt.store_id
    );
    let malformed = {
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
        let MutationOutcome::NativeGraphChanged { transition } = &mut stored.receipt.outcome else {
            panic!()
        };
        transition.records[0]
            .before
            .as_mut()
            .unwrap()
            .content
            .profile_id = Some(profile.clone());
        transition.records[0].after.content.profile_id = Some(profile.clone());
        tx.open_table(control::RECEIPTS)
            .unwrap()
            .insert(key.as_slice(), control::encode(&stored).unwrap().as_slice())
            .unwrap();
        // Restore normally clears the old-origin outbox. If retained, keep the
        // same receipt so the regression tests the proof, not a trivial mismatch.
        let mut table = tx.open_table(effects::EFFECTS).unwrap();
        let pending = table
            .get(receipt.operation_id.as_bytes().as_slice())
            .unwrap()
            .map(|v| v.value().to_vec());
        if let Some(pending) = pending {
            let mut effect: effects::PendingEffects = control::decode(&pending).unwrap();
            effect.receipt = stored.receipt.clone();
            table
                .insert(
                    receipt.operation_id.as_bytes().as_slice(),
                    control::encode(&effect).unwrap().as_slice(),
                )
                .unwrap();
        }
        drop(table);
        tx.commit().unwrap();
        stored
    };
    let before = bytes(&d.path);
    assert!(
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
        "foreign-origin graph receipt accepted invalid profile commitment"
    );
    assert_eq!(bytes(&d.path), before);
    assert!(
        crate::storage::logical_export::export_store(&d.path, &root.path().join("rejected.jsonl"))
            .is_err()
    );
    assert_eq!(bytes(&d.path), before);
    let MutationOutcome::NativeGraphChanged { transition } = &malformed.receipt.outcome else {
        panic!()
    };
    assert_eq!(transition.validate(&malformed.receipt).is_ok(), well_formed);
}

#[test]
fn native_graph_process_exit_recovery_before_after_metadata_and_edges() {
    for point in [
        "before_metadata_commit",
        "after_metadata_commit",
        "after_edge_commit",
        "after_canonical_ack",
    ] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock.clone());
        let id = add(&mut session, &d, "source");
        let prior = session.engine.get_record(id).unwrap().unwrap();
        let before = session.metadata().control_state().unwrap();
        let key = ctx(&session, &d, "crash");
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
        let committed = point != "before_metadata_commit";
        assert_eq!(
            session
                .metadata()
                .control_state()
                .unwrap()
                .committed_generation
                .get(),
            before.committed_generation.get() + u64::from(committed)
        );
        assert_eq!(
            session.engine.load_all_edges().unwrap().len(),
            if committed { 3 } else { 0 }
        );
        let receipt = session
            .put_graph(&key, scope(&d), &plan(id, d.parent))
            .unwrap()
            .unwrap();
        assert_eq!(receipt.replayed, committed);
        assert_eq!(
            token(&session.engine.get_record(id).unwrap().unwrap()),
            token(&prior).next().unwrap()
        );
        drop(session);
        let session = open(&d, clock);
        assert_eq!(session.engine.load_all_edges().unwrap().len(), 3);
    }
}

fn bytes(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(at: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.insert(p.clone(), std::fs::read(p).unwrap());
            }
        }
    }
    let mut out = Default::default();
    walk(path, &mut out);
    out
}

#[test]
fn native_graph_corrupt_proof_outbox_and_physical_edges_reject_before_live_open() {
    for case in 0..4 {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut session = open(&d, clock.clone());
        let id = add(&mut session, &d, "source");
        let receipt = session
            .put_graph(&ctx(&session, &d, "edges"), scope(&d), &plan(id, d.parent))
            .unwrap()
            .unwrap();
        drop(session);
        // Corrupt only a real committed fixture, under both actual ownership locks.
        {
            let _owner = crate::storage::ownership::StoreOwnership::acquire(&d.path).unwrap();
            let _directory = crate::storage::ownership::lock_directory(&d.path).unwrap();
            if case == 3 {
                let edges = crate::storage::EdgeStore::open(&d.path.join("edges.db")).unwrap();
                let mut edge = edges.load_all_edges().unwrap().remove(0);
                edge.weight = 0.125;
                edges.apply_effects(&[edge], &[]).unwrap();
            } else {
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
                let MutationOutcome::NativeGraphChanged { transition } =
                    &mut stored.receipt.outcome
                else {
                    panic!()
                };
                let source = transition
                    .records
                    .iter_mut()
                    .find(|r| r.memory_id == id)
                    .unwrap();
                match case {
                    0 => source.after.edge_count += 1,
                    1 => source.after.lifecycle.lifecycle.lifecycle_updated_at += 1,
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
                if case == 2 {
                    effect.puts[0].weight = 0.125;
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
        }
        let before = bytes(&d.path);
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
            "case {case}"
        );
        assert_eq!(
            bytes(&d.path),
            before,
            "case {case} modified original files"
        );
    }
}

#[tokio::test]
async fn native_graph_export_import_restore_preserves_history_and_foreign_lineage_rejection() {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let id = add(&mut session, &d, "source");
    let input = plan(id, d.parent);
    let key = ctx(&session, &d, "edges");
    let receipt = session.put_graph(&key, scope(&d), &input).unwrap().unwrap();
    let expected = session.engine.load_all_edges().unwrap();
    drop(session);
    let stream = root.path().join("graph.jsonl");
    let imported = root.path().join("imported");
    let exported = export_store(&d.path, &stream).unwrap();
    assert_eq!(import_store(&stream, &imported).unwrap(), exported);
    let mut restored =
        NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    assert_eq!(restored.engine.load_all_edges().unwrap(), expected);
    assert!(
        restored
            .put_graph(&key, scope(&d), &input)
            .unwrap()
            .unwrap()
            .replayed
    );
    drop(restored);
    let backup = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("graph.zip"),
        Some(&d.path),
        false,
    )
    .await
    .unwrap();
    let mut session = open(&d, clock.clone());
    let terminal = session.capture_tombstone(id, None).unwrap();
    session
        .tombstone(&ctx(&session, &d, "forget"), &terminal)
        .unwrap();
    drop(session);
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: d.path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let mut restored = open(&d, clock);
    assert_ne!(
        restored.metadata().control_state().unwrap().store_id,
        receipt.store_id
    );
    assert_eq!(
        restored.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
    assert!(matches!(
        restored.put_graph(&key, scope(&d), &input),
        Err(StorageError::StoreLineageMismatch)
    ));
    let read = restored.metadata().db.begin_read().unwrap();
    let table = read.open_table(control::RECEIPTS).unwrap();
    let historical = control::validate_receipt(
        table
            .get(
                control::scoped_key(
                    receipt.store_id,
                    MutationScope::Namespace(d.ns),
                    receipt.operation_id.as_bytes(),
                )
                .as_slice(),
            )
            .unwrap()
            .unwrap()
            .value(),
    )
    .unwrap();
    assert_eq!(historical.receipt.outcome, receipt.outcome);
}

#[test]
fn native_graph_multi_edge_updates_noops_replay_and_history() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let id = add(&mut session, &d, "source");
    let before = session.engine.get_record(id).unwrap().unwrap();
    let target = session.engine.get_record(d.parent).unwrap().unwrap();
    let input = plan(id, d.parent);
    let key = ctx(&session, &d, "edges");
    let calls = clock.0.load(Ordering::SeqCst);
    let receipt = session.put_graph(&key, scope(&d), &input).unwrap().unwrap();
    assert_eq!(clock.0.load(Ordering::SeqCst), calls + 1);
    let mut expected = before.clone();
    expected.edge_count = 3;
    expected
        .schema
        .as_mut()
        .unwrap()
        .lifecycle
        .record_generation = token(&before).next().unwrap();
    assert_eq!(
        session.engine.get_record(id).unwrap().unwrap().to_bytes(),
        expected.to_bytes()
    );
    assert_eq!(
        session
            .engine
            .get_record(d.parent)
            .unwrap()
            .unwrap()
            .to_bytes(),
        target.to_bytes()
    );
    assert_eq!(session.engine.load_all_edges().unwrap().len(), 3);
    let control = session.metadata().control_state().unwrap();
    let mut unkeyed = ctx(&session, &d, "noop");
    unkeyed.idempotency_key = None;
    assert!(
        session
            .put_graph(&unkeyed, scope(&d), &input)
            .unwrap()
            .is_none()
    );
    assert_eq!(session.metadata().control_state().unwrap(), control);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls + 1);
    let mut changed = input.clone();
    changed.edges[0].weight = 0.5;
    assert!(matches!(
        session.put_graph(&key, scope(&d), &changed),
        Err(StorageError::IdempotencyConflict)
    ));
    session
        .put_graph(&ctx(&session, &d, "update"), scope(&d), &changed)
        .unwrap()
        .unwrap();
    let updated = session.engine.get_record(id).unwrap().unwrap();
    assert_eq!(updated.edge_count, 3);
    assert_eq!(token(&updated), token(&expected).next().unwrap());
    // Replay returns historical count/proof after another update and tombstone.
    let terminal = session.capture_tombstone(id, None).unwrap();
    session
        .tombstone(&ctx(&session, &d, "forget"), &terminal)
        .unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    let control = session.metadata().control_state().unwrap();
    let mut replay_scope = scope(&d);
    replay_scope.namespace_name = "not current".into();
    let replay = session
        .put_graph(&key, replay_scope, &input)
        .unwrap()
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(session.metadata().control_state().unwrap(), control);
    drop(session);
    inspect_schema(&d.path).unwrap();
    let session = open(&d, clock);
    assert_eq!(session.engine.load_all_edges().unwrap().len(), 3);
}

#[test]
fn native_graph_capture_cas_scope_eligibility_and_input_bounds_reject_without_write() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let id = add(&mut session, &d, "source");
    let target = add(&mut session, &d, "target");
    let input = plan(id, target);
    let stale = session.capture_graph(scope(&d), &input).unwrap();
    let feedback = session.capture_feedback(target, None, 3).unwrap();
    session
        .apply_feedback(&ctx(&session, &d, "feedback"), &feedback)
        .unwrap();
    let before = session.metadata().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    assert!(matches!(
        session.put_graph_prepared(&ctx(&session, &d, "stale"), &stale),
        Err(StorageError::LifecycleConflict(_))
    ));
    let mut cas = ctx(&session, &d, "cas");
    cas.expected_generation = Some(Generation::default());
    assert!(session.put_graph(&cas, scope(&d), &input).is_err());
    let mut foreign = scope(&d);
    foreign.namespace_id = NamespaceId::new(999);
    assert!(
        session
            .put_graph(&ctx(&session, &d, "scope"), foreign, &input)
            .is_err()
    );
    let mut invalid = input.clone();
    invalid.edges.push(invalid.edges[0].clone());
    assert!(
        session
            .put_graph(&ctx(&session, &d, "duplicate"), scope(&d), &invalid)
            .is_err()
    );
    invalid.edges = vec![invalid.edges[0].clone(); MAX_NATIVE_GRAPH_EDGES + 1];
    assert!(session.capture_graph(scope(&d), &invalid).is_err());
    invalid = input.clone();
    invalid.edges[0].weight = f32::NAN;
    assert!(session.capture_graph(scope(&d), &invalid).is_err());
    assert_eq!(session.metadata().control_state().unwrap(), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    let terminal = session.capture_tombstone(target, None).unwrap();
    session
        .tombstone(&ctx(&session, &d, "terminal"), &terminal)
        .unwrap();
    let before = session.metadata().control_state().unwrap();
    assert!(matches!(
        session.put_graph(&ctx(&session, &d, "dead"), scope(&d), &input),
        Err(StorageError::LifecycleContentUnavailable)
    ));
    assert_eq!(session.metadata().control_state().unwrap(), before);
    // Retained structural target is allowed for Supersedes, never association.
    let mut structural = input.clone();
    structural.edges.truncate(1);
    structural.edges[0].edge_type = EdgeType::Supersedes;
    session
        .put_graph(&ctx(&session, &d, "supersedes"), scope(&d), &structural)
        .unwrap()
        .unwrap();
    drop(session);
    inspect_schema(&d.path).unwrap();
}

#[test]
fn native_graph_supersedes_cycle_branch_and_multi_source_token_once() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock);
    let a = add(&mut session, &d, "a");
    let b = add(&mut session, &d, "b");
    let mut input = plan(a, b);
    input.edges.truncate(1);
    input.edges[0].edge_type = EdgeType::Supersedes;
    session
        .put_graph(&ctx(&session, &d, "sup"), scope(&d), &input)
        .unwrap();
    let before = session.metadata().control_state().unwrap();
    let mut cycle = input.clone();
    cycle.edges[0].source = b;
    cycle.edges[0].target = a;
    assert!(matches!(
        session.put_graph(&ctx(&session, &d, "cycle"), scope(&d), &cycle),
        Err(StorageError::EdgeConflict(_))
    ));
    let mut branch = input.clone();
    branch.edges[0].source = d.parent;
    assert!(matches!(
        session.put_graph(&ctx(&session, &d, "branch"), scope(&d), &branch),
        Err(StorageError::EdgeConflict(_))
    ));
    assert_eq!(session.metadata().control_state().unwrap(), before);
    let before_a = session.engine.get_record(a).unwrap().unwrap();
    let before_b = session.engine.get_record(b).unwrap().unwrap();
    let mut input = plan(a, b);
    input.edges.extend(plan(b, a).edges);
    session
        .put_graph(&ctx(&session, &d, "reciprocal"), scope(&d), &input)
        .unwrap();
    assert_eq!(
        token(&session.engine.get_record(a).unwrap().unwrap()),
        token(&before_a).next().unwrap()
    );
    assert_eq!(
        token(&session.engine.get_record(b).unwrap().unwrap()),
        token(&before_b).next().unwrap()
    );
    drop(session);
    inspect_schema(&d.path).unwrap();
}
