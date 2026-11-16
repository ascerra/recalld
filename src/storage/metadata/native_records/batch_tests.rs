use super::*;
use crate::mcp::bridge::StorageEngine as _;
use crate::storage::batch::{BatchInput, BatchItemPlan, BatchItemState, BatchKind, BatchStatus};
use crate::test_support::Fixture;
use serde_json::json;

fn disabled_aux_config() -> Arc<crate::config::RecalldConfig> {
    let mut config = crate::config::RecalldConfig::default();
    config.graph.autolink_enabled = false;
    Arc::new(config)
}
fn disabled_aux_adapter(
    f: &Fixture,
    clock: Arc<FixedClock>,
) -> crate::mcp::bridge_adapters::McpStorageAdapter {
    crate::mcp::bridge_adapters::McpStorageAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        Arc::new(crate::embedding::PassthroughProvider::new(4)),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
        disabled_aux_config(),
        chrono_tz::UTC,
    )
    .with_clock(clock)
}

fn request(d: &Data, store_id: uuid::Uuid, key: &str) -> BatchInput {
    BatchInput {
        mutation_version: 2,
        result_version: 1,
        kind: BatchKind::Insert,
        items: vec![
            json!({"summary":"native batch", "fullText":"payload é🙂", "entities":["José","John Smith","Alice's"],
            "embedding":[2.,0.,0.,0.], "embeddingProfile":d.profile, "checkDuplicates":false}),
        ],
        default_namespace: "native".into(),
        store_id: Some(store_id),
        idempotency_key: Some(key.into()),
        batch_operation_id: None,
    }
}
fn plans(r: &BatchInput) -> Vec<BatchItemPlan> {
    crate::mcp::batches::prepare(r)
        .unwrap()
        .into_iter()
        .map(|p| p.plan)
        .collect()
}
fn insert_reserved(
    s: &mut NativeLifecycleSession,
    d: &Data,
    r: &BatchInput,
    p: &crate::storage::batch::BatchProgress,
) -> WriteReceipt {
    let ps = plans(r);
    let BatchItemPlan::NativeInsert { input, .. } = &ps[0] else {
        panic!("not native input")
    };
    let intent = s.capture_insert(scope(d), input).unwrap();
    let child = p.child(0).unwrap();
    s.engine
        .insert_native_record_reserved(
            &child.context(),
            &intent,
            &generated(),
            &s.policy,
            s.clock.as_ref(),
            None,
            Some(&child),
        )
        .unwrap()
}
fn completion(f: &Fixture) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}

#[test]
fn native_batch_pending_reopen_same_tx_child_and_immutable_replay() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let state = s.metadata().control_state().unwrap();
    let r = request(&d, state.store_id, "native-resume");
    let p = s
        .metadata()
        .reserve_batch(&r.context(), r.kind, &plans(&r))
        .unwrap();
    assert_eq!(p.version, 2);
    assert_eq!(
        p.reservation_receipt.committed_generation,
        state.committed_generation.next().unwrap()
    );
    assert_ne!(
        p.child(0).unwrap().operation_id,
        p.reservation_receipt.operation_id
    );
    drop(s);
    let mut s = open(&d, clock.clone());
    let resumed = s
        .metadata()
        .reserve_batch(&r.context(), r.kind, &plans(&r))
        .unwrap();
    assert!(resumed.reservation_receipt.replayed);
    assert_eq!(resumed.items, p.items);
    let receipt = insert_reserved(&mut s, &d, &r, &p);
    assert_eq!(receipt.operation_id, p.child(0).unwrap().operation_id);
    assert_eq!(
        receipt.committed_generation,
        p.reservation_receipt.committed_generation.next().unwrap()
    );
    let stored = s
        .metadata()
        .lookup_batch(state.store_id, p.reservation_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.items[0].state,
        BatchItemState::Ok {
            receipt: receipt.clone()
        }
    );
    let id = inserted(&receipt);
    let forget = s.capture_tombstone(id, None).unwrap();
    s.tombstone(&ctx(&s, &d, "forget-after-child"), &forget)
        .unwrap();
    let before = s.metadata().control_state().unwrap();
    drop(s);
    let s = open(&d, clock);
    let replay = s
        .metadata()
        .reserve_batch(&r.context(), r.kind, &plans(&r))
        .unwrap();
    assert_eq!(replay.items, stored.items);
    assert_eq!(s.metadata().control_state().unwrap(), before);
    assert_eq!(
        s.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
}

