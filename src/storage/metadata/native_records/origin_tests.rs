//! Original application version is independent of native/lifecycle schema.
use super::*;
use crate::serialization::native_lifecycle::parse_private;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_api_feedback_socket_preserves_one_two_replay_and_conflicts() {
    let (f, d, clock, bridge, state) = setup().await;
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        bridge.storage.clone(),
        &socket.path().join("origin.sock"),
    )
    .await;
    for origin in [1, 2] {
        let mut value = request(&f, &d, &format!("origin-{origin}"));
        value["contractVersion"] = json!(origin);
        let parsed = parse_private(&value, Operation::Feedback).unwrap();
        assert_eq!(parsed.original_contract_version, origin);
        assert_eq!(parsed.wire(Operation::Feedback)["contractVersion"], origin);
        let first = remote.feedback_native(parsed.clone()).await;
        assert!(first.error.is_none(), "{:?}", first.error);
        let receipt = first.receipt.unwrap();
        assert!(!receipt.replayed);
        assert!(first.readiness.unwrap().ready);
        let after = bytes(&f, d.parent);
        assert_eq!(clock.0.load(Ordering::SeqCst), origin as usize);
        // Original CAS is now stale. Retry must use original identity first.
        let replay = remote.feedback_native(parsed.clone()).await;
        assert!(replay.error.is_none(), "{:?}", replay.error);
        assert!(replay.receipt.as_ref().unwrap().replayed);
        assert_eq!(replay.receipt.unwrap().operation_id, receipt.operation_id);
        let mut cross = parsed.clone();
        cross.original_contract_version = 3 - origin;
        let error = remote.feedback_native(cross).await;
        assert_eq!(error.error.unwrap().code, "IDEMPOTENCY_CONFLICT");
        assert_eq!(bytes(&f, d.parent), after);
        assert_eq!(clock.0.load(Ordering::SeqCst), origin as usize);
        // Replayed original intent must not resolve changed/invalid fresh policy
        // or sample a regressed clock. NoProvider panics on any provider access.
        struct NeverClock;
        impl Clock for NeverClock {
            fn now_millis(&self) -> i64 {
                panic!("replay sampled clock")
            }
        }
        let replay = f
            .storage_adapter()
            .with_clock(Arc::new(NeverClock))
            .with_native_lifecycle_policy(NativePolicyInputs {
                multiplier: f64::NAN,
                ..Default::default()
            })
            .feedback_native(parsed)
            .await;
        assert!(replay.error.is_none(), "{:?}", replay.error);
        assert_eq!(replay.receipt.unwrap().operation_id, receipt.operation_id);
        assert_eq!(bytes(&f, d.parent), after);
    }
    let mut unkeyed = request(&f, &d, "unused");
    unkeyed["contractVersion"] = json!(1);
    unkeyed.as_object_mut().unwrap().remove("idempotencyKey");
    let reply = remote
        .feedback_native(parse_private(&unkeyed, Operation::Feedback).unwrap())
        .await;
    assert!(reply.error.is_none(), "{:?}", reply.error);
    assert_eq!(clock.0.load(Ordering::SeqCst), 3);
    drop(remote);
    task.await.unwrap();
    drop(state);
    drop(bridge);
    let _root = f.close();
    inspect_schema(&d.path).unwrap();
    let reopened = open(&d, clock.clone());
    assert_eq!(
        reopened
            .engine
            .get_record(d.parent)
            .unwrap()
            .unwrap()
            .access_history
            .len(),
        3
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_api_private_requires_supported_explicit_origin_without_work() {
    let (f, d, clock, bridge, _state) = setup().await;
    let before = bytes(&f, d.parent);
    for bad in [
        None,
        Some(json!(0)),
        Some(json!(3)),
        Some(json!(null)),
        Some(json!(true)),
        Some(json!(false)),
        Some(json!("1")),
    ] {
        let mut value = request(&f, &d, "bad");
        value.as_object_mut().unwrap().remove("contractVersion");
        if let Some(bad) = bad {
            value["contractVersion"] = bad;
        }
        let reply = crate::mcp::native_lifecycle_adapter::dispatch_private(
            bridge.storage.as_ref(),
            Operation::Feedback,
            value,
        )
        .await;
        assert_eq!(reply.error.unwrap().commit_state, CommitState::NotCommitted);
        assert!(reply.receipt.is_none());
        assert_eq!(bytes(&f, d.parent), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), 0);
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
    let mut one = request(&f, &d, "one");
    one["contractVersion"] = json!(1);
    assert!(
        parse(&one, Operation::Feedback, true, None).is_err(),
        "prepared public2 stays strict"
    );
    assert_eq!(bytes(&f, d.parent), before);
    // Origin acceptance does not bypass the independent native owner/schema gate.
    let legacy = Fixture::new();
    let reply = legacy
        .storage_adapter()
        .with_native_lifecycle_policy(Default::default())
        .feedback_native(parse_private(&one, Operation::Feedback).unwrap())
        .await;
    assert!(reply.error.is_some());
    assert!(reply.receipt.is_none());
}

fn raw_receipts(session: &NativeLifecycleSession) -> Vec<(Vec<u8>, Vec<u8>)> {
    session
        .metadata()
        .db
        .begin_read()
        .unwrap()
        .open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|row| {
            let (k, v) = row.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_api_genuine_old_two_reopen_replay_export_and_restore() {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "fixtures/original-api-old2.zip"
    )))
    .unwrap()
    .extract(root.path())
    .unwrap();
    let expected: Value =
        serde_json::from_slice(&std::fs::read(root.path().join("expected.json")).unwrap()).unwrap();
    let path = root.path().join("data");
    let publication = inspect_schema(&path).unwrap().schema;
    let raw: Vec<(Vec<u8>, Vec<u8>)> =
        serde_json::from_value(expected["rawReceipts"].clone()).unwrap();
    let id: MemoryId = serde_json::from_value(expected["memoryId"].clone()).unwrap();
    let ns: NamespaceId = serde_json::from_value(expected["namespaceId"].clone()).unwrap();
    let first: WriteReceipt = serde_json::from_value(expected["feedback"].clone()).unwrap();
    let forgotten: WriteReceipt = serde_json::from_value(expected["forget"].clone()).unwrap();
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = NativeLifecycleSession::open(&path, Default::default(), clock.clone()).unwrap();
    assert_eq!(raw_receipts(&session), raw);
    let f = Fixture::from_open_engine(tempfile::tempdir().unwrap(), session.engine);
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let key = MutationContext {
        store_id: Some(first.store_id),
        scope: MutationScope::Namespace(ns),
        idempotency_key: Some("genuine-old2-feedback".into()),
        contract_version: 2,
        expected_generation: serde_json::from_value(expected["feedbackExpectedGeneration"].clone())
            .unwrap(),
    };
    let replay = owner
        .feedback_native_with_work(
            key.clone(),
            id,
            3,
            None,
            Default::default(),
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, first.operation_id);
    assert_eq!(replay.outcome, first.outcome);
    let mut forget_key = key.clone();
    forget_key.idempotency_key = Some("genuine-old2-forget".into());
    forget_key.expected_generation = Some(first.committed_generation);
    let replay = owner
        .forget_native_with_work(
            forget_key,
            id,
            None,
            Default::default(),
            clock.clone(),
            None,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, forgotten.operation_id);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    drop(owner);
    let _derived = f.close();
    let exported = root.path().join("export.jsonl");
    let copy = root.path().join("copy");
    assert_eq!(
        export_store(&path, &exported).unwrap(),
        import_store(&exported, &copy).unwrap()
    );
    let session = NativeLifecycleSession::open(&copy, Default::default(), clock.clone()).unwrap();
    assert_eq!(raw_receipts(&session), raw);
    drop(session);
    let backup = run_backup(
        &Default::default(),
        &root.path().join("backup.zip"),
        Some(&path),
        false,
    )
    .await
    .unwrap();
    restore_from_backup(RestoreOptions {
        backup_path: backup,
        data_dir: path.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let report = inspect_schema(&path).unwrap();
    assert_eq!(report.schema, publication);
    assert_ne!(report.control.store_id, first.store_id);
    let session = NativeLifecycleSession::open(&path, Default::default(), clock.clone()).unwrap();
    assert_eq!(raw_receipts(&session), raw);
    assert_eq!(
        session.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
    let f = Fixture::from_open_engine(tempfile::tempdir().unwrap(), session.engine);
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    assert!(matches!(
        owner
            .feedback_native_with_work(key, id, 3, None, Default::default(), clock.clone(), None)
            .await,
        Err(StorageError::StoreLineageMismatch)
    ));
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_api_old_private_two_handler_rejects_one_without_mutating_fallback() {
    use crate::daemon::{DaemonClient, DaemonResponse, protocol::*};
    use crate::serialization::native_lifecycle::Reply;
    let socket = tempfile::tempdir().unwrap();
    let path = socket.path().join("old-private.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        for (method, operation) in [
            ("feedback_native_v1", Operation::Feedback),
            ("forget_native_v1", Operation::Forget),
        ] {
            let request: crate::daemon::DaemonRequest =
                read_framed_message(&mut stream, MAX_MESSAGE_SIZE)
                    .await
                    .unwrap();
            assert_eq!(
                request.method, method,
                "no fallback to an old mutating method"
            );
            assert_eq!(request.params["contractVersion"], 1);
            // Emulate the pre-change private handler path with the actual
            // unchanged strict2 parser; this is not a historical daemon binary.
            // Rejection happens before invoking any storage.
            let error = parse(&request.params, operation, true, None).unwrap_err();
            assert_eq!(error.field, "/contractVersion");
            let reply = Reply::input(error);
            let response = DaemonResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: Some(serde_json::to_value(reply).unwrap()),
                error: None,
            };
            write_framed_message(&mut stream, &response, MAX_MESSAGE_SIZE)
                .await
                .unwrap();
        }
        // Dropping the client must end the connection, not issue a retry/fallback.
        let final_read = read_framed_message(&mut stream, MAX_MESSAGE_SIZE).await;
        assert!(final_read.is_err());
    });
    let remote = crate::daemon::bridge_adapters::RemoteStorageAdapter::new(Arc::new(
        DaemonClient::connect(&path).await.unwrap(),
    ));
    for operation in [Operation::Feedback, Operation::Forget] {
        let request = parse_private(
            &json!({"contractVersion":1,"id":uuid::Uuid::new_v4()}),
            operation,
        )
        .unwrap();
        let reply = if operation == Operation::Feedback {
            remote.feedback_native(request).await
        } else {
            remote.forget_native(request).await
        };
        assert!(reply.receipt.is_none());
        assert_eq!(reply.error.unwrap().commit_state, CommitState::NotCommitted);
    }
    drop(remote);
    task.await.unwrap();
}
