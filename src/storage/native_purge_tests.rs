use super::control::{MutationContext, MutationScope};
use super::native_lifecycle::{LifecycleScope, NativeLifecycleSession, NativePolicyInputs};
use super::purge::{PurgeProgress, PurgeStage};
use super::*;
use crate::model::{DecayPhase, MemoryId, NamespaceConfig, NamespaceId};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct NoClock;
impl crate::time::Clock for NoClock {
    fn now_millis(&self) -> i64 {
        panic!("purge sampled clock")
    }
}
pub(super) fn open(path: &Path) -> NativeLifecycleSession {
    NativeLifecycleSession::open(path, NativePolicyInputs::default(), Arc::new(NoClock)).unwrap()
}
pub(super) fn fixture(root: &Path) -> (PathBuf, NamespaceId, Vec<MemoryId>) {
    let source = root.join("legacy");
    let target = root.join("native");
    let mut e = RedbStorageEngine::open(&source).unwrap();
    let mut ns = NamespaceConfig::default_namespace(100);
    ns.name = "doomed".into();
    ns.embedding_dim = 4;
    let id = e.create_namespace(&ns).unwrap();
    ns.id = id;
    let mut ids = Vec::new();
    for n in 0..4 {
        let r = e
            .insert_memory_with_receipt(
                &MutationContext::unkeyed(id),
                &insertion::InsertMemory {
                    id: None,
                    summary: format!("private summary {n}"),
                    full_text: Some(format!("private full text {n}")),
                    tags: vec![],
                    initial_stability: None,
                    created_at: Some(1000),
                    embedding: Some(vec![1., 0., 0., 0.]),
                    parent_id: None,
                    supersedes: None,
                },
                &[],
                1000,
            )
            .unwrap();
        ids.push(insertion::inserted_id(&r).unwrap());
    }
    e.add_edge(
        ids[0],
        ids[1],
        crate::model::EdgeType::ParentChild,
        0.5,
        false,
        0,
    )
    .unwrap();
    for i in [1, 2] {
        let before = e.get_record(ids[i]).unwrap().unwrap();
        assert!(
            e.apply_legacy_decay(ids[i], &before, &ns, DecayPhase::Summary, 0.2, 0.2)
                .unwrap()
        );
    }
    let before = e.get_record(ids[2]).unwrap().unwrap();
    assert!(
        e.apply_legacy_decay(ids[2], &before, &ns, DecayPhase::Ghost, 0.05, 0.05)
            .unwrap()
    );
    e.tombstone_memory_with_receipt(&MutationContext::unkeyed(id), ids[3])
        .unwrap();
    let mut keep = NamespaceConfig::default_namespace(100);
    keep.name = "kept".into();
    keep.embedding_dim = 4;
    let keep_id = e.create_namespace(&keep).unwrap();
    let kept = e
        .insert_memory_with_receipt(
            &MutationContext::unkeyed(keep_id),
            &insertion::InsertMemory {
                id: None,
                summary: "surviving summary".into(),
                full_text: Some("surviving full text".into()),
                tags: vec![],
                initial_stability: None,
                created_at: Some(1000),
                embedding: Some(vec![1., 0., 0., 0.]),
                parent_id: None,
                supersedes: None,
            },
            &[],
            1000,
        )
        .unwrap();
    let kept = insertion::inserted_id(&kept).unwrap();
    // Historical cross-namespace edge fixture, before schema publication. New
    // native graph admission does not create such edges; purge must remove them.
    e.edge_store()
        .add_edge(
            kept,
            ids[0],
            crate::model::EdgeType::Associative,
            0.25,
            false,
            0,
        )
        .unwrap();
    e.meta_store().update_edge_count(kept, 1).unwrap();
    e.sync().unwrap();
    drop(e);
    schema_migration::migrate_schema(
        &source,
        &target,
        &schema_migration::MigrationOptions {
            migration_now: 2000,
            expected_source: schema_migration::inspect_schema(&source)
                .unwrap()
                .control
                .generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    (target, id, ids)
}
fn ctx(s: &NativeLifecycleSession, ns: NamespaceId, key: &str) -> MutationContext {
    let c = s.engine.control_state().unwrap();
    MutationContext {
        store_id: Some(c.store_id),
        scope: MutationScope::Namespace(ns),
        idempotency_key: Some(key.into()),
        expected_generation: Some(c.committed_generation),
        contract_version: 2,
    }
}
fn step(s: &mut NativeLifecycleSession, p: &PurgeProgress) -> PurgeProgress {
    s.engine
        .resume_namespace_purge(p.store_id, p.namespace_id, p.start_receipt.operation_id, 1)
        .unwrap()
}
fn done(s: &mut NativeLifecycleSession, mut p: PurgeProgress) -> PurgeProgress {
    while p.stage != PurgeStage::Complete {
        p = step(s, &p);
    }
    p
}
fn policy(s: &mut NativeLifecycleSession, id: NamespaceId) {
    let config = s.engine.get_namespace(id).unwrap().unwrap();
    let scope = LifecycleScope {
        namespace_id: id,
        namespace_name: config.name,
        source: None,
    };
    let mut replacement =
        catalog::NamespacePolicy::from(&s.engine.get_namespace(id).unwrap().unwrap());
    s.update_policy(&ctx(s, id, "noop"), scope.clone(), &replacement)
        .unwrap()
        .unwrap();
    replacement.initial_stability = 9.;
    s.update_policy(&ctx(s, id, "numeric"), scope, &replacement)
        .unwrap()
        .unwrap();
}
#[test]
fn native_purge_mixed_chunks_restart_replay_policy_and_ledger() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, ids) = fixture(root.path());
    let mut s = open(&path);
    policy(&mut s, id);
    let key = ctx(&s, id, "purge");
    let tombstone = s.capture_tombstone(ids[0], Some(id)).unwrap();
    let kept = s.engine.get_namespace_by_name("kept").unwrap().unwrap().id;
    let p = s.engine.begin_namespace_purge(&key, true).unwrap();
    let mut other = key.clone();
    other.idempotency_key = Some("blocked-tombstone".into());
    other.expected_generation = None;
    assert!(matches!(
        s.tombstone(&other, &tombstone),
        Err(StorageError::RecoveryRequired(_))
    ));
    other.scope = MutationScope::Namespace(kept);
    other.idempotency_key = Some("second-purge".into());
    assert!(matches!(
        s.engine.begin_namespace_purge(&other, true),
        Err(StorageError::RecoveryRequired(_))
    ));
    assert_eq!(p.version, 2);
    let r = s.engine.begin_namespace_purge(&key, true).unwrap();
    assert!(r.start_receipt.replayed);
    assert_eq!(r.next_operation_id, p.next_operation_id);
    assert!(matches!(
        s.engine.begin_namespace_purge(&key, false),
        Err(StorageError::IdempotencyConflict)
    ));
    assert!(s.capture_tombstone(ids[0], Some(id)).is_err());
    let partial = step(&mut s, &p);
    assert_eq!(partial.removed, 1);
    assert_eq!(
        partial.last_receipt.operation_id,
        p.next_operation_id.unwrap()
    );
    assert!(s.engine.get_record(ids[0]).is_err());
    drop(s);
    let before = std::fs::read(path.join("meta.db")).unwrap();
    assert!(validation::canonical_files(&path).is_err());
    assert_eq!(before, std::fs::read(path.join("meta.db")).unwrap());
    let mut s = open(&path);
    let p = s
        .engine
        .lookup_namespace_purge(p.store_id, id, p.start_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(p.stage, PurgeStage::Complete);
    assert_eq!(p.removed, 4);
    assert_eq!(p.tombstones_removed, 1);
    assert_eq!(p.edges_removed, 2);
    assert!(!path.join("doomed").exists());
    assert!(s.engine.get_namespace(id).unwrap().is_none());
    for id in ids {
        assert!(s.engine.get_record(id).unwrap().is_none());
    }
    let before = s.engine.control_state().unwrap();
    assert!(
        s.engine
            .begin_namespace_purge(&key, true)
            .unwrap()
            .start_receipt
            .replayed
    );
    assert_eq!(before, s.engine.control_state().unwrap());
    drop(s);
    validation::canonical_files(&path).unwrap();
}
#[tokio::test]
async fn native_purge_complete_export_import_backup_fork_preserves_witness() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    crate::test_support::explicitly_adopt_inherited_policy(&path, id, 1., 2000);
    let mut s = open(&path);
    policy(&mut s, id);
    let memory = s
        .metadata()
        .scan_all()
        .unwrap()
        .into_iter()
        .find(|(_, r)| r.namespace_id == id.get() && r.phase == DecayPhase::Full)
        .unwrap()
        .0;
    let policy = NativePolicyInputs::default();
    let intent = s
        .metadata()
        .capture_native_decay(memory, id, &policy, 10_000 * 86_400_000_i64)
        .unwrap();
    assert!(matches!(
        s.engine
            .apply_native_decay(&ctx(&s, id, "phase-before-purge"), &intent, &policy, None)
            .unwrap(),
        native_maintenance::NativeDecayResult::Committed(_)
    ));
    let p = s
        .engine
        .begin_namespace_purge(&ctx(&s, id, "purge"), true)
        .unwrap();
    let p = done(&mut s, p);
    let receipt = p.start_receipt.clone();
    drop(s);
    let stream = root.path().join("logical.jsonl");
    let imported = root.path().join("imported");
    assert_eq!(
        logical_export::export_store(&path, &stream).unwrap(),
        logical_export::import_store(&stream, &imported).unwrap()
    );
    let text = std::fs::read_to_string(&stream).unwrap();
    assert!(!text.contains("private summary"));
    assert!(!text.contains("private full text"));
    let archive = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("purge.zip"),
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
    let s = open(&imported);
    assert_ne!(s.engine.control_state().unwrap().store_id, receipt.store_id);
    // The actual four-view barrier must accept completed validated ancestry
    // using original receipt keys, not the new fork's store ID.
    let fx = crate::test_support::Fixture::from_open_engine(root, s.engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    publisher.rebuild_all().await.unwrap();
    let target = fx
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    assert!(
        publisher
            .readiness_barrier(
                target,
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
            .await
            .unwrap()
            .ready
    );
    drop(publisher);
    let _root = fx.close();
    let (_copy, db) = validation::inspect_database(&imported.join("meta.db")).unwrap();
    let read = db.begin_read().unwrap();
    let state = control::validate_existing(&read).unwrap();
    let progress = metadata::purge::validate_existing(&read, state.as_ref()).unwrap();
    assert_eq!(progress, vec![p]);
    drop(read);
    validation::canonical_files(&imported).unwrap();
}

#[test]
fn native_purge_process_child() {
    let Some(path) = std::env::var_os("RECALLD_NATIVE_PURGE_TEST_STORE") else {
        return;
    };
    let mut s = open(Path::new(&path));
    let id = NamespaceId::new(
        std::env::var("RECALLD_NATIVE_PURGE_TEST_NS")
            .unwrap()
            .parse()
            .unwrap(),
    );
    let p = s
        .engine
        .begin_namespace_purge(&ctx(&s, id, "process-purge"), true)
        .unwrap();
    done(&mut s, p);
    panic!("fault boundary not reached");
}
fn crash(path: &Path, id: NamespaceId, point: &str) {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::native_purge_tests::native_purge_process_child",
            "--nocapture",
        ])
        .env("RECALLD_NATIVE_PURGE_TEST_STORE", path)
        .env("RECALLD_NATIVE_PURGE_TEST_NS", id.get().to_string())
        .env("RECALLD_TEST_EFFECT_CRASH", point)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(91),
        "{point}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn native_purge_interrupted_publication_recovers_exactly_once() {
    for point in [
        "before_purge_commit",
        "after_purge_commit",
        "after_metadata_commit",
        "after_edge_commit",
        "after_canonical_ack",
        "after_purge_directory_cleanup",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (path, id, ids) = fixture(root.path());
        crash(&path, id, point);
        for _ in 0..2 {
            let s = open(&path);
            let progress = s.metadata().purge_progress().unwrap();
            if point == "before_purge_commit" {
                assert!(progress.is_empty());
                assert!(s.engine.get_record(ids[0]).unwrap().is_some());
            } else {
                assert_eq!(progress.len(), 1);
                assert_eq!(progress[0].stage, PurgeStage::Complete);
                assert_eq!(progress[0].removed, 4);
                assert_eq!(progress[0].edges_removed, 2);
            }
        }
    }
}
#[test]
fn native_purge_corrupt_pending_and_history_reject_without_source_changes() {
    use redb::ReadableTable;
    for mode in [
        "pending-extra-edge",
        "pending-missing-peer",
        "before-policy",
        "before-profile",
        "chunk-token",
        "progress-version",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (path, id, _) = fixture(root.path());
        crash(&path, id, "after_metadata_commit");
        // Mutate only an isolated synthetic database. Both pending canonical and
        // immutable history must fail before any writable engine open.
        {
            let db = redb::Database::open(path.join("meta.db")).unwrap();
            let tx = db.begin_write().unwrap();
            if mode.starts_with("pending") {
                let mut table = tx.open_table(metadata::effects::EFFECTS).unwrap();
                let (key, mut e) = table
                    .iter()
                    .unwrap()
                    .map(|row| {
                        let (k, v) = row.unwrap();
                        (
                            k.value().to_vec(),
                            control::decode::<metadata::effects::PendingEffects>(v.value())
                                .unwrap(),
                        )
                    })
                    .find(|(_, e)| {
                        !e.canonical_applied
                            && matches!(
                                e.receipt.outcome,
                                control::MutationOutcome::NativeNamespacePurgeAdvanced { .. }
                            )
                    })
                    .expect("exact pending native chunk");
                if mode == "pending-extra-edge" {
                    e.deletes.push(edges::PersistedEdge {
                        source: MemoryId::from(uuid::Uuid::new_v4()),
                        target: MemoryId::from(uuid::Uuid::new_v4()),
                        edge_type: crate::model::EdgeType::Associative,
                        weight: 0.5,
                        auto_created: false,
                        created_at: 0,
                    });
                } else {
                    e.invalidated_ids.push(MemoryId::from(uuid::Uuid::new_v4()));
                }
                table
                    .insert(key.as_slice(), control::encode(&e).unwrap().as_slice())
                    .unwrap();
            } else {
                let mut table = tx.open_table(metadata::purge::PROGRESS).unwrap();
                let (row_key, bytes) = table.iter().unwrap().next().unwrap().unwrap();
                let key = row_key.value().to_vec();
                drop(row_key);
                let mut p: PurgeProgress = control::decode(bytes.value()).unwrap();
                drop(bytes);
                if mode == "progress-version" {
                    p.version = 1;
                } else {
                    let mut receipts = tx.open_table(control::RECEIPTS).unwrap();
                    let receipt = if mode == "chunk-token" {
                        &mut p.last_receipt
                    } else {
                        &mut p.start_receipt
                    };
                    let key = control::scoped_key(
                        receipt.store_id,
                        MutationScope::Namespace(id),
                        receipt.operation_id.as_bytes(),
                    );
                    let mut stored: control::StoredReceipt =
                        control::decode(receipts.get(key.as_slice()).unwrap().unwrap().value())
                            .unwrap();
                    match &mut stored.receipt.outcome {
                        control::MutationOutcome::NativeNamespacePurgeStarted { transition: t } => {
                            if mode == "before-policy" {
                                t.before_namespace.schema.policy_revision =
                                    t.before_namespace.schema.policy_revision.next().unwrap();
                            } else {
                                t.before_namespace.embedding_dim = 5;
                            }
                        }
                        control::MutationOutcome::NativeNamespacePurgeAdvanced {
                            transition: t,
                        } => {
                            let d = &mut t.chunk.as_mut().unwrap().deletions[0];
                            d.before.lifecycle.lifecycle.record_generation = d
                                .before
                                .lifecycle
                                .lifecycle
                                .record_generation
                                .next()
                                .unwrap();
                        }
                        _ => panic!("unexpected proof"),
                    }
                    *receipt = stored.receipt.clone();
                    receipts
                        .insert(key.as_slice(), control::encode(&stored).unwrap().as_slice())
                        .unwrap();
                }
                table
                    .insert(key.as_slice(), control::encode(&p).unwrap().as_slice())
                    .unwrap();
            }
            tx.commit().unwrap();
        }
        let before = std::fs::read(path.join("meta.db")).unwrap();
        let edges = std::fs::read(path.join("edges.db")).unwrap();
        assert!(
            NativeLifecycleSession::open(&path, NativePolicyInputs::default(), Arc::new(NoClock))
                .is_err(),
            "{mode}"
        );
        assert_eq!(
            before,
            std::fs::read(path.join("meta.db")).unwrap(),
            "{mode}"
        );
        assert_eq!(
            edges,
            std::fs::read(path.join("edges.db")).unwrap(),
            "{mode}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_purge_accepted_completion_failure_retains_latest_receipt_then_retries() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, ids) = fixture(root.path());
    let s = open(&path);
    let key = ctx(&s, id, "accepted-purge");
    let fx = crate::test_support::Fixture::from_open_engine(root, s.engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    publisher.rebuild_all().await.unwrap();
    let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
    let root = tracker.begin().unwrap();
    *fx.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    let result = publisher
        .purge_native_accepted(key.clone(), true, &root)
        .await;
    let Err(StorageError::CommittedMutation { receipt, .. }) = result else {
        panic!("expected exact committed receipt: {result:?}");
    };
    // The joined completion owner preserves the exact pending effect receipt.
    // Canonical purge completion and derived acknowledgement are separate facts.
    let (pending, canonical) = {
        let storage = fx.storage.read().unwrap();
        assert_eq!(
            storage
                .meta_store()
                .lookup_operation(
                    receipt.store_id,
                    MutationScope::Namespace(id),
                    receipt.operation_id
                )
                .unwrap()
                .unwrap(),
            *receipt
        );
        (
            storage.meta_store().pending_effects().unwrap(),
            storage
                .meta_store()
                .purge_progress()
                .unwrap()
                .into_iter()
                .find(|p| p.namespace_id == id),
        )
    };
    let canonical = canonical.unwrap();
    assert_eq!(canonical.stage, PurgeStage::Complete);
    assert_eq!(pending.first().unwrap().receipt, *receipt);
    assert!(pending.iter().all(|effect| effect.canonical_applied));
    let control::MutationOutcome::NativeNamespacePurgeAdvanced { transition } = &receipt.outcome
    else {
        panic!("wrong child receipt: {receipt:?}")
    };
    assert_eq!(transition.stage, PurgeStage::Deleting);
    assert!(receipt.committed_generation < canonical.last_receipt.committed_generation);
    assert!(
        fx.storage
            .read()
            .unwrap()
            .meta_store()
            .effects_stamp()
            .is_err()
    );
    let target = fx
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    assert_eq!(
        canonical.last_receipt.committed_generation,
        target.generation
    );
    {
        let storage = fx.storage.read().unwrap();
        assert_eq!(
            storage
                .meta_store()
                .lookup_operation(
                    canonical.store_id,
                    MutationScope::Namespace(id),
                    canonical.last_receipt.operation_id
                )
                .unwrap()
                .unwrap(),
            canonical.last_receipt
        );
        let state = storage.meta_store().publication_state().unwrap();
        assert_eq!(state.canonical, target);
        assert!(state.applied.iter().all(|at| *at != Some(target)));
    }
    // Finish the known caller before draining; never make a barrier wait on its
    // own held parent. The broken view must reject the actual four-view barrier.
    root.finish();
    assert!(
        publisher
            .readiness_barrier(
                target,
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
            .await
            .is_err()
    );
    assert_eq!(
        fx.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref(),
        target
    );
    let root = tracker.begin().unwrap();
    *fx.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    let completed = publisher
        .purge_native_accepted(key, true, &root)
        .await
        .unwrap();
    assert_eq!(completed, canonical);
    assert_eq!(
        fx.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref(),
        target
    );
    assert_eq!(completed.stage, PurgeStage::Complete);
    root.finish();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    let ready = publisher
        .readiness_barrier(
            target,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(ready.ready);
    assert_eq!(ready.derived.len(), 4);
    assert!(
        ready
            .derived
            .iter()
            .all(|view| view.ready && view.applied_generation == target.generation)
    );

    assert!(
        fx.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    for id in ids {
        assert!(!fx.graph.read().await.contains(&id));
    }
    let rows = fx.storage.read().unwrap().scan_all().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1.edge_count, 0);
    assert_eq!(
        rows[0]
            .1
            .schema
            .as_ref()
            .unwrap()
            .lifecycle
            .record_generation
            .get(),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_purge_accepted_descendants_complete_after_close() {
    use std::time::{Duration, Instant};
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let s = open(&path);
    let key = ctx(&s, id, "closing-purge");
    let fx = crate::test_support::Fixture::from_open_engine(root, s.engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
    let accepted = tracker.begin().unwrap();
    let closing = tracker.clone();
    let drain = tokio::task::spawn_blocking(move || {
        closing.close_and_drain(Instant::now() + Duration::from_secs(15))
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while tracker.status().admission_open {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(tracker.begin().is_err());
    let done = tokio::time::timeout(
        Duration::from_secs(10),
        publisher.purge_native_accepted(key, true, &accepted),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(done.stage, PurgeStage::Complete);
    accepted.finish();
    let freeze = drain.await.unwrap().unwrap();
    let storage = fx.storage.read().unwrap();
    assert!(storage.meta_store().pending_effects().unwrap().is_empty());
    let capture = storage.meta_store().freeze_metadata(freeze).unwrap();
    assert_eq!(
        capture.generation(),
        storage.control_state().unwrap().generation_ref()
    );
    assert_eq!(tracker.status().pending, 0);
}

#[test]
fn native_purge_native_catalog_binding_empty_cleanup_and_recreated_name() {
    struct FixedClock;
    impl crate::time::Clock for FixedClock {
        fn now_millis(&self) -> i64 {
            2000
        }
    }
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let input = native_catalog::NativeNamespaceInput {
        name: "native-born".into(),
        embedding_dim: Some(4),
        embedding_profile_id: None,
        initial_stability: None,
        desired_retention: None,
        decay_rate_multiplier: None,
    };
    let mut catalog = ctx(&s, id, "born");
    catalog.scope = MutationScope::Catalog;
    let receipt = s
        .engine
        .create_native_namespace(
            &catalog,
            &input,
            &crate::embedding::runtime::tests::profile(),
            &NativePolicyInputs::default(),
            &FixedClock,
            None,
        )
        .unwrap();
    let control::MutationOutcome::NativeNamespaceCreated { transition: t } = receipt.outcome else {
        panic!("wrong receipt")
    };
    let born = t.namespace_id;
    policy(&mut s, born);
    let key = ctx(&s, born, "empty-purge");
    let start = s.engine.begin_namespace_purge(&key, false).unwrap();
    let cleanup = step(&mut s, &start);
    assert_eq!(cleanup.stage, PurgeStage::Cleanup);
    drop(s);
    let bytes = std::fs::read(path.join("meta.db")).unwrap();
    assert!(validation::canonical_files(&path).is_err());
    assert_eq!(bytes, std::fs::read(path.join("meta.db")).unwrap());
    let mut s = open(&path);
    let mut next = ctx(&s, id, "reborn");
    next.scope = MutationScope::Catalog;
    let new = s
        .engine
        .create_native_namespace(
            &next,
            &input,
            &crate::embedding::runtime::tests::profile(),
            &NativePolicyInputs::default(),
            &FixedClock,
            None,
        )
        .unwrap();
    let control::MutationOutcome::NativeNamespaceCreated { transition: t } = new.outcome else {
        panic!("wrong receipt")
    };
    assert_ne!(t.namespace_id, born);
    let before = s.engine.control_state().unwrap();
    let replay = s.engine.begin_namespace_purge(&key, false).unwrap();
    assert!(replay.start_receipt.replayed);
    assert_eq!(replay.stage, PurgeStage::Complete);
    assert_eq!(before, s.engine.control_state().unwrap());
    assert!(s.engine.get_namespace(t.namespace_id).unwrap().is_some());
    drop(s);
    validation::canonical_files(&path).unwrap();
}

fn extension_admission_probe(namespace_extension: bool) {
    use redb::ReadableTable;
    let root = tempfile::tempdir().unwrap();
    let (path, id, ids) = fixture(root.path());
    {
        let db = redb::Database::open(path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        if namespace_extension {
            let mut table = tx.open_table(metadata::NAMESPACE_TABLE).unwrap();
            let mut ns =
                NamespaceConfig::decode(table.get(id.get()).unwrap().unwrap().value()).unwrap();
            ns.schema.as_mut().unwrap().extensions.insert(
                "future_payload".into(),
                serde_json::json!({"secret":"not allowed in purge receipt"}),
            );
            table
                .insert(id.get(), control::encode(&ns).unwrap().as_slice())
                .unwrap();
        } else {
            let mut table = tx.open_table(metadata::META_TABLE).unwrap();
            let mut r = crate::model::DiskRecord::from_bytes(
                table
                    .get(ids[0].as_bytes().as_slice())
                    .unwrap()
                    .unwrap()
                    .value(),
            )
            .unwrap();
            r.schema.as_mut().unwrap().extensions.insert(
                "future_payload".into(),
                serde_json::json!({"secret":"not allowed in purge receipt"}),
            );
            table
                .insert(r.id.as_slice(), r.encode().unwrap().as_slice())
                .unwrap();
        }
        tx.commit().unwrap();
    }
    let mut s = open(&path);
    let before = s.engine.control_state().unwrap();
    let bytes = std::fs::read(path.join("meta.db")).unwrap();
    let result = s
        .engine
        .begin_namespace_purge(&ctx(&s, id, "unknown-extension"), true);
    assert!(
        result.is_err(),
        "unsupported extension accepted durable Purging: {result:?}"
    );
    assert_eq!(before, s.engine.control_state().unwrap());
    assert!(s.metadata().purge_progress().unwrap().is_empty());
    assert_eq!(bytes, std::fs::read(path.join("meta.db")).unwrap());
}
#[test]
fn native_purge_namespace_extension_rejected_before_receipt_retention() {
    extension_admission_probe(true);
}
#[test]
fn native_purge_record_extension_rejected_before_undrainable_progress() {
    extension_admission_probe(false);
}

#[tokio::test]
async fn native_purge_foreign_terminal_boundary_rejects_later_noop() {
    use redb::ReadableTable;
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    policy(&mut s, id);
    let config = s.engine.get_namespace(id).unwrap().unwrap();
    let current = catalog::NamespacePolicy::from(&config);
    let receipt = s
        .update_policy(
            &ctx(&s, id, "terminal-noop"),
            LifecycleScope {
                namespace_id: id,
                namespace_name: config.name,
                source: None,
            },
            &current,
        )
        .unwrap()
        .unwrap();
    let p = s
        .engine
        .begin_namespace_purge(&ctx(&s, id, "terminal-purge"), true)
        .unwrap();
    let p = done(&mut s, p);
    drop(s);
    let archive = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("terminal.zip"),
        Some(&path),
        false,
    )
    .await
    .unwrap();
    crate::backup::restore_from_backup(crate::backup::RestoreOptions {
        backup_path: archive,
        data_dir: path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    {
        let db = redb::Database::open(path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        let mut table = tx.open_table(control::RECEIPTS).unwrap();
        let key = control::scoped_key(
            receipt.store_id,
            MutationScope::Namespace(id),
            receipt.operation_id.as_bytes(),
        );
        let mut stored: control::StoredReceipt =
            control::decode(table.get(key.as_slice()).unwrap().unwrap().value()).unwrap();
        stored.receipt.committed_generation = p.last_receipt.committed_generation;
        let control::MutationOutcome::NativeNamespacePolicyUnchanged { transition: t } =
            &mut stored.receipt.outcome
        else {
            panic!("wrong proof")
        };
        t.previous_generation = control::Generation::try_from(
            (p.last_receipt.committed_generation.get() - 1).to_string(),
        )
        .unwrap();
        table
            .insert(key.as_slice(), control::encode(&stored).unwrap().as_slice())
            .unwrap();
        drop(table);
        tx.commit().unwrap();
    }
    let before = std::fs::read(path.join("meta.db")).unwrap();
    assert!(
        validation::canonical_files(&path).is_err(),
        "foreign same-revision no-op after terminal purge accepted"
    );
    assert!(
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), Arc::new(NoClock))
            .is_err()
    );
    assert_eq!(before, std::fs::read(path.join("meta.db")).unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_purge_accepted_unknown_child_preserves_reserved_id_and_reopens() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let key = ctx(&s, id, "unknown-child");
    let start = s.engine.begin_namespace_purge(&key, true).unwrap();
    let fx = crate::test_support::Fixture::from_open_engine(root, s.engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
    let root = tracker.begin().unwrap();
    fx.storage
        .read()
        .unwrap()
        .meta_store()
        .fail_purge_child_commit
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let error = publisher
        .purge_native_accepted(key, true, &root)
        .await
        .unwrap_err();
    assert!(
        matches!(error,StorageError::CommitOutcomeUnknown{operation_id,..} if Some(operation_id)==start.next_operation_id),
        "unexpected outcome: {error:?}"
    );
    let committed = fx
        .storage
        .read()
        .unwrap()
        .lookup_namespace_purge(start.store_id, id, start.start_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        committed.last_receipt.operation_id,
        start.next_operation_id.unwrap()
    );
    assert_eq!(committed.removed, 4);
    assert!(tracker.status().unresolved > 0);
    root.finish();
    drop(publisher);
    let _directory = fx.close();
    let s = open(&path);
    let complete = s
        .engine
        .lookup_namespace_purge(start.store_id, id, start.start_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(complete.stage, PurgeStage::Complete);
    assert_eq!(complete.removed, 4);
    assert_eq!(complete.sequence, 3);
}

#[tokio::test]
async fn native_purge_foreign_peer_requires_its_actual_policy_revision() {
    use redb::ReadableTable;
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let p = s
        .engine
        .begin_namespace_purge(&ctx(&s, id, "peer-proof"), true)
        .unwrap();
    done(&mut s, p);
    drop(s);
    let archive = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("peer.zip"),
        Some(&path),
        false,
    )
    .await
    .unwrap();
    crate::backup::restore_from_backup(crate::backup::RestoreOptions {
        backup_path: archive,
        data_dir: path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    {
        let db = redb::Database::open(path.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        let mut table = tx.open_table(control::RECEIPTS).unwrap();
        let selected = table
            .iter()
            .unwrap()
            .map(|row| {
                let (k, v) = row.unwrap();
                (
                    k.value().to_vec(),
                    control::decode::<control::StoredReceipt>(v.value()).unwrap(),
                )
            })
            .find(|(_, r)| match &r.receipt.outcome {
                control::MutationOutcome::NativeNamespacePurgeAdvanced { transition: t } => t
                    .chunk
                    .as_ref()
                    .is_some_and(|c| c.peers.iter().any(|p| p.namespace_id != id)),
                _ => false,
            })
            .expect("actual historical cross-namespace peer");
        let (key, mut stored) = selected;
        let control::MutationOutcome::NativeNamespacePurgeAdvanced { transition: t } =
            &mut stored.receipt.outcome
        else {
            unreachable!()
        };
        let peer = t
            .chunk
            .as_mut()
            .unwrap()
            .peers
            .iter_mut()
            .find(|p| p.namespace_id != id)
            .unwrap();
        assert_eq!(
            peer.change.after.lifecycle.lifecycle.policy_revision,
            control::Generation::default()
        );
        peer.change
            .before
            .as_mut()
            .unwrap()
            .lifecycle
            .lifecycle
            .policy_revision = control::Generation::default().next().unwrap();
        peer.change.after.lifecycle.lifecycle.policy_revision =
            control::Generation::default().next().unwrap();
        table
            .insert(key.as_slice(), control::encode(&stored).unwrap().as_slice())
            .unwrap();
        drop(table);
        tx.commit().unwrap();
    }
    let before = std::fs::read(path.join("meta.db")).unwrap();
    assert!(
        validation::canonical_files(&path).is_err(),
        "foreign peer with nonexistent policy revision accepted"
    );
    assert!(
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), Arc::new(NoClock))
            .is_err()
    );
    assert_eq!(before, std::fs::read(path.join("meta.db")).unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_purge_cancelled_waiter_keeps_worker_publication_lease() {
    use std::time::Duration;
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let s = open(&path);
    let key = ctx(&s, id, "cancelled-waiter");
    let fx = crate::test_support::Fixture::from_open_engine(root, s.engine);
    let publisher = crate::derived_completion::DerivedCompletion {
        storage: fx.storage.clone(),
        cache: fx.cache.clone(),
        graph: fx.graph.clone(),
        vectors: fx.vector_index.clone(),
        fts: fx.fts_index.clone(),
        entities: fx.entity_index.clone(),
    };
    let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
    let accepted = tracker.begin().unwrap();
    let gate = crate::derived_completion::publication_gate(&fx.storage)
        .await
        .unwrap();
    let graph_block = fx.graph.clone().write_owned().await;
    let task =
        tokio::spawn(async move { publisher.purge_native_accepted(key, true, &accepted).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tracker.status().pending == 2 && gate.try_write().is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let storage = fx.storage.clone();
    let (held_tx, held_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        let _guard = storage.write().unwrap();
        held_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    held_rx.await.unwrap();
    drop(graph_block);
    tokio::time::timeout(Duration::from_secs(5), async {
        while tracker.status().pending != 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let publication_held = gate.try_write().is_err();
    let graph_held = fx.graph.try_write().is_err();
    // Always release the worker before assertions/fixture cleanup, including RED.
    release_tx.send(()).unwrap();
    blocker.await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while tracker.status().pending > 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let lease = tokio::time::timeout(Duration::from_secs(5), gate.write())
        .await
        .unwrap();
    drop(lease);
    assert!(graph_held, "worker graph guard was not retained");
    assert!(
        tracker.status().unresolved >= 1,
        "cancelled waiter must remain unresolved"
    );
    assert!(
        publication_held,
        "cancelled waiter released publication lease before blocked worker completed"
    );
}

#[path = "native_purge_tests/request_context.rs"]
mod request_context;
