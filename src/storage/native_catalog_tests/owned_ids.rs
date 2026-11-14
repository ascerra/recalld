// Actual owned allocator consumer regressions, derived from Halley's prepared draft.
use super::*;
use crate::storage::catalog::NamespacePolicy;
use crate::storage::ids::{IdKind, IdSource, IdState, SeededIds};
use crate::storage::native_lifecycle::LifecycleScope;

fn cursor(s: &NativeLifecycleSession) -> IdState {
    s.engine
        .meta_store()
        .id_source()
        .checkpoint()
        .unwrap()
        .unwrap()
}
fn with_ids(path: &Path, clock: Arc<Clock>, ids: SeededIds) -> NativeLifecycleSession {
    NativeLifecycleSession {
        engine: RedbStorageEngine::open_with_ids(path, true, Box::new(ids)).unwrap(),
        policy: NativePolicyInputs::default(),
        clock,
    }
}
fn owned(root: &Path) -> (PathBuf, Arc<Clock>, NativeLifecycleSession) {
    // Existing fixture creates a genuine migrated native prefix. Its random
    // migration identity is NOT a claim of deterministic native genesis.
    let (path, clock, session) = fixture(root);
    drop(session);
    let session = with_ids(&path, clock.clone(), SeededIds::new([41; 32]));
    (path, clock, session)
}
fn expected_replay(r: &WriteReceipt) -> WriteReceipt {
    let mut r = r.clone();
    r.replayed = true;
    r
}
fn assert_receipt_cursor(s: &NativeLifecycleSession, oracle: &SeededIds) {
    assert_eq!(Some(cursor(s)), oracle.checkpoint().unwrap());
}
fn policy_scope(s: &NativeLifecycleSession, id: NamespaceId) -> LifecycleScope {
    LifecycleScope {
        namespace_id: id,
        namespace_name: s.engine.get_namespace(id).unwrap().unwrap().name,
        source: None,
    }
}

#[test]
fn owned_native_catalog_create_replay_conflict_and_closed_reopen_keep_allocator() {
    for api in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let (path, clock, mut s) = owned(root.path());
        let before = cursor(&s);
        let oracle = SeededIds::from_state(before).unwrap();
        let operation = oracle.uuid(IdKind::Operation).unwrap();
        let mut ctx = context(&s, "owned-create");
        ctx.contract_version = api;
        let receipt = s.create_namespace(&ctx, &input(), &Provider(true)).unwrap();
        assert_eq!(receipt.operation_id, operation);
        assert_receipt_cursor(&s, &oracle);
        let MutationOutcome::NativeNamespaceCreated { transition } = &receipt.outcome else {
            panic!("wrong native creation receipt");
        };
        let ns = s
            .engine
            .get_namespace(transition.namespace_id)
            .unwrap()
            .unwrap();
        assert_eq!(ns.created_at, 2000);
        assert_eq!(transition.created_at, ns.created_at);
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        let state = s.engine.control_state().unwrap();
        let saved = cursor(&s);
        assert_eq!(
            s.create_namespace(&ctx, &input(), &Provider(false))
                .unwrap(),
            expected_replay(&receipt)
        );
        let mut changed = input();
        changed.initial_stability = Some(4.);
        assert!(matches!(
            s.create_namespace(&ctx, &changed, &Provider(false)),
            Err(StorageError::IdempotencyConflict)
        ));
        let mut other_api = ctx.clone();
        other_api.contract_version = if api == 1 { 2 } else { 1 };
        assert!(matches!(
            s.create_namespace(&other_api, &input(), &Provider(false)),
            Err(StorageError::IdempotencyConflict)
        ));
        assert_eq!(cursor(&s), saved);
        assert_eq!(s.engine.control_state().unwrap(), state);
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        s.engine.sync().unwrap();
        drop(s);
        let mut s = with_ids(
            &path,
            clock.clone(),
            SeededIds::from_state(saved.clone()).unwrap(),
        );
        assert_eq!(cursor(&s), saved, "open must not reserve new IDs");
        assert_eq!(
            s.create_namespace(&ctx, &input(), &Provider(false))
                .unwrap(),
            expected_replay(&receipt)
        );
        assert_eq!(cursor(&s), saved);
        let mut next = input();
        next.name = "next-created".into();
        let mut key = context(&s, "next-create");
        key.contract_version = api;
        let next_operation = oracle.uuid(IdKind::Operation).unwrap();
        assert_eq!(
            s.create_namespace(&key, &next, &Provider(true))
                .unwrap()
                .operation_id,
            next_operation
        );
        assert_receipt_cursor(&s, &oracle);
    }
}

