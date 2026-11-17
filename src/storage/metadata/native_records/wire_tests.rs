//! Prepared public handlers and actual private socket; public2 stays rejected.
use super::*;
use crate::mcp::bridge::StorageEngine as _;
use crate::serialization::{
    contract::CommitState,
    native_lifecycle::{Operation, parse},
};
use crate::storage::native_lifecycle::NativePolicyInputs;
use axum::{body::Body, http::Request};

async fn setup() -> (
    Fixture,
    Data,
    Arc<FixedClock>,
    McpBridge,
    crate::api::AppState,
) {
    setup_policy(LifecycleVersion::ExplicitFeedbackV2).await
}
// Only callers that create fresh native vectors opt into certification.
// Default lifecycle-only/history setups remain genuinely uncertified.
async fn setup_certified() -> (
    Fixture,
    Data,
    Arc<FixedClock>,
    McpBridge,
    crate::api::AppState,
) {
    // Select the real lifecycle policy and adoption first, then release every
    // owner before the offline remedy. Preserve this public operation clock.
    let (f, d, clock, bridge, state) = setup().await;
    drop((bridge, state));
    let root = f.close();
    let d = certified_positive_fixture::certify_positive(root.path(), d);
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    let (bridge, state) =
        configured_public_adapters(&f, clock.clone(), NativePolicyInputs::default());
    (f, d, clock, bridge, state)
}
async fn setup_policy(
    version: LifecycleVersion,
) -> (
    Fixture,
    Data,
    Arc<FixedClock>,
    McpBridge,
    crate::api::AppState,
) {
    let root = tempfile::tempdir().unwrap();
    let mut d = fixture(root.path());
    if version == LifecycleVersion::LegacyAccessV1 {
        // The real schema migration destination retains the legacy lifecycle.
        // No raw policy edits or fabricated proof receipts.
        d.path = root.path().join("schema");
        // This intended-positive public fixture selects a different real migration
        // destination. Adopt there explicitly; unknown-history cases opt out.
        crate::test_support::explicitly_adopt_inherited_policy(&d.path, d.ns, 1., 2000);
    }
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    let (bridge, state) = public_adapters(&f, clock.clone());
    (f, d, clock, bridge, state)
}
fn public_adapters(f: &Fixture, clock: Arc<dyn Clock>) -> (McpBridge, crate::api::AppState) {
    configured_public_adapters(f, clock, NativePolicyInputs::default())
}
fn configured_public_adapters(
    f: &Fixture,
    clock: Arc<dyn Clock>,
    policy: NativePolicyInputs,
) -> (McpBridge, crate::api::AppState) {
    public_adapters_with_query(f, clock, policy, false)
}
fn public_adapters_with_query(
    f: &Fixture,
    clock: Arc<dyn Clock>,
    policy: NativePolicyInputs,
    native_exposure: bool,
) -> (McpBridge, crate::api::AppState) {
    let (_, mut bridge, engine) =
        readers_with_policy(f, clock.clone(), native_exposure, policy.clone());
    let mut config = crate::config::RecalldConfig::default();
    config.decay.decay_rate_multiplier = policy.multiplier;
    let config = Arc::new(config);
    bridge.storage = Arc::new(
        McpStorageAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            Arc::new(NoProvider),
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
            f.graph.clone(),
            config.clone(),
            chrono_tz::UTC,
        )
        .with_clock(clock.clone())
        .with_native_lifecycle_policy(policy.clone())
        .with_query_engine(engine.clone()),
    );
    bridge.default_namespace = "deliberately-wrong".into();
    let state = crate::api::AppState::new(
        Arc::new(SearchPipelineAdapter::new(
            engine,
            Arc::new(NoProvider),
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
            .with_native_lifecycle_policy(policy.clone()),
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
            .with_clock(clock.clone())
            .with_native_lifecycle_policy(policy),
        ),
        Arc::new(NamespaceRegistryAdapter::new(
            f.storage.clone(),
            std::sync::Arc::new(crate::embedding::PassthroughProvider::new(
                crate::test_support::DIM,
            )),
        )),
        Arc::new(NoopMetricsCollector),
    );
    (bridge, state)
}
fn request(f: &Fixture, d: &Data, key: &str) -> Value {
    let control = f.storage.read().unwrap().control_state().unwrap();
    json!({"contractVersion":2,"id":d.parent,"storeId":control.store_id,"idempotencyKey":key,"expectedGeneration":control.committed_generation})
}
async fn prepared_http(
    state: crate::api::AppState,
    id: MemoryId,
    operation: Operation,
    mut value: Value,
) -> (u16, Value) {
    let object = value.as_object_mut().unwrap();
    object.remove("contractVersion");
    object.remove("id");
    // Exercise header-only key normalization and the actual HTTP body reader.
    let key = object.remove("idempotencyKey");
    let mut req = Request::builder()
        .method(if operation == Operation::Forget {
            "DELETE"
        } else {
            "POST"
        })
        .uri(format!("/memories/{id}"))
        .header("content-type", "application/json");
    if let Some(key) = key {
        req = req.header("idempotency-key", key.as_str().unwrap());
    }
    let response = crate::api::native_lifecycle::prepared(
        state,
        id.to_string(),
        req.body(Body::from(value.to_string())).unwrap(),
        operation,
    )
    .await;
    let status = response.status().as_u16();
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value)
}
async fn mcp(bridge: &McpBridge, operation: Operation, value: Value) -> Value {
    let result = crate::mcp::tools::dispatch_v2(
        bridge,
        if operation == Operation::Forget {
            "forget_memory"
        } else {
            "reinforce_memory"
        },
        value,
    )
    .await;
    let encoded = serde_json::to_value(result).unwrap();
    let text: Value =
        serde_json::from_str(encoded["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(encoded["structuredContent"], text);
    text
}
fn bytes(f: &Fixture, id: MemoryId) -> (Vec<u8>, crate::storage::control::Generation) {
    (
        f.record(id).unwrap().encode().unwrap(),
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
    )
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_wire_mcp_http_rpc_replay_cas_terminal_current_readiness() {
    let (f, d, clock, bridge, state) = setup().await;
    let original = request(&f, &d, "feedback");
    let before = f.record(d.parent).unwrap().access_history.len();
    let first = mcp(&bridge, Operation::Feedback, original.clone()).await;
    assert_eq!(first["status"], "ok", "{first}");
    assert_eq!(first["scope"]["namespace"], "native");
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let after = bytes(&f, d.parent);
    let (status, replay) = prepared_http(
        state.clone(),
        d.parent,
        Operation::Feedback,
        original.clone(),
    )
    .await;
    assert_eq!(status, 200, "{replay}");
    assert_eq!(
        replay["receipt"]["operationId"],
        first["receipt"]["operationId"]
    );
    assert_eq!(replay["receipt"]["replayed"], true);
    assert_eq!(after, bytes(&f, d.parent));
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let mut conflict = original.clone();
    conflict["quality"] = json!(4);
    let (status, error) =
        prepared_http(state.clone(), d.parent, Operation::Feedback, conflict).await;
    assert_eq!(status, 409);
    assert_eq!(error["error"]["code"], "IDEMPOTENCY_CONFLICT");
    let mut stale = original.clone();
    stale["idempotencyKey"] = json!("stale");
    let error = mcp(&bridge, Operation::Feedback, stale).await;
    assert_eq!(error["error"]["code"], "GENERATION_CONFLICT");
    assert_eq!(after, bytes(&f, d.parent));
    let forget_request = request(&f, &d, "forget");
    let (status, forgotten) = prepared_http(
        state.clone(),
        d.parent,
        Operation::Forget,
        forget_request.clone(),
    )
    .await;
    assert_eq!(status, 200, "{forgotten}");
    assert_eq!(f.record(d.parent).unwrap().phase, DecayPhase::Tombstone);
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        bridge.storage.clone(),
        &socket.path().join("wire.sock"),
    )
    .await;
    let replay = remote
        .feedback_native(parse(&original, Operation::Feedback, true, None).unwrap())
        .await;
    assert!(replay.error.is_none(), "{:?}", replay.error);
    assert!(replay.receipt.as_ref().unwrap().replayed);
    assert!(replay.data.as_ref().unwrap().historical);
    assert!(
        replay.readiness.as_ref().unwrap().committed_generation
            > replay.receipt.as_ref().unwrap().committed_generation
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), 2);
    assert_eq!(f.record(d.parent).unwrap().access_history.len(), before + 1);
    let error = mcp(
        &bridge,
        Operation::Feedback,
        request(&f, &d, "after-forget"),
    )
    .await;
    assert_eq!(error["error"]["code"], "CONTENT_UNAVAILABLE");
    let mut cross = original;
    cross.as_object_mut().unwrap().remove("quality");
    let error = mcp(&bridge, Operation::Forget, cross).await;
    assert_eq!(error["error"]["code"], "IDEMPOTENCY_CONFLICT");
    let forgotten_replay = remote
        .forget_native(parse(&forget_request, Operation::Forget, true, None).unwrap())
        .await;
    assert!(
        forgotten_replay.error.is_none(),
        "{:?}",
        forgotten_replay.error
    );
    assert!(forgotten_replay.receipt.as_ref().unwrap().replayed);
    assert_eq!(
        json!(forgotten_replay.receipt.unwrap().operation_id),
        forgotten["receipt"]["operationId"]
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), 2);
    drop(remote);
    task.await.unwrap();
    drop(state);
    drop(bridge);
    let _root = f.close();
    inspect_schema(&d.path).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_wire_strict_invalid_scope_and_public_gates_are_immutable() {
    use tower::ServiceExt;
    let (f, d, clock, bridge, state) = setup().await;
    let before = bytes(&f, d.parent);
    for (field, bad) in [
        ("quality", json!(null)),
        ("quality", json!("3")),
        ("quality", json!(3.5)),
        ("quality", json!(0)),
        ("quality", json!(5)),
        ("namespaceId", json!(0)),
        ("namespaceId", json!(4294967296u64)),
        ("namespaceId", json!("1")),
        ("namespace", json!(null)),
        ("expectedGeneration", json!("01")),
        ("expectedGeneration", json!(1)),
        ("unknown", json!(true)),
    ] {
        let mut value = request(&f, &d, "invalid");
        value[field] = bad;
        let out = mcp(&bridge, Operation::Feedback, value.clone()).await;
        assert_eq!(out["status"], "error", "{field}: {out}");
        let (status, _) = prepared_http(state.clone(), d.parent, Operation::Feedback, value).await;
        assert_eq!(status, 400, "{field}");
        assert_eq!(bytes(&f, d.parent), before);
    }
    for (field, bad) in [
        ("namespace", json!("other")),
        ("namespaceId", json!(d.ns.get() + 1)),
        ("storeId", json!(uuid::Uuid::new_v4())),
    ] {
        let mut value = request(&f, &d, "scope");
        value[field] = bad;
        let (status, _) = prepared_http(state.clone(), d.parent, Operation::Feedback, value).await;
        assert_eq!(status, 409, "{field}");
    }
    let result =
        crate::mcp::tools::dispatch_tool(&bridge, "reinforce_memory", request(&f, &d, "gate"))
            .await;
    assert_eq!(result.is_error, Some(true));
    let router = crate::api::router(state, &crate::api::ApiConfig::default());
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/memories/{}/reinforce", d.parent))
                .header("recalld-contract-version", "2")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(bytes(&f, d.parent), before);
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
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_wire_publication_failure_retains_receipt_and_replay_recovers() {
    let (f, d, clock, bridge, state) = setup().await;
    let original = request(&f, &d, "publication");
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    let (status, error) = prepared_http(
        state.clone(),
        d.parent,
        Operation::Feedback,
        original.clone(),
    )
    .await;
    assert_eq!(status, 503, "{error}");
    assert_eq!(error["error"]["commitState"], "committed");
    assert!(error["receipt"]["operationId"].is_string());
    assert!(error.get("readiness").is_none());
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    let retry = mcp(&bridge, Operation::Feedback, original).await;
    assert_eq!(retry["status"], "ok", "{retry}");
    assert_eq!(
        retry["receipt"]["operationId"],
        error["receipt"]["operationId"]
    );
    assert_eq!(retry["receipt"]["replayed"], true);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(retry["readiness"]["ready"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_wire_actual_unknown_commit_never_reports_not_committed() {
    struct FaultClock;
    impl Clock for FaultClock {
        fn now_millis(&self) -> i64 {
            crate::storage::metadata::native_lifecycle::FAIL_AFTER_NATIVE_COMMIT
                .with(|f| f.set(true));
            86_400_000
        }
    }
    let (f, d, _clock, bridge, state) = setup().await;
    let original = request(&f, &d, "unknown");
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(FaultClock))
        .with_native_lifecycle_policy(NativePolicyInputs::default());
    let reply = adapter
        .feedback_native(parse(&original, Operation::Feedback, true, None).unwrap())
        .await;
    let error = reply.error.unwrap();
    assert_eq!(error.commit_state, CommitState::Unknown);
    assert!(error.operation_id.is_some());
    assert!(reply.receipt.is_none());
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .status()
            .unresolved
            > 0
    );
    drop(adapter);
    drop(bridge);
    drop(state);
    let root = f.close();
    let reopened = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let f = Fixture::from_open_engine(root, reopened.engine);
    completion(&f).rebuild_all().await.unwrap();
    let replay = f
        .storage_adapter()
        .with_native_lifecycle_policy(NativePolicyInputs::default())
        .feedback_native(parse(&original, Operation::Feedback, true, None).unwrap())
        .await;
    assert!(replay.error.is_none(), "{:?}", replay.error);
    assert_eq!(
        Some(replay.receipt.unwrap().operation_id),
        error.operation_id
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_wire_retained_terminal_identity_replays_after_purge_reopen() {
    let (f, d, clock, bridge, state) = setup().await;
    let original = request(&f, &d, "retired");
    let first = mcp(&bridge, Operation::Feedback, original.clone()).await;
    assert_eq!(first["status"], "ok", "{first}");
    drop(bridge);
    drop(state);
    let root = f.close();
    let mut session = open(&d, clock.clone());
    let mut progress = session
        .engine
        .begin_namespace_purge(&ctx(&session, &d, "purge"), true)
        .unwrap();
    while progress.stage != crate::storage::purge::PurgeStage::Complete {
        progress = session
            .engine
            .resume_namespace_purge(
                progress.store_id,
                d.ns,
                progress.start_receipt.operation_id,
                1,
            )
            .unwrap();
    }
    drop(session);
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    let before = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    let calls = clock.0.load(Ordering::SeqCst);
    let adapter = f
        .storage_adapter()
        .with_clock(clock.clone())
        .with_native_lifecycle_policy(NativePolicyInputs {
            multiplier: f64::NAN,
            ..Default::default()
        });
    for explicit in [false, true] {
        let mut value = original.clone();
        value["namespace"] = json!("native");
        if explicit {
            value["namespaceId"] = json!(d.ns);
        }
        let reply = adapter
            .feedback_native(parse(&value, Operation::Feedback, true, None).unwrap())
            .await;
        assert!(reply.error.is_none(), "{:?}", reply.error);
        assert!(reply.receipt.as_ref().unwrap().replayed);
        assert_eq!(reply.scope.unwrap(), (d.ns, "native".into()));
        assert_eq!(
            json!(reply.receipt.unwrap().operation_id),
            first["receipt"]["operationId"]
        );
    }
    let mut wrong = original;
    wrong["namespaceId"] = json!(d.ns);
    wrong["namespace"] = json!("reused-or-wrong");
    let failure = adapter
        .feedback_native(parse(&wrong, Operation::Feedback, true, None).unwrap())
        .await;
    assert_eq!(failure.error.as_ref().unwrap().code, "NAMESPACE_MISMATCH");
    assert_eq!(
        failure.error.as_ref().unwrap().commit_state,
        CommitState::Committed
    );
    assert!(failure.receipt.is_some());
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        before
    );
    assert!(f.record(d.parent).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_wire_old_rpc_unknown_method_fails_closed_without_legacy_fallback() {
    use crate::daemon::{DaemonClient, DaemonResponse, DaemonRpcError, protocol::*};
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("old.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        for method in ["feedback_native_v1", "forget_native_v1"] {
            let value: crate::daemon::DaemonRequest =
                read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
            assert_eq!(
                value.method, method,
                "a native operation fell back to a legacy mutating RPC"
            );
            // Exact old-handler unknown-method rejection, through real framing.
            let response = DaemonResponse {
                jsonrpc: "2.0".into(),
                id: value.id,
                result: None,
                error: Some(DaemonRpcError {
                    code: ERR_INVALID_INPUT,
                    message: format!("unknown method: {}", value.method),
                }),
            };
            write_framed_message(&mut stream, &response, MAX_MESSAGE_SIZE)
                .await
                .unwrap();
        }
    });
    let remote = crate::daemon::bridge_adapters::RemoteStorageAdapter::new(Arc::new(
        DaemonClient::connect(&socket).await.unwrap(),
    ));
    let value = json!({"contractVersion":2,"id":uuid::Uuid::new_v4()});
    for operation in [Operation::Feedback, Operation::Forget] {
        let request =
            crate::serialization::native_lifecycle::parse_private(&value, operation).unwrap();
        let reply = if operation == Operation::Feedback {
            remote.feedback_native(request).await
        } else {
            remote.forget_native(request).await
        };
        assert!(reply.receipt.is_none());
        assert_eq!(reply.error.unwrap().commit_state, CommitState::NotCommitted);
    }
    task.await.unwrap();
}

#[path = "origin_tests.rs"]
mod origin;

#[path = "forget_origin_tests.rs"]
mod forget_origin;

fn decoder_native_response_matrix(
    request: crate::serialization::native_lifecycle::Request,
    success: crate::serialization::native_lifecycle::Reply,
    operation: Operation,
    mismatched: crate::serialization::native_lifecycle::Request,
) {
    use crate::serialization::native_lifecycle::Reply;
    let receipt = success.receipt.as_ref().unwrap().clone();
    let error = Reply::storage(crate::storage::StorageError::CommittedMutation {
        receipt: Box::new(receipt.clone()),
        source: Box::new(crate::storage::StorageError::RecoveryRequired(
            "synthetic peer publication error".into(),
        )),
    });
    let success = serde_json::to_value(success).unwrap();
    let mut malformed = success.clone();
    malformed["unexpectedField"] = json!(true);
    let mut failures = Vec::new();
    for (name, value) in [
        ("success", success),
        ("error", serde_json::to_value(error).unwrap()),
        ("malformed", malformed),
    ] {
        let matching = Reply::decode_response(value.clone(), &request, operation);
        assert_eq!(matching.receipt.as_ref().unwrap(), &receipt);
        if name != "success" {
            assert_eq!(matching.error.unwrap().commit_state, CommitState::Committed);
        } else {
            assert!(matching.error.is_none());
        }
        let wrong = Reply::decode_response(value, &mismatched, operation);
        if wrong.receipt.is_some()
            || wrong
                .error
                .as_ref()
                .is_none_or(|e| e.commit_state != CommitState::Unknown)
        {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "mismatched native receipt accepted by {failures:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decoder_native_feedback_quality_binds_success_error_and_malformed_receipts() {
    let (f, d, clock, bridge, _state) = setup().await;
    let request = parse(
        &request(&f, &d, "decoder-quality"),
        Operation::Feedback,
        true,
        None,
    )
    .unwrap();
    let reply = bridge.storage.feedback_native(request.clone()).await;
    let before = bytes(&f, d.parent);
    let calls = clock.0.load(Ordering::SeqCst);
    let mut wrong = request.clone();
    wrong.quality = 4;
    assert_ne!(wrong.quality, request.quality);
    decoder_native_response_matrix(request, reply, Operation::Feedback, wrong);
    assert_eq!(bytes(&f, d.parent), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decoder_native_namespace_binds_success_error_and_malformed_receipts() {
    let (f, d, clock, bridge, _state) = setup().await;
    for operation in [Operation::Feedback, Operation::Forget] {
        let mut request = parse(
            &request(
                &f,
                &d,
                if operation == Operation::Feedback {
                    "decoder-scope-feedback"
                } else {
                    "decoder-scope-forget"
                },
            ),
            operation,
            true,
            None,
        )
        .unwrap();
        let reply = if operation == Operation::Feedback {
            bridge.storage.feedback_native(request.clone()).await
        } else {
            bridge.storage.forget_native(request.clone()).await
        };
        request.namespace_id = Some(reply.scope.as_ref().unwrap().0);
        let before = bytes(&f, d.parent);
        let calls = clock.0.load(Ordering::SeqCst);
        let mut wrong = request.clone();
        wrong.namespace_id = Some(crate::model::NamespaceId::new(
            request.namespace_id.unwrap().get() + 1,
        ));
        decoder_native_response_matrix(request, reply, operation, wrong);
        assert_eq!(bytes(&f, d.parent), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decoder_native_committed_requires_receipt_but_unknown_can_keep_allocated_id() {
    use crate::serialization::native_lifecycle::Reply;
    let (f, d, clock, bridge, _state) = setup().await;
    let request = parse(
        &request(&f, &d, "decoder-state"),
        Operation::Feedback,
        true,
        None,
    )
    .unwrap();
    let receipt = bridge
        .storage
        .feedback_native(request.clone())
        .await
        .receipt
        .unwrap();
    let before = bytes(&f, d.parent);
    let calls = clock.0.load(Ordering::SeqCst);
    let error = Reply::storage(crate::storage::StorageError::CommittedMutation {
        receipt: Box::new(receipt.clone()),
        source: Box::new(crate::storage::StorageError::RecoveryRequired(
            "synthetic peer failure".into(),
        )),
    });
    let mut missing = serde_json::to_value(error).unwrap();
    missing["receipt"] = Value::Null;
    let mut unknown = missing.clone();
    unknown["error"]["commitState"] = json!("unknown");
    let decoded = Reply::decode_response(unknown, &request, Operation::Feedback);
    let error = decoded.error.unwrap();
    assert_eq!(error.commit_state, CommitState::Unknown);
    assert_eq!(error.operation_id, Some(receipt.operation_id));
    assert_ne!(error.code, "INVALID_RESPONSE");
    assert!(decoded.receipt.is_none());
    let mut not_committed = missing.clone();
    not_committed["error"]["commitState"] = json!("notCommitted");
    not_committed["error"]["operationId"] = Value::Null;
    let decoded = Reply::decode_response(not_committed, &request, Operation::Feedback);
    assert_eq!(
        decoded.error.as_ref().unwrap().commit_state,
        CommitState::NotCommitted
    );
    assert!(decoded.error.unwrap().operation_id.is_none());
    assert!(decoded.receipt.is_none());
    let decoded = Reply::decode_response(missing, &request, Operation::Feedback);
    let error = decoded.error.unwrap();
    assert_eq!(
        error.commit_state,
        CommitState::Unknown,
        "receipt-free committed claim is malformed"
    );
    assert_eq!(error.code, "INVALID_RESPONSE");
    assert!(decoded.receipt.is_none());
    assert_eq!(bytes(&f, d.parent), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
}

#[path = "public_api1_tests.rs"]
mod public_api1;

#[path = "public_matrix_tests.rs"]
mod public_matrix;

#[path = "adopted_public_tests.rs"]
mod adopted_public;

#[path = "query_feedback_chronology_tests.rs"]
mod query_feedback_chronology;