#[test]
fn native_batch_hash_expands_numeric_once_preserves_vectors_and_rejects_foreign_child() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock);
    let r = request(
        &d,
        s.metadata().control_state().unwrap().store_id,
        "identity",
    );
    let ps = plans(&r);
    let p = s
        .metadata()
        .reserve_batch(&r.context(), r.kind, &ps)
        .unwrap();
    let child = p.child(0).unwrap();
    let BatchItemPlan::NativeInsert { input, .. } = &ps[0] else {
        panic!()
    };
    let hash = crate::storage::batch::native::insert_hash(&child.context(), input, &child.defaults)
        .unwrap();
    let mut expanded = input.clone();
    expanded.memory.initial_stability = Some(f32::from_bits(child.defaults.initial_stability_bits));
    assert_eq!(
        hash,
        crate::storage::batch::native::insert_hash(&child.context(), &expanded, &child.defaults)
            .unwrap()
    );
    for vector in [vec![1., 0., 0., 0.], vec![2., -0., 0., 0.]] {
        let mut changed = input.clone();
        changed.memory.embedding = Some(vector);
        assert_ne!(
            hash,
            crate::storage::batch::native::insert_hash(&child.context(), &changed, &child.defaults)
                .unwrap()
        );
        assert!(matches!(
            s.metadata()
                .lookup_native_insert_child(&child, &changed, None),
            Err(StorageError::IdempotencyConflict)
        ));
    }
    let before = s.metadata().control_state().unwrap();
    let intent = s.capture_insert(scope(&d), input).unwrap();
    let mut wrong = child.clone();
    wrong.version = 1;
    assert!(
        s.engine
            .insert_native_record_reserved(
                &wrong.context(),
                &intent,
                &generated(),
                &s.policy,
                s.clock.as_ref(),
                None,
                Some(&wrong)
            )
            .is_err()
    );
    assert_eq!(before, s.metadata().control_state().unwrap());
    let mut changed = r.clone();
    changed.default_namespace = "default".into();
    assert!(matches!(
        s.metadata()
            .reserve_batch(&changed.context(), changed.kind, &plans(&changed)),
        Err(StorageError::IdempotencyConflict)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_batch_actual_adapter_commits_labels_and_replays_then_forgets() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let r = request(
        &d,
        s.metadata().control_state().unwrap().store_id,
        "adapter-native",
    );
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let adapter = disabled_aux_adapter(&f, clock.clone());
    let (out, trace) =
        crate::performance::tests::capture_full(adapter.mutate_batch(r.clone())).await;
    let out = out.unwrap();
    crate::performance::tests::assert_no_legacy_batch_events(&trace);
    assert!(out.all_ready(), "{out:?}");
    let receipt = out.items[0].receipt.clone().unwrap();
    let id = inserted(&receipt);
    let record = f.storage.read().unwrap().get_record(id).unwrap().unwrap();
    assert_eq!(
        record.schema.unwrap().labels.entities,
        vec!["José", "John Smith", "Alice's"]
    );
    let before = f.storage.read().unwrap().control_state().unwrap();
    let replay = adapter.mutate_batch(r.clone()).await.unwrap();
    assert!(replay.all_ready());
    assert_eq!(
        replay.items[0].receipt.as_ref().unwrap().operation_id,
        receipt.operation_id
    );
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
    let mut forget = r.clone();
    forget.kind = BatchKind::Forget;
    forget.items = vec![json!(id.to_string())];
    forget.idempotency_key = Some("batch-forget".into());
    let deleted = adapter.mutate_batch(forget.clone()).await.unwrap();
    assert!(deleted.all_ready(), "{deleted:?}");
    assert_eq!(deleted.items[0].result.as_ref().unwrap()["deleted"], true);
    assert!(adapter.mutate_batch(forget).await.unwrap().all_ready());
    drop(adapter);
    let root = f.close();
    let s = open(&d, clock);
    assert_eq!(
        s.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
    drop(s);
    drop(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_batch_unknown_child_reconciles_original_id_after_strict_reopen() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let r = request(
        &d,
        s.metadata().control_state().unwrap().store_id,
        "unknown-child",
    );
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .fail_batch_commit
        .store(3, Ordering::SeqCst);
    let adapter = disabled_aux_adapter(&f, clock.clone());
    let out = adapter.mutate_batch(r.clone()).await.unwrap();
    assert_eq!(out.items[0].status, BatchStatus::Unknown);
    let child = out.items[0].operation_id.unwrap();
    assert_ne!(child, out.batch_operation_id);
    assert!(!out.all_ready());
    drop(adapter);
    let root = f.close();
    let s = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, s.engine);
    let adapter = disabled_aux_adapter(&f, clock);
    let resumed = adapter.mutate_batch(r).await.unwrap();
    assert!(resumed.all_ready(), "{resumed:?}");
    assert_eq!(
        resumed.items[0].receipt.as_ref().unwrap().operation_id,
        child
    );
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
}

#[test]
fn native_batch_forged_version_defaults_or_child_identity_fail_closed() {
    for corruption in [0, 1, 2, 3] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let s = open(&d, clock.clone());
        let r = request(&d, s.metadata().control_state().unwrap().store_id, "forged");
        let mut p = s
            .metadata()
            .reserve_batch(&r.context(), r.kind, &plans(&r))
            .unwrap();
        match corruption {
            0 => p.version = 1,
            1 => p.items[0].defaults.as_mut().unwrap().native = None,
            2 => p.items[0].operation_id = p.reservation_receipt.operation_id,
            _ => p.items[0].canonical_hash = Some([0; 32]),
        }
        let tx = s.metadata().db.begin_write().unwrap();
        tx.open_table(crate::storage::metadata::batch::BATCHES)
            .unwrap()
            .insert(
                p.reservation_receipt.operation_id.as_bytes().as_slice(),
                control::encode(&p).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
        drop(s);
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
            "forgery {corruption}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_batch_mcp_daemon_parity_invalid_items_replay_and_current_publication_receipt() {
    for socket in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let s = open(&d, clock.clone());
        let mut r = request(
            &d,
            s.metadata().control_state().unwrap().store_id,
            "transport",
        );
        r.items.push(json!({"summary":"bad tags","tags":[""]}));
        let f = Fixture::from_open_engine(root, s.engine);
        completion(&f).rebuild_all().await.unwrap();
        let adapter = Arc::new(disabled_aux_adapter(&f, clock));
        let before = f.storage.read().unwrap().control_state().unwrap();
        let out = if socket {
            let response = crate::daemon::server::native_batch_test_rpc(adapter.clone(), &r).await;
            assert!(response.error.is_none(), "{response:?}");
            serde_json::from_value::<crate::storage::batch::BatchOutcome>(response.result.unwrap())
                .unwrap()
        } else {
            let mut bridge = crate::test_support::StubBridge::default().into_bridge();
            bridge.storage = adapter.clone();
            bridge.default_namespace = "native".into();
            let response=crate::mcp::tools::dispatch_tool(&bridge,"store_memories",json!({"memories":r.items,"storeId":r.store_id,"idempotencyKey":r.idempotency_key,"batchMutationVersion":2,"batchResultVersion":1})).await;
            assert_eq!(response.is_error, Some(true));
            let crate::mcp::protocol::ContentBlock::Text { text } = &response.content[0];
            let v: serde_json::Value = serde_json::from_str(text).unwrap();
            serde_json::from_value(v["batch"].clone()).unwrap()
        };
        assert_eq!((out.succeeded, out.failed, out.unresolved), (1, 1, 0));
        assert_eq!(out.items[1].error.as_ref().unwrap().code, "INVALID_INPUT");
        assert!(out.items[1].receipt.is_none());
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .committed_generation,
            before.committed_generation.next().unwrap().next().unwrap()
        );
        let child = out.items[0].receipt.as_ref().unwrap().operation_id;
        let replay = adapter.mutate_batch(r.clone()).await.unwrap();
        assert_eq!(
            replay.items[0].receipt.as_ref().unwrap().operation_id,
            child
        );
        let mut conflicting = r.clone();
        conflicting.items[0]["embedding"] = json!([1., 0., 0., 0.]);
        assert!(adapter.mutate_batch(conflicting).await.is_err());
        // Reservation publication precedes provider work. Preserve its fail-closed
        // boundary separately from the later committed-child publication failure.
        struct Paused {
            profile: crate::model::schema::EmbeddingProfile,
            entered: Arc<tokio::sync::Notify>,
            release: Arc<tokio::sync::Notify>,
            calls: Arc<AtomicUsize>,
        }
        #[async_trait::async_trait]
        impl crate::embedding::EmbeddingProvider for Paused {
            fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
                Some(self.profile.clone())
            }
            fn dimensions(&self) -> usize {
                4
            }
            fn model_name(&self) -> &str {
                "native-publication-boundary"
            }
            async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.entered.notify_one();
                self.release.notified().await;
                Ok(vec![2., 0., 0., 0.])
            }
        }
        r.idempotency_key = Some("publication".into());
        r.items.truncate(1);
        r.items[0].as_object_mut().unwrap().remove("embedding");
        r.items[0]
            .as_object_mut()
            .unwrap()
            .remove("embeddingProfile");
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let adapter = Arc::new(
            crate::mcp::bridge_adapters::McpStorageAdapter::new(
                f.storage.clone(),
                f.cache.clone(),
                Arc::new(Paused {
                    profile: d.profile.clone(),
                    entered: entered.clone(),
                    release: release.clone(),
                    calls: calls.clone(),
                }),
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
                f.graph.clone(),
                disabled_aux_config(),
                chrono_tz::UTC,
            )
            .with_clock(Arc::new(FixedClock(AtomicUsize::new(0)))),
        );
        let rows_before_reservation = f.storage.read().unwrap().scan_all().unwrap().len();
        *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
        let reservation_failed = adapter.mutate_batch(r.clone()).await.unwrap();
        assert_eq!(
            reservation_failed.items[0].status,
            BatchStatus::NotAttempted
        );
        assert!(reservation_failed.items[0].receipt.is_none());
        assert!(!reservation_failed.items[0].ready);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            f.storage.read().unwrap().scan_all().unwrap().len(),
            rows_before_reservation
        );
        let reservation = reservation_failed.reservation_receipt.as_ref().unwrap();
        assert_eq!(
            reservation.operation_id,
            reservation_failed.batch_operation_id
        );
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .lookup_operation(
                    reservation.store_id,
                    MutationScope::Catalog,
                    reservation.operation_id
                )
                .unwrap()
                .unwrap(),
            *reservation
        );
        *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
        let pending = r.clone();
        let writer = adapter.clone();
        let task = tokio::spawn(async move { writer.mutate_batch(pending).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
            .await
            .unwrap();
        let before_child = f.storage.read().unwrap().control_state().unwrap();
        assert!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
        let row_count = f.storage.read().unwrap().scan_all().unwrap().len();
        // Now the actual reservation publisher completed. The valid generated
        // vector lets the fresh child commit before its own publication fails.
        *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
        release.notify_one();
        let failed = task.await.unwrap().unwrap();
        assert_eq!(failed.items[0].status, BatchStatus::Ok);
        assert!(!failed.items[0].ready);
        let committed = failed.items[0].receipt.as_ref().unwrap();
        assert_ne!(committed.operation_id, failed.batch_operation_id);
        assert_ne!(committed.operation_id, child);
        assert_eq!(
            committed.committed_generation,
            before_child.committed_generation.next().unwrap()
        );
        assert_eq!(
            f.storage.read().unwrap().scan_all().unwrap().len(),
            row_count + 1
        );
        *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
        let recovered = adapter.mutate_batch(r).await.unwrap();
        assert!(recovered.all_ready(), "{recovered:?}");
        assert!(!committed.replayed);
        let mut replayed = committed.clone();
        replayed.replayed = true;
        assert_eq!(recovered.items[0].receipt.as_ref().unwrap(), &replayed);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .lookup_operation(
                    committed.store_id,
                    MutationScope::Namespace(d.ns),
                    committed.operation_id
                )
                .unwrap()
                .unwrap(),
            *committed
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "replay must not call provider"
        );
        assert_eq!(
            f.storage.read().unwrap().scan_all().unwrap().len(),
            row_count + 1
        );
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .committed_generation,
            committed.committed_generation
        );
    }
}

