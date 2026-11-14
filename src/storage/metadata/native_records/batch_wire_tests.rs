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

async fn single_http(a: &McpStorageAdapter, value: &Value) -> (u16, Value) {
    let r = crate::api::single_store_v2(a, "wrong-default", value, None).await;
    let status = r.status().as_u16();
    let bytes = axum::body::to_bytes(r.into_body(), crate::daemon::protocol::MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
async fn single_mcp(a: Arc<McpStorageAdapter>, mut value: Value) -> (bool, Value) {
    value["contractVersion"] = json!(2);
    let r = crate::mcp::tools::dispatch_v2(&bridge(a), "store_memory", value).await;
    assert!(serde_json::to_vec(&r).unwrap().len() < crate::daemon::protocol::MAX_MESSAGE_SIZE);
    let packet = serde_json::to_value(&r).unwrap();
    let text: Value = serde_json::from_str(packet["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(packet["structuredContent"], text);
    (r.is_error == Some(true), text)
}

#[tokio::test]
async fn native_single_wire_actual_mcp_http_rpc_profile_cas_and_canonical_labels() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let state = session.metadata().control_state().unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    let provider = Arc::new(RecoveringProvider {
        profile: d.profile.clone(),
        unavailable: false.into(),
        calls: AtomicUsize::new(0),
        profiles: AtomicUsize::new(0),
        corrupt: Default::default(),
    });
    let a = Arc::new(adapter(
        &f,
        provider.clone(),
        clock.clone(),
        Default::default(),
    ));
    let body = json!({"namespaceId":d.ns,"storeId":state.store_id,"expectedGeneration":state.committed_generation,"idempotencyKey":"single","summary":"Unicode native single","entities":["John Smith","José","Alice's"],"embedding":[2.,-0.,0.,0.],"embeddingProfileId":d.profile.profile_id().unwrap(),"checkDuplicates":false});
    let (status, h) = single_http(&a, &body).await;
    assert_eq!(status, 200, "{h}");
    assert_eq!(h["scope"]["namespace"], "native");
    assert_eq!(h["readiness"]["ready"], true);
    let receipt: WriteReceipt = serde_json::from_value(h["receipt"].clone()).unwrap();
    assert_eq!(h["receipt"], canonical(&receipt));
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &receipt.outcome else {
        panic!()
    };
    let record = f
        .storage
        .read()
        .unwrap()
        .get_record(transition.memory_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        record.schema.unwrap().labels.entities,
        ["John Smith", "José", "Alice's"]
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(provider.profiles.load(Ordering::SeqCst), 0);
    let before = readers::bytes(&d.path);
    let ticks = clock.0.load(Ordering::SeqCst);
    let (error, m) = single_mcp(a.clone(), body.clone()).await;
    assert!(!error, "{m}");
    let mut expected = receipt.clone();
    expected.replayed = true;
    assert_eq!(m["receipt"], canonical(&expected));
    let (_, h_replay) = single_http(&a, &body).await;
    assert_eq!(m, h_replay);
    let request =
        crate::serialization::native_single::Request::parse(&body, true, None, "wrong-default")
            .unwrap();
    let rpc = crate::daemon::server::native_method_test_rpc(
        a.clone(),
        serde_json::to_value(request).unwrap(),
        "store_native_single_observed_v1",
    )
    .await;
    assert!(rpc.error.is_none(), "{rpc:?}");
    let reply: crate::serialization::native_single::Reply =
        serde_json::from_value(rpc.result.unwrap()).unwrap();
    assert_eq!(reply.receipt.as_ref(), Some(&expected));
    let (_, envelope) = reply.response();
    assert_eq!(canonical(&envelope), m);
    assert_eq!(readers::bytes(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
    for (field, value, code) in [
        ("summary", json!("changed"), "IDEMPOTENCY_CONFLICT"),
        ("embedding", json!([1., 0., 0., 0.]), "IDEMPOTENCY_CONFLICT"),
    ] {
        let mut changed = body.clone();
        changed[field] = value;
        let (status, out) = single_http(&a, &changed).await;
        assert_eq!(status, 409, "{out}");
        assert_eq!(out["error"]["code"], code);
    }
    let mut fresh = body.clone();
    fresh["idempotencyKey"] = json!("stale");
    let (status, out) = single_http(&a, &fresh).await;
    assert_eq!(status, 409, "{out}");
    assert_eq!(out["error"]["code"], "GENERATION_CONFLICT");
    assert_eq!(readers::bytes(&d.path), before);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
}

#[tokio::test]
async fn native_single_wire_fresh_budget_scope_and_profile_fail_before_provider_or_allocation() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let store_id = s.metadata().control_state().unwrap().store_id;
    let f = Fixture::from_open_engine(root, s.engine);
    let provider = Arc::new(RecoveringProvider {
        profile: d.profile.clone(),
        unavailable: false.into(),
        calls: AtomicUsize::new(0),
        profiles: AtomicUsize::new(0),
        corrupt: Default::default(),
    });
    let mut config = crate::config::RecalldConfig::default();
    config.graph.max_auto_links = 1024;
    let a = adapter(&f, provider.clone(), clock.clone(), config);
    let base = json!({"namespaceId":d.ns,"storeId":store_id,"idempotencyKey":"budget","summary":"bounded"});
    let before = readers::bytes(&d.path);
    let (status, out) = single_http(&a, &base).await;
    assert_eq!(status, 400, "{out}");
    assert_eq!(out["error"]["code"], "RESPONSE_TOO_LARGE");
    assert!(out.get("receipt").is_none());
    let mut wrong = base.clone();
    wrong["namespace"] = json!("wrong");
    let (status, out) = single_http(&a, &wrong).await;
    assert_eq!(status, 400, "{out}");
    assert_eq!(out["error"]["code"], "NAMESPACE_MISMATCH");
    let mut profile = base.clone();
    profile["embedding"] = json!([1., 0., 0., 0.]);
    profile["embeddingProfileId"] = json!(format!("ep1:{}", "a".repeat(64)));
    let (status, out) = single_http(&a, &profile).await;
    assert_eq!(status, 400, "{out}");
    assert_eq!(out["error"]["code"], "EMBEDDING_PROFILE_MISMATCH");
    assert_eq!(readers::bytes(&d.path), before);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(provider.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn native_single_wire_known_publication_failure_retains_full_receipt_and_repairs_key() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let store_id = s.metadata().control_state().unwrap().store_id;
    let f = Fixture::from_open_engine(root, s.engine);
    let provider = Arc::new(RecoveringProvider {
        profile: d.profile.clone(),
        unavailable: false.into(),
        calls: AtomicUsize::new(0),
        profiles: AtomicUsize::new(0),
        corrupt: std::sync::Mutex::new(Some(f.vector_index.clone())),
    });
    let a = Arc::new(adapter(
        &f,
        provider.clone(),
        clock.clone(),
        Default::default(),
    ));
    let body = json!({"namespaceId":d.ns,"storeId":store_id,"idempotencyKey":"publish-fault","summary":"known receipt","checkDuplicates":false});
    let (status, out) = single_http(&a, &body).await;
    assert_eq!(status, 503, "{out}");
    assert_eq!(out["error"]["commitState"], "committed");
    let receipt: WriteReceipt = serde_json::from_value(out["receipt"].clone()).unwrap();
    assert_eq!(out["receipt"], canonical(&receipt));
    assert_eq!(out["readiness"]["ready"], false);
    assert!(
        out["readiness"]["derived"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["appliedGeneration"].is_null())
    );
    let calls = provider.calls.load(Ordering::SeqCst);
    let profiles = provider.profiles.load(Ordering::SeqCst);
    let ticks = clock.0.load(Ordering::SeqCst);
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    let (error, replay) = single_mcp(a.clone(), body).await;
    assert!(!error, "{replay}");
    assert_eq!(replay["readiness"]["ready"], true);
    let mut expected = receipt;
    expected.replayed = true;
    assert_eq!(replay["receipt"], canonical(&expected));
    assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
    assert_eq!(provider.profiles.load(Ordering::SeqCst), profiles);
    assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
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
    profiles: AtomicUsize,
    corrupt: std::sync::Mutex<Option<Arc<tokio::sync::RwLock<crate::search::FlatVectorIndex>>>>,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for RecoveringProvider {
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        self.profiles.fetch_add(1, Ordering::SeqCst);
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
            let corrupt = self.corrupt.lock().unwrap().take();
            if let Some(index) = corrupt {
                *index.write().await = crate::search::FlatVectorIndex::new(8);
            }
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
        profiles: AtomicUsize::new(0),
        corrupt: std::sync::Mutex::new(None),
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
    assert_eq!(pending["readiness"]["ready"], false);
    assert_eq!(pending["readiness"]["pendingWorkScope"], "acceptedTickets");
    assert_eq!(pending["readiness"]["pendingWork"], 0);
    assert_eq!(pending["readiness"]["observedWork"]["pendingBatchItems"], 1);
    let parent = pending["data"]["batchOperationId"].clone();
    provider.unavailable.store(false, Ordering::SeqCst);
    let (status, done) = http(&a, &body(&r)).await;
    assert_eq!(status, 200, "{done}");
    assert_eq!(done["data"]["batchOperationId"], parent);
    assert_eq!(done["data"]["outcomes"][0]["status"], "ok");
    assert_eq!(done["readiness"]["ready"], true);
    assert_eq!(done["readiness"]["pendingWork"], 0);
    assert!(done["readiness"].get("pendingWorkScope").is_none());
    let current = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    assert_eq!(done["readiness"]["committedGeneration"], json!(current));
    assert!(
        done["readiness"]["derived"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["appliedGeneration"] == json!(current) && v["ready"] == true)
    );
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
    // Corrupt only after reservation publication and the real provider call;
    // corrupting before reservation correctly prevents any child commitment.
    *provider.corrupt.lock().unwrap() = Some(f.vector_index.clone());
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
    assert_eq!(unready["readiness"]["ready"], false);
    assert_eq!(unready["readiness"]["pendingWorkScope"], "acceptedTickets");
    assert_eq!(unready["readiness"]["pendingWork"], 0);
    assert!(
        unready["readiness"]["observedWork"]["pendingEffects"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        unready["readiness"]["derived"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["appliedGeneration"].is_null())
    );
    assert_ne!(
        item["receipt"]["operationId"],
        unready["data"]["batchOperationId"]
    );
    let calls = provider.calls.load(Ordering::SeqCst);
    let ticks = clock.0.load(Ordering::SeqCst);
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    let (status, recovered) = http(&a, &body(&r)).await;
    assert_eq!(status, 200, "{recovered}");
    assert_eq!(recovered["readiness"]["ready"], true);
    let current = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    assert_eq!(
        recovered["readiness"]["committedGeneration"],
        json!(current)
    );
    assert!(
        recovered["readiness"]["derived"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["appliedGeneration"] == json!(current) && v["ready"] == true)
    );
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

#[tokio::test]
async fn batch_wire_final_barrier_failure_after_children_preserves_receipts_and_repairs_same_key() {
    for fault in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let d = fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let session = open(&d, clock.clone());
        let mut r = request(&d, session.metadata().control_state().unwrap().store_id);
        r.items = vec![json!({"summary":"final publication boundary","checkDuplicates":false})];
        let f = Fixture::from_open_engine(root, session.engine);
        let provider = Arc::new(RecoveringProvider {
            profile: d.profile.clone(),
            calls: AtomicUsize::new(0),
            profiles: AtomicUsize::new(0),
            unavailable: false.into(),
            corrupt: std::sync::Mutex::new(None),
        });
        let a = Arc::new(adapter(
            &f,
            provider.clone(),
            clock.clone(),
            Default::default(),
        ));
        a.fail_native_batch_after_children
            .store(fault, Ordering::SeqCst);
        let failed = if fault == 1 {
            let (status, failed) = http(&a, &body(&r)).await;
            assert_eq!(status, 207, "{failed}");
            failed
        } else {
            let (is_error, failed) = mcp(&bridge(a.clone()), body(&r)).await;
            assert!(is_error, "{failed}");
            failed
        };
        assert_eq!(failed["status"], "partial");
        assert_eq!(a.fail_native_batch_after_children.load(Ordering::SeqCst), 0);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .status()
                .pending,
            0
        );
        assert_eq!(failed["data"]["succeeded"], "1");
        let item = &failed["data"]["outcomes"][0];
        assert_eq!(item["status"], "ok");
        assert_eq!(item["ready"], false);
        assert_eq!(item["error"]["commitState"], "committed");
        let receipt: WriteReceipt = serde_json::from_value(item["receipt"].clone()).unwrap();
        assert_eq!(item["receipt"], canonical(&receipt));
        if fault == 1 {
            assert_eq!(failed["readiness"]["ready"], false);
            assert_eq!(failed["readiness"]["pendingWork"], 0);
            assert_eq!(failed["readiness"]["pendingWorkScope"], "acceptedTickets");
            assert!(
                failed["readiness"]["derived"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["name"] == "fts" && v["appliedGeneration"].is_null())
            );
        } else {
            assert!(failed.get("readiness").is_none());
            assert_eq!(failed["warnings"], json!(["READINESS_UNKNOWN"]));
        }
        let calls = provider.calls.load(Ordering::SeqCst);
        let profiles = provider.profiles.load(Ordering::SeqCst);
        let ticks = clock.0.load(Ordering::SeqCst);
        assert_eq!(calls, 1);
        drop(a);
        let root = f.close();
        let session = open(&d, clock.clone());
        let f = Fixture::from_open_engine(root, session.engine);
        let before = readers::bytes(&d.path);
        let a = Arc::new(adapter(
            &f,
            provider.clone(),
            clock.clone(),
            Default::default(),
        ));
        let (is_error, repaired) = mcp(&bridge(a), body(&r)).await;
        assert!(!is_error, "{repaired}");
        assert_eq!(repaired["readiness"]["ready"], true);
        assert_eq!(repaired["data"]["outcomes"][0]["ready"], true);
        let mut expected = receipt;
        expected.replayed = true;
        assert_eq!(
            repaired["data"]["outcomes"][0]["receipt"],
            canonical(&expected)
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), calls);
        assert_eq!(provider.profiles.load(Ordering::SeqCst), profiles);
        assert_eq!(clock.0.load(Ordering::SeqCst), ticks);
        assert_eq!(readers::bytes(&d.path), before);
    }
}
