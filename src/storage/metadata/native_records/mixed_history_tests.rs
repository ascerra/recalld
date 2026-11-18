//! Real composed writer histories; no direct proof seeding for positive cases.
use super::*;
use crate::model::{DecayPhase, EdgeType};
use crate::storage::native_graph::{NativeEdgeInput, NativeGraphInput};
use crate::storage::native_maintenance::NativeDecayResult;
use crate::storage::native_store::{FrozenInsertLinks, NativeStoreInput, NativeStoreSettings};
use crate::storage::{logical_export, validation};

pub(super) fn linked(s: &mut NativeLifecycleSession, d: &Data, key: &str) -> MemoryId {
    let mut original = input(d);
    original.memory.initial_stability = Some(1.);
    let original = NativeStoreInput {
        insert: original,
        check_duplicates: false,
    };
    let settings = NativeStoreSettings {
        autolink_enabled: true,
        auto_link_threshold_bits: 0.8_f64.to_bits(),
        max_auto_links: 1,
        max_entity_links: 1,
        temporal_window_ms: 0,
        max_temporal_links: 0,
    };
    let intent = s.capture_store(scope(d), &original, &settings).unwrap();
    let plan = FrozenInsertLinks {
        generation: intent.insert_intent().capture_generation(),
        profile_id: Some(d.profile.profile_id().unwrap()),
        static_links: vec![(d.parent, EdgeType::Associative, 0.9)],
        temporal: vec![],
        advisory: None,
    };
    let prepared = s.prepare_store_links(&intent, &plan).unwrap();
    let receipt = s
        .store_prepared(&ctx(s, d, key), &prepared, &generated())
        .unwrap();
    match receipt.outcome {
        MutationOutcome::NativeMemoryInsertedWithLinks { transition } => transition.memory_id,
        _ => panic!("wrong creation proof"),
    }
}
pub(super) fn graph(s: &mut NativeLifecycleSession, d: &Data, id: MemoryId) {
    let input = NativeGraphInput {
        edges: vec![NativeEdgeInput {
            source: id,
            target: d.parent,
            edge_type: EdgeType::Entity,
            weight: 0.75,
            auto_created: true,
        }],
    };
    s.put_graph(&ctx(s, d, "graph"), scope(d), &input)
        .unwrap()
        .unwrap();
    let key = ctx(s, d, "graph-noop");
    let receipt = s.put_graph(&key, scope(d), &input).unwrap().unwrap();
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativeGraphUnchanged { .. }
    ));
    let config = s.engine.get_namespace(d.ns).unwrap().unwrap();
    let mut replacement = crate::storage::catalog::NamespacePolicy::from(&config);
    replacement.initial_stability = 9.;
    s.update_policy(&ctx(s, d, "numeric"), scope(d), &replacement)
        .unwrap()
        .unwrap();
    let mut changed = input.clone();
    changed.edges[0].weight = 0.5;
    s.put_graph(&ctx(s, d, "graph-change"), scope(d), &changed)
        .unwrap()
        .unwrap();
    let before = s.engine.control_state().unwrap();
    let replay = s.put_graph(&key, scope(d), &input).unwrap().unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, receipt.outcome);
    assert_eq!(replay.committed_generation, receipt.committed_generation);
    assert_eq!(s.engine.control_state().unwrap(), before);
}
pub(super) fn phases(s: &mut NativeLifecycleSession, d: &Data, id: MemoryId, count: usize) {
    let policy = NativePolicyInputs::default();
    let original = s.engine.get_record(id).unwrap().unwrap();
    for n in 0..count {
        let intent = s
            .metadata()
            .capture_native_decay(id, d.ns, &policy, 10_000 * 86_400_000_i64)
            .unwrap();
        assert!(matches!(
            s.engine
                .apply_native_decay(
                    &ctx(s, d, &format!("phase-{id}-{n}")),
                    &intent,
                    &policy,
                    None
                )
                .unwrap(),
            NativeDecayResult::Committed(_)
        ));
        if n == 1 {
            let ghost = s.engine.get_record(id).unwrap().unwrap();
            assert_eq!(ghost.phase, DecayPhase::Ghost);
            assert_eq!(
                ghost.schema.as_ref().unwrap().vector,
                original.schema.as_ref().unwrap().vector
            );
            assert!(ghost.schema.as_ref().unwrap().labels.is_empty());
        }
    }
}
pub(super) fn purge(s: &mut NativeLifecycleSession, d: &Data) {
    let mut p = s
        .engine
        .begin_namespace_purge(&ctx(s, d, "purge"), true)
        .unwrap();
    while p.stage != crate::storage::purge::PurgeStage::Complete {
        p = s
            .engine
            .resume_namespace_purge(p.store_id, d.ns, p.start_receipt.operation_id, 1)
            .unwrap();
    }
    assert!(s.engine.get_namespace(d.ns).unwrap().is_none());
}
#[test]
fn mixed_history_harddelete_reverses_removed_edges_before_graph_noop_and_store() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let mut s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let id = linked(&mut s, &d, "linked");
    graph(&mut s, &d, id);
    phases(&mut s, &d, id, 3);
    assert!(s.engine.get_record(id).unwrap().is_none());
    drop(s);
    inspect_schema(&d.path).expect("valid graph/store history must survive hard deletion");
}
#[test]
fn mixed_history_purge_binds_retired_namespace_before_graph_and_store_validation() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let mut s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let id = linked(&mut s, &d, "linked");
    graph(&mut s, &d, id);
    phases(&mut s, &d, id, 2);
    purge(&mut s, &d);
    drop(s);
    inspect_schema(&d.path).expect("valid graph/store history must bind to retired namespace");
}

