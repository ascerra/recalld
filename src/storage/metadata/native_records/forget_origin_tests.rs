//! Private origin propagation with the actual record writer. Public routing stays gated.
use super::*;
use crate::serialization::native_lifecycle::parse_private;

struct NeverClock;
impl Clock for NeverClock {
    fn now_millis(&self) -> i64 {
        panic!("matching forget replay sampled clock")
    }
}
fn original(f: &Fixture, d: &Data, version: u32, key: &str) -> Value {
    let mut value = request(f, d, key);
    value["contractVersion"] = json!(version);
    value
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_origin_socket_one_two_replay_scope_cas_and_strict_reopen() {
    for version in [1, 2] {
        let (f, d, clock, bridge, state) = setup().await;
        let value = original(&f, &d, version, "forget-origin");
        let parsed = parse_private(&value, Operation::Forget).unwrap();
        let socket = tempfile::tempdir().unwrap();
        let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
            bridge.storage.clone(),
            &socket.path().join("forget.sock"),
        )
        .await;
        let before = bytes(&f, d.parent);
        let mut stale = parsed.clone();
        stale.options.expected_generation = Some(Generation::default());
        let rejected = remote.forget_native(stale).await;
        assert_eq!(rejected.error.unwrap().code, "GENERATION_CONFLICT");
        let mut foreign = parsed.clone();
        foreign.namespace_id = Some(NamespaceId::new(u32::MAX));
        let rejected = remote.forget_native(foreign).await;
        assert_eq!(rejected.error.unwrap().code, "NAMESPACE_MISMATCH");
        assert_eq!(bytes(&f, d.parent), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), 0);
        if version == 1 {
            assert!(parse(&value, Operation::Forget, true, None).is_err());
        }
        let reply = remote.forget_native(parsed.clone()).await;
        assert!(reply.error.is_none(), "{:?}", reply.error);
        assert_eq!(reply.scope, Some((d.ns, "native".into())));
        assert!(reply.readiness.unwrap().ready);
        let receipt = reply.receipt.unwrap();
        assert!(!receipt.replayed);
        assert!(matches!(
            receipt.outcome,
            MutationOutcome::NativeMemoryTombstoned { .. }
        ));
        assert_eq!(f.record(d.parent).unwrap().phase, DecayPhase::Tombstone);
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        let after = bytes(&f, d.parent);
        let replay = remote.forget_native(parsed.clone()).await;
        assert!(replay.error.is_none(), "{:?}", replay.error);
        let replay = replay.receipt.unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.operation_id, receipt.operation_id);
        assert_eq!(replay.committed_generation, receipt.committed_generation);
        let mut cross = parsed.clone();
        cross.original_contract_version = 3 - version;
        let conflict = remote.forget_native(cross).await;
        assert_eq!(conflict.error.unwrap().code, "IDEMPOTENCY_CONFLICT");
        assert_eq!(bytes(&f, d.parent), after);
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        drop(remote);
        task.await.unwrap();
        drop((bridge, state));
        let root = f.close();
        inspect_schema(&d.path).unwrap();
        let reopened = NativeLifecycleSession::open(
            &d.path,
            NativePolicyInputs::default(),
            Arc::new(NeverClock),
        )
        .unwrap();
        let f = Fixture::from_open_engine(root, reopened.engine);
        completion(&f).rebuild_all().await.unwrap();
        let adapter = f
            .storage_adapter()
            .with_clock(Arc::new(NeverClock))
            .with_native_lifecycle_policy(NativePolicyInputs {
                multiplier: f64::NAN,
                ..Default::default()
            });
        let replay = adapter.forget_native(parsed).await;
        assert!(replay.error.is_none(), "{:?}", replay.error);
        assert!(replay.receipt.as_ref().unwrap().replayed);
        assert_eq!(replay.receipt.unwrap().operation_id, receipt.operation_id);
        assert_eq!(bytes(&f, d.parent), after);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_origin_known_publication_failure_keeps_full_receipt_and_replays_after_repair() {
    let (f, d, clock, bridge, _state) = setup_certified().await;
    // Keep a real live vector after forgetting the target; an empty vector view
    // does not exercise dimension validation during publication.
    let survivor = {
        let mut storage = f.storage.write().unwrap();
        let policy = NativePolicyInputs::default();
        let intent = storage
            .meta_store()
            .capture_native_insert(scope(&d), &input(&d), &policy)
            .unwrap();
        let mut context = MutationContext::unkeyed(d.ns);
        context.contract_version = 2;
        inserted(
            &storage
                .insert_native_record(
                    &context,
                    &intent,
                    &generated(),
                    &policy,
                    &crate::time::ManualClock::new(3000),
                    None,
                )
                .unwrap(),
        )
    };
    completion(&f).rebuild_all().await.unwrap();
    assert_eq!(f.record(survivor).unwrap().phase, DecayPhase::Full);
    let value = original(&f, &d, 1, "forget-failed-publication");
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    let reply = crate::mcp::native_lifecycle_adapter::dispatch_private(
        bridge.storage.as_ref(),
        Operation::Forget,
        value.clone(),
    )
    .await;
    assert_eq!(
        reply.error.as_ref().unwrap().commit_state,
        CommitState::Committed
    );
    assert!(reply.readiness.is_none());
    let receipt = reply.receipt.unwrap();
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativeMemoryTombstoned { .. }
    ));
    assert_eq!(f.record(d.parent).unwrap().phase, DecayPhase::Tombstone);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let after = bytes(&f, d.parent);
    let tracker = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .work_tracker()
        .clone();
    assert_eq!(tracker.status().pending, 0);
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(NeverClock))
        .with_native_lifecycle_policy(NativePolicyInputs {
            multiplier: f64::NAN,
            ..Default::default()
        });
    let replay = adapter
        .forget_native(parse_private(&value, Operation::Forget).unwrap())
        .await;
    assert!(replay.error.is_none(), "{:?}", replay.error);
    assert!(replay.receipt.as_ref().unwrap().replayed);
    assert_eq!(replay.receipt.unwrap().operation_id, receipt.operation_id);
    assert!(replay.readiness.unwrap().ready);
    assert_eq!(tracker.status().unresolved, 0);
    assert_eq!(bytes(&f, d.parent), after);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_origin_unknown_commit_preserves_identity_and_reopens_without_reobserving() {
    struct FaultClock;
    impl Clock for FaultClock {
        fn now_millis(&self) -> i64 {
            effects::FAIL_AFTER_RECORD_COMMIT.with(|f| f.set(true));
            86_400_000
        }
    }
    let (f, d, _clock, bridge, state) = setup().await;
    let value = original(&f, &d, 1, "forget-unknown");
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(FaultClock))
        .with_native_lifecycle_policy(NativePolicyInputs::default());
    let reply = adapter
        .forget_native(parse_private(&value, Operation::Forget).unwrap())
        .await;
    let error = reply.error.unwrap();
    assert_eq!(error.commit_state, CommitState::Unknown);
    assert!(error.operation_id.is_some());
    assert!(reply.receipt.is_none());
    assert!(reply.readiness.is_none());
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
    drop((adapter, bridge, state));
    let root = f.close();
    let reopened =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), Arc::new(NeverClock))
            .unwrap();
    let f = Fixture::from_open_engine(root, reopened.engine);
    completion(&f).rebuild_all().await.unwrap();
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(NeverClock))
        .with_native_lifecycle_policy(NativePolicyInputs::default());
    let replay = adapter
        .forget_native(parse_private(&value, Operation::Forget).unwrap())
        .await;
    assert!(replay.error.is_none(), "{:?}", replay.error);
    assert!(replay.receipt.as_ref().unwrap().replayed);
    assert_eq!(
        Some(replay.receipt.unwrap().operation_id),
        error.operation_id
    );
    assert!(replay.readiness.unwrap().ready);
    assert_eq!(f.record(d.parent).unwrap().phase, DecayPhase::Tombstone);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_origin_retired_terminal_scope_replays_after_purge_and_keeps_known_errors() {
    let (f, d, clock, bridge, state) = setup().await;
    let value = original(&f, &d, 1, "retired-forget");
    let first = bridge
        .storage
        .forget_native(parse_private(&value, Operation::Forget).unwrap())
        .await;
    assert!(first.error.is_none(), "{:?}", first.error);
    let receipt = first.receipt.unwrap();
    drop((bridge, state));
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
    inspect_schema(&d.path).unwrap();
    let session =
        NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), Arc::new(NeverClock))
            .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let adapter = f
        .storage_adapter()
        .with_clock(Arc::new(NeverClock))
        .with_native_lifecycle_policy(NativePolicyInputs {
            multiplier: f64::NAN,
            ..Default::default()
        });
    for explicit in [false, true] {
        let mut retry = value.clone();
        retry["namespace"] = json!("native");
        if explicit {
            retry["namespaceId"] = json!(d.ns);
        }
        let replay = adapter
            .forget_native(parse_private(&retry, Operation::Forget).unwrap())
            .await;
        assert!(replay.error.is_none(), "{:?}", replay.error);
        assert_eq!(replay.scope, Some((d.ns, "native".into())));
        assert!(replay.receipt.as_ref().unwrap().replayed);
        assert_eq!(replay.receipt.unwrap().operation_id, receipt.operation_id);
    }
    let mut wrong = value;
    wrong["namespaceId"] = json!(d.ns);
    wrong["namespace"] = json!("wrong");
    let error = adapter
        .forget_native(parse_private(&wrong, Operation::Forget).unwrap())
        .await;
    assert_eq!(error.error.as_ref().unwrap().code, "NAMESPACE_MISMATCH");
    assert_eq!(error.error.unwrap().commit_state, CommitState::Committed);
    assert_eq!(error.receipt.unwrap().operation_id, receipt.operation_id);
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert!(f.record(d.parent).is_none());
}