#[test]
fn native_batch_crash_child() {
    let Ok(path) = std::env::var("RECALLD_NATIVE_BATCH_CHILD") else {
        return;
    };
    let request_path = std::env::var("RECALLD_NATIVE_BATCH_REQUEST").unwrap();
    let r: BatchInput = serde_json::from_slice(&std::fs::read(request_path).unwrap()).unwrap();
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session =
        NativeLifecycleSession::open(Path::new(&path), NativePolicyInputs::default(), clock)
            .unwrap();
    let ps = plans(&r);
    let p = session
        .metadata()
        .reserve_batch(&r.context(), r.kind, &ps)
        .unwrap();
    let child = p.child(0).unwrap();
    let BatchItemPlan::NativeInsert { input, .. } = &ps[0] else {
        panic!()
    };
    let scope = LifecycleScope {
        namespace_id: child.namespace(),
        namespace_name: child.namespace_name().into(),
        source: None,
    };
    let intent = session.capture_insert(scope, input).unwrap();
    session
        .engine
        .insert_native_record_reserved(
            &child.context(),
            &intent,
            &generated(),
            &session.policy,
            session.clock.as_ref(),
            None,
            Some(&child),
        )
        .unwrap();
    panic!("crash hook did not exit")
}
#[test]
fn native_batch_process_crash_before_and_after_child_commit_keeps_reserved_identity() {
    for point in ["before_metadata_commit", "after_metadata_commit"] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let s = open(&d, clock.clone());
        let r = request(&d, s.metadata().control_state().unwrap().store_id, "crash");
        let p = s
            .metadata()
            .reserve_batch(&r.context(), r.kind, &plans(&r))
            .unwrap();
        let id = p.child(0).unwrap().operation_id;
        drop(s);
        let request_file = root.path().join("resubmission.json");
        std::fs::write(&request_file, serde_json::to_vec(&r).unwrap()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::metadata::native_records::tests::batches::native_batch_crash_child",
                "--nocapture",
            ])
            .env("RECALLD_NATIVE_BATCH_CHILD", &d.path)
            .env("RECALLD_NATIVE_BATCH_REQUEST", request_file)
            .env("RECALLD_TEST_EFFECT_CRASH", point)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(91));
        let mut s = open(&d, clock);
        let replay = s
            .metadata()
            .reserve_batch(&r.context(), r.kind, &plans(&r))
            .unwrap();
        if point == "after_metadata_commit" {
            assert!(matches!(replay.items[0].state, BatchItemState::Ok { .. }));
        } else {
            assert!(matches!(
                replay.items[0].state,
                BatchItemState::NotAttempted {}
            ));
        }
        let receipt = insert_reserved(&mut s, &d, &r, &replay);
        assert_eq!(receipt.operation_id, id);
        assert_eq!(s.engine.scan_all().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn native_batch_complete_export_import_restore_preserves_receipts_pending_rejects() {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let r = request(
        &d,
        s.metadata().control_state().unwrap().store_id,
        "offline",
    );
    let p = s
        .metadata()
        .reserve_batch(&r.context(), r.kind, &plans(&r))
        .unwrap();
    drop(s);
    let stream = root.path().join("batch.jsonl");
    assert!(export_store(&d.path, &stream).is_err());
    assert!(!stream.exists());
    s = open(&d, clock.clone());
    let receipt = insert_reserved(&mut s, &d, &r, &p);
    let id = inserted(&receipt);
    drop(s);
    let expected = export_store(&d.path, &stream).unwrap();
    let imported = root.path().join("imported");
    assert_eq!(import_store(&stream, &imported).unwrap(), expected);
    let imported =
        NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let replay = imported
        .metadata()
        .reserve_batch(&r.context(), r.kind, &plans(&r))
        .unwrap();
    assert!(replay.reservation_receipt.replayed);
    assert_eq!(
        replay.items[0].state,
        BatchItemState::Ok {
            receipt: receipt.clone()
        }
    );
    drop(imported);
    let backup = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("snapshot.zip"),
        Some(&d.path),
        false,
    )
    .await
    .unwrap();
    let mut s = open(&d, clock.clone());
    let intent = s.capture_tombstone(id, None).unwrap();
    s.tombstone(&ctx(&s, &d, "postbackup-delete"), &intent)
        .unwrap();
    drop(s);
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: d.path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let s = open(&d, clock);
    assert_ne!(
        s.metadata().control_state().unwrap().store_id,
        receipt.store_id
    );
    assert_eq!(
        s.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
    assert!(matches!(
        s.metadata().reserve_batch(&r.context(), r.kind, &plans(&r)),
        Err(StorageError::StoreLineageMismatch)
    ));
}

#[test]
fn native_batch_caller_fixed_defaults_equivalent_arrays_and_original_bits_conflict() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let mut r = request(&d, uuid::Uuid::new_v4(), "hash");
    r.items[0]
        .as_object_mut()
        .unwrap()
        .remove("checkDuplicates");
    let hash = plans(&r)[0].request_hash();
    let mut expanded = r.clone();
    expanded.items[0]["checkDuplicates"] = json!(true);
    expanded.items[0]["tags"] = json!([]);
    expanded.items[0]["topics"] = json!([]);
    expanded.items[0]["emotions"] = json!([]);
    assert_eq!(hash, plans(&expanded)[0].request_hash());
    for field in ["entities", "embedding"] {
        let mut changed = r.clone();
        changed.items[0][field].as_array_mut().unwrap().reverse();
        assert_ne!(hash, plans(&changed)[0].request_hash());
    }
    let mut changed = r.clone();
    changed.items[0]["embedding"] = json!([2., -0., 0., 0.]);
    assert_ne!(hash, plans(&changed)[0].request_hash());
    let mut legacy = r;
    legacy.mutation_version = 1;
    assert!(
        serde_json::to_value(legacy)
            .unwrap()
            .get("mutationVersion")
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_batch_generated_invalid_output_stays_pending_without_payload_then_replays_without_provider()
 {
    struct Provider {
        calls: Arc<AtomicUsize>,
        profile: crate::model::schema::EmbeddingProfile,
    }
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for Provider {
        fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
            Some(self.profile.clone())
        }
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> &str {
            "synthetic-native-batch"
        }
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            Ok(if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                vec![0.; 4]
            } else {
                vec![2., 0., 0., 0.]
            })
        }
    }
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let mut r = request(
        &d,
        s.metadata().control_state().unwrap().store_id,
        "provider",
    );
    r.items[0].as_object_mut().unwrap().remove("embedding");
    r.items[0]
        .as_object_mut()
        .unwrap()
        .remove("embeddingProfile");
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let adapter = crate::mcp::bridge_adapters::McpStorageAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        Arc::new(Provider {
            calls: calls.clone(),
            profile: d.profile.clone(),
        }),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
        disabled_aux_config(),
        chrono_tz::UTC,
    )
    .with_clock(clock);
    let before = f.storage.read().unwrap().control_state().unwrap();
    let failed = adapter.mutate_batch(r.clone()).await.unwrap();
    assert_eq!(failed.items[0].status, BatchStatus::NotAttempted);
    assert_eq!(failed.items[0].error.as_ref().unwrap().code, "UNAVAILABLE");
    assert!(failed.items[0].retryable);
    assert!(failed.items[0].receipt.is_none());
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 1);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        1
    );
    let completed = adapter.mutate_batch(r.clone()).await.unwrap();
    assert!(completed.all_ready(), "{completed:?}");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let replay = adapter.mutate_batch(r).await.unwrap();
    assert!(replay.all_ready());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        replay.items[0].receipt.as_ref().unwrap().operation_id,
        completed.items[0].receipt.as_ref().unwrap().operation_id
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_batch_root_before_provider_drains_descendants_during_close() {
    struct Paused {
        profile: crate::model::schema::EmbeddingProfile,
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for Paused {
        fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
            Some(self.profile.clone())
        }
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> &str {
            "native-close-proof"
        }
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            self.entered.notify_one();
            self.release.notified().await;
            Ok(vec![1., 0., 0., 0.])
        }
    }
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let mut r = request(
        &d,
        s.metadata().control_state().unwrap().store_id,
        "closing",
    );
    r.items[0].as_object_mut().unwrap().remove("embedding");
    r.items[0]
        .as_object_mut()
        .unwrap()
        .remove("embeddingProfile");
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let adapter = crate::mcp::bridge_adapters::McpStorageAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        Arc::new(Paused {
            profile: d.profile,
            entered: entered.clone(),
            release: release.clone(),
        }),
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
        disabled_aux_config(),
        chrono_tz::UTC,
    )
    .with_clock(clock);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let request = r.clone();
    let task = tokio::spawn(async move { adapter.mutate_batch(request).await });
    tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
        .await
        .unwrap();
    assert!(tracker.status().pending > 0);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        1
    );
    let closing = tracker.clone();
    let drain = tokio::task::spawn_blocking(move || {
        closing.close_and_drain(std::time::Instant::now() + std::time::Duration::from_secs(10))
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while tracker.status().admission_open {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    let before = f.storage.read().unwrap().control_state().unwrap();
    assert!(f.storage_adapter().mutate_batch(r).await.is_err());
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    release.notify_one();
    let outcome = task.await.unwrap().unwrap();
    assert!(outcome.all_ready(), "{outcome:?}");
    let freeze = drain.await.unwrap().unwrap();
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        0
    );
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    drop(freeze);
}

// Independent review probes. Keep these as end-to-end regressions: all the
// individual receipts below are valid; the manifest's chronological claim is not.
#[test]
fn native_batch_review_reversed_error_generations_reject_unchanged_source() {
    for middle_success in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let mut s = open(&d, clock.clone());
        let mut r = request(
            &d,
            s.metadata().control_state().unwrap().store_id,
            "reverse-errors",
        );
        r.items.push(r.items[0].clone());
        if middle_success {
            r.items.push(r.items[0].clone());
        }
        let ps = plans(&r);
        let p = s
            .metadata()
            .reserve_batch(&r.context(), r.kind, &ps)
            .unwrap();
        s.metadata()
            .finalize_batch_error(
                &p.child(0).unwrap(),
                crate::storage::batch::BatchError::new("INVALID_INPUT"),
            )
            .unwrap();
        if middle_success {
            let BatchItemPlan::NativeInsert { input, .. } = &ps[1] else {
                panic!()
            };
            let intent = s.capture_insert(scope(&d), input).unwrap();
            let child = p.child(1).unwrap();
            s.engine
                .insert_native_record_reserved(
                    &child.context(),
                    &intent,
                    &generated(),
                    &s.policy,
                    s.clock.as_ref(),
                    None,
                    Some(&child),
                )
                .unwrap();
        }
        let last = r.items.len() - 1;
        let mut p = s
            .metadata()
            .finalize_batch_error(
                &p.child(last).unwrap(),
                crate::storage::batch::BatchError::new("INVALID_INPUT"),
            )
            .unwrap();
        let state = s.metadata().control_state().unwrap();
        crate::storage::metadata::batch::validate_existing(
            &s.metadata().db.begin_read().unwrap(),
            Some(&state),
        )
        .unwrap();
        let tx = s.metadata().db.begin_write().unwrap();
        for receipt in &mut p.bookkeeping_receipts {
            let MutationOutcome::BatchItemRejected { index, .. } = &mut receipt.outcome else {
                panic!()
            };
            *index = last as u32 - *index;
            let hash = control::request_hash(
                &r.context(),
                "batch_item_error_v2",
                &[
                    p.reservation_receipt.operation_id.as_bytes(),
                    &index.to_be_bytes(),
                ],
            );
            let key = control::scoped_key(
                receipt.store_id,
                MutationScope::Catalog,
                receipt.operation_id.as_bytes(),
            );
            let stored = control::StoredReceipt {
                request_hash_version: 1,
                request_hash: hash,
                receipt: receipt.clone(),
            };
            tx.open_table(control::RECEIPTS)
                .unwrap()
                .insert(key.as_slice(), control::encode(&stored).unwrap().as_slice())
                .unwrap();
        }
        tx.open_table(crate::storage::metadata::batch::BATCHES)
            .unwrap()
            .insert(
                p.reservation_receipt.operation_id.as_bytes().as_slice(),
                control::encode(&p).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
        drop(s);
        let before = std::fs::read(d.path.join("meta.db")).unwrap();
        assert!(
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err(),
            "impossible final-error history was admitted (middle success={middle_success})"
        );
        assert_eq!(before, std::fs::read(d.path.join("meta.db")).unwrap());
    }
}

#[tokio::test]
async fn native_batch_review_unknown_profile_forget_matches_single_and_preserves_offline() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("legacy");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let mut config = NamespaceConfig::default_namespace(0);
    config.name = "unknown".into();
    config.embedding_dim = 4;
    let ns = engine.create_namespace(&config).unwrap();
    let original = crate::storage::insertion::InsertMemory {
        id: None,
        summary: "retained unknown provenance".into(),
        full_text: None,
        tags: vec![],
        initial_stability: None,
        created_at: Some(1000),
        embedding: Some(vec![1., 0., 0., 0.]),
        parent_id: None,
        supersedes: None,
    };
    let mut ids = vec![];
    for _ in 0..2 {
        let receipt = engine
            .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &original, &[], 1000)
            .unwrap();
        ids.push(crate::storage::insertion::inserted_id(&receipt).unwrap());
    }
    drop(engine);
    let path = root.path().join("native");
    migrate_schema(
        &source,
        &path,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock.clone()).unwrap();
    let store_id = s.metadata().control_state().unwrap().store_id;
    let context = MutationContext {
        store_id: Some(store_id),
        scope: MutationScope::Namespace(ns),
        idempotency_key: Some("single-unknown-forget".into()),
        expected_generation: None,
        contract_version: 2,
    };
    let intent = s.capture_tombstone(ids[0], None).unwrap();
    s.tombstone(&context, &intent).unwrap();
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).complete().await.unwrap();
    let request = BatchInput {
        mutation_version: 2,
        result_version: 1,
        kind: BatchKind::Forget,
        items: vec![json!(ids[1].to_string())],
        default_namespace: "irrelevant-by-id".into(),
        store_id: Some(store_id),
        idempotency_key: Some("batch-unknown-forget".into()),
        batch_operation_id: None,
    };
    let response = crate::daemon::server::native_batch_test_rpc(
        Arc::new(disabled_aux_adapter(&f, clock.clone())),
        &request,
    )
    .await;
    assert!(response.error.is_none(), "{response:?}");
    let outcome: crate::storage::batch::BatchOutcome =
        serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(
        outcome.all_ready(),
        "native batch forget rejected unknown provenance: {outcome:?}"
    );
    let p = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .lookup_batch(store_id, outcome.batch_operation_id)
        .unwrap()
        .unwrap();
    assert!(matches!(
        p.items[0]
            .defaults
            .as_ref()
            .unwrap()
            .native
            .as_ref()
            .unwrap()
            .embedding_binding,
        crate::model::schema::EmbeddingBinding::UnknownLegacy
    ));
    let root = f.close();
    let stream = root.path().join("unknown.jsonl");
    let imported = root.path().join("imported");
    let exported = crate::storage::logical_export::export_store(&path, &stream).unwrap();
    assert_eq!(
        crate::storage::logical_export::import_store(&stream, &imported).unwrap(),
        exported
    );
    let s = NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), clock).unwrap();
    let replay = s
        .metadata()
        .reserve_batch(&request.context(), request.kind, &plans(&request))
        .unwrap();
    assert!(replay.reservation_receipt.replayed);
    assert_eq!(replay.items, p.items);
}

