//! Actual legacy API2 semantic admission and immutable BEFORE157 histories.
use super::*;
use crate::storage::batch::{BatchOutcome, BatchStatus};
use std::sync::atomic::{AtomicUsize, Ordering};

fn canonical() -> InsertMemory {
    InsertMemory {
        id: None,
        summary: "legacy semantic boundary".into(),
        full_text: Some("synthetic retained payload".into()),
        tags: vec![],
        initial_stability: None,
        created_at: None,
        embedding: Some(vec![1., 0., 0., 0.]),
        parent_id: None,
        supersedes: None,
    }
}
fn context(f: &Fixture) -> MutationContext {
    MutationContext {
        contract_version: 2,
        store_id: Some(current(f).store_id),
        scope: MutationScope::Namespace(NamespaceId::new(1)),
        idempotency_key: Some("legacy-semantic-single".into()),
        expected_generation: Some(current(f).committed_generation),
    }
}

#[test]
fn legacy_semantic_core_fresh_rejects_before_payload() {
    let f = Fixture::new();
    let ctx = context(&f);
    let before = current(&f);
    let bytes = file_hashes(f.path());
    let result =
        f.storage
            .write()
            .unwrap()
            .insert_memory_with_receipt(&ctx, &canonical(), &[], 100);
    assert!(
        matches!(
            result,
            Err(crate::storage::StorageError::UnsupportedMutation(_))
        ),
        "fresh unknown-profile API2 vector store accepted: {result:?}; before={before:?}, after={:?}",
        current(&f)
    );
    assert_eq!(current(&f), before);
    assert_eq!(file_hashes(f.path()), bytes);
}

