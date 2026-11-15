//! Public API1 handlers, not prepared/private-only dispatch.
use super::*;
use tower::ServiceExt;

async fn public_action(operation: Operation, mcp: bool) {
    let (f, d, clock, bridge, state) = setup().await;
    let id = d.parent;
    if mcp {
        let name = if operation == Operation::Feedback {
            "reinforce_memory"
        } else {
            "forget_memory"
        };
        let result = crate::mcp::tools::dispatch_tool(&bridge, name, json!({"id":id})).await;
        assert_ne!(
            result.is_error,
            Some(true),
            "public native API1 {name}: {result:?}"
        );
    } else {
        let router = crate::api::router(state.clone(), &Default::default());
        let (method, uri, body) = if operation == Operation::Feedback {
            ("POST", format!("/memories/{id}/reinforce"), "{}")
        } else {
            ("DELETE", format!("/memories/{id}"), "")
        };
        let response = router
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            status.as_u16(),
            200,
            "public native API1: {}",
            String::from_utf8_lossy(&body)
        );
    }
    let record = f.record(id).unwrap();
    if operation == Operation::Forget {
        assert_eq!(record.phase, DecayPhase::Tombstone);
    } else {
        assert!(record.schema.unwrap().lifecycle.reinforced_at.is_some());
    }
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
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
    drop((bridge, state));
    let root = f.close();
    let reopened = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    drop((reopened, root));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_mcp_feedback() {
    public_action(Operation::Feedback, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_mcp_forget() {
    public_action(Operation::Forget, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_http_feedback() {
    public_action(Operation::Feedback, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_http_forget() {
    public_action(Operation::Forget, false).await;
}

async fn public_http(state: crate::api::AppState, id: MemoryId, op: Operation) -> (u16, Value) {
    let (method, uri, body) = if op == Operation::Feedback {
        ("POST", format!("/memories/{id}/reinforce"), "{}")
    } else {
        ("DELETE", format!("/memories/{id}"), "")
    };
    let response = crate::api::router(state, &Default::default())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    (
        response.status().as_u16(),
        serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_failures_keep_receipts_and_unknown_identity() {
    for operation in [Operation::Feedback, Operation::Forget] {
        for http in [false, true] {
            let (f, d, clock, bridge, state) = setup().await;
            if operation == Operation::Forget {
                let mut engine = f.storage.write().unwrap();
                let policy = NativePolicyInputs::default();
                let intent = engine
                    .meta_store()
                    .capture_native_insert(scope(&d), &input(&d), &policy)
                    .unwrap();
                engine
                    .insert_native_record(
                        &MutationContext::unkeyed(d.ns),
                        &intent,
                        &generated(),
                        &policy,
                        &FixedClock(AtomicUsize::new(0)),
                        None,
                    )
                    .unwrap();
                drop(engine);
                completion(&f).rebuild_all().await.unwrap();
            }
            *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
            let reply = if http {
                let (status, body) = public_http(state.clone(), d.parent, operation).await;
                assert_eq!(status, 503, "{body}");
                body
            } else {
                let tool = if operation == Operation::Feedback {
                    "reinforce_memory"
                } else {
                    "forget_memory"
                };
                let result =
                    crate::mcp::tools::dispatch_tool(&bridge, tool, json!({"id":d.parent})).await;
                assert_eq!(result.is_error, Some(true));
                let raw = serde_json::to_value(result).unwrap();
                serde_json::from_str(raw["content"][0]["text"].as_str().unwrap()).unwrap()
            };
            // API1 HTTP wraps errors; MCP exposes the lifecycle-owned envelope.
            let receipt = reply
                .get("receipt")
                .or_else(|| reply.pointer("/error/receipt"))
                .expect("full known receipt");
            let receipt: crate::storage::control::WriteReceipt =
                serde_json::from_value(receipt.clone()).unwrap();
            assert!(!receipt.operation_id.is_nil());
            assert_eq!(clock.0.load(Ordering::SeqCst), 1);
            assert!(reply.to_string().contains("committed"));
            assert!(reply["readiness"].is_null());
            *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
            completion(&f).complete().await.unwrap();
            drop((bridge, state));
            let root = f.close();
            let reopened = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
            drop((reopened, root));
        }
    }
    struct FaultClock;
    impl Clock for FaultClock {
        fn now_millis(&self) -> i64 {
            crate::storage::metadata::native_lifecycle::FAIL_AFTER_NATIVE_COMMIT
                .with(|v| v.set(true));
            86_400_000
        }
    }
    for http in [false, true] {
        let (f, d, _clock, mut bridge, mut state) = setup().await;
        bridge.storage = Arc::new(
            f.storage_adapter()
                .with_clock(Arc::new(FaultClock))
                .with_native_lifecycle_policy(Default::default()),
        );
        state.decay = Arc::new(
            FsrsEngineAdapter::new(
                f.storage.clone(),
                f.cache.clone(),
                f.graph.clone(),
                Arc::new(Default::default()),
                false,
            )
            .with_indexes(
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
            )
            .with_clock(Arc::new(FaultClock))
            .with_native_lifecycle_policy(Default::default()),
        );
        if http {
            let (status, reply) = public_http(state, d.parent, Operation::Feedback).await;
            assert_eq!(status, 503, "{reply}");
            assert_eq!(reply["observation"]["commitState"], "unknown");
            assert!(
                reply["observation"]["unknownOperationId"].is_string(),
                "{reply}"
            );
            assert_eq!(reply["observation"]["receipts"], json!([]));
        } else {
            let result = crate::mcp::tools::dispatch_tool(
                &bridge,
                "reinforce_memory",
                json!({"id":d.parent}),
            )
            .await;
            assert_eq!(result.is_error, Some(true));
            let raw = serde_json::to_value(result).unwrap();
            let reply: Value =
                serde_json::from_str(raw["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(reply["error"]["commitState"], "unknown");
            assert!(reply["error"]["operationId"].is_string());
            assert!(reply["receipt"].is_null());
        }
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
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_remote_decode_and_terminal_no_resurrection() {
    use crate::serialization::lifecycle_api1 as api1;
    let (f, d, clock, bridge, state) = setup().await;
    // Actual canonical insertions build a retained fan of edges. Forget must
    // leave that graph intact without growing the tombstone receipt by degree.
    {
        let mut engine = f.storage.write().unwrap();
        let policy = NativePolicyInputs::default();
        let insert_clock = FixedClock(AtomicUsize::new(0));
        for _ in 0..32 {
            let intent = engine
                .meta_store()
                .capture_native_insert(scope(&d), &input(&d), &policy)
                .unwrap();
            engine
                .insert_native_record(
                    &MutationContext::unkeyed(d.ns),
                    &intent,
                    &generated(),
                    &policy,
                    &insert_clock,
                    None,
                )
                .unwrap();
        }
    }
    completion(&f).rebuild_all().await.unwrap();
    let retained_edges = f.storage.read().unwrap().load_all_edges().unwrap();
    assert_eq!(retained_edges.len(), 32);
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        bridge.storage.clone(),
        &socket.path().join("api1.sock"),
    )
    .await;
    let before_old = bytes(&f, d.parent);
    for method in ["reinforce_memory", "delete_memory"] {
        assert!(
            remote
                .test_client()
                .call(method, json!({"id":d.parent,"quality":4}))
                .await
                .is_err()
        );
        assert_eq!(
            before_old,
            bytes(&f, d.parent),
            "old native-incompatible alias rejects before mutation"
        );
    }
    let request = api1::request(d.parent, 4, Some("NATIVE"));
    let reply = remote
        .api1_lifecycle(request.clone(), Operation::Feedback)
        .await;
    assert!(
        matches!(reply.value, Some(api1::Value::Feedback { .. })),
        "{reply:?}"
    );
    // Full error envelope retains the real receipt while bounding diagnostics.
    let bounded = api1::Reply::failure(crate::serialization::native_lifecycle::Reply::storage(
        StorageError::CommittedMutation {
            receipt: Box::new(reply.lifecycle.as_ref().unwrap().receipt.clone().unwrap()),
            source: Box::new(StorageError::RecoveryRequired(
                "\u{0001}".repeat(2 * 1024 * 1024),
            )),
        },
    ));
    let frame =
        crate::daemon::DaemonResponse::success(u64::MAX, serde_json::to_value(&bounded).unwrap());
    assert!(serde_json::to_vec(&frame).unwrap().len() < 64 * 1024);
    assert_eq!(
        bounded.lifecycle.as_ref().unwrap().receipt,
        reply.lifecycle.as_ref().unwrap().receipt
    );
    let encoded = serde_json::to_value(&reply).unwrap();
    for mutate in [0, 1, 2, 3] {
        let mut corrupt = encoded.clone();
        match mutate {
            0 => corrupt["value"]["result"]["stability"] = json!(999.),
            1 => {
                corrupt["value"]["result"]["stability"] = json!(999.);
                corrupt["lifecycle"]["data"]["stability"] = json!(999.);
            }
            2 => corrupt["value"]["unknownExtension"] = json!(true),
            _ => corrupt["value"] = json!(null),
        }
        let bad = api1::Reply::decode(corrupt, &request, Operation::Feedback);
        assert!(bad.value.is_none());
        let evidence = bad.lifecycle.unwrap();
        assert_eq!(evidence.error.unwrap().commit_state, CommitState::Committed);
        assert_eq!(evidence.receipt, reply.lifecycle.as_ref().unwrap().receipt);
    }
    let request = api1::request(d.parent, 3, Some("NaTiVe"));
    let first = remote
        .api1_lifecycle(request.clone(), Operation::Forget)
        .await;
    assert!(
        matches!(first.value, Some(api1::Value::Forget { deleted: true })),
        "{first:?}"
    );
    let crate::storage::control::MutationOutcome::NativeMemoryTombstoned { transition } = &first
        .lifecycle
        .as_ref()
        .unwrap()
        .receipt
        .as_ref()
        .unwrap()
        .outcome
    else {
        panic!("wrong outcome")
    };
    assert_eq!(
        f.storage.read().unwrap().load_all_edges().unwrap(),
        retained_edges
    );
    assert_eq!(transition.records.len(), 1);
    assert!(
        transition.edges.is_empty(),
        "forget retains edges; no peer proof expansion"
    );
    assert!(serde_json::to_vec(&first).unwrap().len() < 32 * 1024);
    let repeated = remote
        .api1_lifecycle(request.clone(), Operation::Forget)
        .await;
    assert!(
        matches!(repeated.value, Some(api1::Value::Forget { deleted: false })),
        "{repeated:?}"
    );
    let before = bytes(&f, d.parent);
    let calls = clock.0.load(Ordering::SeqCst);
    let denied = remote.api1_lifecycle(request, Operation::Feedback).await;
    assert!(denied.value.is_none());
    assert_eq!(
        denied.lifecycle.unwrap().error.unwrap().code,
        "CONTENT_UNAVAILABLE"
    );
    assert_eq!(before, bytes(&f, d.parent));
    assert_eq!(calls, clock.0.load(Ordering::SeqCst));
    task.abort();
    let _ = task.await;
    drop((remote, bridge, state));
    let root = f.close();
    let reopened = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    drop((reopened, root));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_legacy_policy_keeps_original1_and_configured_clock() {
    use crate::serialization::lifecycle_api1 as api1;
    let (f, d, clock, bridge, state) = setup_policy(LifecycleVersion::LegacyAccessV1).await;
    assert_eq!(
        f.record(d.parent)
            .unwrap()
            .schema
            .unwrap()
            .lifecycle
            .lifecycle_version,
        LifecycleVersion::LegacyAccessV1
    );
    let receipt = bridge
        .storage
        .api1_lifecycle(api1::request(d.parent, 4, None), Operation::Feedback)
        .await
        .lifecycle
        .unwrap()
        .receipt
        .unwrap();
    let crate::storage::control::MutationOutcome::LifecycleFeedbackApplied { transition } =
        &receipt.outcome
    else {
        panic!("not native feedback")
    };
    assert_eq!(
        transition.lifecycle_version,
        LifecycleVersion::LegacyAccessV1
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let (status, body) = public_http(state.clone(), d.parent, Operation::Forget).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(clock.0.load(Ordering::SeqCst), 2);
    drop((bridge, state));
    let root = f.close();
    let reopened = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    drop((reopened, root));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_feedback_uses_configured_policy_and_original1_hash() {
    use redb::ReadableTable as _;
    let (f, d, clock, bridge, state) = setup().await;
    // This positive multiplier2 assertion now requires its real offline adoption.
    // Close every adapter/owner first; never rewrite a binding or receipt in place.
    drop((bridge, state));
    let root = f.close();
    crate::test_support::explicitly_adopt_inherited_policy(&d.path, d.ns, 2., 2000);
    let policy = NativePolicyInputs {
        multiplier: 2.,
        ..Default::default()
    };
    let session = NativeLifecycleSession::open(&d.path, policy.clone(), clock.clone()).unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    let (mut bridge, state) = configured_public_adapters(&f, clock.clone(), policy);
    bridge.storage = Arc::new(
        f.storage_adapter()
            .with_clock(clock.clone())
            .with_native_lifecycle_policy(NativePolicyInputs {
                multiplier: 2.,
                ..Default::default()
            }),
    );
    let result = crate::mcp::tools::dispatch_tool(
        &bridge,
        "reinforce_memory",
        json!({"id":d.parent,"quality":4,"ignoredLegacyField":true}),
    )
    .await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    {
        let engine = f.storage.read().unwrap();
        let read = engine.meta_store().db.begin_read().unwrap();
        let stored = read
            .open_table(crate::storage::control::RECEIPTS)
            .unwrap()
            .iter()
            .unwrap()
            .map(|r| {
                let (_, v) = r.unwrap();
                crate::storage::control::validate_receipt(v.value()).unwrap()
            })
            .find(|r| {
                matches!(
                    r.receipt.outcome,
                    MutationOutcome::LifecycleFeedbackApplied { .. }
                )
            })
            .unwrap();
        let MutationOutcome::LifecycleFeedbackApplied { transition } = &stored.receipt.outcome
        else {
            unreachable!()
        };
        assert_eq!(
            transition
                .feedback
                .as_ref()
                .unwrap()
                .decay_rate_multiplier
                .get(),
            2.
        );
        let semantic = crate::storage::control::encode(&(
            scope(&d),
            vec![(d.parent, AccessKind::ManualReinforcement)],
            Some(4u8),
        ))
        .unwrap();
        let ctx = MutationContext {
            store_id: None,
            scope: MutationScope::Namespace(d.ns),
            idempotency_key: None,
            expected_generation: None,
            contract_version: 1,
        };
        assert_eq!(
            stored.request_hash,
            crate::storage::control::request_hash(&ctx, "native_feedback", &[&semantic])
        );
    }
    drop((bridge, state));
    let root = f.close();
    let reopened = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    drop((reopened, root));
}

async fn worker_join_unknown(operation: Operation) {
    // Actual worker unwind at clock sampling, BEFORE canonical commit. This
    // proves conservative JoinError classification, not postcommit durability.
    struct PanicClock;
    impl Clock for PanicClock {
        fn now_millis(&self) -> i64 {
            panic!("injected precommit clock panic");
        }
    }
    let (f, d, _clock, mut bridge, _state) = setup().await;
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    bridge.storage = Arc::new(
        f.storage_adapter()
            .with_clock(Arc::new(PanicClock))
            .with_native_lifecycle_policy(Default::default()),
    );
    let tool = if operation == Operation::Feedback {
        "reinforce_memory"
    } else {
        "forget_memory"
    };
    let result = crate::mcp::tools::dispatch_tool(&bridge, tool, json!({"id":d.parent})).await;
    assert_eq!(result.is_error, Some(true));
    let wire = serde_json::to_value(result).unwrap();
    let reply: Value = serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(reply["error"]["commitState"], "unknown", "{reply}");
    assert!(reply["receipt"].is_null());
    assert!(reply["error"]["operationId"].is_null());
    assert!(reply["readiness"].is_null());
    assert!(tracker.status().unresolved > 0);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_worker_join_feedback_is_unknown() {
    worker_join_unknown(Operation::Feedback).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_public_native_worker_join_forget_is_unknown() {
    worker_join_unknown(Operation::Forget).await;
}