#[tokio::test]
async fn native_batch_configured_auxiliary_work_is_pending_not_silently_skipped_and_replay_precedes_gate()
 {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let mut r = request(
        &d,
        s.metadata().control_state().unwrap().store_id,
        "auxiliary",
    );
    let f = Fixture::from_open_engine(root, s.engine);
    completion(&f).rebuild_all().await.unwrap();
    let configured = f.storage_adapter().with_clock(clock.clone());
    let before = f.storage.read().unwrap().control_state().unwrap();
    let blocked = configured.mutate_batch(r.clone()).await.unwrap();
    assert_eq!(blocked.items[0].status, BatchStatus::NotAttempted);
    assert_eq!(
        blocked.items[0].error.as_ref().unwrap().field.as_deref(),
        Some("graph.autolinkEnabled")
    );
    assert_eq!(blocked.items[0].error.as_ref().unwrap().code, "UNAVAILABLE");
    assert!(blocked.items[0].receipt.is_none());
    assert!(!blocked.items[0].ready);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 1);
    let disabled = disabled_aux_adapter(&f, clock);
    let completed = disabled.mutate_batch(r.clone()).await.unwrap();
    assert!(completed.all_ready(), "{completed:?}");
    let before = f.storage.read().unwrap().control_state().unwrap();
    let replay = configured.mutate_batch(r.clone()).await.unwrap();
    assert!(replay.all_ready());
    assert_eq!(
        replay.items[0].receipt.as_ref().unwrap().operation_id,
        completed.items[0].receipt.as_ref().unwrap().operation_id
    );
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
    r.idempotency_key = Some("advisory".into());
    r.items[0]["checkDuplicates"] = json!(true);
    let blocked = disabled.mutate_batch(r).await.unwrap();
    assert_eq!(blocked.items[0].status, BatchStatus::NotAttempted);
    assert_eq!(
        blocked.items[0].error.as_ref().unwrap().field.as_deref(),
        Some("checkDuplicates")
    );
    assert!(blocked.items[0].receipt.is_none());
    assert!(!blocked.all_ready());
    assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 2);
}