async fn fresh_rpc(generated: bool) {
    let f = Fixture::new();
    let provider = Arc::new(SpyProvider(AtomicUsize::new(0)));
    let clock = Arc::new(SpyClock(AtomicUsize::new(0)));
    let mut adapter = f.storage_adapter().with_clock(clock.clone());
    adapter.embedding = provider.clone();
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let mut value = item();
    if generated {
        value.as_object_mut().unwrap().remove("embedding");
    }
    let input = request(&f, "fresh-semantic-rpc", 4, vec![value]);
    let before = current(&f);
    let bytes = file_hashes(f.path());
    let reply = crate::daemon::server::native_batch_test_rpc(Arc::new(adapter), &input).await;
    assert!(
        reply.error.is_some(),
        "fresh semantic RPC succeeded: {:?}; before={before:?}, after={:?}, provider={}",
        reply.result,
        current(&f),
        provider.0.load(Ordering::SeqCst)
    );
    assert!(reply.result.is_none());
    assert_eq!(current(&f), before);
    assert_eq!(file_hashes(f.path()), bytes);
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_semantic_rpc_supplied_fresh_rejects_before_reservation() {
    fresh_rpc(false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_semantic_rpc_generated_fresh_rejects_before_provider() {
    fresh_rpc(true).await;
}

struct UnavailableProvider(AtomicUsize);
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for UnavailableProvider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(crate::embedding::EmbeddingError::Unavailable(
            "synthetic pending provider".into(),
        ))
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "synthetic-unavailable"
    }
}

/// Closed synthetic fixture snapshot, not a production backup/restore shortcut.
/// Pending work is deliberately retained; ordinary offline tools may reject it.
fn snapshot(root: &std::path::Path, destination: &std::path::Path) {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::fs::File::create(destination).unwrap());
    for (name, bytes) in files(root) {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "genuine157 exporter; execute only on recorded pre-admission source"]
async fn legacy_semantic_export_genuine157() {
    let destination =
        std::path::PathBuf::from(std::env::var("RECALLD_LEGACY_SEMANTIC_BEFORE").unwrap());
    std::fs::create_dir_all(&destination).unwrap();
    let f = Fixture::new();
    let ctx = context(&f);
    let single = f
        .storage_adapter()
        .batch_completion()
        .insert(ctx.clone(), canonical(), vec![].into(), 100)
        .await
        .unwrap()
        .receipt;
    let mut adapter = f.storage_adapter();
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let mut completed = vec![];
    for cas in [false, true] {
        for invalid in [false, true] {
            let mut input = request(
                &f,
                &format!("genuine157-{cas}-{invalid}"),
                4,
                vec![if invalid { json!(17) } else { item() }],
            );
            input.expected_generation = cas.then_some(current(&f).committed_generation);
            let outcome = adapter.mutate_batch(input.clone()).await.unwrap();
            assert_eq!(outcome.unresolved, 0);
            completed.push(json!({"input":input,"outcome":outcome}));
        }
    }
    drop(adapter);
    let root = f.close();
    crate::backup::run_backup(
        &Default::default(),
        &destination.join("completed.zip"),
        Some(root.path()),
        false,
    )
    .await
    .unwrap();
    let complete_rows = historical_rows(root.path());
    let engine = RedbStorageEngine::open(root.path()).unwrap();
    let f = Fixture::from_open_engine(root, engine);
    let provider = Arc::new(UnavailableProvider(AtomicUsize::new(0)));
    let mut adapter = f.storage_adapter();
    adapter.embedding = provider.clone();
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let mut generated = item();
    generated.as_object_mut().unwrap().remove("embedding");
    let mut pending = vec![];
    for partial in [false, true] {
        let mut input = request(
            &f,
            &format!("pending-{partial}"),
            4,
            if partial {
                vec![item(), generated.clone()]
            } else {
                vec![generated.clone()]
            },
        );
        if !partial {
            input.idempotency_key = None;
        }
        let outcome = adapter.mutate_batch(input.clone()).await.unwrap();
        assert_eq!(outcome.unresolved, 1);
        assert_eq!(outcome.succeeded, usize::from(partial));
        assert_eq!(
            outcome.items.last().unwrap().status,
            BatchStatus::NotAttempted
        );
        pending.push(json!({"input":input,"outcome":outcome}));
    }
    assert_eq!(provider.0.load(Ordering::SeqCst), 2);
    let pending_control = current(&f);
    drop(adapter);
    let root = f.close();
    snapshot(root.path(), &destination.join("pending.zip"));
    let pending_rows = historical_rows(root.path());
    std::fs::write(destination.join("history.json"), serde_json::to_vec_pretty(&json!({
        "single":{"input":canonical(),"receipt":single,"expectedGeneration":ctx.expected_generation},
        "completed":completed,"completeRows":complete_rows,
        "pending":pending,"pendingRows":pending_rows,"pendingControl":pending_control,
    })).unwrap()).unwrap();
}

// Capture actual historical fault boundaries while the production writer is157.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "genuine157 fault exporter; pre-admission source only"]
async fn legacy_semantic_export_genuine157_faults() {
    let dest = std::path::PathBuf::from(std::env::var("RECALLD_LEGACY_SEMANTIC_BEFORE").unwrap());
    std::fs::create_dir_all(&dest).unwrap();
    let mut cases = vec![];
    for unknown in [false, true] {
        let f = Fixture::new();
        let mut adapter = f.storage_adapter();
        Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        adapter.embedding = Arc::new(BlockedProvider {
            entered: entered.clone(),
            release: release.clone(),
            calls: calls.clone(),
        });
        let adapter = Arc::new(adapter);
        let mut value = item();
        value.as_object_mut().unwrap().remove("embedding");
        let input = request(&f, "historical-fault", 4, vec![value]);
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
        if unknown {
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .fail_batch_commit
                .store(3, Ordering::SeqCst);
        } else {
            *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
        }
        release.notify_one();
        let reply = task.await.unwrap();
        assert!(reply.error.is_none(), "{:?}", reply.error);
        let outcome: BatchOutcome = serde_json::from_value(reply.result.unwrap()).unwrap();
        assert_eq!(
            outcome.items[0].status,
            if unknown {
                BatchStatus::Unknown
            } else {
                BatchStatus::Ok
            }
        );
        assert!(!outcome.items[0].ready);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        drop(adapter);
        let root = f.close();
        let name = format!("fault-{unknown}.zip");
        snapshot(root.path(), &dest.join(&name));
        // Resolve actual persisted receipt with the OLD recovery implementation;
        // the later test compares this exact receipt, never just any success.
        let engine = RedbStorageEngine::open(root.path()).unwrap();
        let receipt = engine
            .lookup_operation(
                outcome.store_id,
                MutationScope::Namespace(NamespaceId::new(1)),
                outcome.items[0].operation_id.unwrap(),
            )
            .unwrap()
            .unwrap();
        cases.push(json!({"archive":name,"input":input,"outcome":outcome,"receipt":receipt}));
    }
    std::fs::write(
        dest.join("faults.json"),
        serde_json::to_vec_pretty(&cases).unwrap(),
    )
    .unwrap();
    let mut cases = vec![];
    for point in [
        "before_batch_commit",
        "after_batch_commit",
        "after_insert_payload_sync",
        "before_metadata_commit",
        "after_metadata_commit",
        "after_edge_commit",
        "after_canonical_ack",
    ] {
        let f = Fixture::new();
        let before = current(&f);
        let root = f.close();
        let result=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","storage::metadata::batch::legacy_origin_tests::legacy_compound_origin_process_child","--nocapture"])
            .env("RECALLD_LEGACY_ORIGIN_STORE",root.path()).env("RECALLD_LEGACY_ORIGIN_CAS",before.committed_generation.get().to_string())
            .env("RECALLD_TEST_EFFECT_CRASH",point).output().unwrap();
        assert_eq!(
            result.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let name = format!("process-{point}.zip");
        snapshot(root.path(), &dest.join(&name));
        cases.push(
            json!({"archive":name,"point":point,"before":before,"exit":result.status.code()}),
        );
    }
    std::fs::write(
        dest.join("process.json"),
        serde_json::to_vec_pretty(&cases).unwrap(),
    )
    .unwrap();
}

fn file_hashes(root: &std::path::Path) -> Vec<(String, String)> {
    use sha2::{Digest, Sha256};
    files(root)
        .into_iter()
        .map(|(path, bytes)| (path, format!("{:02x?}", Sha256::digest(bytes).as_slice())))
        .collect()
}
// Restored owners may rebuild disposable FTS/WAL on first completion. Keep
// every canonical DB, vector/text payload and retained history byte invariant.
fn canonical_hashes(root: &std::path::Path) -> Vec<(String, String)> {
    file_hashes(root)
        .into_iter()
        .filter(|(path, _)| !path.starts_with("fts.db"))
        .collect()
}
pub(super) fn history() -> Value {
    serde_json::from_slice(include_bytes!(
        "fixtures/legacy-semantic-before/history.json"
    ))
    .unwrap()
}
pub(super) fn restored(bytes: &[u8]) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .unwrap()
        .extract(root.path())
        .unwrap();
    let engine = RedbStorageEngine::open(root.path()).unwrap();
    Fixture::from_open_engine(root, engine)
}
pub(super) fn completed() -> (Fixture, Value) {
    (
        restored(include_bytes!(
            "fixtures/legacy-semantic-before/completed.zip"
        )),
        history(),
    )
}
pub(super) fn completed_case(h: &Value, index: usize) -> (BatchInput, BatchOutcome) {
    let case = &h["completed"][index];
    (
        serde_json::from_value(case["input"].clone()).unwrap(),
        serde_json::from_value(case["outcome"].clone()).unwrap(),
    )
}
fn same_receipt(
    actual: &crate::storage::control::WriteReceipt,
    original: &crate::storage::control::WriteReceipt,
) {
    let mut actual = actual.clone();
    actual.replayed = original.replayed;
    assert_eq!(&actual, original);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_semantic_genuine_single_replay_precedes_profile_clock_and_cas() {
    let (f, h) = completed();
    let original: crate::storage::control::WriteReceipt =
        serde_json::from_value(h["single"]["receipt"].clone()).unwrap();
    let mut ctx = context(&f);
    ctx.store_id = Some(original.store_id);
    ctx.expected_generation =
        serde_json::from_value(h["single"]["expectedGeneration"].clone()).unwrap();
    assert_eq!(
        serde_json::to_value(canonical()).unwrap(),
        h["single"]["input"]
    );
    let before = current(&f);
    let bytes = canonical_hashes(f.path());
    let owner = f.storage_adapter().batch_completion();
    let replay = owner
        .insert(ctx.clone(), canonical(), vec![f32::NAN].into(), i64::MAX)
        .await
        .unwrap()
        .receipt;
    assert!(replay.replayed);
    same_receipt(&replay, &original);
    for mode in 0..4 {
        let mut other = ctx.clone();
        let mut input = canonical();
        match mode {
            0 => other.contract_version = 1,
            1 => input.embedding = Some(vec![2., 0., 0., 0.]),
            2 => input.embedding = Some(vec![1., -0., 0., 0.]),
            3 => other.expected_generation = None,
            _ => unreachable!(),
        }
        assert!(matches!(
            owner.insert(other, input, vec![].into(), 100).await,
            Err(crate::storage::StorageError::IdempotencyConflict)
        ));
    }
    let mut foreign = ctx;
    foreign.store_id = Some(uuid::Uuid::new_v4());
    assert!(matches!(
        owner.insert(foreign, canonical(), vec![].into(), 100).await,
        Err(crate::storage::StorageError::StoreLineageMismatch)
    ));
    assert_eq!(current(&f), before);
    assert_eq!(canonical_hashes(f.path()), bytes);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_semantic_genuine_completed_rpc_replays_all_receipts_no_work() {
    let (f, h) = completed();
    let before = current(&f);
    let bytes = canonical_hashes(f.path());
    let spy = Arc::new(SpyProvider(Default::default()));
    let clock = Arc::new(SpyClock(Default::default()));
    let mut adapter = f.storage_adapter().with_clock(clock.clone());
    adapter.embedding = spy.clone();
    assert!(adapter.config.graph.autolink_enabled);
    let adapter = Arc::new(adapter);
    for index in 0..4 {
        let (input, original) = completed_case(&h, index);
        let reply = crate::daemon::server::native_batch_test_rpc(adapter.clone(), &input).await;
        assert!(reply.error.is_none(), "{:?}", reply.error);
        let replay: BatchOutcome = serde_json::from_value(reply.result.unwrap()).unwrap();
        assert_eq!(replay.batch_operation_id, original.batch_operation_id);
        same_receipt(
            replay.reservation_receipt.as_ref().unwrap(),
            original.reservation_receipt.as_ref().unwrap(),
        );
        assert_eq!(replay.bookkeeping_receipts, original.bookkeeping_receipts);
        assert_eq!(replay.unresolved, 0);
        for (a, b) in replay.items.iter().zip(&original.items) {
            assert_eq!(a.operation_id, b.operation_id);
            assert_eq!(a.status, b.status);
            match (&a.receipt, &b.receipt) {
                (Some(a), Some(b)) => same_receipt(a, b),
                (None, None) => {}
                _ => panic!("receipt lost"),
            }
        }
    }
    assert_eq!(current(&f), before);
    assert_eq!(canonical_hashes(f.path()), bytes);
    assert_eq!(spy.0.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    drop(adapter);
    let root = f.close();
    assert_eq!(historical_rows(root.path()), h["completeRows"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_semantic_genuine_pending_rpc_keeps_parent_child_and_no_fake_completion() {
    let f = restored(include_bytes!(
        "fixtures/legacy-semantic-before/pending.zip"
    ));
    let h = history();
    let before = current(&f);
    let bytes = canonical_hashes(f.path());
    let spy = Arc::new(SpyProvider(Default::default()));
    let clock = Arc::new(SpyClock(Default::default()));
    let mut adapter = f.storage_adapter().with_clock(clock.clone());
    adapter.embedding = spy.clone();
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let adapter = Arc::new(adapter);
    for case in h["pending"].as_array().unwrap() {
        let original: BatchOutcome = serde_json::from_value(case["outcome"].clone()).unwrap();
        let mut input: BatchInput = serde_json::from_value(case["input"].clone()).unwrap();
        if input.idempotency_key.is_none() {
            input.batch_operation_id = Some(original.batch_operation_id);
        }
        let reply = crate::daemon::server::native_batch_test_rpc(adapter.clone(), &input).await;
        assert!(reply.error.is_none(), "{:?}", reply.error);
        let result: BatchOutcome = serde_json::from_value(reply.result.unwrap()).unwrap();
        assert_eq!(result.batch_operation_id, original.batch_operation_id);
        same_receipt(
            result.reservation_receipt.as_ref().unwrap(),
            original.reservation_receipt.as_ref().unwrap(),
        );
        assert_eq!(result.bookkeeping_receipts, original.bookkeeping_receipts);
        assert_eq!(result.unresolved, 1);
        assert_eq!(result.succeeded, original.succeeded);
        for (a, b) in result.items.iter().zip(&original.items) {
            assert_eq!(a.operation_id, b.operation_id);
            assert_eq!(a.status, b.status);
            if let Some(original) = &b.receipt {
                same_receipt(a.receipt.as_ref().unwrap(), original);
            } else {
                assert!(a.receipt.is_none());
                assert!(a.result.is_none());
                assert!(!a.ready);
                assert_eq!(a.error.as_ref().unwrap().code, "UNAVAILABLE");
            }
        }
        // Resubmission cannot reinterpret an accepted request or foreign lineage.
        for foreign in [false, true] {
            let mut other = input.clone();
            if foreign {
                other.store_id = Some(uuid::Uuid::new_v4());
            } else {
                other.items[0]["summary"] = json!("changed");
            }
            let reply = crate::daemon::server::native_batch_test_rpc(adapter.clone(), &other).await;
            assert!(reply.error.is_some());
            assert!(reply.result.is_none());
        }
    }
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        2
    );
    assert_eq!(spy.0.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(current(&f), before);
    assert_eq!(canonical_hashes(f.path()), bytes);
    drop(adapter);
    let root = f.close();
    assert_eq!(historical_rows(root.path()), h["pendingRows"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_semantic_historical_fault_rpc_keeps_exact_current_child() {
    let cases: Value = serde_json::from_slice(include_bytes!(
        "fixtures/legacy-semantic-before/faults.json"
    ))
    .unwrap();
    for (case, zip) in cases.as_array().unwrap().iter().zip([
        include_bytes!("fixtures/legacy-semantic-before/fault-false.zip").as_slice(),
        include_bytes!("fixtures/legacy-semantic-before/fault-true.zip").as_slice(),
    ]) {
        let f = restored(zip);
        let before = current(&f);
        let input: BatchInput = serde_json::from_value(case["input"].clone()).unwrap();
        let original: crate::storage::control::WriteReceipt =
            serde_json::from_value(case["receipt"].clone()).unwrap();
        let failed: BatchOutcome = serde_json::from_value(case["outcome"].clone()).unwrap();
        assert_eq!(failed.items[0].operation_id, Some(original.operation_id));
        assert_ne!(failed.batch_operation_id, original.operation_id);
        let spy = Arc::new(SpyProvider(Default::default()));
        let clock = Arc::new(SpyClock(Default::default()));
        let mut adapter = f.storage_adapter().with_clock(clock.clone());
        adapter.embedding = spy.clone();
        let reply = crate::daemon::server::native_batch_test_rpc(Arc::new(adapter), &input).await;
        assert!(reply.error.is_none(), "{:?}", reply.error);
        let replay: BatchOutcome = serde_json::from_value(reply.result.unwrap()).unwrap();
        assert!(replay.all_ready());
        same_receipt(replay.items[0].receipt.as_ref().unwrap(), &original);
        same_receipt(
            replay.reservation_receipt.as_ref().unwrap(),
            failed.reservation_receipt.as_ref().unwrap(),
        );
        assert_eq!(current(&f), before);
        assert_eq!(spy.0.load(Ordering::SeqCst), 0);
        assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn legacy_semantic_core_generated_and_reserved_bypass_are_final_guarded() {
    let f = Fixture::new();
    let mut input = canonical();
    input.embedding = None;
    let before = current(&f);
    let bytes = file_hashes(f.path());
    let ctx = context(&f);
    let result =
        f.storage
            .write()
            .unwrap()
            .insert_memory_with_receipt(&ctx, &input, &[1., 0., 0., 0.], 100);
    assert!(matches!(
        result,
        Err(crate::storage::StorageError::UnsupportedMutation(_))
    ));
    assert_eq!(current(&f), before);
    assert_eq!(file_hashes(f.path()), bytes);
    // A real retained reservation is not an authority to create fresh vectors.
    let f = restored(include_bytes!(
        "fixtures/legacy-semantic-before/pending.zip"
    ));
    let h = history();
    let case = &h["pending"][0];
    let mut request: BatchInput = serde_json::from_value(case["input"].clone()).unwrap();
    let original: BatchOutcome = serde_json::from_value(case["outcome"].clone()).unwrap();
    request.batch_operation_id = Some(original.batch_operation_id);
    let plans: Vec<_> = crate::mcp::batches::prepare(&request)
        .unwrap()
        .into_iter()
        .map(|i| i.plan)
        .collect();
    let before = current(&f);
    let bytes = canonical_hashes(f.path());
    let p = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .reserve_batch_version_with_work(
            &request.context(),
            4,
            request.kind,
            &plans,
            request.batch_operation_id,
            None,
            || panic!("retained work consulted fresh settings"),
        )
        .unwrap();
    let child = p.child(0).unwrap();
    let crate::storage::batch::BatchItemPlan::Insert { input, .. } = &plans[0] else {
        panic!()
    };
    let result = f.storage.write().unwrap().insert_memory_reserved(
        &child.context(),
        input,
        &[1., 0., 0., 0.],
        100,
        Some(&child),
    );
    assert!(matches!(
        result,
        Err(crate::storage::StorageError::UnsupportedMutation(_))
    ));
    assert_eq!(current(&f), before);
    assert_eq!(canonical_hashes(f.path()), bytes);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_batch_items()
            .unwrap(),
        2
    );
}

// The actual prepared API2 consumers already reject the legacy owner; they must
// not fall back to the API1 producer. Public activation remains separately gated.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_semantic_prepared_mcp_http_single_never_fall_back_to_legacy() {
    let f = Fixture::new();
    let before = current(&f);
    let bytes = file_hashes(f.path());
    let spy = Arc::new(SpyProvider(Default::default()));
    let clock = Arc::new(SpyClock(Default::default()));
    let mut adapter = f.storage_adapter().with_clock(clock.clone());
    adapter.embedding = spy.clone();
    Arc::make_mut(&mut adapter.config).graph.autolink_enabled = false;
    let adapter = Arc::new(adapter);
    let body = json!({"summary":"unknown legacy profile","namespace":"default","storeId":before.store_id,"checkDuplicates":false});
    let response = crate::api::single_store_v2(adapter.as_ref(), "default", &body, None).await;
    assert_ne!(response.status().as_u16(), 200);
    let value: Value = serde_json::from_slice(
        &axum::body::to_bytes(
            response.into_body(),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await
        .unwrap(),
    )
    .unwrap();
    // Existing prepared-native wrong-owner diagnostic is conservative Unknown.
    // This control proves no legacy fallback, not corrected commit classification.
    assert_eq!(value["error"]["category"], "unavailable", "{value}");
    assert_eq!(value["error"]["code"], "COMMIT_OUTCOME_UNKNOWN");
    assert_eq!(value["error"]["commitState"], "unknown");
    assert!(value["error"].get("operationId").is_none());
    assert!(value.get("receipt").is_none());
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = adapter;
    let mut body = body;
    body["contractVersion"] = json!(2);
    let result = crate::mcp::tools::dispatch_v2(&bridge, "store_memory", body).await;
    assert_eq!(result.is_error, Some(true));
    let packet = serde_json::to_value(result).unwrap();
    let text: Value = serde_json::from_str(packet["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(packet["structuredContent"], text);
    assert_eq!(text["error"]["code"], "COMMIT_OUTCOME_UNKNOWN", "{text}");
    assert_eq!(text["error"]["commitState"], "unknown");
    assert!(text["error"].get("operationId").is_none());
    assert!(text.get("receipt").is_none());
    assert_eq!(spy.0.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(current(&f), before);
    assert_eq!(file_hashes(f.path()), bytes);
}