#[test]
fn owned_native_numeric_policy_update_and_keyed_nochange_reserve_once() {
    for api in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let (_, clock, mut s) = owned(root.path());
        let mut create = context(&s, "policy-namespace");
        create.contract_version = api;
        let receipt = s
            .create_namespace(&create, &input(), &Provider(true))
            .unwrap();
        let MutationOutcome::NativeNamespaceCreated { transition } = receipt.outcome else {
            panic!()
        };
        let id = transition.namespace_id;
        let oracle = SeededIds::from_state(cursor(&s)).unwrap();
        let mut replacement = NamespacePolicy::from(&s.engine.get_namespace(id).unwrap().unwrap());
        replacement.initial_stability = 4.;
        for (key, changed) in [("policy-update", true), ("policy-nochange", false)] {
            let mut ctx = context(&s, key);
            ctx.scope = MutationScope::Namespace(id);
            ctx.contract_version = api;
            let operation = oracle.uuid(IdKind::Operation).unwrap();
            let scope = policy_scope(&s, id);
            let receipt = s
                .update_policy(&ctx, scope.clone(), &replacement)
                .unwrap()
                .unwrap();
            assert_eq!(receipt.operation_id, operation);
            assert!(
                matches!(
                    &receipt.outcome,
                    MutationOutcome::NativeNamespacePolicyUpdated { .. }
                ) == changed
            );
            assert!(
                matches!(
                    &receipt.outcome,
                    MutationOutcome::NativeNamespacePolicyUnchanged { .. }
                ) != changed
            );
            assert_receipt_cursor(&s, &oracle);
            let state = s.engine.control_state().unwrap();
            assert_eq!(
                s.update_policy(&ctx, scope.clone(), &replacement)
                    .unwrap()
                    .unwrap(),
                expected_replay(&receipt)
            );
            let mut different = replacement.clone();
            different.initial_stability = 5.;
            assert!(matches!(
                s.update_policy(&ctx, scope.clone(), &different),
                Err(StorageError::IdempotencyConflict)
            ));
            let mut other_api = ctx.clone();
            other_api.contract_version = if api == 1 { 2 } else { 1 };
            assert!(matches!(
                s.update_policy(&other_api, scope, &replacement),
                Err(StorageError::IdempotencyConflict)
            ));
            assert_receipt_cursor(&s, &oracle);
            assert_eq!(s.engine.control_state().unwrap(), state);
        }
        let mut ctx = context(&s, "unused-key");
        ctx.scope = MutationScope::Namespace(id);
        ctx.contract_version = api;
        ctx.idempotency_key = None;
        let before = s.engine.control_state().unwrap();
        let scope = policy_scope(&s, id);
        assert!(
            s.update_policy(&ctx, scope, &replacement)
                .unwrap()
                .is_none()
        );
        assert_receipt_cursor(&s, &oracle);
        assert_eq!(s.engine.control_state().unwrap(), before);
        assert_eq!(
            clock.0.load(Ordering::SeqCst),
            1,
            "policy operations must not sample clock"
        );
    }
}

