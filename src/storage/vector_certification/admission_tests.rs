//! Actual admission controls and genuine candidate027 retained pending history.
use super::*;
use crate::mcp::bridge::StorageEngine as _;
use crate::storage::batch::{BatchInput, BatchItemState, BatchKind, BatchStatus};
use crate::storage::ids::SeededIds;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[path = "pending_tests.rs"]
mod pending;

struct CountClock(AtomicUsize);
impl crate::time::Clock for CountClock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        100_000_000
    }
}
fn input(request: &ReembedVectors, provider: &Provider) -> BatchInput {
    BatchInput {
        request_context: None,
        expected_generation: None,
        mutation_version: 2,
        result_version: 1,
        kind: BatchKind::Insert,
        items: vec![serde_json::json!({"summary":"pending before certification",
            "embedding":[2.,0.,0.,0.], "embeddingProfile":provider.profile,
            "checkDuplicates":false})],
        default_namespace: request.namespace_name.clone(),
        store_id: Some(request.source.store_id),
        idempotency_key: Some("uncertified-batch-before-remedy".into()),
        batch_operation_id: None,
    }
}
async fn adapter(
    f: &crate::test_support::Fixture,
    provider: Arc<dyn crate::embedding::EmbeddingProvider>,
    clock: Arc<CountClock>,
) -> crate::mcp::bridge_adapters::McpStorageAdapter {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
    .rebuild_all()
    .await
    .unwrap();
    let mut config = crate::config::RecalldConfig::default();
    config.graph.autolink_enabled = false;
    crate::mcp::bridge_adapters::McpStorageAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        provider,
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
        f.graph.clone(),
        Arc::new(config),
        chrono_tz::UTC,
    )
    .with_clock(clock)
}
fn raw_hashes(path: &Path) -> BTreeMap<PathBuf, [u8; 32]> {
    fn visit(root: &Path, path: &Path, hashes: &mut BTreeMap<PathBuf, [u8; 32]>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, hashes);
            } else {
                hashes.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    hash_bytes(&std::fs::read(path).unwrap()),
                );
            }
        }
    }
    let mut hashes = BTreeMap::new();
    visit(path, path, &mut hashes);
    hashes
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vector_certification_fresh_batch_rejects_without_stranding_offline_remedy() {
    let root = tempfile::tempdir().unwrap();
    let (source, mut remedy, provider) = fixture(root.path());
    crate::test_support::explicitly_adopt_inherited_policy(
        &source,
        remedy.namespace_id,
        1.,
        100_000_000,
    );
    let engine =
        RedbStorageEngine::open_with_ids(&source, true, Box::new(SeededIds::new([59; 32])))
            .unwrap();
    let before = engine.control_state().unwrap();
    let ids = engine.meta_store().id_source().checkpoint().unwrap();
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let provider = Arc::new(provider);
    let clock = Arc::new(CountClock(AtomicUsize::new(0)));
    let a = adapter(&f, provider.clone(), clock.clone()).await;
    let bytes = raw_hashes(&source);
    let original = input(&remedy, &provider);
    for generated in [false, true] {
        let mut single = fresh_input();
        if !generated {
            single.memory.embedding = Some(vec![2., 0., 0., 0.]);
            single.embedding_profile = Some(provider.profile.clone());
        }
        let error = a
            .store_native_single(
                MutationContext {
                    contract_version: 2,
                    ..fresh_context(&remedy, "fresh-single-rejection")
                },
                crate::storage::native_store::NativeStoreInput {
                    insert: single,
                    check_duplicates: false,
                },
                crate::storage::native_lifecycle::NativePolicyInputs::default(),
                None,
            )
            .await
            .err()
            .expect("uncertified single must reject before mutation");
        assert!(
            matches!(error, StorageError::VectorCertificationRequired { .. }),
            "{error:?}"
        );
        let reply = crate::serialization::native_single::Reply::failure(error);
        let error = reply.error.unwrap();
        assert_eq!(
            error.commit_state,
            crate::serialization::contract::CommitState::NotCommitted
        );
        assert_eq!(error.code, "UNAVAILABLE");
        assert!(reply.receipt.is_none() && error.operation_id.is_none());
        let mut request = original.clone();
        if generated {
            request.items[0]
                .as_object_mut()
                .unwrap()
                .remove("embedding");
            request.items[0]
                .as_object_mut()
                .unwrap()
                .remove("embeddingProfile");
        }
        for mixed in [false, true] {
            let mut request = request.clone();
            if mixed {
                request.items.push(serde_json::json!(true));
            }
            let error = a.run_batch_typed(request).await.unwrap_err();
            let crate::mcp::bridge_adapters::StoreWriteError::Canonical(error) = error else {
                panic!("wrong error {error:?}")
            };
            assert!(
                matches!(error, StorageError::VectorCertificationRequired { namespace_id } if namespace_id == remedy.namespace_id.get())
            );
            let projected = crate::serialization::native_batch::BatchFailure::from_storage(error);
            assert_eq!(
                projected.error.commit_state,
                crate::serialization::contract::CommitState::NotCommitted
            );
            assert_eq!(projected.error.code, "UNAVAILABLE");
            assert!(
                projected.receipt.is_none()
                    && projected.reservation_receipt.is_none()
                    && projected.error.operation_id.is_none()
            );
        }
    }
    assert!(matches!(
        a.mutate_batch(original.clone()).await,
        Err(crate::mcp::bridge::BridgeError::Unavailable(_))
    ));
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
    assert_eq!(
        ids,
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .id_source()
            .checkpoint()
            .unwrap()
    );
    assert_eq!(bytes, raw_hashes(&source));
    assert!(provider.texts.lock().unwrap().is_empty());
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    // All-invalid bookkeeping is nonsemantic and remains available.
    let mut invalid = original;
    invalid.items = vec![serde_json::json!(true)];
    let out = a.mutate_batch(invalid).await.unwrap();
    assert_eq!(out.failed, 1);
    assert_eq!(out.unresolved, 0);
    assert_eq!(out.items[0].status, BatchStatus::Error);
    let after = f.storage.read().unwrap().control_state().unwrap();
    assert_eq!(
        after.committed_generation,
        before.committed_generation.next().unwrap()
    );
    drop(a);
    let root = f.close();
    remedy.source = after.generation_ref();
    remedy.context.expected_generation = Some(after.committed_generation);
    let unchanged = raw_hashes(&source);
    let target = root.path().join("remedied-after-rejection");
    reembed_fresh(&source, &target, &remedy, provider.as_ref())
        .await
        .unwrap();
    assert_eq!(unchanged, raw_hashes(&source));
    assert!(!provider.texts.lock().unwrap().is_empty());
    OfflineSnapshot::open(&target).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vector_certification_genuine_pending_keeps_parent_child_then_real_remedy_resumes_once() {
    let root = tempfile::tempdir().unwrap();
    let (_, _, provider) = fixture(root.path());
    let extracted = root.path().join("genuine-pending");
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "fixtures/pending-before-certification.zip"
    )))
    .unwrap()
    .extract(&extracted)
    .unwrap();
    let remedy: ReembedVectors =
        serde_json::from_slice(&std::fs::read(extracted.join("remedy.json")).unwrap()).unwrap();
    let pending: crate::storage::batch::BatchProgress =
        serde_json::from_slice(&std::fs::read(extracted.join("pending.json")).unwrap()).unwrap();
    let source = extracted.join("store");
    let engine = RedbStorageEngine::open_native(&source).unwrap();
    let before = engine.control_state().unwrap();
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let provider = Arc::new(provider);
    let clock = Arc::new(CountClock(AtomicUsize::new(0)));
    let a = adapter(&f, provider.clone(), clock.clone()).await;
    let out = a.mutate_batch(input(&remedy, &provider)).await.unwrap();
    let mut receipt = pending.reservation_receipt.clone();
    receipt.replayed = true;
    assert_eq!(out.reservation_receipt, Some(receipt));
    assert_eq!(
        out.items[0].operation_id,
        Some(pending.items[0].operation_id)
    );
    assert_eq!(out.items[0].status, BatchStatus::NotAttempted);
    assert_eq!(out.items[0].error.as_ref().unwrap().code, "UNAVAILABLE");
    assert!(!out.items[0].retryable && out.items[0].receipt.is_none());
    assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
    let retained = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .lookup_batch(before.store_id, pending.reservation_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(retained, pending);
    // In-hand batch reservation evidence takes precedence over this typed cause.
    let reply = crate::serialization::native_batch::BatchFailure::from_storage(
        StorageError::CommittedMutation {
            receipt: Box::new(pending.reservation_receipt.clone()),
            source: Box::new(StorageError::VectorCertificationRequired {
                namespace_id: remedy.namespace_id.get(),
            }),
        },
    );
    assert_eq!(
        reply.reservation_receipt,
        Some(pending.reservation_receipt.clone())
    );
    assert!(reply.receipt.is_none());
    assert_eq!(
        reply.error.commit_state,
        crate::serialization::contract::CommitState::Committed
    );
    assert_eq!(
        reply.error.operation_id,
        Some(pending.reservation_receipt.operation_id)
    );
    assert!(reply.readiness.is_none());
    assert_eq!(reply.error.code, "INDEX_NOT_READY");
    assert!(
        reply
            .warnings
            .iter()
            .any(|warning| warning == "READINESS_UNKNOWN")
    );
    assert!(matches!(
        retained.items[0].state,
        BatchItemState::NotAttempted {}
    ));
    assert!(provider.texts.lock().unwrap().is_empty());
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    drop(a);
    let root = f.close();
    let unchanged = raw_hashes(&source);
    assert!(
        OfflineSnapshot::open(&source)
            .err()
            .expect("ordinary source inspection rejects pending")
            .to_string()
            .contains("unfinished batch")
    );
    let target = root.path().join("remedied-pending");
    let certificate = reembed_fresh(&source, &target, &remedy, provider.as_ref())
        .await
        .unwrap();
    assert_eq!(unchanged, raw_hashes(&source));
    assert!(
        OfflineSnapshot::open(&target)
            .err()
            .expect("ordinary target inspection rejects pending")
            .to_string()
            .contains("unfinished batch")
    );
    let generated = provider.texts.lock().unwrap().len();
    let replay = reembed_fresh(&source, &target, &remedy, provider.as_ref())
        .await
        .unwrap();
    let mut expected = certificate.clone();
    expected.replayed = true;
    assert_eq!(replay, expected);
    assert_eq!(provider.texts.lock().unwrap().len(), generated);
    let engine = RedbStorageEngine::open_native(&target).unwrap();
    assert_eq!(
        engine
            .meta_store()
            .lookup_batch(before.store_id, pending.reservation_receipt.operation_id)
            .unwrap()
            .unwrap(),
        pending
    );
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let a = adapter(&f, provider.clone(), clock.clone()).await;
    let request = input(&remedy, &provider);
    control::take_validation_work();
    let outcome = a.mutate_batch(request.clone()).await.unwrap();
    assert!(outcome.all_ready(), "{outcome:?}");
    let child = outcome.items[0].receipt.as_ref().unwrap();
    assert_eq!(child.operation_id, pending.items[0].operation_id);
    assert_eq!(
        child.committed_generation,
        certificate.committed_generation.next().unwrap()
    );
    let work = control::take_validation_work();
    assert_eq!(work.certification_scans, 0, "{work:?}");
    assert_eq!(work.full_validations, 0, "{work:?}");
    let committed = f.storage.read().unwrap().control_state().unwrap();
    let sampled = clock.0.load(Ordering::SeqCst);
    let replay = a.mutate_batch(request).await.unwrap();
    assert!(replay.all_ready());
    assert_eq!(replay.items[0].operation_id, Some(child.operation_id));
    assert_eq!(
        f.storage.read().unwrap().control_state().unwrap(),
        committed
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), sampled);
    assert_eq!(provider.texts.lock().unwrap().len(), generated);
    drop(a);
    let root = f.close();
    OfflineSnapshot::open(&target).unwrap();
    let exported = root.path().join("completed-pending.jsonl");
    logical_export::export_store(&target, &exported).unwrap();
    let imported = root.path().join("completed-import");
    logical_export::import_store(&exported, &imported).unwrap();
    let engine = RedbStorageEngine::open_native(&imported).unwrap();
    assert_eq!(engine.control_state().unwrap(), committed);
}
