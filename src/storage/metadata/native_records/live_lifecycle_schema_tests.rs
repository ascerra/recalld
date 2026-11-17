//! Actual owner receipts, then explicitly malformed typed peer specimens.
use super::*;
use crate::serialization::lifecycle_api1 as api1;
use crate::storage::control::{MutationOutcome, WriteReceipt};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_lifecycle_schema_retained_codec_is_not_canonical_proof() {
    for operation in [Operation::Feedback, Operation::Forget] {
        let (f, d, clock, bridge, state) = setup().await;
        let request = api1::request(d.parent, 3, None);
        let reply = bridge
            .storage
            .api1_lifecycle(request.clone(), operation)
            .await;
        assert!(reply.value.is_some(), "actual owner success: {reply:?}");
        let original = reply.lifecycle.as_ref().unwrap().receipt.clone().unwrap();
        let mut malformed = original.clone();
        match &mut malformed.outcome {
            MutationOutcome::LifecycleFeedbackApplied { transition } => {
                // FiniteF32 decodes these; canonical LifecycleState does not.
                // Actual typed f32 emitters can spell MAX beyond its exact f64
                // widening. The schema must accept the emitted JSON too.
                transition.records[0].before.stability = f32::MAX.try_into().unwrap();
                transition.records[0].after.difficulty = (-f32::MAX).try_into().unwrap();
                transition.feedback.as_mut().unwrap().decay_rate_multiplier =
                    (-3_f32).try_into().unwrap();
            }
            MutationOutcome::NativeMemoryTombstoned { transition } => {
                transition.records.push(transition.records[0].clone());
                transition
                    .edges
                    .push(crate::storage::native_records::NativeEdgeProof {
                        source: d.parent,
                        target: d.parent,
                        edge_type: crate::model::EdgeType::Associative,
                        weight_bits: u32::MAX,
                        auto_created: true,
                        created_at: u64::MAX,
                    });
                transition.vector_execution =
                    Some(crate::storage::vector_certification::VectorExecution {
                        certification: crate::storage::vector_certification::VectorCertification {
                            version: u32::MAX,
                            origin: crate::storage::control::GenerationRef {
                                store_id: original.store_id,
                                generation: original.committed_generation,
                            },
                            operation_id: original.operation_id,
                            inventory_root: [255; 32],
                            vector_count: u64::MAX,
                        },
                        source: crate::storage::vector_certification::VectorSource::Supplied {
                            profile_id: "authored-codec-only-not-a-certified-profile".into(),
                        },
                    });
            }
            other => panic!("unexpected actual owner outcome: {other:?}"),
        }
        let canonical = serde_json::to_string(&malformed).unwrap();
        let typed: WriteReceipt = serde_json::from_str(&canonical).unwrap();
        assert_eq!(serde_json::to_string(&typed).unwrap(), canonical);
        let mut peer = serde_json::to_value(reply).unwrap();
        peer["lifecycle"]["receipt"] = serde_json::to_value(&malformed).unwrap();
        // Force the real malformed-wrapper path without changing request identity.
        peer["unexpectedPeerField"] = json!(true);
        let before = bytes(&f, d.parent);
        let calls = clock.0.load(Ordering::SeqCst);
        let result =
            api1::Reply::decode(peer.clone(), &request, operation).mcp(&d.parent.to_string());
        assert_eq!(result.is_error, Some(true));
        let value = Value::Object(result.structured_content.unwrap());
        assert_eq!(value["error"]["code"], "INVALID_RESPONSE");
        assert_eq!(value["error"]["commitState"], "committed");
        assert_eq!(
            value["error"]["operationId"],
            original.operation_id.to_string()
        );
        assert!(value["data"].is_null() && value["readiness"].is_null());
        record_case(
            if operation == Operation::Feedback {
                "malformed-feedback-codec"
            } else {
                "malformed-forget-codec"
            },
            tool(operation),
            &value,
            "actual API1 decode and MCP projection of an authored malformed copy of a real owner receipt; not canonical proof",
        );
        let retained: WriteReceipt = serde_json::from_value(value["receipt"].clone()).unwrap();
        assert_eq!(serde_json::to_string(&retained).unwrap(), canonical);
        assert_eq!(bytes(&f, d.parent), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);

        if operation == Operation::Forget {
            // The optional proof's codec, unlike its semantic validator, rejects
            // present null and unknown enum values. Historical absence remains.
            for phase_rule in [Value::Null, json!("future_rule")] {
                let mut rejected = peer.clone();
                rejected["lifecycle"]["receipt"]["outcome"]["transition"]["phaseTimeRule"] =
                    phase_rule;
                let result =
                    api1::Reply::decode(rejected, &request, operation).mcp(&d.parent.to_string());
                let value = result.structured_content.unwrap();
                assert!(value["receipt"].is_null());
                assert_eq!(value["error"]["commitState"], "unknown");
                record_case(
                    "malformed-proof-unknown",
                    tool(operation),
                    &Value::Object(value),
                    "actual decode→MCP rejection of authored codec-invalid receipt; no canonical write",
                );
            }
            let mut absent = peer;
            absent["lifecycle"]["receipt"]["outcome"]["transition"]
                .as_object_mut()
                .unwrap()
                .remove("phaseTimeRule");
            let result =
                api1::Reply::decode(absent, &request, operation).mcp(&d.parent.to_string());
            assert!(result.structured_content.unwrap()["receipt"].is_object());
        }
        assert_eq!(bytes(&f, d.parent), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        drop((bridge, state));
    }
}

fn tool(operation: Operation) -> &'static str {
    if operation == Operation::Feedback {
        "reinforce_memory"
    } else {
        "forget_memory"
    }
}