fn bytes(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(path: &Path, result: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(&path, result);
            } else {
                result.insert(path.clone(), std::fs::read(path).unwrap());
            }
        }
    }
    let mut result = Default::default();
    visit(path, &mut result);
    result
}
pub(super) fn receipts(path: &Path) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
    use redb::ReadableTable;
    let (_copy, db) = validation::inspect_database(&path.join("meta.db")).unwrap();
    let read = db.begin_read().unwrap();
    read.open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|row| {
            let (k, v) = row.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}
pub(super) fn payloads(stream: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(stream)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let line: serde_json::Value = serde_json::from_str(line).unwrap();
            let item = &line["data"]["item"];
            matches!(item["kind"].as_str(), Some("memory" | "namespace")).then(|| item.clone())
        })
        .collect()
}
fn kept(s: &mut NativeLifecycleSession, d: &Data) -> MemoryId {
    use crate::storage::native_catalog::NativeNamespaceInput;
    let mut key = ctx(s, d, "kept-namespace");
    key.scope = MutationScope::Catalog;
    let receipt = s
        .engine
        .create_native_namespace(
            &key,
            &NativeNamespaceInput {
                name: "kept".into(),
                embedding_dim: Some(4),
                embedding_profile_id: None,
                initial_stability: None,
                desired_retention: None,
                decay_rate_multiplier: None,
            },
            &d.profile,
            &NativePolicyInputs::default(),
            &FixedClock(AtomicUsize::new(0)),
            None,
        )
        .unwrap();
    let MutationOutcome::NativeNamespaceCreated { transition } = receipt.outcome else {
        panic!()
    };
    let k = Data {
        path: d.path.clone(),
        ns: transition.namespace_id,
        parent: MemoryId::nil(),
        profile: d.profile.clone(),
    };
    let mut original = input(&k);
    original.memory.parent_id = None;
    original.memory.summary = "kept exact summary é中🙂".into();
    original.memory.full_text = Some("kept exact full text é中🙂".into());
    original.memory.embedding = Some(vec![2., -0., 0., 0.]);
    original.memory.initial_stability = Some(f32::from_bits(0x3f800001));
    let scope = LifecycleScope {
        namespace_id: k.ns,
        namespace_name: "kept".into(),
        source: None,
    };
    let intent = s.capture_insert(scope, &original).unwrap();
    let r = s
        .insert_prepared(&ctx(s, &k, "kept-record"), &intent, &generated())
        .unwrap();
    inserted(&r)
}
pub(super) async fn export_import_fork(root: &Path, d: &Data) -> PathBuf {
    let original = receipts(&d.path);
    let stream = root.join("mixed.jsonl");
    let imported = root.join("imported");
    let before = bytes(&d.path);
    let exported = logical_export::export_store(&d.path, &stream).unwrap();
    assert_eq!(bytes(&d.path), before);
    assert_eq!(
        logical_export::import_store(&stream, &imported).unwrap(),
        exported
    );
    let imported_stream = root.join("imported.jsonl");
    let again = logical_export::export_store(&imported, &imported_stream).unwrap();
    assert_eq!(again.source, exported.source);
    assert_eq!(again.logical_sha256, exported.logical_sha256);
    assert_eq!(payloads(&stream), payloads(&imported_stream));
    assert_eq!(receipts(&imported), original);
    let archive = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.join("mixed.zip"),
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
    let fork = inspect_schema(&imported).unwrap();
    assert_ne!(fork.control.store_id, exported.source.store_id);
    let fork_stream = root.join("fork.jsonl");
    logical_export::export_store(&imported, &fork_stream).unwrap();
    assert_eq!(payloads(&stream), payloads(&fork_stream));
    let fork_receipts = receipts(&imported);
    for (key, value) in original {
        assert_eq!(fork_receipts.get(&key), Some(&value));
    }
    imported
}
#[tokio::test]
async fn mixed_history_policy_graph_maintenance_purge_export_import_fork_preserves_payload() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let deleted = linked(&mut s, &d, "deleted");
    graph(&mut s, &d, deleted);
    let ghost = linked(&mut s, &d, "ghost");
    let kept = kept(&mut s, &d);
    phases(&mut s, &d, deleted, 3);
    phases(&mut s, &d, ghost, 2);
    drop(s);
    inspect_schema(&d.path).unwrap();
    let mut s = open(&d, clock.clone());
    purge(&mut s, &d);
    let ledger = s.metadata().removal_ledger_export().unwrap();
    assert!(
        ledger
            .entries
            .iter()
            .any(|e| e.memory_uuid == deleted.into_inner())
    );
    assert!(
        ledger
            .entries
            .iter()
            .any(|e| e.memory_uuid == ghost.into_inner())
    );
    assert_eq!(s.metadata().scan_all().unwrap().len(), 1);
    let record = s.engine.get_record(kept).unwrap().unwrap();
    assert_eq!(record.stability.to_bits(), 0x3f800001);
    assert_eq!(record.schema.as_ref().unwrap().labels, input(&d).labels());
    assert_eq!(
        s.engine
            .get_vector(NamespaceId::new(record.namespace_id), record.vector_slot)
            .unwrap()
            .unwrap()[1]
            .to_bits(),
        (-0_f32).to_bits()
    );
    drop(s);
    let imported = export_import_fork(root.path(), &d).await;
    for _ in 0..2 {
        let s =
            NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), clock.clone())
                .unwrap();
        assert_eq!(s.metadata().scan_all().unwrap().len(), 1);
        let after = s.engine.get_record(kept).unwrap().unwrap();
        assert_eq!(
            after.schema.as_ref().unwrap().labels,
            record.schema.as_ref().unwrap().labels
        );
        assert_eq!(
            after.schema.as_ref().unwrap().lifecycle,
            record.schema.as_ref().unwrap().lifecycle
        );
        let fork_ledger = s.metadata().removal_ledger_export().unwrap();
        assert_eq!(fork_ledger.entries, ledger.entries);
        assert_eq!(
            fork_ledger.state.tracking_started_at.store_id,
            s.metadata().control_state().unwrap().store_id
        );
        assert_eq!(
            fork_ledger.state.tracking_started_at.generation,
            Generation::default()
        );
        assert_eq!(
            fork_ledger.state.prior_history,
            crate::storage::metadata::removal_ledger::PriorHistory::Unknown
        );
    }
    // A fork has fresh conservative coverage. Restore capture also downgrades
    // unproven pre-lineage coverage to Unknown, while retaining its origin boundary.
    let (_copy, db) = validation::inspect_database(&imported.join("meta.db")).unwrap();
    let read = db.begin_read().unwrap();
    let control = control::validate_existing(&read).unwrap().unwrap();
    let lineage = crate::storage::lineage::read_lineage(&read, &control)
        .unwrap()
        .unwrap();
    assert!(lineage.ledger_sources.iter().any(|source| {
        source.coverage.ledger_version == ledger.state.ledger_version
            && source.coverage.tracking_started_at == ledger.state.tracking_started_at
            && source.coverage.prior_history
                == crate::storage::metadata::removal_ledger::PriorHistory::Unknown
    }));
    let payload = payloads(&root.path().join("fork.jsonl"));
    let memories: Vec<_> = payload.iter().filter(|p| p["kind"] == "memory").collect();
    assert_eq!(memories.len(), 1);
    assert_eq!(
        memories[0]["data"]["fullText"],
        "kept exact full text é中🙂"
    );
    assert_eq!(memories[0]["data"]["embeddingBits"][1], 0x80000000_u32);
}
#[tokio::test]
async fn mixed_history_foreign_purged_bindings_and_noop_key_reject_immutably() {
    use redb::ReadableTable;
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let mut s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let id = linked(&mut s, &d, "linked");
    graph(&mut s, &d, id);
    purge(&mut s, &d);
    drop(s);
    let path = export_import_fork(root.path(), &d).await;
    let original_db = std::fs::read(path.join("meta.db")).unwrap();
    for case in 0..4 {
        {
            let _owner = crate::storage::ownership::StoreOwnership::acquire(&path).unwrap();
            let _directory = crate::storage::ownership::lock_directory(&path).unwrap();
            let db = redb::Database::open(path.join("meta.db")).unwrap();
            let tx = db.begin_write().unwrap();
            let mut table = tx.open_table(control::RECEIPTS).unwrap();
            let (key, mut stored) = table
                .iter()
                .unwrap()
                .map(|row| {
                    let (key, value) = row.unwrap();
                    (
                        key.value().to_vec(),
                        control::decode::<control::StoredReceipt>(value.value()).unwrap(),
                    )
                })
                .find(|(_, r)| {
                    if case == 0 {
                        matches!(
                            r.receipt.outcome,
                            MutationOutcome::NativeMemoryInsertedWithLinks { .. }
                        )
                    } else {
                        matches!(
                            r.receipt.outcome,
                            MutationOutcome::NativeGraphUnchanged { .. }
                        )
                    }
                })
                .unwrap();
            match &mut stored.receipt.outcome {
                MutationOutcome::NativeMemoryInsertedWithLinks { transition } => {
                    transition.embedding_dim = 5
                }
                MutationOutcome::NativeGraphUnchanged { transition } => match case {
                    1 => {
                        for r in &mut transition.records {
                            r.after.content.profile_id = Some(format!("ep1:{}", "0".repeat(64)));
                            r.before = Some(r.after.clone());
                        }
                    }
                    2 => {
                        transition.policy_revision =
                            Generation::try_from("99".to_string()).unwrap();
                        for r in &mut transition.records {
                            r.after.lifecycle.lifecycle.policy_revision =
                                transition.policy_revision;
                            r.before = Some(r.after.clone());
                        }
                    }
                    3 => {
                        let mut keys = tx.open_table(control::KEYS).unwrap();
                        let remove = keys
                            .iter()
                            .unwrap()
                            .map(|row| {
                                let (k, v) = row.unwrap();
                                (k.value().to_vec(), v.value().to_vec())
                            })
                            .find(|(_, v)| *v == key)
                            .unwrap()
                            .0;
                        keys.remove(remove.as_slice()).unwrap();
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
            let encoded = control::encode(&stored).unwrap();
            control::validate_receipt(&encoded).unwrap(); // Cross-history rejection, not malformed local shape.
            table.insert(key.as_slice(), encoded.as_slice()).unwrap();
            drop(table);
            tx.commit().unwrap();
        }
        let before = bytes(&path);
        assert!(validation::canonical_files(&path).is_err(), "case{case}");
        assert!(
            NativeLifecycleSession::open(
                &path,
                NativePolicyInputs::default(),
                Arc::new(FixedClock(AtomicUsize::new(0)))
            )
            .is_err(),
            "case{case}"
        );
        assert!(
            logical_export::export_store(&path, &root.path().join(format!("bad-{case}.jsonl")))
                .is_err(),
            "case{case}"
        );
        assert_eq!(bytes(&path), before, "case{case}");
        std::fs::write(path.join("meta.db"), &original_db).unwrap();
    }
    inspect_schema(&path).unwrap();
}

#[test]
fn mixed_history_pending_purge_outbox_reopens_against_prior_graph_proofs() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let id = linked(&mut s, &d, "linked");
    graph(&mut s, &d, id);
    let start = s
        .engine
        .begin_namespace_purge(&ctx(&s, &d, "purge"), true)
        .unwrap();
    s.metadata()
        .fail_effect_application
        .store(true, Ordering::SeqCst);
    let error = s
        .engine
        .resume_namespace_purge(start.store_id, d.ns, start.start_receipt.operation_id, 1)
        .unwrap_err();
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("wrong error {error}")
    };
    assert_eq!(Some(receipt.operation_id), start.next_operation_id);
    let MutationOutcome::NativeNamespacePurgeAdvanced { transition } = &receipt.outcome else {
        panic!()
    };
    assert!(!transition.chunk.as_ref().unwrap().removed_edges.is_empty());
    drop(s);
    let before = bytes(&d.path);
    assert!(logical_export::export_store(&d.path, &root.path().join("unfinished.jsonl")).is_err());
    assert_eq!(bytes(&d.path), before);
    let final_generation;
    {
        let s = open(&d, clock.clone());
        assert!(s.engine.get_namespace(d.ns).unwrap().is_none());
        let p = s
            .engine
            .lookup_namespace_purge(start.store_id, d.ns, start.start_receipt.operation_id)
            .unwrap()
            .unwrap();
        assert_eq!(p.stage, crate::storage::purge::PurgeStage::Complete);
        assert_eq!(p.removed, 2);
        final_generation = s.engine.control_state().unwrap().generation_ref();
    }
    let s = open(&d, clock);
    assert_eq!(
        s.engine.control_state().unwrap().generation_ref(),
        final_generation
    );
    drop(s);
    inspect_schema(&d.path).unwrap();
}
