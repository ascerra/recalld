//! Actual legacy API2 semantic admission and immutable BEFORE157 histories.
use super::*;
use crate::storage::batch::{BatchItemPlan, BatchOutcome, BatchStatus};
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
    let bytes = files(f.path());
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
    assert_eq!(files(f.path()), bytes);
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
    let bytes = files(f.path());
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
    assert_eq!(files(f.path()), bytes);
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