fn record_case(label: &str, tool: &str, value: &Value, provenance: &str) {
    if let Some(root) = std::env::var_os("RECALLD_LIVE_LIFECYCLE_SCHEMA_FIXTURES") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        let schema = crate::mcp::tools::tool_definitions()
            .into_iter()
            .find(|t| t.name == tool)
            .unwrap()
            .output_schema
            .unwrap();
        std::fs::write(
            root.join(format!("{label}-{tool}.json")),
            serde_json::to_vec(&json!({
                "tool":tool,"schema":schema,"value":value,"fixtureProvenance":provenance
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

fn record_result(label: &str, name: &str, result: &Value, provenance: &str) -> Value {
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    record_case(&format!("{label}-text"), name, &text, provenance);
    if let Some(structured) = result.get("structuredContent") {
        record_case(&format!("{label}-structured"), name, structured, provenance);
        if let Some(receipt) = text.get("receipt").filter(|v| !v.is_null()) {
            let text_receipt: WriteReceipt = serde_json::from_value(receipt.clone()).unwrap();
            let structured_receipt: WriteReceipt =
                serde_json::from_value(structured["receipt"].clone()).unwrap();
            assert_eq!(
                serde_json::to_string(&text_receipt).unwrap(),
                serde_json::to_string(&structured_receipt).unwrap()
            );
        }
    }
    text
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_lifecycle_schema_actual_mcp_eras_and_owner_success_failures() {
    use crate::mcp::protocol::{LEGACY_PROTOCOL_VERSION, PROTOCOL_VERSION};
    use crate::mcp::server::{
        McpServer,
        tests::{initialize_request, modern_request, plain_request},
    };
    for format in ["legacy", "native-legacy", "native-explicit"] {
        for modern in [false, true] {
            let (f, native, mut bridge, state) = if format == "legacy" {
                let f = Fixture::new();
                let id = f.insert_memory().await;
                let mut bridge = crate::test_support::StubBridge::default().into_bridge();
                bridge.storage = Arc::new(f.storage_adapter());
                (f, (id, None), bridge, None)
            } else {
                let version = if format == "native-legacy" {
                    LifecycleVersion::LegacyAccessV1
                } else {
                    LifecycleVersion::ExplicitFeedbackV2
                };
                let (f, d, _, bridge, state) = setup_policy(version).await;
                let id = d.parent;
                (f, (id, Some(d)), bridge, Some(state))
            };
            bridge.default_namespace = "deliberately-wrong".into();
            let mut server = McpServer::new(Arc::new(bridge));
            let version = if modern {
                PROTOCOL_VERSION
            } else {
                LEGACY_PROTOCOL_VERSION
            };
            server
                .handle_message(initialize_request(1, version))
                .await
                .unwrap();
            let message = |id, method, args| {
                if modern {
                    modern_request(id, method, args)
                } else {
                    plain_request(id, method, Some(args))
                }
            };
            let catalog = server
                .handle_message(message(2, "tools/list", json!({})))
                .await
                .unwrap()
                .result
                .unwrap();
            for name in ["reinforce_memory", "forget_memory"] {
                let advertised = catalog["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|t| t["name"] == name)
                    .unwrap();
                assert_eq!(advertised.get("outputSchema").is_some(), modern);
                if modern {
                    let direct = crate::mcp::tools::tool_definitions()
                        .into_iter()
                        .find(|t| t.name == name)
                        .unwrap()
                        .output_schema
                        .unwrap();
                    assert_eq!(advertised["outputSchema"], direct);
                }
            }
            let id = native.0;
            let spelling = id.to_string().to_uppercase();
            for (label, name, args, error) in [
                (
                    "wrong-scope",
                    "reinforce_memory",
                    json!({"id":id,"namespace":"wrong"}),
                    true,
                ),
                (
                    "feedback",
                    "reinforce_memory",
                    json!({"id":id,"quality":4}),
                    false,
                ),
                ("forget", "forget_memory", json!({"id":spelling}), false),
                ("repeat", "forget_memory", json!({"id":spelling}), false),
                (
                    "terminal-feedback",
                    "reinforce_memory",
                    json!({"id":id}),
                    true,
                ),
                (
                    "missing",
                    "forget_memory",
                    json!({"id":MemoryId::nil()}),
                    false,
                ),
            ] {
                let result = server
                    .handle_message(message(
                        3,
                        "tools/call",
                        json!({"name":name,"arguments":args}),
                    ))
                    .await
                    .unwrap()
                    .result
                    .unwrap();
                assert_eq!(
                    result["isError"] == true,
                    error,
                    "{format}/{label}: {result}"
                );
                assert_eq!(result.get("structuredContent").is_some(), modern);
                let value = record_result(
                    &format!("{format}-{modern}-{label}"),
                    name,
                    &result,
                    "actual tools/list and tools/call on owned store; API1 native fixtures use explicit offline migration/adoption, not ordinary startup activation",
                );
                if label == "forget" || label == "repeat" {
                    assert_eq!(value["id"], spelling);
                }
                if label == "repeat" || label == "missing" {
                    assert_eq!(value["deleted"], false);
                }
            }
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
            drop((server, state));
            if let Some(d) = native.1 {
                let root = f.close();
                drop(open(&d, Arc::new(FixedClock(AtomicUsize::new(0)))));
                drop(root);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_lifecycle_schema_actual_native_publication_receipts_and_http_mcp() {
    use crate::mcp::protocol::PROTOCOL_VERSION;
    use crate::mcp::server::tests::modern_request;
    use tower::ServiceExt;
    for remote in [false, true] {
        for operation in [Operation::Feedback, Operation::Forget] {
            let (f, d, _, mut bridge, state) = setup_certified().await;
            // Retain a second actual record so both operations require vector publication.
            let policy = NativePolicyInputs::default();
            let intent = f
                .storage
                .read()
                .unwrap()
                .meta_store()
                .capture_native_insert(scope(&d), &input(&d), &policy)
                .unwrap();
            f.storage
                .write()
                .unwrap()
                .insert_native_record(
                    &MutationContext::unkeyed(d.ns),
                    &intent,
                    &generated(),
                    &policy,
                    &FixedClock(AtomicUsize::new(0)),
                    None,
                )
                .unwrap();
            completion(&f).rebuild_all().await.unwrap();
            *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
            let socket = tempfile::tempdir().unwrap();
            let task = if remote {
                let (adapter, task) = crate::daemon::server::native_lifecycle_test_remote(
                    bridge.storage.clone(),
                    &socket.path().join("lifecycle-schema.sock"),
                )
                .await;
                bridge.storage = Arc::new(adapter);
                Some(task)
            } else {
                None
            };
            let app = crate::mcp::http_transport::mcp_router(Arc::new(bridge), Default::default());
            let name = tool(operation);
            let mut result = Value::Null;
            for (method, params) in [
                ("tools/list", json!({})),
                (
                    "tools/call",
                    json!({"name":name,"arguments":{"id":d.parent}}),
                ),
            ] {
                let mut request = Request::post("/mcp")
                    .header("content-type", "application/json")
                    .header("MCP-Protocol-Version", PROTOCOL_VERSION)
                    .header("MCP-Method", method);
                if method == "tools/call" {
                    request = request.header("MCP-Name", name);
                }
                let response = app
                    .clone()
                    .oneshot(
                        request
                            .body(Body::from(
                                serde_json::to_vec(&modern_request(1, method, params)).unwrap(),
                            ))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status().as_u16(), 200);
                let wire: Value = serde_json::from_slice(
                    &axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
                        .await
                        .unwrap(),
                )
                .unwrap();
                assert!(wire.get("error").is_none(), "{wire}");
                if method == "tools/list" {
                    assert!(
                        wire["result"]["tools"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|t| t["name"] == name)
                            .unwrap()["outputSchema"]
                            .is_object()
                    );
                } else {
                    result = wire["result"].clone();
                }
            }
            assert_eq!(
                result["isError"], true,
                "actual publication failure: {result}"
            );
            let value = record_result(
                &format!("native-http-remote-{remote}-publication-failure"),
                name,
                &result,
                "actual HTTP MCP tools/list + tools/call, canonical commit followed by vector publication failure",
            );
            assert_eq!(value["error"]["commitState"], "committed");
            assert!(value["readiness"].is_null());
            let receipt: WriteReceipt = serde_json::from_value(value["receipt"].clone()).unwrap();
            let expected = serde_json::to_string(&receipt).unwrap();
            *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
            completion(&f).complete().await.unwrap();
            drop((app, state));
            if let Some(task) = task {
                tokio::time::timeout(std::time::Duration::from_secs(3), task)
                    .await
                    .unwrap()
                    .unwrap();
            }
            let root = f.close();
            let reopened = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
            let retained = reopened
                .engine
                .lookup_operation(
                    receipt.store_id,
                    crate::storage::control::MutationScope::Namespace(d.ns),
                    receipt.operation_id,
                )
                .unwrap()
                .unwrap();
            assert_eq!(serde_json::to_string(&retained).unwrap(), expected);
            drop((reopened, root));
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_lifecycle_schema_actual_legacy_known_receipts_and_native_unknown() {
    for operation in [Operation::Feedback, Operation::Forget] {
        let f = Fixture::new();
        let id = f.insert_memory().await;
        f.insert_memory().await;
        f.completion().rebuild_all().await.unwrap();
        *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.storage = Arc::new(f.storage_adapter());
        let name = tool(operation);
        let result = crate::mcp::tools::dispatch_tool(&bridge, name, json!({"id":id})).await;
        assert_eq!(result.is_error, Some(true), "{result:?}");
        let value = record_result(
            "legacy-publication-failure",
            name,
            &serde_json::to_value(result).unwrap(),
            "actual legacy owner canonical commit and vector-publication failure through MCP handler",
        );
        assert_eq!(value["error"]["commitState"], "committed");
        let receipt: WriteReceipt = serde_json::from_value(value["receipt"].clone()).unwrap();
        let expected = serde_json::to_string(&receipt).unwrap();
        *f.vector_index.write().await =
            crate::search::VectorIndexOwner::uniform(crate::test_support::DIM);
        f.completion().complete().await.unwrap();
        drop(bridge);
        let root = f.close();
        let reopened = RedbStorageEngine::open(root.path()).unwrap();
        let retained = reopened
            .lookup_operation(
                receipt.store_id,
                crate::storage::control::MutationScope::Namespace(NamespaceId::new(1)),
                receipt.operation_id,
            )
            .unwrap()
            .unwrap();
        assert_eq!(serde_json::to_string(&retained).unwrap(), expected);
        drop((reopened, root));
    }
    struct FaultClock;
    impl Clock for FaultClock {
        fn now_millis(&self) -> i64 {
            crate::storage::metadata::native_lifecycle::FAIL_AFTER_NATIVE_COMMIT
                .with(|flag| flag.set(true));
            86_400_000
        }
    }
    let (f, d, _, mut bridge, state) = setup().await;
    bridge.storage = Arc::new(
        f.storage_adapter()
            .with_clock(Arc::new(FaultClock))
            .with_native_lifecycle_policy(Default::default()),
    );
    let result =
        crate::mcp::tools::dispatch_tool(&bridge, "reinforce_memory", json!({"id":d.parent})).await;
    assert_eq!(result.is_error, Some(true));
    let value = record_result(
        "native-commit-unknown",
        "reinforce_memory",
        &serde_json::to_value(result).unwrap(),
        "actual native after-commit acknowledgement fault; unknown to caller, no fabricated receipt",
    );
    assert_eq!(value["error"]["commitState"], "unknown");
    assert!(value["receipt"].is_null() && value["readiness"].is_null());
    let operation: uuid::Uuid =
        serde_json::from_value(value["error"]["operationId"].clone()).unwrap();
    assert!(!operation.is_nil());
    drop((bridge, state));
    let root = f.close();
    let reopened = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    assert!(
        reopened
            .engine
            .lookup_operation(
                reopened.metadata().control_state().unwrap().store_id,
                crate::storage::control::MutationScope::Namespace(d.ns),
                operation
            )
            .unwrap()
            .is_some()
    );
    drop((reopened, root));
}
