use super::*;
use crate::storage::native_catalog::NativeNamespaceInput;
use crate::storage::native_purge::NativeNamespaceProof;

fn capture(
    s: &NativeLifecycleSession,
    ns: NamespaceId,
) -> (GenerationRef, Option<NativeNamespaceProof>) {
    let metadata = s.metadata();
    let guard = metadata.native_guard().unwrap();
    // Feedback lookup already has this unchanged write transaction. Capturing
    // must neither reacquire publication nor compare against a later snapshot.
    let tx = metadata.db.begin_write().unwrap();
    let result = metadata.capture_namespace_binding(&guard, ns).unwrap();
    assert_eq!(
        result.0,
        control::read_control(&tx).unwrap().generation_ref()
    );
    result
}
fn purge(s: &mut NativeLifecycleSession, d: &Data) {
    let mut progress = s
        .engine
        .begin_namespace_purge(&ctx(s, d, "binding-purge"), true)
        .unwrap();
    while progress.stage != crate::storage::purge::PurgeStage::Complete {
        progress = s
            .engine
            .resume_namespace_purge(
                progress.store_id,
                d.ns,
                progress.start_receipt.operation_id,
                1,
            )
            .unwrap();
    }
}
#[test]
fn namespace_binding_live_retired_and_recreated_name_keep_original_identity() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let before = s.engine.control_state().unwrap();
    let (generation, original) = capture(&s, d.ns);
    let original = original.unwrap();
    assert_eq!(generation, before.generation_ref());
    assert_eq!(original.namespace_id, d.ns);
    assert_eq!(original.name, "native");
    assert_eq!(original.embedding_dim, 4);
    assert_eq!(
        original.schema.embedding_binding,
        s.engine
            .get_namespace(d.ns)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .embedding_binding
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(s.engine.control_state().unwrap(), before);
    purge(&mut s, &d);
    let before = s.engine.control_state().unwrap();
    assert_eq!(
        capture(&s, d.ns),
        (before.generation_ref(), Some(original.clone()))
    );
    let mut key = ctx(&s, &d, "recreated");
    key.scope = MutationScope::Catalog;
    let receipt = s
        .engine
        .create_native_namespace(
            &key,
            &NativeNamespaceInput {
                name: "native".into(),
                embedding_dim: Some(4),
                embedding_profile_id: None,
                initial_stability: None,
                desired_retention: None,
                decay_rate_multiplier: None,
            },
            &d.profile,
            &NativePolicyInputs::default(),
            clock.as_ref(),
            None,
        )
        .unwrap();
    let MutationOutcome::NativeNamespaceCreated { transition } = receipt.outcome else {
        panic!()
    };
    assert_ne!(transition.namespace_id, d.ns);
    let before = s.engine.control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    let retired = capture(&s, d.ns).1.unwrap();
    let live = capture(&s, transition.namespace_id).1.unwrap();
    assert_eq!(retired, original);
    assert_eq!(retired.name, live.name);
    assert_ne!(retired.namespace_id, live.namespace_id);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    assert_eq!(s.engine.control_state().unwrap(), before);
    drop(s);
    let s = open(&d, clock);
    assert_eq!(capture(&s, d.ns).1, Some(original));
}
#[test]
fn namespace_binding_rejects_foreign_guard_and_missing_identity_is_not_a_name_guess() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let da = fixture(a.path());
    let db = fixture(b.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let sa = open(&da, clock.clone());
    let sb = open(&db, clock);
    let guard = sa.metadata().native_guard().unwrap();
    assert!(
        sb.metadata()
            .capture_namespace_binding(&guard, db.ns)
            .is_err()
    );
    drop(guard);
    let before = sb.engine.control_state().unwrap();
    assert_eq!(
        capture(&sb, NamespaceId::new(u32::MAX)),
        (before.generation_ref(), None)
    );
    assert_eq!(sb.engine.control_state().unwrap(), before);
}
#[test]
fn namespace_binding_unknown_profile_stays_unknown_after_purge() {
    let root = tempfile::tempdir().unwrap();
    let mut unknown = NamespaceId::new(0);
    let d = fixture_with_seed(root.path(), |engine| {
        let mut ns = NamespaceConfig::default_namespace(1234);
        ns.name = "unknown".into();
        ns.embedding_dim = 4;
        unknown = engine.create_namespace(&ns).unwrap();
    });
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock);
    let original = capture(&s, unknown).1.unwrap();
    assert!(matches!(
        original.schema.embedding_binding,
        crate::model::schema::EmbeddingBinding::UnknownLegacy
    ));
    let mut key = ctx(&s, &d, "unknown-purge");
    key.scope = MutationScope::Namespace(unknown);
    let mut p = s.engine.begin_namespace_purge(&key, false).unwrap();
    while p.stage != crate::storage::purge::PurgeStage::Complete {
        p = s
            .engine
            .resume_namespace_purge(p.store_id, unknown, p.start_receipt.operation_id, 1)
            .unwrap();
    }
    assert_eq!(capture(&s, unknown).1, Some(original));
}
