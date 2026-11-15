//! Actual legacy-disk consumers and genuine pre-extension format1 fixtures.
#[path = "legacy_semantic_admission_tests.rs"]
mod semantic_admission;
use super::*;
use crate::mcp::bridge::StorageEngine as _;
use crate::storage::batch::{BatchInput, BatchKind};
use crate::storage::control::{MutationContext, MutationScope};
use crate::storage::insertion::InsertMemory;
use crate::test_support::Fixture;
use serde_json::{Value, json};

fn request(f: &Fixture, key: &str, version: u32, items: Vec<Value>) -> BatchInput {
    BatchInput {
        request_context: None,
        expected_generation: None,
        result_version: 1,
        mutation_version: version,
        kind: BatchKind::Insert,
        items,
        default_namespace: "default".into(),
        store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
        idempotency_key: Some(key.into()),
        batch_operation_id: None,
    }
}
fn item() -> Value {
    json!({"summary":"legacy original intent","fullText":"synthetic retained text",
        "embedding":[1.,0.,0.,0.],"checkDuplicates":false})
}
fn current(f: &Fixture) -> crate::storage::control::ControlState {
    f.storage.read().unwrap().control_state().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_actual_single_insert_two() {
    let f = Fixture::new();
    let owner = f.storage_adapter().batch_completion();
    let before = current(&f);
    let ctx = MutationContext {
        contract_version: 2,
        store_id: Some(before.store_id),
        scope: MutationScope::Namespace(NamespaceId::new(1)),
        idempotency_key: Some("actual-legacy-insert2".into()),
        expected_generation: Some(before.committed_generation),
    };
    let input = InsertMemory {
        id: None,
        summary: "legacy API2".into(),
        full_text: Some("synthetic".into()),
        tags: vec![],
        initial_stability: None,
        created_at: None,
        embedding: Some(vec![1., 0., 0., 0.]),
        parent_id: None,
        supersedes: None,
    };
    let result = owner
        .insert(ctx.clone(), input.clone(), vec![].into(), 100)
        .await;
    assert!(
        result.is_ok(),
        "actual legacy2 insertion rejected: {:?}",
        result.as_ref().err()
    );
    assert_eq!(
        current(&f).committed_generation.get(),
        before.committed_generation.get() + 1
    );
    let receipt = result.unwrap().receipt;
    let now = current(&f);
    let mut replay = owner
        .insert(ctx.clone(), input.clone(), vec![f32::NAN].into(), i64::MAX)
        .await
        .unwrap()
        .receipt;
    assert!(replay.replayed);
    replay.replayed = false;
    assert_eq!(replay, receipt);
    let mut other = ctx.clone();
    other.contract_version = 1;
    assert!(matches!(
        owner.insert(other, input.clone(), vec![].into(), 100).await,
        Err(crate::storage::StorageError::IdempotencyConflict)
    ));
    let mut stale = ctx;
    stale.idempotency_key = Some("new-stale-key".into());
    assert!(matches!(
        owner.insert(stale, input, vec![].into(), 100).await,
        Err(crate::storage::StorageError::GenerationConflict { .. })
    ));
    assert_eq!(current(&f), now);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_actual_batch_two() {
    let f = Fixture::new();
    let input = request(&f, "actual-legacy-batch2", 4, vec![item()]);
    let mut adapter = f.storage_adapter();
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let result = adapter.mutate_batch(input).await;
    assert!(result.is_ok(), "actual legacy2 batch rejected: {result:?}");
    assert!(result.unwrap().all_ready());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "genuine BEFORE exporter; only run on recorded pre-extension source"]
async fn legacy_compound_origin_export_genuine_old_one() {
    let Ok(destination) = std::env::var("RECALLD_LEGACY_COMPOUND_BEFORE") else {
        return;
    };
    let f = Fixture::new();
    let mut adapter = f.storage_adapter();
    // Explicit no-autolink fixture: isolate the reservation and child generations.
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let mut cases = Vec::new();
    for with_cas in [false, true] {
        for invalid in [false, true] {
            let key = format!("genuine-one-{with_cas}-{invalid}");
            let before = current(&f);
            let mut input = request(
                &f,
                &key,
                1,
                vec![if invalid {
                    json!({"summary":""})
                } else {
                    item()
                }],
            );
            input.expected_generation = with_cas.then_some(before.committed_generation);
            // Historical Some-CAS is supported by this actual private runner;
            // the old public adapter intentionally does not advertise it.
            let outcome = adapter.run_batch(input.clone()).await.unwrap();
            assert_eq!(
                outcome
                    .reservation_receipt
                    .as_ref()
                    .unwrap()
                    .committed_generation
                    .get(),
                before.committed_generation.get() + 1
            );
            assert_eq!(
                current(&f).committed_generation.get(),
                before.committed_generation.get() + if invalid { 1 } else { 2 }
            );
            assert_eq!(outcome.failed, usize::from(invalid));
            assert_eq!(outcome.succeeded, usize::from(!invalid));
            let replay = adapter.run_batch(input.clone()).await.unwrap();
            assert_eq!(replay.batch_operation_id, outcome.batch_operation_id);
            cases.push(json!({"input":input,"outcome":outcome,"before":before}));
        }
    }
    drop(adapter);
    let root = f.close();
    // Real backup of completed old-format work, not hand-written canonical rows.
    crate::backup::run_backup(
        &Default::default(),
        std::path::Path::new(&destination),
        Some(root.path()),
        false,
    )
    .await
    .unwrap();
    let receipts = historical_rows(root.path());
    std::fs::write(
        format!("{destination}.json"),
        serde_json::to_vec_pretty(&json!({"cases":cases,"rows":receipts})).unwrap(),
    )
    .unwrap();
}

fn historical_rows(root: &std::path::Path) -> Value {
    use redb::ReadableTable;
    let db = redb::Database::open(root.join("meta.db")).unwrap();
    let tx = db.begin_read().unwrap();
    let mut rows = serde_json::Map::new();
    for name in ["mutation_receipts", "mutation_batches"] {
        let table = tx
            .open_table(redb::TableDefinition::<&[u8], &[u8]>::new(name))
            .unwrap();
        let items: Vec<_> = table
            .iter()
            .unwrap()
            .map(|row| {
                let (k, v) = row.unwrap();
                json!({"key":k.value(),"value":String::from_utf8(v.value().to_vec()).unwrap()})
            })
            .collect();
        rows.insert(name.into(), json!(items));
    }
    Value::Object(rows)
}

struct SpyProvider(std::sync::atomic::AtomicUsize);
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for SpyProvider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(vec![1., 0., 0., 0.])
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "synthetic-legacy-origin"
    }
}
struct SpyClock(std::sync::atomic::AtomicUsize);
impl crate::time::Clock for SpyClock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        100
    }
}