#[test]
fn owned_native_catalog_known_and_unknown_commits_keep_id_through_recovery() {
    for api in [1, 2] {
        for point in ["before_handle_publish", "commit_unknown"] {
            let root = tempfile::tempdir().unwrap();
            let (path, clock, mut s) = owned(root.path());
            let oracle = SeededIds::from_state(cursor(&s)).unwrap();
            let operation = oracle.uuid(IdKind::Operation).unwrap();
            let mut ctx = context(&s, "owned-fault");
            ctx.contract_version = api;
            let tracker = s.engine.meta_store().work_tracker();
            crate::storage::catalog::FAILURE.with(|f| f.set(Some(point)));
            let result = s.create_namespace(&ctx, &input(), &Provider(true));
            crate::storage::catalog::FAILURE.with(|f| f.set(None));
            let error = result.unwrap_err();
            let carried = match error {
                StorageError::CommittedMutation { receipt, .. }
                    if point == "before_handle_publish" =>
                {
                    Some(*receipt)
                }
                StorageError::CommitOutcomeUnknown { operation_id, .. }
                    if point == "commit_unknown" =>
                {
                    assert_eq!(operation_id, operation);
                    None
                }
                other => panic!("lost committed/unknown identity at {point}: {other:?}"),
            };
            if let Some(r) = &carried {
                assert_eq!(r.operation_id, operation);
            }
            assert!(tracker.status().unresolved > 0);
            assert!(
                matches!(
                    s.create_namespace(&ctx, &input(), &Provider(false)),
                    Err(StorageError::RecoveryRequired(_))
                ),
                "poisoned engine must reject retry before provider/allocation"
            );
            assert_receipt_cursor(&s, &oracle);
            let saved = cursor(&s);
            eprintln!(
                "owned catalog fault api={api} point={point} operation={operation} receipt={carried:?} cursor={saved:?}"
            );
            drop(s);
            let mut s = with_ids(
                &path,
                clock.clone(),
                SeededIds::from_state(saved.clone()).unwrap(),
            );
            assert_eq!(s.engine.meta_store().work_tracker().status().pending, 0);
            let replay = s
                .create_namespace(&ctx, &input(), &Provider(false))
                .unwrap();
            assert!(replay.replayed);
            assert_eq!(replay.operation_id, operation);
            if let Some(r) = carried {
                assert_eq!(replay, expected_replay(&r));
            }
            assert_eq!(cursor(&s), saved);
            assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        }
    }
}

#[test]
fn owned_native_catalog_abandoned_precommit_consumes_reserved_id_without_rewind() {
    for api in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let (_, clock, mut s) = owned(root.path());
        let before = s.engine.control_state().unwrap();
        let oracle = SeededIds::from_state(cursor(&s)).unwrap();
        let abandoned_operation = oracle.uuid(IdKind::Operation).unwrap();
        let mut ctx = context(&s, "abandoned");
        ctx.contract_version = api;
        // Inspected catalog path stages a receipt before invoking prepare,
        // whose first actual boundary is before_stage. No payload published.
        crate::storage::catalog::FAILURE.with(|f| f.set(Some("before_stage")));
        let result = s.create_namespace(&ctx, &input(), &Provider(true));
        crate::storage::catalog::FAILURE.with(|f| f.set(None));
        assert!(result.is_err());
        assert_eq!(s.engine.control_state().unwrap(), before);
        assert!(
            s.engine
                .lookup_operation(before.store_id, MutationScope::Catalog, abandoned_operation)
                .unwrap()
                .is_none()
        );
        assert!(s.engine.get_namespace_by_name("created").unwrap().is_none());
        assert_receipt_cursor(&s, &oracle);
        assert_eq!(s.engine.meta_store().work_tracker().status().pending, 0);
        let next_operation = oracle.uuid(IdKind::Operation).unwrap();
        let receipt = s.create_namespace(&ctx, &input(), &Provider(true)).unwrap();
        assert!(!receipt.replayed);
        assert_eq!(receipt.operation_id, next_operation);
        assert_ne!(receipt.operation_id, abandoned_operation);
        assert_receipt_cursor(&s, &oracle);
        assert_eq!(clock.0.load(Ordering::SeqCst), 2);
    }
}
