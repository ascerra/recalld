//! Real migrated native owners and immutable BEFORE-106 retained artifacts.
use super::*;
use crate::mcp::bridge::{McpBridge, StorageEngine as _};
use crate::mcp::bridge_adapters::McpStorageAdapter;
use crate::serialization::native_batch::NativeBatchReply;
use crate::test_support::{Fixture, StubBridge};
use serde_json::Value;

fn adapter(
    f: &Fixture,
    provider: Arc<dyn crate::embedding::EmbeddingProvider>,
    clock: Arc<FixedClock>,
    config: crate::config::RecalldConfig,
) -> McpStorageAdapter {
    McpStorageAdapter::new(
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
fn bridge(adapter: Arc<McpStorageAdapter>) -> McpBridge {
    let mut b = StubBridge::default().into_bridge();
    b.storage = adapter;
    b.default_namespace = "native".into();
    b
}
fn body(r: &BatchInput) -> Value {
    let mut value = json!({"memories":r.items,"storeId":r.store_id.unwrap()});
    if let Some(k) = &r.idempotency_key {
        value["idempotencyKey"] = json!(k);
    }
    if let Some(g) = r.expected_generation {
        value["expectedGeneration"] = json!(g);
    }
    if let Some(id) = r.batch_operation_id {
        value["batchOperationId"] = json!(id);
    }
    value
}
async fn http(a: &McpStorageAdapter, value: &Value) -> (u16, Value) {
    let r = crate::api::batch_store_v2(a, "native", value, None).await;
    let status = r.status().as_u16();
    let bytes = axum::body::to_bytes(r.into_body(), crate::daemon::protocol::MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
async fn mcp(b: &McpBridge, mut value: Value) -> (bool, Value) {
    value["contractVersion"] = json!(2);
    let r = crate::mcp::tools::dispatch_v2(b, "store_memories", value).await;
    let encoded = serde_json::to_value(&r).unwrap();
    assert!(serde_json::to_vec(&r).unwrap().len() < crate::daemon::protocol::MAX_MESSAGE_SIZE);
    let text: Value =
        serde_json::from_str(encoded["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(encoded["structuredContent"], text);
    (r.is_error == Some(true), text)
}
fn canonical<T: serde::Serialize>(v: &T) -> Value {
    serde_json::from_slice(&serde_json::to_vec(v).unwrap()).unwrap()
}

#[tokio::test]
async fn batch_wire_historical_oversize_completed_pending_retained_bytes_and_settings() {
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/native-batch-before106-oversize");
    let r: BatchInput =
        serde_json::from_slice(&std::fs::read(artifacts.join("request.json")).unwrap()).unwrap();
    let reservation: WriteReceipt =
        control::decode(&std::fs::read(artifacts.join("reservation.json")).unwrap()).unwrap();
    for state in ["pending", "completed"] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("store");
        zip::ZipArchive::new(std::fs::File::open(artifacts.join(format!("{state}.zip"))).unwrap())
            .unwrap()
            .extract(&path)
            .unwrap();
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let session =
            NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock.clone())
                .unwrap();
        let f = Fixture::from_open_engine(root, session.engine);
        let profile = crate::embedding::runtime::tests::profile();
        let provider = Arc::new(Provider {
            profile,
            calls: AtomicUsize::new(0),
            fail_if_called: true,
        });
        let before = readers::bytes(&path);
        let old = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .lookup_batch(r.store_id.unwrap(), reservation.operation_id)
            .unwrap()
            .unwrap();
        assert_eq!(old.unresolved(), if state == "pending" { 100 } else { 0 });
        for links in [0, u32::MAX as usize] {
            let mut config = crate::config::RecalldConfig::default();
            config.graph.max_auto_links = links;
            config.graph.max_entity_links = 0;
            config.graph.max_temporal_links = 0;
            config.graph.temporal_window_ms = 0;
            let a = Arc::new(adapter(&f, provider.clone(), clock.clone(), config));
            let b = bridge(a.clone());
            for by_id in [false, true] {
                let mut replay = r.clone();
                if by_id {
                    replay.idempotency_key = None;
                    replay.batch_operation_id = Some(reservation.operation_id);
                }
                let failure = a.mutate_native_batch(replay.clone()).await.unwrap_err();
                assert_eq!(failure.error.code, "RESPONSE_TOO_LARGE");
                let rpc = NativeBatchReply::Failure(failure);
                let daemon =
                    crate::daemon::protocol::DaemonResponse::success(u64::MAX, canonical(&rpc));
                assert!(serde_json::to_vec(&daemon).unwrap().len() < 64 * 1024);
                let framed = crate::daemon::server::native_batch_method_test_rpc(
                    a.clone(),
                    &replay,
                    "mutate_native_batch_v1",
                )
                .await;
                assert!(framed.error.is_none());
                assert_eq!(framed.result, daemon.result);
                let decoded: NativeBatchReply =
                    serde_json::from_value(framed.result.unwrap()).unwrap();
                assert_eq!(
                    decoded.result().unwrap_err().error.operation_id,
                    Some(reservation.operation_id)
                );
                let (status, h) = http(&a, &body(&replay)).await;
                assert_eq!(status, 400);
                let (is_error, m) = mcp(&b, body(&replay)).await;
                assert!(is_error);
                assert_eq!(h, m);
                assert_eq!(m["error"]["code"], "RESPONSE_TOO_LARGE");
                assert_eq!(m["error"]["commitState"], "committed");
                assert_eq!(m["batchOperationId"], reservation.operation_id.to_string());
                let mut expected = reservation.clone();
                expected.replayed = true;
                assert_eq!(m["reservationReceipt"], canonical(&expected));
                assert!(m.get("data").is_none());
                assert!(m.get("receipt").is_none());
                assert_eq!(
                    readers::bytes(&path),
                    before,
                    "{state}, links={links}, by_id={by_id}"
                );
                assert_eq!(clock.0.load(Ordering::SeqCst), 0);
                assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
                assert_eq!(
                    f.storage
                        .read()
                        .unwrap()
                        .meta_store()
                        .lookup_batch(r.store_id.unwrap(), reservation.operation_id)
                        .unwrap()
                        .unwrap(),
                    old
                );
            }
        }
    }
}

#[tokio::test]
async fn batch_wire_fresh_budget_rejects_before_reservation_provider_and_clock() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let mut r = request(&d, session.metadata().control_state().unwrap().store_id);
    r.items = (0..100)
        .map(|i| json!({"summary":format!("fresh {i}"),"checkDuplicates":false}))
        .collect();
    let provider = Arc::new(Provider {
        profile: d.profile.clone(),
        calls: AtomicUsize::new(0),
        fail_if_called: true,
    });
    let f = Fixture::from_open_engine(root, session.engine);
    let before = readers::bytes(&d.path);
    let a = Arc::new(adapter(
        &f,
        provider.clone(),
        clock.clone(),
        Default::default(),
    ));
    let (status, h) = http(&a, &body(&r)).await;
    assert_eq!(status, 400);
    assert_eq!(h["error"]["code"], "RESPONSE_TOO_LARGE");
    assert_eq!(h["error"]["commitState"], "notCommitted");
    assert!(h.get("reservationReceipt").is_none());
    let (error, m) = mcp(&bridge(a), body(&r)).await;
    assert!(error);
    assert_eq!(h, m);
    assert_eq!(readers::bytes(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn batch_wire_native_partial_full_receipts_replay_and_cas() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let mut r = request(&d, session.metadata().control_state().unwrap().store_id);
    r.expected_generation = Some(
        session
            .metadata()
            .control_state()
            .unwrap()
            .committed_generation,
    );
    r.items = vec![
        json!({"summary":"native batch Unicode José","entities":["John Smith","José","Alice's"],"checkDuplicates":false}),
        json!({"summary":"invalid field","unknown":true}),
    ];
    let f = Fixture::from_open_engine(root, session.engine);
    let provider = Arc::new(Provider {
        profile: d.profile.clone(),
        calls: AtomicUsize::new(0),
        fail_if_called: false,
    });
    let a = Arc::new(adapter(
        &f,
        provider.clone(),
        clock.clone(),
        Default::default(),
    ));
    let (status, h) = http(&a, &body(&r)).await;
    assert_eq!(status, 207, "{h}");
    assert_eq!(h["data"]["total"], "2");
    assert_eq!(h["data"]["succeeded"], "1");
    assert_eq!(h["data"]["failed"], "1");
    assert_eq!(h["data"]["unresolved"], "0");
    assert_eq!(h["data"]["outcomes"][0]["status"], "ok");
    assert_eq!(h["data"]["outcomes"][1]["status"], "error");
    let receipt: WriteReceipt =
        serde_json::from_value(h["data"]["outcomes"][0]["receipt"].clone()).unwrap();
    assert_eq!(h["data"]["outcomes"][0]["receipt"], canonical(&receipt));
    let before = readers::bytes(&d.path);
    let calls = provider.calls.load(Ordering::SeqCst);
    let ticks = clock.0.load(Ordering::SeqCst);
    let (error, m) = mcp(&bridge(a.clone()), body(&r)).await;
    assert!(error);
    let mut expected_outcomes = h["data"]["outcomes"].clone();
    expected_outcomes[0]["receipt"]["replayed"] = json!(true);
    assert_eq!(m["data"]["outcomes"], expected_outcomes);
    assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
    assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
    assert_eq!(readers::bytes(&d.path), before);
    r.expected_generation = Some(control::Generation::default());
    let (status, h) = http(&a, &body(&r)).await;
    assert_eq!(status, 409);
    assert_eq!(h["error"]["code"], "IDEMPOTENCY_CONFLICT");
    assert_eq!(readers::bytes(&d.path), before);
    let mut gated = body(&r);
    gated["contractVersion"] = json!(2);
    let out = crate::mcp::tools::dispatch_tool(&bridge(a), "store_memories", gated).await;
    assert_eq!(out.is_error, Some(true));
    assert_eq!(readers::bytes(&d.path), before);
}

struct RecoveringProvider {
    profile: crate::model::schema::EmbeddingProfile,
    unavailable: std::sync::atomic::AtomicBool,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for RecoveringProvider {
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        Some(self.profile.clone())
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "isolated-recovering-provider"
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.unavailable.load(Ordering::SeqCst) {
            Err(crate::embedding::EmbeddingError::Unavailable(
                "synthetic offline failure".into(),
            ))
        } else {
            Ok(vec![1., 0., 0., 0.])
        }
    }
}
#[tokio::test]
async fn batch_wire_actual_unavailable_all_failed_and_committed_publication_failure() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let mut r = request(&d, session.metadata().control_state().unwrap().store_id);
    r.items = vec![json!({"summary":"pending provider","checkDuplicates":false})];
    let f = Fixture::from_open_engine(root, session.engine);
    let provider = Arc::new(RecoveringProvider {
        profile: d.profile.clone(),
        unavailable: true.into(),
        calls: AtomicUsize::new(0),
    });
    let a = Arc::new(adapter(
        &f,
        provider.clone(),
        clock.clone(),
        Default::default(),
    ));
    let (status, pending) = http(&a, &body(&r)).await;
    assert_eq!(status, 503, "{pending}");
    assert_eq!(pending["data"]["succeeded"], "0");
    assert_eq!(pending["data"]["unresolved"], "1");
    assert!(pending["data"]["outcomes"][0].get("receipt").is_none());
    let parent = pending["data"]["batchOperationId"].clone();
    provider.unavailable.store(false, Ordering::SeqCst);
    let (status, done) = http(&a, &body(&r)).await;
    assert_eq!(status, 200, "{done}");
    assert_eq!(done["data"]["batchOperationId"], parent);
    assert_eq!(done["data"]["outcomes"][0]["status"], "ok");
    let calls = provider.calls.load(Ordering::SeqCst);
    let ticks = clock.0.load(Ordering::SeqCst);
    let mut bad = r.clone();
    bad.idempotency_key = Some("all-invalid".into());
    bad.items = vec![json!(null), json!({"summary":"invalid","unexpected":true})];
    let (status, failed) = http(&a, &body(&bad)).await;
    assert_eq!(status, 422, "{failed}");
    assert_eq!(failed["data"]["failed"], "2");
    assert_eq!(failed["data"]["outcomes"].as_array().unwrap().len(), 2);
    let (is_error, replayed) = mcp(&bridge(a.clone()), body(&bad)).await;
    assert!(is_error);
    assert_eq!(replayed["data"]["outcomes"], failed["data"]["outcomes"]);
    assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
    assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
    r.idempotency_key = Some("commit-unready".into());
    r.items[0]["summary"] = json!("actual publication failure");
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
    let (status, unready) = http(&a, &body(&r)).await;
    assert_eq!(status, 207, "{unready}");
    assert_eq!(unready["data"]["succeeded"], "1");
    let item = &unready["data"]["outcomes"][0];
    assert_eq!(item["status"], "ok");
    assert_eq!(item["ready"], false);
    assert_eq!(item["retryable"], false);
    assert_eq!(item["error"]["commitState"], "committed");
    let receipt: WriteReceipt = serde_json::from_value(item["receipt"].clone()).unwrap();
    assert_eq!(item["receipt"], canonical(&receipt));
    assert_ne!(
        item["receipt"]["operationId"],
        unready["data"]["batchOperationId"]
    );
    let calls = provider.calls.load(Ordering::SeqCst);
    let ticks = clock.0.load(Ordering::SeqCst);
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    let (status, recovered) = http(&a, &body(&r)).await;
    assert_eq!(status, 200, "{recovered}");
    let mut expected_replay = receipt.clone();
    expected_replay.replayed = true;
    assert_eq!(
        recovered["data"]["outcomes"][0]["receipt"],
        canonical(&expected_replay)
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
    assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
    let retained = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .lookup_batch(
            r.store_id.unwrap(),
            serde_json::from_value(unready["data"]["batchOperationId"].clone()).unwrap(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(retained.items[0].state, BatchItemState::Ok { receipt });
}
