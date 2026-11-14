//! Actual native owner / same-transaction outbox and replay regressions.
use super::*;
use crate::model::EdgeType;
use crate::storage::native_store::*;

fn settings() -> NativeStoreSettings {
    NativeStoreSettings {
        autolink_enabled: true,
        auto_link_threshold_bits: 0.8_f64.to_bits(),
        max_auto_links: 5,
        max_entity_links: 5,
        temporal_window_ms: 10_000,
        max_temporal_links: 5,
    }
}
fn original(d: &Data) -> NativeStoreInput {
    let mut insert = input(d);
    insert.memory.supersedes = Some(d.parent);
    NativeStoreInput {
        insert,
        check_duplicates: true,
    }
}
fn selection(intent: &NativeStoreIntent, d: &Data) -> FrozenInsertLinks {
    FrozenInsertLinks {
        generation: intent.insert_intent().capture_generation(),
        profile_id: d.profile.profile_id().unwrap(),
        static_links: vec![
            (d.parent, EdgeType::Associative, 0.9),
            (d.parent, EdgeType::Entity, 0.5),
        ],
        temporal: vec![(d.parent, 1000)],
        advisory: Some(vec![(d.parent, 0.95)]),
    }
}
fn prepare(
    session: &NativeLifecycleSession,
    d: &Data,
    input: &NativeStoreInput,
) -> PreparedNativeStore {
    let intent = session.capture_store(scope(d), input, &settings()).unwrap();
    session
        .prepare_store_links(&intent, &selection(&intent, d))
        .unwrap()
}
fn proof(receipt: &WriteReceipt) -> &NativeStoreWrite {
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
        panic!("wrong outcome")
    };
    transition
}
fn token(r: &DiskRecord) -> Generation {
    r.schema.as_ref().unwrap().lifecycle.record_generation
}
fn bytes(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(p: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for e in std::fs::read_dir(p).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                visit(&p, out);
            } else {
                out.insert(p.clone(), std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = Default::default();
    visit(path, &mut out);
    out
}

#[test]
fn native_store_atomic_edges_tokens_temporal_clock_and_replay() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let input = original(&d);
    let prepared = prepare(&session, &d, &input);
    let key = ctx(&session, &d, "linked");
    let before = session.metadata().control_state().unwrap();
    let parent = session.engine.get_record(d.parent).unwrap().unwrap();
    let receipt = session
        .store_prepared(&key, &prepared, &generated())
        .unwrap();
    let p = proof(&receipt);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        receipt.committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert_eq!(p.edges.len(), 5);
    assert_eq!(p.records.len(), 2);
    let row = session.engine.get_record(p.memory_id).unwrap().unwrap();
    assert_eq!(token(&row).get(), 1);
    assert_eq!(row.edge_count, 4);
    let after_parent = session.engine.get_record(d.parent).unwrap().unwrap();
    assert_eq!(token(&after_parent), token(&parent).next().unwrap());
    assert_eq!(after_parent.edge_count, parent.edge_count + 1);
    assert_eq!(after_parent.strength.to_bits(), parent.strength.to_bits());
    assert_eq!(after_parent.last_accessed_at, parent.last_accessed_at);
    let temporal = p
        .edges
        .iter()
        .find(|e| e.edge_type == EdgeType::Temporal)
        .unwrap();
    assert_eq!(temporal.weight_bits, 0.8_f32.to_bits());
    assert_eq!(temporal.created_at, 86_400_000);
    assert_eq!(p.advisory_matches(), Some(vec![(d.parent, 0.95)]));
    let saved = bytes(&d.path);
    let replay = session
        .store_prepared(
            &key,
            &prepared,
            &GeneratedEmbedding {
                values: vec![f32::NAN],
                profile: None,
            },
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, receipt.outcome);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(bytes(&d.path), saved);
    let mut changed_settings = settings();
    changed_settings.autolink_enabled = false;
    changed_settings.max_auto_links = 0;
    changed_settings.max_entity_links = 0;
    changed_settings.max_temporal_links = 0;
    changed_settings.temporal_window_ms = 0;
    let current_intent = session
        .capture_store(scope(&d), &input, &changed_settings)
        .unwrap();
    let current_plan = FrozenInsertLinks {
        generation: current_intent.insert_intent().capture_generation(),
        profile_id: d.profile.profile_id().unwrap(),
        static_links: vec![],
        temporal: vec![],
        advisory: Some(vec![]),
    };
    let current_prepared = session
        .prepare_store_links(&current_intent, &current_plan)
        .unwrap();
    let saved = bytes(&d.path);
    let replay = session
        .store_prepared(
            &key,
            &current_prepared,
            &GeneratedEmbedding {
                values: vec![],
                profile: None,
            },
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, receipt.outcome);
    assert!(proof(&replay).settings.autolink_enabled);
    assert_eq!(proof(&replay).edges.len(), 5);
    assert_eq!(
        proof(&replay).advisory_matches(),
        Some(vec![(d.parent, 0.95)])
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(bytes(&d.path), saved);
    let mut changed = input.clone();
    changed.check_duplicates = false;
    assert!(matches!(
        session.lookup_store(&key, &changed),
        Err(StorageError::IdempotencyConflict)
    ));
    // Later content loss/new neighbors cannot rerun the original graph selection.
    let forget = session.capture_tombstone(d.parent, None).unwrap();
    session
        .tombstone(&ctx(&session, &d, "forget"), &forget)
        .unwrap();
    let saved = bytes(&d.path);
    let calls = clock.0.load(Ordering::SeqCst);
    assert_eq!(
        session.lookup_store(&key, &input).unwrap().unwrap().outcome,
        receipt.outcome
    );
    assert_eq!(bytes(&d.path), saved);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    drop(session);
    inspect_schema(&d.path).unwrap();
    let session = open(&d, clock);
    assert_eq!(session.engine.load_all_edges().unwrap().len(), 5);
}

#[test]
fn native_store_plan_generation_scope_profile_and_provider_faults_precede_allocation() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let input = original(&d);
    let intent = session
        .capture_store(scope(&d), &input, &settings())
        .unwrap();
    let base = selection(&intent, &d);
    let before = bytes(&d.path);
    for case in 0..8 {
        let mut bad = base.clone();
        match case {
            0 => bad.profile_id = format!("ep1:{}", "0".repeat(64)),
            1 => bad.static_links[0].2 = f32::NAN,
            2 => bad.static_links.push(bad.static_links[0]),
            3 => bad.temporal[0].1 += 1,
            4 => bad.temporal.push(bad.temporal[0]),
            5 => bad.advisory = None,
            6 => bad.advisory = Some(vec![(d.parent, 0.1)]),
            _ => bad.static_links[0].0 = MemoryId::new(),
        }
        assert!(
            session.prepare_store_links(&intent, &bad).is_err(),
            "case{case}"
        );
        assert_eq!(bytes(&d.path), before);
    }
    let prepared = session.prepare_store_links(&intent, &base).unwrap();
    let mut extra = input.insert.clone();
    extra.memory.parent_id = None;
    extra.memory.supersedes = None;
    let capture = session.capture_insert(scope(&d), &extra).unwrap();
    session
        .insert_prepared(&ctx(&session, &d, "other"), &capture, &generated())
        .unwrap();
    let before = bytes(&d.path);
    let calls = clock.0.load(Ordering::SeqCst);
    assert!(matches!(
        session.store_prepared(&ctx(&session, &d, "stale"), &prepared, &generated()),
        Err(StorageError::LifecycleConflict(_))
    ));
    assert_eq!(bytes(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    // Generated faults are unavailable; supplied faults remain invalid input.
    for supplied in [false, true] {
        let mut bad = input.clone();
        bad.insert.memory.parent_id = None;
        bad.insert.memory.supersedes = None;
        if supplied {
            bad.insert.memory.embedding = Some(vec![0.; 4]);
        } else {
            bad.insert.memory.embedding = None;
            bad.insert.embedding_profile = None;
        }
        let prepared = prepare(&session, &d, &bad);
        let before = bytes(&d.path);
        let result = session.store_prepared(
            &ctx(&session, &d, "bad-vector"),
            &prepared,
            &GeneratedEmbedding {
                values: vec![0.; 4],
                profile: Some(d.profile.clone()),
            },
        );
        assert!(if supplied {
            matches!(result, Err(StorageError::InvalidMutation(_)))
        } else {
            matches!(result, Err(StorageError::ProviderVectorUnavailable(_)))
        });
        assert_eq!(bytes(&d.path), before);
    }
}

#[test]
fn native_store_advisory_disabled_checked_empty_and_temporal_uuid_ties() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock);
    for requested in [false, true] {
        let mut input = original(&d);
        input.insert.memory.parent_id = None;
        input.insert.memory.supersedes = None;
        input.check_duplicates = requested;
        let mut cfg = settings();
        cfg.autolink_enabled = false;
        let intent = session.capture_store(scope(&d), &input, &cfg).unwrap();
        let links = FrozenInsertLinks {
            generation: intent.insert_intent().capture_generation(),
            profile_id: d.profile.profile_id().unwrap(),
            static_links: vec![],
            temporal: vec![],
            advisory: requested.then(Vec::new),
        };
        let prepared = session.prepare_store_links(&intent, &links).unwrap();
        let receipt = session
            .store_prepared(
                &ctx(&session, &d, if requested { "checked" } else { "off" }),
                &prepared,
                &generated(),
            )
            .unwrap();
        assert_eq!(proof(&receipt).advisory_matches(), requested.then(Vec::new));
        assert!(proof(&receipt).edges.is_empty());
    }
    let small = MemoryId::from_bytes([1; 16]);
    let large = MemoryId::from_bytes([2; 16]);
    let mut cfg = settings();
    cfg.max_temporal_links = 1;
    assert_eq!(
        select_temporal_links(&[(large, 1100), (small, 900)], 1000, &cfg)[0].0,
        small
    );
    assert_eq!(
        select_temporal_links(&[(small, 900), (large, 1100)], 1000, &cfg)[0].0,
        small
    );
    assert!(select_temporal_links(&[(small, i64::MIN)], i64::MAX, &cfg).is_empty());
    drop(session);
    inspect_schema(&d.path).unwrap();
}

#[test]
fn native_store_process_child() {
    let Ok(path) = std::env::var("RECALLD_NATIVE_STORE_PATH") else {
        return;
    };
    let d = Data {
        path: path.into(),
        ns: NamespaceId::new(
            std::env::var("RECALLD_NATIVE_STORE_NS")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        parent: MemoryId::from_uuid(
            std::env::var("RECALLD_NATIVE_STORE_PARENT")
                .unwrap()
                .parse()
                .unwrap(),
        ),
        profile: crate::embedding::runtime::tests::profile(),
    };
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock);
    let mut input = original(&d);
    input.insert.memory.id = Some(MemoryId::from_uuid(
        std::env::var("RECALLD_NATIVE_STORE_ID")
            .unwrap()
            .parse()
            .unwrap(),
    ));
    let prepared = prepare(&session, &d, &input);
    session
        .store_prepared(&ctx(&session, &d, "crash"), &prepared, &generated())
        .unwrap();
    panic!("process did not stop at requested boundary");
}
#[test]
fn native_store_process_exit_never_loses_links_or_reexecutes_completed_selection() {
    for point in [
        "before_metadata_commit",
        "after_metadata_commit",
        "after_edge_commit",
        "after_canonical_ack",
    ] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let id = MemoryId::new();
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let session = open(&d, clock.clone());
        let prior = session.metadata().control_state().unwrap();
        let key = ctx(&session, &d, "crash");
        drop(session);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::metadata::native_records::tests::store::native_store_process_child",
                "--nocapture",
            ])
            .env("RECALLD_NATIVE_STORE_PATH", &d.path)
            .env("RECALLD_NATIVE_STORE_NS", d.ns.get().to_string())
            .env("RECALLD_NATIVE_STORE_PARENT", d.parent.to_string())
            .env("RECALLD_NATIVE_STORE_ID", id.to_string())
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
            prior.committed_generation.get() + u64::from(committed)
        );
        let mut input = original(&d);
        input.insert.memory.id = Some(id);
        let receipt = if let Some(r) = session.lookup_store(&key, &input).unwrap() {
            assert!(committed);
            r
        } else {
            assert!(!committed);
            let p = prepare(&session, &d, &input);
            session.store_prepared(&key, &p, &generated()).unwrap()
        };
        assert_eq!(receipt.replayed, committed);
        assert_eq!(proof(&receipt).memory_id, id);
        assert_eq!(session.engine.load_all_edges().unwrap().len(), 5);
        assert_eq!(
            token(&session.engine.get_record(id).unwrap().unwrap()).get(),
            1
        );
        drop(session);
        let session = open(&d, clock);
        assert_eq!(session.engine.load_all_edges().unwrap().len(), 5);
    }
}

