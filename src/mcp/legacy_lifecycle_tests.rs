//! Real legacy-disk consumers; application origin is independent of physical format.
use super::bridge::StorageEngine as _;
use crate::model::{AccessKind, MemoryId, NamespaceId};
use crate::serialization::native_lifecycle::{Operation, parse};
use crate::storage::StorageEngine as _;
use crate::storage::control::{MutationContext, MutationScope};
use crate::test_support::Fixture;
use serde_json::{Value, json};
use std::sync::Arc;

fn owner(f: &Fixture) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}
fn context(f: &Fixture, key: &str, origin: u32) -> MutationContext {
    let state = f.storage.read().unwrap().control_state().unwrap();
    MutationContext {
        store_id: Some(state.store_id),
        scope: MutationScope::Namespace(NamespaceId::new(1)),
        idempotency_key: Some(key.into()),
        expected_generation: Some(state.committed_generation),
        contract_version: origin,
    }
}
fn request(f: &Fixture, id: MemoryId, key: &str) -> Value {
    let ctx = context(f, key, 2);
    json!({"contractVersion":2,"id":id,"storeId":ctx.store_id,"expectedGeneration":ctx.expected_generation,"idempotencyKey":key})
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_prepared_feedback_reaches_actual_legacy_writer() {
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let adapter = f
        .storage_adapter()
        .with_native_lifecycle_policy(Default::default());
    let value = request(&f, id, "legacy-feedback2");
    let reply = adapter
        .feedback_native(parse(&value, Operation::Feedback, true, None).unwrap())
        .await;
    assert!(
        reply.error.is_none(),
        "actual prepared legacy2 feedback failed: {:?}",
        reply.error
    );
    assert!(reply.receipt.is_some());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_prepared_forget_reaches_actual_legacy_writer() {
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let adapter = f
        .storage_adapter()
        .with_native_lifecycle_policy(Default::default());
    let value = request(&f, id, "legacy-forget2");
    let reply = adapter
        .forget_native(parse(&value, Operation::Forget, true, None).unwrap())
        .await;
    assert!(
        reply.error.is_none(),
        "actual prepared legacy2 forget failed: {:?}",
        reply.error
    );
    assert_eq!(
        f.record(id).unwrap().phase,
        crate::model::DecayPhase::Tombstone
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_tombstone_metadata_admits_original_two() {
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let ctx = context(&f, "legacy2-metadata", 2);
    f.storage
        .write()
        .unwrap()
        .tombstone_memory_with_receipt(&ctx, id)
        .unwrap();
}

// Run only on the frozen pre-correction source. These are real API1 writer bytes,
// never fabricated receipt rows. The artifact is retained as a tracked fixture.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "fixture generator: only execute on the recorded pre-correction source"]
async fn legacy_api2_generate_genuine_before_one_fixture() {
    let Ok(destination) = std::env::var("RECALLD_LEGACY_BEFORE_FIXTURE") else {
        return;
    };
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let exposure_ctx = context(&f, "old1-exposure", 1);
    let exposure = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .record_exposure_batch(&exposure_ctx, &[(id, AccessKind::DirectRetrieval)], 100)
        .unwrap()
        .unwrap();
    let feedback_ctx = context(&f, "old1-feedback", 1);
    let feedback = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .record_legacy_feedback(&feedback_ctx, id, 3, 200, 1.0)
        .unwrap();
    let forget_ctx = context(&f, "old1-forget", 1);
    let forget = f
        .storage
        .write()
        .unwrap()
        .tombstone_memory_with_receipt(&forget_ctx, id)
        .unwrap();
    owner(&f).complete().await.unwrap();
    let root = f.close();
    let expected = json!({"memoryId":id,"exposureExpectedGeneration":exposure_ctx.expected_generation,"exposure":exposure,
        "feedbackExpectedGeneration":feedback_ctx.expected_generation,"feedback":feedback,"forgetExpectedGeneration":forget_ctx.expected_generation,"forget":forget});
    let destination = std::path::Path::new(&destination);
    std::fs::write(
        destination.with_extension("json"),
        serde_json::to_vec_pretty(&expected).unwrap(),
    )
    .unwrap();
    crate::backup::run_backup(&Default::default(), destination, Some(root.path()), false)
        .await
        .unwrap();
}

fn state(
    f: &Fixture,
    clock: Arc<dyn crate::time::Clock>,
) -> (crate::api::AppState, Arc<crate::search::QueryEngine>) {
    use crate::api::adapters::*;
    use crate::search::adapters::*;
    let provider = Arc::new(crate::embedding::PassthroughProvider::new(
        crate::test_support::DIM,
    ));
    let config = Arc::new(crate::config::RecalldConfig::default());
    let query = Arc::new(crate::search::QueryEngine::new(
        Arc::new(StorageNamespaceResolver::new(f.storage.clone())),
        Arc::new(SingleEmbeddingRegistry::new(provider.clone())),
        Arc::new(SharedVectorIndexRegistry::new(f.vector_index.clone())),
        Arc::new(SharedFtsIndexRegistry::new(f.fts_index.clone())),
        Arc::new(SharedEntityIndexReader::new(f.entity_index.clone())),
        Arc::new(CacheManagerAdapter::new(f.cache.clone())),
        Arc::new(StorageMetadataAdapter::new(f.storage.clone())),
        Arc::new(SharedGraphReader::new(f.graph.clone())),
        Arc::new(RifProcessorAdapter::new(
            Arc::new(crate::rif::RifEngine::new(Default::default())),
            f.graph.clone(),
        )),
        Arc::new(
            StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
                .with_clock(clock.clone())
                .with_completion(owner(f)),
        ),
    ));
    let state = crate::api::AppState::new(
        Arc::new(SearchPipelineAdapter::new(
            query.clone(),
            provider,
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
        )),
        Arc::new(
            StorageEngineAdapter::new(
                f.storage.clone(),
                f.cache.clone(),
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
                f.graph.clone(),
            )
            .with_clock(clock.clone())
            .with_native_lifecycle_policy(Default::default()),
        ),
        Arc::new(RecordCacheAdapter::new(f.cache.clone())),
        Arc::new(RelationshipGraphAdapter::new(
            f.graph.clone(),
            f.vector_index.clone(),
            f.entity_index.clone(),
            f.storage.clone(),
            f.cache.clone(),
            config.clone(),
        )),
        Arc::new(
            FsrsEngineAdapter::new(
                f.storage.clone(),
                f.cache.clone(),
                f.graph.clone(),
                config,
                false,
            )
            .with_indexes(
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
            )
            .with_clock(clock)
            .with_native_lifecycle_policy(Default::default()),
        ),
        Arc::new(NamespaceRegistryAdapter::new(f.storage.clone())),
        Arc::new(NoopMetricsCollector),
    );
    (state, query)
}

#[derive(Default)]
struct CountingClock(std::sync::atomic::AtomicUsize);
impl crate::time::Clock for CountingClock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        86_400_000
    }
}
struct NeverClock;
impl crate::time::Clock for NeverClock {
    fn now_millis(&self) -> i64 {
        panic!("replay/rejection sampled clock")
    }
}
fn snapshot(f: &Fixture, id: MemoryId) -> (Vec<u8>, crate::storage::control::ControlState) {
    (
        f.record(id).unwrap().encode().unwrap(),
        f.storage.read().unwrap().control_state().unwrap(),
    )
}
async fn http(
    state: crate::api::AppState,
    id: MemoryId,
    op: Operation,
    mut value: Value,
) -> (u16, Value) {
    let object = value.as_object_mut().unwrap();
    object.remove("id");
    object.remove("contractVersion");
    let key = object.remove("idempotencyKey");
    let mut req = axum::http::Request::builder().uri(format!("/memories/{id}"));
    if let Some(key) = key {
        req = req.header("idempotency-key", key.as_str().unwrap());
    }
    let response = crate::api::native_lifecycle::prepared(
        state,
        id.to_string(),
        req.body(axum::body::Body::from(value.to_string())).unwrap(),
        op,
    )
    .await;
    let status = response.status().as_u16();
    (
        status,
        serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap(),
    )
}
fn raw_receipts(path: &std::path::Path) -> Vec<(Vec<u8>, Vec<u8>)> {
    use redb::ReadableTable;
    let (_copy, db) = crate::storage::validation::inspect_database(&path.join("meta.db")).unwrap();
    let read = db.begin_read().unwrap();
    crate::storage::control::validate_existing(&read).unwrap();
    read.open_table(crate::storage::control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|x| {
            let (k, v) = x.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_mcp_http_socket_replay_scope_cas_origin_and_one_clock() {
    use crate::serialization::native_lifecycle::{Reply, parse_lifecycle_private};
    use std::sync::atomic::Ordering;
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let clock = Arc::new(CountingClock::default());
    let adapter = Arc::new(
        f.storage_adapter()
            .with_clock(clock.clone())
            .with_native_lifecycle_policy(Default::default()),
    );
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        adapter.clone(),
        &socket.path().join("legacy.sock"),
    )
    .await;
    let (state, query) = state(&f, clock.clone());
    let value = request(&f, id, "parity2");
    let first = crate::mcp::native_lifecycle_adapter::dispatch(
        adapter.as_ref(),
        Operation::Feedback,
        value.clone(),
    )
    .await;
    assert!(first.error.is_none(), "{:?}", first.error);
    assert!(first.data.is_none());
    assert!(first.legacy_data.is_some());
    assert!(first.readiness.as_ref().unwrap().ready);
    let receipt = first.receipt.clone().unwrap();
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let after = snapshot(&f, id);
    let replay = remote
        .feedback_native(parse_lifecycle_private(&value, Operation::Feedback).unwrap())
        .await;
    assert!(replay.error.is_none(), "{:?}", replay.error);
    assert!(replay.receipt.unwrap().replayed);
    let (status, replay) = http(state.clone(), id, Operation::Feedback, value.clone()).await;
    assert_eq!(status, 200, "{replay}");
    assert_eq!(
        replay["receipt"]["operationId"],
        json!(receipt.operation_id)
    );
    assert_eq!(replay["data"]["historical"], true);
    assert!(replay["data"].get("strength").is_none());
    let never = f
        .storage_adapter()
        .with_clock(Arc::new(NeverClock))
        .with_native_lifecycle_policy(crate::storage::native_lifecycle::NativePolicyInputs {
            multiplier: f64::NAN,
            ..Default::default()
        });
    assert!(
        never
            .feedback_native(parse_lifecycle_private(&value, Operation::Feedback).unwrap())
            .await
            .receipt
            .unwrap()
            .replayed
    );
    let mut cross = value.clone();
    cross["contractVersion"] = json!(1);
    assert_eq!(
        remote
            .feedback_native(parse_lifecycle_private(&cross, Operation::Feedback).unwrap())
            .await
            .error
            .unwrap()
            .code,
        "IDEMPOTENCY_CONFLICT"
    );
    for (field, bad, code) in [
        ("namespace", json!("missing-name"), "NAMESPACE_MISMATCH"),
        ("namespaceId", json!(2), "NAMESPACE_MISMATCH"),
        (
            "storeId",
            json!(uuid::Uuid::new_v4()),
            "STORE_LINEAGE_MISMATCH",
        ),
    ] {
        let mut invalid = request(&f, id, "bad");
        invalid[field] = bad;
        let response = never
            .feedback_native(parse_lifecycle_private(&invalid, Operation::Feedback).unwrap())
            .await;
        assert_eq!(response.error.unwrap().code, code);
    }
    let mut stale = value.clone();
    stale["idempotencyKey"] = json!("new-stale");
    assert_eq!(
        never
            .feedback_native(parse_lifecycle_private(&stale, Operation::Feedback).unwrap())
            .await
            .error
            .unwrap()
            .code,
        "GENERATION_CONFLICT"
    );
    assert_eq!(snapshot(&f, id), after);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    // Separate application1 intent, same physical writer; cross-contract reuse conflicts in both directions.
    let mut one = request(&f, id, "one");
    one["contractVersion"] = json!(1);
    assert!(
        remote
            .feedback_native(parse_lifecycle_private(&one, Operation::Feedback).unwrap())
            .await
            .error
            .is_none()
    );
    one["contractVersion"] = json!(2);
    assert_eq!(
        remote
            .feedback_native(parse_lifecycle_private(&one, Operation::Feedback).unwrap())
            .await
            .error
            .unwrap()
            .code,
        "IDEMPOTENCY_CONFLICT"
    );
    let forgotten = request(&f, id, "forget2");
    let (status, deleted) = http(state.clone(), id, Operation::Forget, forgotten.clone()).await;
    assert_eq!(status, 200, "{deleted}");
    assert_eq!(deleted["data"]["status"], "tombstoned");
    assert!(
        remote
            .forget_native(parse_lifecycle_private(&forgotten, Operation::Forget).unwrap())
            .await
            .receipt
            .unwrap()
            .replayed
    );
    assert_eq!(
        clock.0.load(Ordering::SeqCst),
        2,
        "legacy forget has no timestamp to invent"
    );
    assert!(f.record(id).unwrap().summary.is_empty());
    let historical = never
        .feedback_native(parse_lifecycle_private(&value, Operation::Feedback).unwrap())
        .await;
    assert!(historical.receipt.as_ref().unwrap().replayed);
    assert!(historical.data.is_none());
    assert!(
        never
            .feedback_native(
                parse_lifecycle_private(&request(&f, id, "after-forget"), Operation::Feedback)
                    .unwrap()
            )
            .await
            .error
            .is_some()
    );
    let before = snapshot(&f, id);
    // Malformed new response cannot erase a matching known receipt.
    let mut malformed = serde_json::to_value(historical).unwrap();
    malformed["unexpected"] = json!(true);
    let retained = Reply::decode_response(
        malformed,
        &parse_lifecycle_private(&value, Operation::Feedback).unwrap(),
        Operation::Feedback,
    );
    assert_eq!(retained.receipt.unwrap().operation_id, receipt.operation_id);
    assert_eq!(
        retained.error.unwrap().commit_state,
        crate::serialization::contract::CommitState::Committed
    );
    assert_eq!(snapshot(&f, id), before);
    drop(remote);
    task.await.unwrap();
    drop((adapter, state, query, never));
    let root = f.close();
    crate::storage::validation::canonical_files(root.path()).unwrap();
    let raw = raw_receipts(root.path());
    let reopened = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
    drop(reopened);
    assert_eq!(raw_receipts(root.path()), raw);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_genuine_old_one_reopen_replays_without_rewriting_receipts() {
    use crate::serialization::native_lifecycle::parse_lifecycle_private;
    let root = tempfile::tempdir().unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "fixtures/legacy-lifecycle-old-api1.zip"
    )))
    .unwrap()
    .extract(root.path())
    .unwrap();
    let expected: Value =
        serde_json::from_slice(include_bytes!("fixtures/legacy-lifecycle-old-api1.json")).unwrap();
    let id: MemoryId = serde_json::from_value(expected["memoryId"].clone()).unwrap();
    let before = raw_receipts(root.path());
    let engine = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
    let f = Fixture::from_open_engine(root, engine);
    owner(&f).rebuild_all().await.unwrap();
    let canonical = snapshot(&f, id);
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(NeverClock))
        .with_native_lifecycle_policy(Default::default());
    for (operation, name) in [
        (Operation::Feedback, "feedback"),
        (Operation::Forget, "forget"),
    ] {
        let value = json!({"contractVersion":1,"id":id,"storeId":expected[name]["storeId"],"idempotencyKey":format!("old1-{name}"),"expectedGeneration":expected[format!("{name}ExpectedGeneration")]});
        let request = parse_lifecycle_private(&value, operation).unwrap();
        let reply = match operation {
            Operation::Feedback => adapter.feedback_native(request).await,
            Operation::Forget => adapter.forget_native(request).await,
        };
        assert!(reply.error.is_none(), "{:?}", reply.error);
        let r = reply.receipt.unwrap();
        assert!(r.replayed);
        assert_eq!(json!(r.operation_id), expected[name]["operationId"]);
        assert_eq!(json!(r.outcome), expected[name]["outcome"]);
    }
    let ctx = MutationContext {
        store_id: serde_json::from_value(expected["exposure"]["storeId"].clone()).unwrap(),
        scope: MutationScope::Namespace(NamespaceId::new(1)),
        idempotency_key: Some("old1-exposure".into()),
        expected_generation: serde_json::from_value(expected["exposureExpectedGeneration"].clone())
            .unwrap(),
        contract_version: 1,
    };
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .record_exposure_batch(&ctx, &[(id, AccessKind::DirectRetrieval)], i64::MAX)
            .unwrap()
            .unwrap()
            .replayed
    );
    assert_eq!(snapshot(&f, id), canonical);
    drop(adapter);
    let root = f.close();
    crate::storage::validation::canonical_files(root.path()).unwrap();
    assert_eq!(raw_receipts(root.path()), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_real_get_observer_false_and_original_hash_strict_reopen() {
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let clock = Arc::new(CountingClock::default());
    let (state, query) = state(&f, clock.clone());
    let before = snapshot(&f, id);
    query
        .get_memory_with_origin_work(id, false, None, 2)
        .await
        .unwrap();
    assert_eq!(snapshot(&f, id), before);
    query
        .get_memory_with_origin_work(id, true, None, 2)
        .await
        .unwrap();
    assert_eq!(f.record(id).unwrap().access_history.len(), 1);
    assert_eq!(f.record(id).unwrap().last_accessed_at, 86_400_000);
    assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    let expected_ctx = MutationContext {
        store_id: None,
        scope: MutationScope::Namespace(NamespaceId::new(1)),
        idempotency_key: None,
        expected_generation: None,
        contract_version: 2,
    };
    drop((state, query));
    let root = f.close();
    let receipts = raw_receipts(root.path());
    let exposures: Vec<crate::storage::control::StoredReceipt> = receipts
        .iter()
        .map(|(_, v)| crate::storage::control::validate_receipt(v).unwrap())
        .filter(|r| {
            matches!(
                r.receipt.outcome,
                crate::storage::control::MutationOutcome::ExposuresRecorded { .. }
            )
        })
        .collect();
    assert_eq!(exposures.len(), 1);
    let accesses =
        crate::storage::control::encode(&vec![(id, AccessKind::DirectRetrieval)]).unwrap();
    let scope = crate::storage::control::encode(&crate::storage::control::ExposureScope {
        namespace: "default".into(),
        source: Some(id),
    })
    .unwrap();
    assert_eq!(
        exposures[0].request_hash,
        crate::storage::control::request_hash(
            &expected_ctx,
            "record_exposure_batch",
            &[&accesses, &scope]
        )
    );
    let reopened = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
    drop(reopened);
    assert_eq!(raw_receipts(root.path()), receipts);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_old_native_client_alias_rejects_before_any_write() {
    use crate::serialization::native_lifecycle::parse_private;
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let before = snapshot(&f, id);
    let adapter = Arc::new(
        f.storage_adapter()
            .with_clock(Arc::new(NeverClock))
            .with_native_lifecycle_policy(Default::default()),
    );
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        adapter.clone(),
        &socket.path().join("old-client.sock"),
    )
    .await;
    for operation in [Operation::Feedback, Operation::Forget] {
        let parsed = parse_private(&request(&f, id, "old-native"), operation).unwrap();
        assert!(!parsed.legacy_aware);
        let reply = match operation {
            Operation::Feedback => remote.feedback_native(parsed).await,
            Operation::Forget => remote.forget_native(parsed).await,
        };
        assert_eq!(
            reply.error.unwrap().commit_state,
            crate::serialization::contract::CommitState::NotCommitted
        );
        assert!(reply.receipt.is_none());
        assert!(reply.legacy_data.is_none());
        assert_eq!(snapshot(&f, id), before);
    }
    drop(remote);
    task.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_new_alias_old_handler_unknown_method_no_fallback() {
    use crate::daemon::{DaemonClient, DaemonResponse, DaemonRpcError, protocol::*};
    use crate::serialization::native_lifecycle::parse_lifecycle_private;
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("old-server.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        for method in ["feedback_lifecycle_v1", "forget_lifecycle_v1"] {
            let request: crate::daemon::DaemonRequest =
                read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
            assert_eq!(request.method, method);
            write_framed_message(
                &mut stream,
                &DaemonResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: None,
                    error: Some(DaemonRpcError {
                        code: ERR_INVALID_INPUT,
                        message: format!("unknown method: {}", request.method),
                    }),
                },
                MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap();
        }
        let end: Result<crate::daemon::DaemonRequest, _> =
            read_framed_message(&mut stream, MAX_MESSAGE_SIZE).await;
        assert!(end.is_err(), "no fallback request permitted");
    });
    let remote = crate::daemon::bridge_adapters::RemoteStorageAdapter::new(Arc::new(
        DaemonClient::connect(&socket).await.unwrap(),
    ));
    for operation in [Operation::Feedback, Operation::Forget] {
        let parsed = parse_lifecycle_private(
            &json!({"contractVersion":2,"id":MemoryId::new()}),
            operation,
        )
        .unwrap();
        let reply = match operation {
            Operation::Feedback => remote.feedback_native(parsed).await,
            Operation::Forget => remote.forget_native(parsed).await,
        };
        assert_eq!(
            reply.error.unwrap().commit_state,
            crate::serialization::contract::CommitState::NotCommitted
        );
        assert!(reply.receipt.is_none());
    }
    drop(remote);
    task.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_publication_failure_keeps_full_receipt_and_retry_completes() {
    use crate::serialization::contract::CommitState;
    use crate::serialization::native_lifecycle::parse_lifecycle_private;
    for operation in [Operation::Feedback, Operation::Forget] {
        let f = Fixture::new();
        let id = f.insert_memory().await;
        f.insert_memory().await;
        *f.vector_index.write().await =
            crate::search::VectorIndexOwner::uniform(crate::test_support::DIM + 1);
        let adapter = f
            .storage_adapter()
            .with_clock(Arc::new(CountingClock::default()))
            .with_native_lifecycle_policy(Default::default());
        let value = request(&f, id, "publication");
        let parsed = parse_lifecycle_private(&value, operation).unwrap();
        let failed = match operation {
            Operation::Feedback => adapter.feedback_native(parsed.clone()).await,
            Operation::Forget => adapter.forget_native(parsed.clone()).await,
        };
        assert_eq!(failed.error.unwrap().commit_state, CommitState::Committed);
        assert!(failed.readiness.is_none());
        let receipt = failed.receipt.unwrap();
        let after = snapshot(&f, id);
        *f.vector_index.write().await =
            crate::search::VectorIndexOwner::uniform(crate::test_support::DIM);
        let retry = f
            .storage_adapter()
            .with_clock(Arc::new(NeverClock))
            .with_native_lifecycle_policy(Default::default());
        let replay = match operation {
            Operation::Feedback => retry.feedback_native(parsed).await,
            Operation::Forget => retry.forget_native(parsed).await,
        };
        assert!(replay.error.is_none(), "{:?}", replay.error);
        assert!(replay.readiness.unwrap().ready);
        assert_eq!(replay.receipt.unwrap().operation_id, receipt.operation_id);
        assert_eq!(snapshot(&f, id), after);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_real_unknown_tombstone_reopen_reconciles_original_operation() {
    use crate::serialization::native_lifecycle::{Reply, parse_lifecycle_private};
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let value = request(&f, id, "unknown");
    let ctx = context(&f, "unknown", 2);
    let storage = f.storage.clone();
    // Real canonical commit interruption, in the same thread as the actual engine.
    // No synthetic unknown error or invented receipt is supplied to the mapper.
    let error = tokio::task::spawn_blocking(move || {
        crate::storage::metadata::effects::FAIL_AFTER_RECORD_COMMIT.with(|fault| fault.set(true));
        storage
            .write()
            .unwrap()
            .tombstone_memory_with_receipt(&ctx, id)
            .unwrap_err()
    })
    .await
    .unwrap();
    let failed = Reply::storage(error);
    assert!(failed.receipt.is_none());
    let error = failed.error.unwrap();
    assert_eq!(
        error.commit_state,
        crate::serialization::contract::CommitState::Unknown
    );
    let operation = error.operation_id.unwrap();
    let root = f.close();
    let engine = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
    let f = Fixture::from_open_engine(root, engine);
    owner(&f).rebuild_all().await.unwrap();
    let reply = f
        .storage_adapter()
        .with_clock(Arc::new(NeverClock))
        .with_native_lifecycle_policy(Default::default())
        .forget_native(parse_lifecycle_private(&value, Operation::Forget).unwrap())
        .await;
    assert!(reply.error.is_none(), "{:?}", reply.error);
    let receipt = reply.receipt.unwrap();
    assert!(receipt.replayed);
    assert_eq!(receipt.operation_id, operation);
    assert_eq!(
        f.record(id).unwrap().phase,
        crate::model::DecayPhase::Tombstone
    );
    assert!(f.record(id).unwrap().summary.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_retired_scope_after_real_purge_and_name_reuse() {
    use crate::serialization::native_lifecycle::parse_lifecycle_private;
    let f = Fixture::new();
    let ns = f.create_namespace("retired");
    let id = f.insert_memory_in(ns, &[]).await;
    let value = request(&f, id, "retired2");
    let adapter = f
        .storage_adapter()
        .with_native_lifecycle_policy(Default::default());
    let first = adapter
        .forget_native(parse_lifecycle_private(&value, Operation::Forget).unwrap())
        .await;
    assert!(first.error.is_none(), "{:?}", first.error);
    let receipt = first.receipt.unwrap();
    drop(adapter);
    {
        let mut engine = f.storage.write().unwrap();
        let mut ctx = MutationContext::unkeyed(ns);
        ctx.store_id = Some(receipt.store_id);
        let mut p = engine.begin_namespace_purge(&ctx, true).unwrap();
        while p.stage != crate::storage::purge::PurgeStage::Complete {
            p = engine
                .resume_namespace_purge(p.store_id, ns, p.start_receipt.operation_id, 1)
                .unwrap();
        }
    }
    let reused = f.create_namespace("retired");
    assert_ne!(reused, ns);
    owner(&f).complete().await.unwrap();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(NeverClock))
        .with_native_lifecycle_policy(Default::default());
    for explicit in [false, true] {
        let mut retry = value.clone();
        if explicit {
            retry["namespaceId"] = json!(ns);
        }
        let reply = adapter
            .forget_native(parse_lifecycle_private(&retry, Operation::Forget).unwrap())
            .await;
        assert!(reply.error.is_none(), "{:?}", reply.error);
        assert_eq!(reply.scope.unwrap(), (ns, "retired".into()));
        assert_eq!(reply.receipt.unwrap().operation_id, receipt.operation_id);
    }
    let mut wrong = value.clone();
    wrong["namespaceId"] = json!(reused);
    assert_eq!(
        adapter
            .forget_native(parse_lifecycle_private(&wrong, Operation::Forget).unwrap())
            .await
            .error
            .unwrap()
            .code,
        "NAMESPACE_MISMATCH"
    );
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert!(f.record(id).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_actual_worker_panic_is_unknown_without_invented_receipt() {
    use crate::serialization::native_lifecycle::parse_lifecycle_private;
    struct PanicClock;
    impl crate::time::Clock for PanicClock {
        fn now_millis(&self) -> i64 {
            panic!("actual legacy worker interruption")
        }
    }
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let before = snapshot(&f, id);
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(PanicClock))
        .with_native_lifecycle_policy(Default::default());
    let reply = adapter
        .feedback_native(
            parse_lifecycle_private(&request(&f, id, "worker-panic"), Operation::Feedback).unwrap(),
        )
        .await;
    let error = reply.error.unwrap();
    assert_eq!(
        error.commit_state,
        crate::serialization::contract::CommitState::Unknown
    );
    assert_eq!(error.code, "COMMIT_OUTCOME_UNKNOWN");
    assert!(error.operation_id.is_none());
    assert!(reply.receipt.is_none());
    assert!(reply.readiness.is_none());
    drop(adapter);
    let root = f.close();
    let engine = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
    assert_eq!(
        engine.get_record(id).unwrap().unwrap().encode().unwrap(),
        before.0
    );
    assert_eq!(engine.control_state().unwrap(), before.1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_malformed_new_socket_reply_retains_real_known_receipt() {
    use crate::daemon::{DaemonClient, DaemonResponse, protocol::*};
    use crate::serialization::native_lifecycle::{Data, LegacyData, parse_lifecycle_private};
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let request =
        parse_lifecycle_private(&request(&f, id, "bad-reply"), Operation::Feedback).unwrap();
    let reply = f
        .storage_adapter()
        .with_native_lifecycle_policy(Default::default())
        .feedback_native(request.clone())
        .await;
    assert!(reply.error.is_none());
    let receipt = reply.receipt.clone().unwrap();
    assert!(matches!(
        reply.legacy_data,
        Some(LegacyData::Feedback { .. })
    ));
    let valid = serde_json::to_value(&reply).unwrap();
    let mut absent = valid.clone();
    absent.as_object_mut().unwrap().remove("legacyData");
    let mut both = valid.clone();
    both["data"] = serde_json::to_value(Data {
        id,
        lifecycle_version: crate::model::schema::LifecycleVersion::LegacyAccessV1,
        policy_revision: Default::default(),
        reinforced_at: None,
        phase: crate::model::DecayPhase::Full,
        strength: 1.,
        stability: 1.,
        is_permastore: false,
        historical: false,
    })
    .unwrap();
    let mut malformed = valid.clone();
    malformed["unexpected"] = json!(true);
    let socket = tempfile::tempdir().unwrap();
    let path = socket.path().join("bad-reply.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        for result in [absent, both, malformed] {
            let wire: crate::daemon::DaemonRequest =
                read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
            assert_eq!(wire.method, "feedback_lifecycle_v1");
            write_framed_message(
                &mut stream,
                &DaemonResponse {
                    jsonrpc: "2.0".into(),
                    id: wire.id,
                    result: Some(result),
                    error: None,
                },
                MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap();
        }
    });
    let remote = crate::daemon::bridge_adapters::RemoteStorageAdapter::new(Arc::new(
        DaemonClient::connect(&path).await.unwrap(),
    ));
    for _ in 0..3 {
        let failed = remote.feedback_native(request.clone()).await;
        assert_eq!(failed.receipt.unwrap(), receipt);
        assert_eq!(
            failed.error.unwrap().commit_state,
            crate::serialization::contract::CommitState::Committed
        );
        assert!(failed.data.is_none());
        assert!(failed.legacy_data.is_none());
    }
    drop(remote);
    task.await.unwrap();
}

#[test]
fn legacy_api2_native_reply_bytes_keep_exact_old_shape() {
    use crate::serialization::contract::{ExecutionError, Readiness};
    use crate::serialization::native_lifecycle::{Data, Reply};
    use crate::storage::control::WriteReceipt;
    let root = tempfile::tempdir().unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "../storage/metadata/native_records/fixtures/original-api-old2.zip"
    )))
    .unwrap()
    .extract(root.path())
    .unwrap();
    let expected: Value =
        serde_json::from_slice(&std::fs::read(root.path().join("expected.json")).unwrap()).unwrap();
    let receipt: WriteReceipt = serde_json::from_value(expected["feedback"].clone()).unwrap();
    let id: MemoryId = serde_json::from_value(expected["memoryId"].clone()).unwrap();
    let ns: NamespaceId = serde_json::from_value(expected["namespaceId"].clone()).unwrap();
    let reply = Reply::committed((ns, "native".into()), receipt, id);
    assert!(reply.data.is_some());
    assert!(reply.legacy_data.is_none());
    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct OldReply {
        scope: Option<(NamespaceId, String)>,
        data: Option<Data>,
        receipt: Option<WriteReceipt>,
        error: Option<ExecutionError>,
        readiness: Option<Readiness>,
    }
    let bytes = serde_json::to_vec(&reply).unwrap();
    let old: OldReply = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(serde_json::to_vec(&old).unwrap(), bytes);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_strict_private_validation_and_unkeyed_distinct_feedback() {
    use crate::serialization::native_lifecycle::parse_lifecycle_private;
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let clock = Arc::new(CountingClock::default());
    let adapter = f
        .storage_adapter()
        .with_clock(clock.clone())
        .with_native_lifecycle_policy(Default::default());
    let before = snapshot(&f, id);
    for field in ["contractVersion", "quality"] {
        for bad in [
            json!(null),
            json!(true),
            json!(false),
            json!("2"),
            json!(-1),
            json!(1.5),
            json!(99),
        ] {
            let mut value = request(&f, id, "invalid");
            value[field] = bad;
            let response = crate::mcp::native_lifecycle_adapter::dispatch_lifecycle_private(
                &adapter,
                Operation::Feedback,
                value,
            )
            .await;
            assert_eq!(
                response.error.unwrap().commit_state,
                crate::serialization::contract::CommitState::NotCommitted
            );
            assert!(response.receipt.is_none());
        }
    }
    assert_eq!(snapshot(&f, id), before);
    assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    let request =
        parse_lifecycle_private(&json!({"contractVersion":2,"id":id}), Operation::Feedback)
            .unwrap();
    let first = adapter
        .feedback_native(request.clone())
        .await
        .receipt
        .unwrap();
    let second = adapter.feedback_native(request).await.receipt.unwrap();
    assert_ne!(first.operation_id, second.operation_id);
    assert!(second.committed_generation > first.committed_generation);
    assert_eq!(f.record(id).unwrap().access_history.len(), 2);
    assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_prepared_mcp_tool_text_structured_http_parity_and_public_gate() {
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let clock = Arc::new(CountingClock::default());
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = Arc::new(
        f.storage_adapter()
            .with_clock(clock.clone())
            .with_native_lifecycle_policy(Default::default()),
    );
    bridge.default_namespace = "unrelated-session-default".into();
    let (state, query) = state(&f, clock);
    for (operation, name) in [
        (Operation::Feedback, "reinforce_memory"),
        (Operation::Forget, "forget_memory"),
    ] {
        let value = request(&f, id, name);
        let before = snapshot(&f, id);
        let gated = crate::mcp::tools::dispatch_tool(&bridge, name, value.clone()).await;
        assert_eq!(gated.is_error, Some(true));
        assert_eq!(snapshot(&f, id), before, "ordinary public2 remains closed");
        let result = crate::mcp::tools::dispatch_v2(&bridge, name, value.clone()).await;
        assert_ne!(result.is_error, Some(true));
        let encoded = serde_json::to_value(result).unwrap();
        let text: Value =
            serde_json::from_str(encoded["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(encoded["structuredContent"], text);
        assert!(text["receipt"].is_object());
        let (status, http) = http(state.clone(), id, operation, value).await;
        assert_eq!(status, 200, "{http}");
        assert_eq!(
            http["receipt"]["operationId"],
            text["receipt"]["operationId"]
        );
        assert_eq!(http["receipt"]["outcome"], text["receipt"]["outcome"]);
        assert_eq!(http["data"]["id"], text["data"]["id"]);
        assert_eq!(http["data"]["historical"], true);
        assert!(text["data"].get("strength").is_none());
    }
    drop((bridge, state, query));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_api2_error_reply_cannot_claim_an_unrelated_or_future_receipt() {
    use crate::daemon::{DaemonClient, DaemonResponse, protocol::*};
    use crate::serialization::contract::CommitState;
    use crate::serialization::native_lifecycle::{Reply, parse_lifecycle_private};
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let request =
        parse_lifecycle_private(&request(&f, id, "error-identity"), Operation::Feedback).unwrap();
    let first = f
        .storage_adapter()
        .with_native_lifecycle_policy(Default::default())
        .feedback_native(request.clone())
        .await;
    let receipt = first.receipt.unwrap();
    let before = snapshot(&f, id);
    let mut wrong_target = receipt.clone();
    if let crate::storage::control::MutationOutcome::LegacyFeedbackRecorded { memory_id, .. } =
        &mut wrong_target.outcome
    {
        *memory_id = MemoryId::new();
    } else {
        panic!("actual legacy feedback receipt required");
    }
    let mut wrong_operation = receipt.clone();
    wrong_operation.outcome = crate::storage::control::MutationOutcome::MemoryTombstoned {
        memory_id: id,
        status: crate::storage::tombstone::TombstoneStatus::Tombstoned,
    };
    let mut future = receipt;
    future.receipt_version = 2;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("error-receipt.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        for receipt in [wrong_target, wrong_operation, future] {
            let wire: crate::daemon::DaemonRequest =
                read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
            assert_eq!(wire.method, "feedback_lifecycle_v1");
            let reply = Reply::storage(crate::storage::StorageError::CommittedMutation {
                receipt: Box::new(receipt),
                source: Box::new(crate::storage::StorageError::RecoveryRequired(
                    "synthetic malformed peer response".into(),
                )),
            });
            write_framed_message(
                &mut stream,
                &DaemonResponse {
                    jsonrpc: "2.0".into(),
                    id: wire.id,
                    result: Some(serde_json::to_value(reply).unwrap()),
                    error: None,
                },
                MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap();
        }
    });
    let remote = crate::daemon::bridge_adapters::RemoteStorageAdapter::new(Arc::new(
        DaemonClient::connect(&path).await.unwrap(),
    ));
    for _ in 0..3 {
        let failed = remote.feedback_native(request.clone()).await;
        assert_eq!(
            failed.error.unwrap().commit_state,
            CommitState::Unknown,
            "an unrelated/future receipt cannot certify this request"
        );
        assert!(failed.receipt.is_none());
    }
    drop(remote);
    task.await.unwrap();
    assert_eq!(snapshot(&f, id), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_legacy_success_shapes() {
    use tower::ServiceExt;
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let clock = Arc::new(CountingClock::default());
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = Arc::new(
        f.storage_adapter()
            .with_clock(clock.clone())
            .with_native_lifecycle_policy(Default::default()),
    );
    let (state, _query) = state(&f, clock);
    for name in ["reinforce_memory", "forget_memory"] {
        let result = crate::mcp::tools::dispatch_tool(&bridge, name, json!({"id":id})).await;
        assert_ne!(result.is_error, Some(true), "{result:?}");
        let result = serde_json::to_value(result).unwrap();
        let data: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let expected = if name == "reinforce_memory" {
            vec!["id", "isPermastore", "phase", "stability", "strength"]
        } else {
            vec!["deleted", "id"]
        };
        assert_eq!(
            data.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );
        println!("legacy MCP {name}: {data}");
    }
    let id = f.insert_memory().await;
    for (method, uri, body) in [
        ("POST", format!("/memories/{id}/reinforce"), "{}"),
        ("DELETE", format!("/memories/{id}"), ""),
    ] {
        let response = crate::api::router(state.clone(), &Default::default())
            .oneshot(
                axum::http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        let value: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            value
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["data", "tookUs"]
        );
        let expected = if method == "POST" {
            vec!["id", "isPermastore", "phase", "stability", "strength"]
        } else {
            vec!["deleted", "id"]
        };
        assert_eq!(
            value["data"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );
        println!("legacy HTTP {method}: {value}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_legacy_projection_is_same_tx_under_later_accepted_write() {
    use crate::serialization::lifecycle_api1 as api1;
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let owner = owner(&f);
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let held = owner.accept_work(None).unwrap();
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(CountingClock::default()));
    let first = tokio::spawn(async move {
        adapter
            .api1_lifecycle(api1::request(id, 4, None), Operation::Feedback)
            .await
    });
    let wait = tracker.clone();
    tokio::task::spawn_blocking(move || wait.wait_until_closed())
        .await
        .unwrap();
    let committed_first = f.record(id).unwrap();
    assert_eq!(committed_first.access_history.len(), 1);
    let ctx = MutationContext {
        store_id: None,
        scope: MutationScope::Namespace(NamespaceId::new(1)),
        idempotency_key: None,
        expected_generation: None,
        contract_version: 1,
    };
    // This previously accepted producer may finish while the first response's
    // real barrier drains. It commits a different numerical state before return.
    owner
        .legacy_lifecycle_with_work(
            ctx.clone(),
            id,
            Some(1),
            None,
            Default::default(),
            Arc::new(CountingClock::default()),
            Some(&held),
        )
        .await
        .unwrap();
    assert_ne!(f.record(id).unwrap().stability, committed_first.stability);
    owner.finish_known_work(held);
    let first = first.await.unwrap();
    let Some(api1::Value::Feedback { result }) = first.value else {
        panic!("{first:?}")
    };
    assert_eq!(result.stability, committed_first.stability);
    assert_eq!(result.strength, committed_first.strength);
    assert_eq!(tracker.status().pending, 0);
    let receipt = first.lifecycle.unwrap().receipt.unwrap();
    drop(owner);
    let root = f.close();
    let stored = raw_receipts(root.path())
        .into_iter()
        .map(|(_, v)| crate::storage::control::validate_receipt(&v).unwrap())
        .find(|r| r.receipt.operation_id == receipt.operation_id)
        .unwrap();
    assert_eq!(
        stored.request_hash,
        crate::storage::control::request_hash(
            &ctx,
            "record_legacy_feedback",
            &[id.as_bytes(), &[4]]
        ),
        "unchanged original1 unscoped request identity"
    );
    drop(crate::storage::RedbStorageEngine::open(root.path()).unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_legacy_missing_scope_terminal_and_old_alias_new_server() {
    use crate::serialization::lifecycle_api1 as api1;
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let clock = Arc::new(CountingClock::default());
    let adapter = Arc::new(f.storage_adapter().with_clock(clock.clone()));
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        adapter.clone(),
        &socket.path().join("new.sock"),
    )
    .await;
    let client = remote.test_client();
    let before = snapshot(&f, id);
    // New alias without ping/frame negotiation rejects before touching metadata.
    assert!(
        client
            .call(
                "forget_api1_v1",
                api1::request(id, 3, None).wire(Operation::Forget)
            )
            .await
            .is_err()
    );
    assert_eq!(before, snapshot(&f, id));
    assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    // Old client method on a new daemon retains its exact old scalar shape.
    assert_eq!(
        client
            .call("delete_memory", json!({"id":id}))
            .await
            .unwrap(),
        json!(true)
    );
    let terminal = f.record(id).unwrap();
    assert_eq!(terminal.phase, crate::model::DecayPhase::Tombstone);
    let repeated = adapter
        .api1_lifecycle(api1::request(id, 3, None), Operation::Forget)
        .await;
    assert!(
        matches!(repeated.value, Some(api1::Value::Forget { deleted: false })),
        "{repeated:?}"
    );
    let missing = adapter
        .api1_lifecycle(api1::request(MemoryId::nil(), 3, None), Operation::Forget)
        .await;
    assert!(
        matches!(missing.value, Some(api1::Value::Forget { deleted: false })),
        "{missing:?}"
    );
    let wrong = adapter
        .api1_lifecycle(api1::request(id, 3, Some("wrong")), Operation::Forget)
        .await;
    assert!(wrong.value.is_none());
    assert_eq!(
        wrong.lifecycle.unwrap().error.unwrap().code,
        "NAMESPACE_MISMATCH"
    );
    let denied = adapter
        .api1_lifecycle(api1::request(id, 3, None), Operation::Feedback)
        .await;
    assert!(denied.value.is_none());
    assert_eq!(
        denied.lifecycle.unwrap().error.unwrap().code,
        "CONTENT_UNAVAILABLE"
    );
    use tower::ServiceExt;
    let (state, _query) = state(&f, clock.clone());
    for hard_delete in [false, true] {
        if hard_delete {
            f.storage.write().unwrap().delete_memory(id).unwrap();
            owner(&f).complete().await.unwrap();
        }
        let before = f.storage.read().unwrap().control_state().unwrap();
        let calls = clock.0.load(std::sync::atomic::Ordering::SeqCst);
        let response = crate::api::router(state.clone(), &Default::default())
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(format!("/memories/{id}/reinforce"))
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            409,
            "terminal feedback remains a conflict"
        );
        assert_eq!(before, f.storage.read().unwrap().control_state().unwrap());
        assert_eq!(calls, clock.0.load(std::sync::atomic::Ordering::SeqCst));
    }
    task.abort();
    let _ = task.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_mixed_case_canonical_scope_through_public_and_remote() {
    use crate::serialization::lifecycle_api1 as api1;
    use tower::ServiceExt;
    let f = Fixture::new();
    let ns = f.create_namespace("MiXeD-Case");
    f.create_namespace("Foreign");
    let id = f.insert_memory_in(ns, &[]).await;
    let clock = Arc::new(CountingClock::default());
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = Arc::new(f.storage_adapter().with_clock(clock.clone()));
    bridge.default_namespace = "Foreign".into();
    let (state, query) = state(&f, clock.clone());
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        bridge.storage.clone(),
        &socket.path().join("mixed.sock"),
    )
    .await;
    let before = snapshot(&f, id);
    for assertion in ["missing", "FOREIGN"] {
        for operation in [Operation::Feedback, Operation::Forget] {
            let failed = remote
                .api1_lifecycle(api1::request(id, 4, Some(assertion)), operation)
                .await;
            assert!(failed.value.is_none());
            assert_eq!(
                failed.lifecycle.unwrap().error.unwrap().code,
                "NAMESPACE_MISMATCH"
            );
            assert_eq!(before, snapshot(&f, id));
            assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
    }
    let missing = remote
        .api1_lifecycle(
            api1::request(MemoryId::new(), 4, Some("mixed-case")),
            Operation::Forget,
        )
        .await;
    assert!(missing.value.is_none());
    assert_eq!(missing.lifecycle.unwrap().error.unwrap().code, "NOT_FOUND");
    assert_eq!(before, snapshot(&f, id));
    let success = crate::mcp::tools::dispatch_tool(
        &bridge,
        "reinforce_memory",
        json!({"id":id,"namespace":"MIXED-case","quality":4}),
    )
    .await;
    assert_ne!(success.is_error, Some(true), "{success:?}");
    let response = crate::api::router(state.clone(), &Default::default())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/memories/{id}/reinforce"))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(r#"{"namespace":"mixed-CASE"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let success = remote
        .api1_lifecycle(
            api1::request(id, 4, Some("mixed-case")),
            Operation::Feedback,
        )
        .await;
    assert!(
        matches!(success.value, Some(api1::Value::Feedback { .. })),
        "{success:?}"
    );
    assert_eq!(
        success.lifecycle.unwrap().scope.unwrap(),
        (ns, "MiXeD-Case".into())
    );
    let forgotten = remote
        .api1_lifecycle(api1::request(id, 3, Some("mixed-CASE")), Operation::Forget)
        .await;
    assert!(
        matches!(forgotten.value, Some(api1::Value::Forget { deleted: true })),
        "{forgotten:?}"
    );
    let spelling = id.to_string().to_uppercase();
    let again = crate::mcp::tools::dispatch_tool(
        &bridge,
        "forget_memory",
        json!({"id":spelling,"namespace":"MIXED-CASE"}),
    )
    .await;
    assert_ne!(again.is_error, Some(true), "{again:?}");
    let wire = serde_json::to_value(again).unwrap();
    let value: Value = serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(value, json!({"id":spelling,"deleted":false}));
    let id2 = f.insert_memory_in(ns, &[]).await;
    let response = crate::api::router(state.clone(), &Default::default())
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/memories/{id2}?namespace=MIXED-case"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .get_namespace(ns)
            .unwrap()
            .unwrap()
            .name,
        "MiXeD-Case"
    );
    task.abort();
    let _ = task.await;
    drop((remote, bridge, state, query));
    let root = f.close();
    drop(crate::storage::RedbStorageEngine::open(root.path()).unwrap());
}
