use super::*;
use crate::storage::control::WriteReceipt;
use crate::storage::native_catalog::NativeNamespaceInput;

async fn invoke(
    publisher: &crate::derived_completion::DerivedCompletion,
    feedback: bool,
    context: MutationContext,
    id: MemoryId,
    name: Option<&str>,
    clock: Arc<FixedClock>,
    retry: bool,
) -> Result<WriteReceipt, StorageError> {
    let policy = NativePolicyInputs {
        multiplier: if retry { f64::NAN } else { 1.0 },
        ..Default::default()
    };
    if feedback {
        publisher
            .feedback_native_with_work(context, id, 3, name.map(str::to_owned), policy, clock, None)
            .await
    } else {
        publisher
            .forget_native_with_work(context, id, name.map(str::to_owned), policy, clock, None)
            .await
    }
}

async fn retired_replay(feedback: bool) {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let intent = s.capture_insert(scope(&d), &input(&d)).unwrap();
    let created = s
        .insert_prepared(&ctx(&s, &d, "create"), &intent, &generated())
        .unwrap();
    let id = inserted(&created);
    let mut context = ctx(&s, &d, "original-operation");
    context.expected_generation = Some(created.committed_generation);
    let f = Fixture::from_open_engine(root, s.engine);
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let receipt = invoke(
        &publisher,
        feedback,
        context.clone(),
        id,
        Some("native"),
        clock.clone(),
        false,
    )
    .await
    .unwrap();
    drop(publisher);
    let root = f.close();
    let mut s = open(&d, clock.clone());
    let mut p = s
        .engine
        .begin_namespace_purge(&ctx(&s, &d, "purge"), true)
        .unwrap();
    while p.stage != crate::storage::purge::PurgeStage::Complete {
        p = s
            .engine
            .resume_namespace_purge(p.store_id, d.ns, p.start_receipt.operation_id, 1)
            .unwrap();
    }
    assert!(s.engine.get_record(id).unwrap().is_none());
    assert!(s.engine.get_namespace(d.ns).unwrap().is_none());
    let mut f = Fixture::from_open_engine(root, s.engine);
    // Exercise the actual completion consumer before reuse, after recreation,
    // and after reopening the real durable mixed history.
    for stage in 0..3 {
        let publisher = completion(&f);
        publisher.rebuild_all().await.unwrap();
        let current = f
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
        let calls = clock.0.load(Ordering::SeqCst);
        let before = bytes(&d.path);
        for name in [None, Some("native")] {
            let replay = invoke(
                &publisher,
                feedback,
                context.clone(),
                id,
                name,
                clock.clone(),
                true,
            )
            .await
            .unwrap();
            let mut expected = receipt.clone();
            expected.replayed = true;
            assert_eq!(replay, expected);
        }
        let error = invoke(
            &publisher,
            feedback,
            context.clone(),
            id,
            Some("wrong-name"),
            clock.clone(),
            true,
        )
        .await
        .unwrap_err();
        match error {
            StorageError::CommittedMutation {
                receipt: known,
                source,
            } => {
                let mut expected = receipt.clone();
                expected.replayed = true;
                assert_eq!(*known, expected);
                assert!(matches!(*source, StorageError::NamespaceMismatch { .. }));
            }
            other => panic!("lost known receipt: {other:?}"),
        }
        let mut changed = context.clone();
        changed.expected_generation = None;
        assert!(matches!(
            invoke(&publisher, feedback, changed, id, None, clock.clone(), true).await,
            Err(StorageError::IdempotencyConflict)
        ));
        let mut foreign = context.clone();
        foreign.store_id = Some(uuid::Uuid::new_v4());
        assert!(matches!(
            invoke(&publisher, feedback, foreign, id, None, clock.clone(), true).await,
            Err(StorageError::StoreLineageMismatch)
        ));
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .generation_ref(),
            current
        );
        assert_eq!(bytes(&d.path), before);
        drop(publisher);
        let root = f.close();
        let mut s = open(&d, clock.clone());
        if stage == 0 {
            let mut catalog = ctx(&s, &d, "reuse-name");
            catalog.scope = MutationScope::Catalog;
            let created = s
                .engine
                .create_native_namespace(
                    &catalog,
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
            let MutationOutcome::NativeNamespaceCreated { transition } = created.outcome else {
                panic!()
            };
            assert_ne!(transition.namespace_id, d.ns);
        }
        f = Fixture::from_open_engine(root, s.engine);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retired_binding_feedback_consumer_replays_original_hash_after_purge_name_reuse_reopen() {
    retired_replay(true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retired_binding_forget_consumer_replays_original_receipt_after_purge_name_reuse_reopen() {
    retired_replay(false).await;
}