struct BlockedProvider {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for BlockedProvider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.entered.notify_one();
        self.release.notified().await;
        Ok(vec![1., 0., 0., 0.])
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "synthetic-blocked-origin"
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_actual_rpc_publication_and_unknown_keep_current_child() {
    use crate::storage::batch::{BatchOutcome, BatchStatus};
    for unknown in [false, true] {
        let f = Fixture::new();
        let mut adapter = f.storage_adapter();
        Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        adapter.embedding = Arc::new(BlockedProvider {
            entered: entered.clone(),
            release: release.clone(),
            calls: calls.clone(),
        });
        let adapter = Arc::new(adapter);
        let mut value = item();
        value.as_object_mut().unwrap().remove("embedding");
        let input = request(&f, "publication", 4, vec![value]);
        let task = {
            let adapter = adapter.clone();
            let input = input.clone();
            tokio::spawn(async move {
                crate::daemon::server::native_batch_test_rpc(adapter, &input).await
            })
        };
        tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
            .await
            .unwrap();
        let before_child = current(&f);
        if unknown {
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .fail_batch_commit
                .store(3, std::sync::atomic::Ordering::SeqCst);
        } else {
            // Reservation publication already completed; fail the actual CHILD rebuild.
            *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
        }
        release.notify_one();
        let reply = task.await.unwrap();
        assert!(reply.error.is_none(), "{:?}", reply.error);
        let failed: BatchOutcome = serde_json::from_value(reply.result.unwrap()).unwrap();
        assert_eq!(
            failed.items[0].status,
            if unknown {
                BatchStatus::Unknown
            } else {
                BatchStatus::Ok
            }
        );
        assert!(!failed.items[0].ready);
        let child_id = failed.items[0].operation_id.unwrap();
        assert_ne!(child_id, failed.batch_operation_id);
        assert_eq!(
            failed
                .reservation_receipt
                .as_ref()
                .unwrap()
                .committed_generation,
            before_child.committed_generation
        );
        if let Some(receipt) = &failed.items[0].receipt {
            assert_eq!(receipt.operation_id, child_id);
            assert_eq!(
                receipt.committed_generation.get(),
                before_child.committed_generation.get() + 1
            );
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        drop(adapter);
        let root = f.close();
        let engine = RedbStorageEngine::open(root.path()).unwrap();
        let f = Fixture::from_open_engine(root, engine);
        let before = current(&f);
        let spy = Arc::new(SpyProvider(Default::default()));
        let mut adapter = f.storage_adapter();
        adapter.embedding = spy.clone();
        // Reopen repairs canonical/unknown state; completed replay ignores current auto settings.
        let result = adapter.mutate_batch(input).await.unwrap();
        assert!(result.all_ready());
        let mut receipt = result.items[0].receipt.clone().unwrap();
        assert!(receipt.replayed);
        receipt.replayed = false;
        assert_eq!(receipt.operation_id, child_id);
        if let Some(original) = &failed.items[0].receipt {
            assert_eq!(&receipt, original);
        }
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .lookup_operation(
                    receipt.store_id,
                    MutationScope::Namespace(NamespaceId::new(1)),
                    child_id
                )
                .unwrap(),
            Some(receipt)
        );
        assert_eq!(current(&f), before);
        assert_eq!(spy.0.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(f.storage.read().unwrap().scan_all().unwrap().len(), 1);
    }
}
fn files(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    fn visit(base: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(root).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                visit(base, &p, out)
            } else if p.extension().is_none_or(|e| e != "lock") {
                out.push((
                    p.strip_prefix(base).unwrap().to_string_lossy().into(),
                    std::fs::read(p).unwrap(),
                ));
            }
        }
    }
    let mut out = vec![];
    visit(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_genuine_old_one_replay_preserves_bytes() {
    let root = tempfile::tempdir().unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "fixtures/legacy-compound-before.zip"
    )))
    .unwrap()
    .extract(root.path())
    .unwrap();
    let expected: Value =
        serde_json::from_slice(include_bytes!("fixtures/legacy-compound-before.zip.json")).unwrap();
    assert_eq!(historical_rows(root.path()), expected["rows"]);
    let engine = RedbStorageEngine::open(root.path()).unwrap();
    let f = Fixture::from_open_engine(root, engine);
    let before = current(&f);
    let spy = Arc::new(SpyProvider(Default::default()));
    let clock = Arc::new(SpyClock(Default::default()));
    let mut adapter = f.storage_adapter().with_clock(clock.clone());
    adapter.embedding = spy.clone();
    // Current settings differ deliberately; completed replay precedes selection.
    assert!(adapter.config.graph.autolink_enabled);
    for case in expected["cases"].as_array().unwrap() {
        let input: BatchInput = serde_json::from_value(case["input"].clone()).unwrap();
        let original: crate::storage::batch::BatchOutcome =
            serde_json::from_value(case["outcome"].clone()).unwrap();
        let replay = adapter.run_batch(input.clone()).await.unwrap();
        let mut receipt = replay.reservation_receipt.unwrap();
        assert!(receipt.replayed);
        receipt.replayed = false;
        assert_eq!(Some(receipt), original.reservation_receipt);
        for (a, b) in replay.items.iter().zip(&original.items) {
            let mut a = a.receipt.clone();
            if let Some(r) = &mut a {
                r.replayed = false
            };
            assert_eq!(a, b.receipt);
        }
        let mut changed = input;
        changed.items[0] = json!({"summary":"different request"});
        assert!(adapter.run_batch(changed).await.is_err());
        assert_eq!(current(&f), before);
    }
    assert_eq!(spy.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(adapter);
    let root = f.close();
    assert_eq!(historical_rows(root.path()), expected["rows"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_fresh_selection_gate_no_mutation_and_completed_replay_first() {
    let f = Fixture::new();
    let spy = Arc::new(SpyProvider(Default::default()));
    let clock = Arc::new(SpyClock(Default::default()));
    let mut adapter = f.storage_adapter().with_clock(clock.clone());
    adapter.embedding = spy.clone();
    for advisory in [false, true] {
        Arc::make_mut(&mut adapter.config).graph.autolink_enabled = !advisory;
        let mut value = item();
        value.as_object_mut().unwrap().remove("embedding");
        value["checkDuplicates"] = json!(advisory);
        let input = request(&f, "gated", 4, vec![value]);
        let before = current(&f);
        let bytes = files(f.path());
        let error = adapter.mutate_batch(input).await.unwrap_err();
        assert!(
            matches!(error, bridge::BridgeError::Unavailable(_)),
            "{error}"
        );
        assert_eq!(current(&f), before);
        assert_eq!(files(f.path()), bytes);
        assert_eq!(spy.0.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let mut value = item();
    value.as_object_mut().unwrap().remove("embedding");
    let mut input = request(&f, "complete", 4, vec![value]);
    input.expected_generation = Some(current(&f).committed_generation);
    let first = adapter.mutate_batch(input.clone()).await.unwrap();
    assert!(first.all_ready());
    let before = current(&f);
    let calls = spy.0.load(std::sync::atomic::Ordering::SeqCst);
    let clocks = clock.0.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(calls, 1);
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = true;
    let reply = crate::daemon::server::native_batch_test_rpc(Arc::new(adapter), &input).await;
    assert!(reply.error.is_none(), "{:?}", reply.error);
    let replay: crate::storage::batch::BatchOutcome =
        serde_json::from_value(reply.result.unwrap()).unwrap();
    assert!(replay.all_ready());
    assert_eq!(replay.batch_operation_id, first.batch_operation_id);
    let mut receipt = replay.items[0].receipt.clone().unwrap();
    assert!(receipt.replayed);
    receipt.replayed = false;
    assert_eq!(Some(receipt), first.items[0].receipt);
    assert_eq!(current(&f), before);
    assert_eq!(spy.0.load(std::sync::atomic::Ordering::SeqCst), calls);
    assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), clocks);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_pending_selection_gate_keeps_exact_parent_rpc() {
    let f = Fixture::new();
    let input = request(&f, "pending", 4, vec![item()]);
    let plans: Vec<_> = crate::mcp::batches::prepare(&input)
        .unwrap()
        .into_iter()
        .map(|p| p.plan)
        .collect();
    let p = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .reserve_batch_version_with_work(
            &input.context(),
            4,
            input.kind,
            &plans,
            None,
            None,
            || Ok(None),
        )
        .unwrap();
    let before = current(&f);
    let adapter = f.storage_adapter();
    assert!(adapter.config.graph.autolink_enabled);
    let reply = crate::daemon::server::native_batch_test_rpc(Arc::new(adapter), &input).await;
    assert!(reply.error.is_none(), "{:?}", reply.error);
    let outcome: crate::storage::batch::BatchOutcome =
        serde_json::from_value(reply.result.unwrap()).unwrap();
    let mut receipt = outcome.reservation_receipt.unwrap();
    receipt.replayed = false;
    assert_eq!(receipt, p.reservation_receipt);
    assert_eq!(
        outcome.items[0].status,
        crate::storage::batch::BatchStatus::NotAttempted
    );
    assert_eq!(outcome.items[0].operation_id, Some(p.items[0].operation_id));
    assert!(outcome.items[0].receipt.is_none());
    assert_eq!(outcome.items[0].error.as_ref().unwrap().code, "UNAVAILABLE");
    assert_eq!(current(&f), before);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_invalid_only_records_parent_and_fixed_default_hashes() {
    let f = Fixture::new();
    let adapter = f.storage_adapter();
    let before = current(&f);
    let input = request(
        &f,
        "invalid-only",
        4,
        vec![json!(17), json!({"summary":""})],
    );
    let result = adapter.mutate_batch(input.clone()).await.unwrap();
    assert_eq!(result.failed, 2);
    assert_eq!(result.succeeded, 0);
    assert_eq!(result.unresolved, 0);
    assert!(result.items.iter().all(|i| i.receipt.is_none()));
    assert_eq!(
        current(&f).committed_generation.get(),
        before.committed_generation.get() + 1
    );
    let replay = adapter.mutate_batch(input).await.unwrap();
    assert_eq!(replay.batch_operation_id, result.batch_operation_id);
    let mut explicit = item();
    explicit["checkDuplicates"] = json!(true);
    let mut absent = explicit.clone();
    absent.as_object_mut().unwrap().remove("checkDuplicates");
    let a = crate::mcp::batches::prepare(&request(&f, "hash", 4, vec![explicit])).unwrap();
    let b = crate::mcp::batches::prepare(&request(&f, "hash", 4, vec![absent])).unwrap();
    assert_eq!(a[0].plan.request_hash(), b[0].plan.request_hash());
    let c = crate::mcp::batches::prepare(&request(&f, "hash", 4, vec![item()])).unwrap();
    assert_ne!(a[0].plan.request_hash(), c[0].plan.request_hash());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_exact_hash_conflicts_and_forget_rpc() {
    let f = Fixture::new();
    let mut adapter = f.storage_adapter();
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let mut input = request(&f, "identity", 4, vec![item()]);
    input.expected_generation = Some(current(&f).committed_generation);
    let first = adapter.mutate_batch(input.clone()).await.unwrap();
    assert!(first.all_ready());
    let before = current(&f);
    let bytes = files(f.path());
    for mode in 0..5 {
        let mut changed = input.clone();
        match mode {
            0 => changed.default_namespace = "different-context".into(),
            1 => changed.items[0]["embedding"] = json!([2., 0., 0., 0.]),
            2 => changed.items[0]["embedding"] = json!([1., -0.0, 0., 0.]),
            3 => changed.expected_generation = None,
            4 => {
                changed.mutation_version = 1;
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            adapter.run_batch_typed(changed).await,
            Err(StoreWriteError::Canonical(
                crate::storage::StorageError::IdempotencyConflict
            ))
        ));
        assert_eq!(current(&f), before);
        assert_eq!(files(f.path()), bytes);
    }
    let id =
        crate::storage::insertion::inserted_id(first.items[0].receipt.as_ref().unwrap()).unwrap();
    let mut forget = request(&f, "forget2", 4, vec![json!(id.to_string())]);
    forget.kind = BatchKind::Forget;
    forget.expected_generation = Some(before.committed_generation);
    let reply = crate::daemon::server::native_batch_test_rpc(Arc::new(adapter), &forget).await;
    assert!(reply.error.is_none(), "{:?}", reply.error);
    let outcome: crate::storage::batch::BatchOutcome =
        serde_json::from_value(reply.result.unwrap()).unwrap();
    assert!(outcome.all_ready());
    assert_eq!(
        current(&f).committed_generation.get(),
        before.committed_generation.get() + 2
    );
    assert_eq!(
        f.record(id).unwrap().phase,
        crate::model::DecayPhase::Tombstone
    );
    let receipt = outcome.items[0].receipt.clone().unwrap();
    let root = f.close();
    let engine = RedbStorageEngine::open(root.path()).unwrap();
    assert_eq!(
        engine
            .meta_store()
            .lookup_operation(
                receipt.store_id,
                MutationScope::Namespace(NamespaceId::new(1)),
                receipt.operation_id
            )
            .unwrap(),
        Some(receipt)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_offline_transfer_and_fork_preserve_old_and_new_history() {
    use crate::backup::restore::{RestoreOptions, restore_from_backup};
    use crate::storage::metadata::restoration::capture;
    use crate::storage::{logical_export, schema_migration};
    let all = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir_in(all.path()).unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "fixtures/legacy-compound-before.zip"
    )))
    .unwrap()
    .extract(source.path())
    .unwrap();
    let engine = RedbStorageEngine::open(source.path()).unwrap();
    let f = Fixture::from_open_engine(source, engine);
    let mut adapter = f.storage_adapter();
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let mut original_requests = vec![];
    for cas in [false, true] {
        for invalid in [false, true] {
            let mut input = request(
                &f,
                &format!("four-{cas}-{invalid}"),
                4,
                vec![if invalid { json!(17) } else { item() }],
            );
            input.expected_generation = cas.then_some(current(&f).committed_generation);
            let outcome = adapter.mutate_batch(input.clone()).await.unwrap();
            assert_eq!(outcome.unresolved, 0);
            original_requests.push(input);
        }
    }
    drop(adapter);
    let source = f.close();
    let original = capture(source.path()).unwrap();
    let bytes = std::fs::read(source.path().join("meta.db")).unwrap();
    let gated = source.path().to_path_buf();
    let backup = all.path().join("complete.zip");
    crate::backup::run_backup(&Default::default(), &backup, Some(source.path()), false)
        .await
        .unwrap();
    assert_eq!(std::fs::read(source.path().join("meta.db")).unwrap(), bytes);
    // Retained-state restore requires the actual existing target/ledger, even
    // when restoring its own snapshot. Fresh destinations are historical only.
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: gated.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let fork = capture(&gated).unwrap();
    assert_eq!(fork.batch_progress, original.batch_progress);
    assert!(original.receipts.iter().all(|r| fork.receipts.contains(r)));
    // Actual serving legacy fork rejects original store-ID before settings/provider.
    let engine = RedbStorageEngine::open(&gated).unwrap();
    let meta = engine.meta_store();
    let control = engine.control_state().unwrap();
    for input in &original_requests {
        let plans: Vec<_> = crate::mcp::batches::prepare(input)
            .unwrap()
            .into_iter()
            .map(|i| i.plan)
            .collect();
        assert!(matches!(
            meta.reserve_batch_version_with_work(
                &input.context(),
                4,
                input.kind,
                &plans,
                None,
                None,
                || panic!("foreign replay consulted fresh settings")
            ),
            Err(crate::storage::StorageError::StoreLineageMismatch)
        ));
        assert_eq!(engine.control_state().unwrap(), control);
    }
    drop(engine);
    // Schema migration and actual logical transfer preserve completed format4;
    // they do not authorize serving or rebind old request identities.
    let native = all.path().join("canonical");
    let state = schema_migration::inspect_schema(&gated).unwrap();
    schema_migration::migrate_schema(
        &gated,
        &native,
        &schema_migration::MigrationOptions {
            migration_now: 200,
            expected_source: state.control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let stream = all.path().join("logical.jsonl");
    let exported = logical_export::export_store(&native, &stream).unwrap();
    let imported = all.path().join("imported");
    assert_eq!(
        logical_export::import_store(&stream, &imported).unwrap(),
        exported
    );
    let copied = capture(&imported).unwrap();
    assert_eq!(copied.batch_progress, original.batch_progress);
    assert!(
        original
            .receipts
            .iter()
            .all(|r| copied.receipts.contains(r))
    );
    let backup = all.path().join("native.zip");
    crate::backup::run_backup(&Default::default(), &backup, Some(&imported), false)
        .await
        .unwrap();
    let historical = all.path().join("historical");
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: historical.clone(),
        force: true,
        no_stop_daemon: true,
        historical: true,
    })
    .await
    .unwrap();
    let copied = capture(&historical).unwrap();
    assert_eq!(copied.batch_progress, original.batch_progress);
    assert!(
        original
            .receipts
            .iter()
            .all(|r| copied.receipts.contains(r))
    );
    assert!(RedbStorageEngine::open(&historical).is_err());
}