#[tokio::test]
async fn native_store_export_restore_retains_proof_and_rejects_foreign_binding_tampering() {
    reject_foreign_store_binding(false).await;
}

#[tokio::test]
async fn native_store_foreign_receipt_dimension_must_match_namespace() {
    reject_foreign_store_binding(true).await;
}

async fn reject_foreign_store_binding(wrong_dimension: bool) {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let input = original(&d);
    let p = prepare(&session, &d, &input);
    let key = ctx(&session, &d, "linked");
    let receipt = session.store_prepared(&key, &p, &generated()).unwrap();
    let id = proof(&receipt).memory_id;
    drop(session);
    let stream = root.path().join("store.jsonl");
    let imported = root.path().join("imported");
    let exported = export_store(&d.path, &stream).unwrap();
    assert_eq!(import_store(&stream, &imported).unwrap(), exported);
    let restored =
        NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    assert!(
        restored
            .lookup_store(&key, &input)
            .unwrap()
            .unwrap()
            .replayed
    );
    assert_eq!(restored.engine.load_all_edges().unwrap().len(), 5);
    drop(restored);
    let archive = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("store.zip"),
        Some(&d.path),
        false,
    )
    .await
    .unwrap();
    let mut session = open(&d, clock.clone());
    let t = session.capture_tombstone(id, None).unwrap();
    session.tombstone(&ctx(&session, &d, "forget"), &t).unwrap();
    drop(session);
    restore_from_backup(RestoreOptions {
        backup_path: archive,
        data_dir: d.path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let session = open(&d, clock.clone());
    assert_ne!(
        session.metadata().control_state().unwrap().store_id,
        receipt.store_id
    );
    assert!(matches!(
        session.lookup_store(&key, &input),
        Err(StorageError::StoreLineageMismatch)
    ));
    assert_eq!(
        session.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
    drop(session);
    {
        let _owner = crate::storage::ownership::StoreOwnership::acquire(&d.path).unwrap();
        let _directory = crate::storage::ownership::lock_directory(&d.path).unwrap();
        let db = redb::Database::open(d.path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        let scoped = control::scoped_key(
            receipt.store_id,
            MutationScope::Namespace(d.ns),
            receipt.operation_id.as_bytes(),
        );
        let mut stored: control::StoredReceipt = control::decode(
            tx.open_table(control::RECEIPTS)
                .unwrap()
                .get(scoped.as_slice())
                .unwrap()
                .unwrap()
                .value(),
        )
        .unwrap();
        let MutationOutcome::NativeMemoryInsertedWithLinks { transition } =
            &mut stored.receipt.outcome
        else {
            panic!()
        };
        if wrong_dimension {
            transition.embedding_dim = 5;
        } else {
            transition.records[0].after.content.profile_id =
                Some(format!("ep1:{}", "0".repeat(64)));
        }
        tx.open_table(control::RECEIPTS)
            .unwrap()
            .insert(
                scoped.as_slice(),
                control::encode(&stored).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
    }
    let before = bytes(&d.path);
    assert!(
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
        "foreign receipt with wrong immutable binding accepted (dimension={wrong_dimension})"
    );
    assert_eq!(bytes(&d.path), before);
    assert!(export_store(&d.path, &root.path().join("rejected.jsonl")).is_err());
    assert_eq!(bytes(&d.path), before);
}

#[test]
fn native_store_proof_rejects_bad_tokens_edges_advisory_and_old_proof_widening() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock);
    let input = original(&d);
    let p = prepare(&session, &d, &input);
    let receipt = session
        .store_prepared(&ctx(&session, &d, "linked"), &p, &generated())
        .unwrap();
    for case in 0..8 {
        let mut bad = receipt.clone();
        let MutationOutcome::NativeMemoryInsertedWithLinks { transition: t } = &mut bad.outcome
        else {
            panic!()
        };
        match case {
            0 => {
                t.records[0].after.lifecycle.lifecycle.record_generation =
                    Generation::try_from("2".to_string()).unwrap()
            }
            1 => t.records[1].after.edge_count += 1,
            2 => t.edges[0].weight_bits = 0.5_f32.to_bits(),
            3 => t.advisory = None,
            4 => t.advisory.as_mut().unwrap()[0].score_bits = f32::NAN.to_bits(),
            5 => t.capture_generation.generation = Generation::default(),
            6 => t.settings.max_auto_links = 0,
            _ => {
                t.records[1].after.content.profile_id = None;
            }
        }
        assert!(proof(&bad).validate(&bad).is_err(), "case{case}");
    }
    // The old two-edge outcome remains strict even if all common fields fit.
    let p = proof(&receipt);
    let old = NativeRecordWrite {
        namespace_id: p.namespace_id,
        memory_id: p.memory_id,
        lifecycle_version: p.lifecycle_version,
        policy_revision: p.policy_revision,
        previous_generation: p.previous_generation,
        capture_generation: p.capture_generation,
        sampled_at: p.sampled_at,
        embedding_dim: p.embedding_dim,
        records: p.records.clone(),
        edges: p.edges.clone(),
        phase_time_rule: None,
    };
    assert!(old.validate(&receipt, true).is_err());
}

#[test]
fn native_store_canonical_foreign_and_terminal_candidates_reject_without_writes() {
    let root = tempfile::tempdir().unwrap();
    let mut foreign = MemoryId::nil();
    let d = fixture_with_seed(root.path(), |engine| {
        let mut ns = NamespaceConfig::default_namespace(0);
        ns.name = "foreign".into();
        ns.embedding_dim = 4;
        let ns = engine.create_namespace(&ns).unwrap();
        let input = crate::storage::insertion::InsertMemory {
            id: None,
            summary: "foreign candidate".into(),
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
    });
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock);
    let input = original(&d);
    let intent = session
        .capture_store(scope(&d), &input, &settings())
        .unwrap();
    let mut links = selection(&intent, &d);
    links.static_links[0].0 = foreign;
    let before = bytes(&d.path);
    assert!(matches!(
        session.prepare_store_links(&intent, &links),
        Err(StorageError::MutationNamespaceMismatch)
    ));
    assert_eq!(bytes(&d.path), before);
    let tombstone = session.capture_tombstone(d.parent, None).unwrap();
    session
        .tombstone(&ctx(&session, &d, "terminal"), &tombstone)
        .unwrap();
    let intent = session
        .capture_store(scope(&d), &input, &settings())
        .unwrap();
    let before = bytes(&d.path);
    assert!(matches!(
        session.prepare_store_links(&intent, &selection(&intent, &d)),
        Err(StorageError::LifecycleContentUnavailable)
    ));
    assert_eq!(bytes(&d.path), before);
}
