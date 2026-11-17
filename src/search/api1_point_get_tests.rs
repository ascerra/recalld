// Real ordinary get owners; included in the existing isolated read-view fixture.
fn ordinary_adapter(
    f: &Fixture,
    engine: Arc<QueryEngine>,
    clock: Arc<dyn crate::time::Clock>,
    timezone: chrono_tz::Tz,
) -> Arc<crate::mcp::bridge_adapters::McpStorageAdapter> {
    Arc::new(
        crate::mcp::bridge_adapters::McpStorageAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            Arc::new(crate::embedding::PassthroughProvider::new(4)),
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
            f.graph.clone(),
            Arc::new(crate::config::RecalldConfig::default()),
            timezone,
        )
        .with_clock(clock)
        .with_query_engine(engine),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_point_native_receipt_and_owning_timezone_survive_actual_rpc() {
    use crate::mcp::bridge::StorageEngine as _;
    use crate::serialization::get_api1::{Reply, Request};
    for remote in [false, true] {
        let f = similar_fixture().await;
        let engine = Arc::new(f.engine);
        let timezone = chrono_tz::Asia::Kathmandu;
        let adapter = ordinary_adapter(&f.f, engine, f.clock.clone(), timezone);
        let before = f.f.record(f.foreign).unwrap();
        let n = before.access_history.len();
        f.clock.0.store(0, Ordering::SeqCst);
        let root = tempfile::tempdir().unwrap();
        let (target, task): (Arc<dyn crate::mcp::bridge::StorageEngine>, _) = if remote {
            let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
                adapter.clone(),
                &root.path().join("ordinary.sock"),
            )
            .await;
            (Arc::new(remote), Some(task))
        } else {
            (adapter.clone(), None)
        };
        let reply = target
            .get_api1(
                Request {
                    id: f.foreign,
                    namespace: Some("other".into()),
                    record_access: true,
                },
                crate::daemon::api1_get::response_limit(),
            )
            .await;
        let Reply::Point {
            reply,
            timezone: actual_timezone,
            native,
        } = reply
        else {
            panic!("actual owned reply")
        };
        assert!(native);
        assert_eq!(actual_timezone, timezone);
        assert_eq!(reply.http_status, 200, "{reply:?}");
        assert_eq!(f.clock.0.load(Ordering::SeqCst), 1);
        assert_eq!(f.f.record(f.foreign).unwrap().access_history.len(), n + 1);
        let receipt = reply.envelope.receipt.as_ref().unwrap();
        assert_eq!(reply.observation_receipts, vec![receipt.clone()]);
        assert_eq!(
            f.f.storage
                .read()
                .unwrap()
                .lookup_operation(
                    receipt.store_id,
                    crate::storage::control::MutationScope::Namespace(f.other),
                    receipt.operation_id
                )
                .unwrap(),
            Some(receipt.clone())
        );
        assert!(reply.envelope.readiness.as_ref().unwrap().ready);
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.storage = target;
        bridge.timezone = chrono_tz::America::Los_Angeles;
        let snapshot = bytes(&f.f);
        let result = crate::mcp::tools::dispatch_tool(
            &bridge,
            "get_memory",
            serde_json::json!({"id":f.foreign.to_string(),"recordAccess":false}),
        )
        .await;
        let value = result.structured_content.unwrap();
        assert_eq!(
            value["createdAt"],
            crate::time::format_timestamp(before.created_at, timezone)
        );
        assert_ne!(
            value["createdAt"],
            crate::time::format_timestamp(before.created_at, bridge.timezone)
        );
        assert!(!value.contains_key("receipt") && !value.contains_key("contractVersion"));
        assert_eq!(bytes(&f.f), snapshot);
        drop(bridge);
        drop(adapter);
        if let Some(task) = task {
            tokio::time::timeout(std::time::Duration::from_secs(3), task)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_point_native_unknown_mcp_rpc_keeps_operation_and_pending_root() {
    let f = similar_fixture().await;
    let count = f.f.record(f.foreign).unwrap().access_history.len();
    let store =
        f.f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .store_id;
    let adapter = ordinary_adapter(&f.f, Arc::new(f.engine), f.clock.clone(), chrono_tz::UTC);
    let tracker = f.f.storage.read().unwrap().meta_store().work_tracker();
    f.f.storage
        .read()
        .unwrap()
        .meta_store()
        .observation_fault
        .store(3, Ordering::SeqCst);
    let root = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        adapter.clone(),
        &root.path().join("unknown.sock"),
    )
    .await;
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = Arc::new(remote);
    let public = crate::mcp::tools::dispatch_tool(
        &bridge,
        "get_memory",
        serde_json::json!({"id":f.foreign.to_string()}),
    )
    .await;
    assert_eq!(public.is_error, Some(true));
    let value = public.structured_content.unwrap();
    assert_eq!(value["commitState"], "unknown");
    assert!(
        !value.contains_key("receipt")
            && !value.contains_key("data")
            && !value.contains_key("receivedEvidence")
    );
    let operation = uuid::Uuid::parse_str(value["operationId"].as_str().unwrap()).unwrap();
    assert!(tracker.status().unresolved > 0);
    drop(bridge);
    drop(adapter);
    tokio::time::timeout(std::time::Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    drop(f.owner);
    let data_root = f.f.close();
    let reopened = crate::storage::RedbStorageEngine::open_native(data_root.path()).unwrap();
    let receipt = reopened
        .lookup_operation(
            store,
            crate::storage::control::MutationScope::Namespace(f.other),
            operation,
        )
        .unwrap()
        .unwrap();
    assert_eq!(receipt.operation_id, operation);
    assert_eq!(
        reopened
            .get_record(f.foreign)
            .unwrap()
            .unwrap()
            .access_history
            .len(),
        count + 1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_point_native_old_alias_and_preobserve_budget_do_not_write() {
    use crate::mcp::bridge::StorageEngine as _;
    use crate::serialization::get_api1::{Reply, Request};
    let f = similar_fixture().await;
    let adapter = ordinary_adapter(&f.f, Arc::new(f.engine), f.clock.clone(), chrono_tz::UTC);
    f.clock.0.store(0, Ordering::SeqCst);
    let before = bytes(&f.f);
    for access in [false, true] {
        assert!(adapter.get_memory(f.foreign, access, None).await.is_err());
    }
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(bytes(&f.f), before);
    let reply = adapter
        .get_api1(
            Request {
                id: f.foreign,
                namespace: None,
                record_access: true,
            },
            1024,
        )
        .await;
    let Reply::Point { reply, .. } = reply else {
        panic!()
    };
    assert_ne!(reply.http_status, 200);
    assert_eq!(
        reply.envelope.error.unwrap().commit_state,
        crate::serialization::contract::CommitState::NotCommitted
    );
    assert!(reply.observation_receipts.is_empty());
    assert_eq!(bytes(&f.f), before);
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_point_native_nil_scope_spelling_and_null_are_compatible() {
    let f = similar_fixture().await;
    let adapter = ordinary_adapter(&f.f, Arc::new(f.engine), f.clock.clone(), chrono_tz::UTC);
    let root = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        adapter,
        &root.path().join("missing.sock"),
    )
    .await;
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = Arc::new(remote);
    bridge.default_namespace = "wrong-session".into();
    let before = bytes(&f.f);
    for scoped in [false, true] {
        for id in [
            uuid::Uuid::nil().to_string(),
            uuid::Uuid::from_u128(0xaaaabbbbccccdddd)
                .to_string()
                .to_uppercase(),
        ] {
            let mut args = serde_json::json!({"id":id,"recordAccess":false});
            if scoped {
                args["namespace"] = serde_json::json!("other");
            }
            let result = crate::mcp::tools::dispatch_tool(&bridge, "get_memory", args).await;
            let crate::mcp::protocol::ContentBlock::Text { text, .. } = &result.content[0] else {
                panic!()
            };
            assert!(text.contains(&id));
            assert!(
                text.starts_with(if scoped {
                    "Failed to get memory:"
                } else {
                    "Memory not found:"
                }),
                "{text}"
            );
        }
    }
    f.clock.0.store(0, Ordering::SeqCst);
    let result = crate::mcp::tools::dispatch_tool(
        &bridge,
        "get_memory",
        serde_json::json!({"id":f.foreign.to_string(),"namespace":null,"recordAccess":false}),
    )
    .await;
    assert_eq!(result.is_error, Some(true));
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 0);
    let result = crate::mcp::tools::dispatch_tool(
        &bridge,
        "get_memory",
        serde_json::json!({"id":f.foreign.to_string(),"namespace":"default","recordAccess":false}),
    )
    .await;
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap()["error"],
        "NAMESPACE_MISMATCH"
    );
    assert_eq!(bytes(&f.f), before);
    drop(bridge);
    tokio::time::timeout(std::time::Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_point_native_cancelled_graph_wait_retains_unresolved_work() {
    let f = similar_fixture().await;
    let adapter = ordinary_adapter(&f.f, Arc::new(f.engine), f.clock.clone(), chrono_tz::UTC);
    let tracker = f.f.storage.read().unwrap().meta_store().work_tracker();
    let count = f.f.record(f.foreign).unwrap().access_history.len();
    let lease = f.f.graph.write().await;
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.storage = adapter;
    let id = f.foreign;
    let work = tokio::spawn(async move {
        crate::mcp::tools::dispatch_tool(
            &bridge,
            "get_memory",
            serde_json::json!({"id":id.to_string()}),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while tracker.status().pending == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    work.abort();
    let _ = work.await;
    drop(lease);
    assert!(tracker.status().unresolved > 0);
    assert_eq!(f.f.record(f.foreign).unwrap().access_history.len(), count);
}
