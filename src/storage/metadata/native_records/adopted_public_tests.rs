//! Actual public API1 after offline adoption; private keyed replay is separate.
use super::*;
use crate::storage::control::{self, StoredReceipt};
use crate::storage::inherited_policy::{
    AdoptInheritedDecay, adopt_inherited_decay, preview_inherited_decay,
};
use redb::ReadableTable;
use std::collections::BTreeMap;
use tower::ServiceExt;

#[derive(Clone, Copy, Debug)]
enum Lane {
    Http,
    Mcp,
    Remote,
}
const LANES: [Lane; 3] = [Lane::Http, Lane::Mcp, Lane::Remote];
fn policy(multiplier: f64) -> NativePolicyInputs {
    NativePolicyInputs {
        multiplier,
        ..Default::default()
    }
}
fn fresh(multiplier: Option<f32>) -> (tempfile::TempDir, Data) {
    let root = tempfile::tempdir().unwrap();
    let d = fixture_with_policy(root.path(), |_| {}, false, multiplier, false);
    (root, d)
}
async fn opened(
    root: tempfile::TempDir,
    d: &Data,
    c: Arc<FixedClock>,
    multiplier: f64,
) -> (Fixture, McpBridge, crate::api::AppState) {
    let session = NativeLifecycleSession::open(&d.path, policy(multiplier), c.clone()).unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    let (bridge, state) = configured_public_adapters(&f, c, policy(multiplier));
    (f, bridge, state)
}
fn closed(
    f: Fixture,
    bridge: McpBridge,
    state: crate::api::AppState,
    d: &Data,
) -> tempfile::TempDir {
    drop((bridge, state));
    let root = f.close();
    inspect_schema(&d.path).unwrap();
    root
}
pub(super) fn receipts(f: &Fixture) -> BTreeMap<Vec<u8>, Vec<u8>> {
    let store = f.storage.read().unwrap();
    let read = store.meta_store().db.begin_read().unwrap();
    read.open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|r| {
            let (k, v) = r.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}
fn snapshot(f: &Fixture, d: &Data) -> Vec<u8> {
    let s = f.storage.read().unwrap();
    control::encode(&(
        s.get_record(d.parent).unwrap().unwrap().encode().unwrap(),
        s.get_namespace(d.ns).unwrap(),
        s.control_state().unwrap(),
        s.meta_store().removal_ledger_export().unwrap(),
        receipts(f).into_iter().collect::<Vec<_>>(),
    ))
    .unwrap()
}
pub(super) fn no_pending(f: &Fixture) {
    let status = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .work_tracker()
        .status();
    assert_eq!(status.pending, 0);
    assert_eq!(status.unresolved, 0);
}
fn preserved(old: &BTreeMap<Vec<u8>, Vec<u8>>, f: &Fixture) {
    let current = receipts(f);
    for (k, v) in old {
        assert_eq!(current.get(k), Some(v));
    }
}
fn latest(f: &Fixture, operation: Operation) -> StoredReceipt {
    receipts(f)
        .values()
        .map(|v| control::validate_receipt(v).unwrap())
        .filter(|r| {
            matches!(
                (&r.receipt.outcome, operation),
                (
                    MutationOutcome::LifecycleFeedbackApplied { .. },
                    Operation::Feedback
                ) | (
                    MutationOutcome::NativeMemoryTombstoned { .. },
                    Operation::Forget
                )
            )
        })
        .max_by_key(|r| r.receipt.committed_generation)
        .unwrap()
}
fn original_one(f: &Fixture, d: &Data, operation: Operation) {
    let r = latest(f, operation);
    let mut ctx = MutationContext::unkeyed(d.ns);
    ctx.contract_version = 1;
    let hash = match operation {
        Operation::Feedback => {
            let bytes = control::encode(&(
                scope(d),
                vec![(d.parent, AccessKind::ManualReinforcement)],
                Some(3u8),
            ))
            .unwrap();
            control::request_hash(&ctx, "native_feedback", &[&bytes])
        }
        Operation::Forget => {
            // The actual typed request, not captured tokens or a public retry key.
            control::request_hash(&ctx, "native_tombstone", &[d.parent.as_bytes()])
        }
    };
    assert_eq!(r.request_hash, hash);
}
async fn invoke(
    lane: Lane,
    bridge: &McpBridge,
    state: &crate::api::AppState,
    d: &Data,
    op: Operation,
) -> bool {
    match lane {
        Lane::Http => {
            let (method, uri, body) = match op {
                Operation::Feedback => ("POST", format!("/memories/{}/reinforce", d.parent), "{}"),
                Operation::Forget => ("DELETE", format!("/memories/{}", d.parent), ""),
            };
            let r = crate::api::router(state.clone(), &Default::default())
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
            let status = r.status();
            let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
                .await
                .unwrap();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            if status.is_success() {
                assert!(value.get("data").is_some(), "{value}");
                assert!(value.get("receipt").is_none(), "old success shape");
            } else {
                // API1 keeps its existing simple typed input-error projection.
                assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
                assert_eq!(value["error"], "BAD_REQUEST", "{value}");
                assert!(value.get("observation").is_none());
                assert!(value.get("receipt").is_none());
                assert!(value.to_string().contains("inherited"), "{value}");
            }
            status.is_success()
        }
        Lane::Mcp => {
            let tool = if op == Operation::Feedback {
                "reinforce_memory"
            } else {
                "forget_memory"
            };
            let r = crate::mcp::tools::dispatch_tool(bridge, tool, json!({"id":d.parent})).await;
            if r.is_error == Some(true) {
                assert!(
                    serde_json::to_string(&r).unwrap().contains("inherited"),
                    "{r:?}"
                );
            }
            r.is_error != Some(true)
        }
        Lane::Remote => {
            let temp = tempfile::tempdir().unwrap();
            let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
                bridge.storage.clone(),
                &temp.path().join("peer.sock"),
            )
            .await;
            let remote_bridge = McpBridge {
                search: bridge.search.clone(),
                storage: Arc::new(remote),
                namespaces: bridge.namespaces.clone(),
                health: bridge.health.clone(),
                default_namespace: bridge.default_namespace.clone(),
                timezone: bridge.timezone,
            };
            let tool = if op == Operation::Feedback {
                "reinforce_memory"
            } else {
                "forget_memory"
            };
            let reply =
                crate::mcp::tools::dispatch_tool(&remote_bridge, tool, json!({"id":d.parent}))
                    .await;
            let ok = reply.is_error != Some(true);
            if !ok {
                assert!(
                    serde_json::to_string(&reply).unwrap().contains("inherited"),
                    "{reply:?}"
                );
            }
            drop(remote_bridge);
            task.await.unwrap();
            ok
        }
    }
}
fn adopt(
    d: &Data,
    multiplier: f64,
    key: &str,
    c: &FixedClock,
) -> crate::storage::control::WriteReceipt {
    let source = inspect_schema(&d.path).unwrap().control.generation_ref();
    let preview = preview_inherited_decay(&d.path, d.ns, multiplier, source, c).unwrap();
    let request = AdoptInheritedDecay {
        context: MutationContext {
            store_id: Some(source.store_id),
            scope: MutationScope::Namespace(d.ns),
            idempotency_key: Some(key.into()),
            expected_generation: Some(source.generation),
            contract_version: 1,
        },
        preview,
    };
    adopt_inherited_decay(&d.path, &request).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopted_public_unknown_feedback_rejects_but_structural_forget_is_unblocked() {
    for lane in LANES {
        let (root, d) = fresh(None);
        let c = Arc::new(FixedClock(AtomicUsize::new(0)));
        let (f, bridge, state) = opened(root, &d, c.clone(), 1.).await;
        let before = snapshot(&f, &d);
        assert!(
            !invoke(lane, &bridge, &state, &d, Operation::Feedback).await,
            "{lane:?}"
        );
        assert_eq!(snapshot(&f, &d), before);
        assert_eq!(c.0.load(Ordering::SeqCst), 0);
        no_pending(&f);
        assert!(
            invoke(lane, &bridge, &state, &d, Operation::Forget).await,
            "{lane:?}"
        );
        assert_eq!(c.0.load(Ordering::SeqCst), 1);
        original_one(&f, &d, Operation::Forget);
        no_pending(&f);
        assert_eq!(f.record(d.parent).unwrap().phase, DecayPhase::Tombstone);
        assert!(
            f.storage
                .read()
                .unwrap()
                .get_namespace(d.ns)
                .unwrap()
                .unwrap()
                .schema
                .unwrap()
                .inherited_decay
                .is_none()
        );
        let root = closed(f, bridge, state, &d);
        let (f, b, s) = opened(root, &d, c.clone(), 1.).await;
        assert_eq!(f.record(d.parent).unwrap().phase, DecayPhase::Tombstone);
        closed(f, b, s, &d);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopted_public_feedback_requires_actual_adoption_after_global_change() {
    for lane in LANES {
        let (root, d) = fresh(None);
        let c = Arc::new(FixedClock(AtomicUsize::new(0)));
        let (f, b, s) = opened(root, &d, c.clone(), 1.).await;
        let original = f.record(d.parent).unwrap();
        let old = receipts(&f);
        let root = closed(f, b, s, &d);
        adopt(&d, 1., "first-adoption", c.as_ref());
        let (f, b, s) = opened(root, &d, c.clone(), 1.).await;
        preserved(&old, &f);
        let mut expected = original.clone();
        let schema = &mut expected.schema.as_mut().unwrap().lifecycle;
        schema.policy_revision = schema.policy_revision.next().unwrap();
        schema.record_generation = schema.record_generation.next().unwrap();
        assert_eq!(
            f.record(d.parent).unwrap().encode().unwrap(),
            expected.encode().unwrap()
        );
        let calls = c.0.load(Ordering::SeqCst);
        assert!(
            invoke(lane, &b, &s, &d, Operation::Feedback).await,
            "{lane:?}"
        );
        assert_eq!(c.0.load(Ordering::SeqCst), calls + 1);
        original_one(&f, &d, Operation::Feedback);
        no_pending(&f);
        let old = receipts(&f);
        let root = closed(f, b, s, &d);
        let (f, b, s) = opened(root, &d, c.clone(), 0.).await;
        // An unadopted global change does not apply: the adopted value governs.
        let calls = c.0.load(Ordering::SeqCst);
        assert!(
            invoke(lane, &b, &s, &d, Operation::Feedback).await,
            "{lane:?}"
        );
        assert_eq!(c.0.load(Ordering::SeqCst), calls + 1);
        original_one(&f, &d, Operation::Feedback);
        let MutationOutcome::LifecycleFeedbackApplied { transition } =
            latest(&f, Operation::Feedback).receipt.outcome
        else {
            panic!()
        };
        assert_eq!(transition.feedback.unwrap().decay_rate_multiplier.get(), 1.);
        preserved(&old, &f);
        no_pending(&f);
        let old = receipts(&f);
        let before = f.record(d.parent).unwrap();
        let root = closed(f, b, s, &d);
        adopt(&d, 0., "second-adoption", c.as_ref());
        let (f, b, s) = opened(root, &d, c.clone(), 0.).await;
        preserved(&old, &f);
        let mut expected = before;
        let schema = &mut expected.schema.as_mut().unwrap().lifecycle;
        schema.policy_revision = schema.policy_revision.next().unwrap();
        schema.record_generation = schema.record_generation.next().unwrap();
        assert_eq!(
            f.record(d.parent).unwrap().encode().unwrap(),
            expected.encode().unwrap()
        );
        let calls = c.0.load(Ordering::SeqCst);
        assert!(invoke(lane, &b, &s, &d, Operation::Feedback).await);
        assert_eq!(c.0.load(Ordering::SeqCst), calls + 1);
        original_one(&f, &d, Operation::Feedback);
        preserved(&old, &f);
        no_pending(&f);
        let receipt = latest(&f, Operation::Feedback);
        let MutationOutcome::LifecycleFeedbackApplied { transition } = receipt.receipt.outcome
        else {
            panic!()
        };
        assert_eq!(transition.feedback.unwrap().decay_rate_multiplier.get(), 0.);
        let final_receipts = receipts(&f);
        let root = closed(f, b, s, &d);
        let (f, b, s) = opened(root, &d, c.clone(), 1.).await;
        assert_eq!(receipts(&f), final_receipts);
        let calls = c.0.load(Ordering::SeqCst);
        assert!(
            invoke(lane, &b, &s, &d, Operation::Forget).await,
            "changed global cannot block structural forget"
        );
        assert_eq!(c.0.load(Ordering::SeqCst), calls + 1);
        original_one(&f, &d, Operation::Forget);
        preserved(&final_receipts, &f);
        no_pending(&f);
        let final_receipts = receipts(&f);
        let root = closed(f, b, s, &d);
        let (f, b, s) = opened(root, &d, c.clone(), 0.).await;
        assert_eq!(receipts(&f), final_receipts);
        assert_eq!(f.record(d.parent).unwrap().phase, DecayPhase::Tombstone);
        closed(f, b, s, &d);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopted_public_explicit_override_ignores_unused_global_without_adoption() {
    for lane in LANES {
        let (root, d) = fresh(Some(1.));
        let c = Arc::new(FixedClock(AtomicUsize::new(0)));
        let (f, b, s) = opened(root, &d, c.clone(), 0.).await;
        let ns = f
            .storage
            .read()
            .unwrap()
            .get_namespace(d.ns)
            .unwrap()
            .unwrap();
        let before = f.record(d.parent).unwrap();
        let resolved =
            crate::decay::lifecycle::resolve_policy(&ns, &Default::default(), 0.).unwrap();
        let plan =
            crate::decay::lifecycle::plan_feedback(&before, &resolved, 86_400_000, 3).unwrap();
        assert!(invoke(lane, &b, &s, &d, Operation::Feedback).await);
        assert_eq!(c.0.load(Ordering::SeqCst), 1);
        let after = f.record(d.parent).unwrap();
        assert_eq!(after.stability, plan.stability);
        assert_eq!(
            control::encode(
                &f.storage
                    .read()
                    .unwrap()
                    .get_namespace(d.ns)
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            control::encode(&ns).unwrap()
        );
        assert!(ns.schema.unwrap().inherited_decay.is_none());
        original_one(&f, &d, Operation::Feedback);
        no_pending(&f);
        closed(f, b, s, &d);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopted_public_private_keyed_history_replays_before_new_policy_and_cas() {
    let (root, d) = fresh(None);
    let c = Arc::new(FixedClock(AtomicUsize::new(0)));
    adopt(&d, 1., "first", c.as_ref());
    let (f, b, s) = opened(root, &d, c.clone(), 1.).await;
    let control = f.storage.read().unwrap().control_state().unwrap();
    let wire = json!({"contractVersion":1,"id":d.parent,"quality":3,"storeId":control.store_id,"idempotencyKey":"private-feedback","expectedGeneration":control.committed_generation});
    let req =
        crate::serialization::native_lifecycle::parse_lifecycle_private(&wire, Operation::Feedback)
            .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        b.storage.clone(),
        &temp.path().join("first.sock"),
    )
    .await;
    let first = remote.feedback_native(req.clone()).await;
    assert!(first.error.is_none(), "{first:?}");
    let first = first.receipt.unwrap();
    drop(remote);
    task.await.unwrap();
    let old = receipts(&f);
    let root = closed(f, b, s, &d);
    adopt(&d, 0., "second", c.as_ref());
    // Deliberately configure1 against canonical0. Replay must precede this fresh error.
    let (f, b, s) = opened(root, &d, c.clone(), 1.).await;
    preserved(&old, &f);
    let before = snapshot(&f, &d);
    let calls = c.0.load(Ordering::SeqCst);
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        b.storage.clone(),
        &temp.path().join("retry.sock"),
    )
    .await;
    let replay = remote.feedback_native(req.clone()).await;
    assert!(replay.error.is_none(), "{replay:?}");
    let replay = replay.receipt.unwrap();
    let mut expected = first;
    expected.replayed = true;
    assert_eq!(replay, expected);
    let mut other = req;
    other.quality = 4;
    let error = remote.feedback_native(other).await;
    assert_eq!(error.error.unwrap().code, "IDEMPOTENCY_CONFLICT");
    assert_eq!(snapshot(&f, &d), before);
    assert_eq!(c.0.load(Ordering::SeqCst), calls);
    no_pending(&f);
    drop(remote);
    task.await.unwrap();
    let root = closed(f, b, s, &d);
    let (f, b, s) = opened(root, &d, c, 0.).await;
    preserved(&old, &f);
    closed(f, b, s, &d);
}
