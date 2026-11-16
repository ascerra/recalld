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
    let d = certified_fixture(root.path());
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
        let different_payload = if feedback {
            publisher
                .feedback_native_with_work(
                    context.clone(),
                    id,
                    4,
                    None,
                    NativePolicyInputs::default(),
                    clock.clone(),
                    None,
                )
                .await
        } else {
            publisher
                .forget_native_with_work(
                    context.clone(),
                    MemoryId::from(uuid::Uuid::new_v4()),
                    None,
                    NativePolicyInputs::default(),
                    clock.clone(),
                    None,
                )
                .await
        };
        assert!(matches!(
            different_payload,
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
    // Simulate malformed retained evidence on the synthetic owning handle only.
    // Both consumers must retain the known receipt and reject without writes.
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    {
        let storage = f.storage.read().unwrap();
        let tx = storage.meta_store().db.begin_write().unwrap();
        tx.open_table(crate::storage::metadata::purge::PROGRESS)
            .unwrap()
            .remove(p.start_receipt.operation_id.as_bytes().as_slice())
            .unwrap();
        tx.commit().unwrap();
    }
    let before = bytes(&d.path);
    let calls = clock.0.load(Ordering::SeqCst);
    let error = invoke(
        &publisher,
        feedback,
        context,
        id,
        Some("native"),
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
            let mut expected = receipt;
            expected.replayed = !feedback;
            assert_eq!(*known, expected);
            assert!(matches!(*source, StorageError::InvalidControl(_)));
        }
        other => panic!("binding corruption lost committed receipt: {other:?}"),
    }
    assert_eq!(bytes(&d.path), before);
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retired_binding_feedback_consumer_replays_original_hash_after_purge_name_reuse_reopen() {
    retired_replay(true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retired_binding_forget_consumer_replays_original_receipt_after_purge_name_reuse_reopen() {
    retired_replay(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_current_purge_reader_rejects_future_malformed_rows_and_missing_anchors_immutably() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock);
    let mut progress = s
        .engine
        .begin_namespace_purge(&ctx(&s, &d, "purge"), true)
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
    let f = Fixture::from_open_engine(root, s.engine);
    let publisher = completion(&f);
    publisher.rebuild_all().await.unwrap();
    let target = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    let deadline = || std::time::Instant::now() + std::time::Duration::from_secs(5);
    assert!(
        publisher
            .readiness_barrier(target, deadline())
            .await
            .unwrap()
            .ready
    );
    let before_schema_check = bytes(&d.path);
    {
        let storage = f.storage.read().unwrap();
        let meta = storage.meta_store();
        let read = meta.db.begin_read().unwrap();
        let mut legacy_control = meta.control_state().unwrap();
        legacy_control.control_schema_version = 1;
        assert!(matches!(
            crate::storage::metadata::publication::check_current_pending(
                &read,
                &legacy_control,
                || Ok(())
            ),
            Err(StorageError::InvalidControl(_))
        ));
    }
    assert_eq!(bytes(&d.path), before_schema_check);
    for case in 0..9 {
        let mut changed = progress.clone();
        match case {
            0 => changed.version = 3,
            1 => changed.version = 1,
            2 => changed.name = "forged-name".into(),
            3 => changed.start_receipt.store_id = uuid::Uuid::new_v4(),
            4 => changed.last_receipt.operation_id = uuid::Uuid::new_v4(),
            5 => changed.next_operation_id = Some(uuid::Uuid::new_v4()),
            6 => changed.sequence += 1,
            7 => {
                let MutationOutcome::NativeNamespacePurgeAdvanced { transition } =
                    &mut changed.last_receipt.outcome
                else {
                    panic!()
                };
                transition.namespace_id = NamespaceId::new(u32::MAX);
            }
            _ => changed.last_receipt.receipt_version = 2,
        }
        {
            let storage = f.storage.read().unwrap();
            let tx = storage.meta_store().db.begin_write().unwrap();
            tx.open_table(crate::storage::metadata::purge::PROGRESS)
                .unwrap()
                .insert(
                    progress.start_receipt.operation_id.as_bytes().as_slice(),
                    control::encode(&changed).unwrap().as_slice(),
                )
                .unwrap();
            tx.commit().unwrap();
        }
        let before = bytes(&d.path);
        assert!(
            matches!(
                publisher.readiness_barrier(target, deadline()).await,
                Err(StorageError::InvalidControl(_))
            ),
            "case{case}"
        );
        assert_eq!(bytes(&d.path), before, "case{case}");
        {
            let storage = f.storage.read().unwrap();
            let tx = storage.meta_store().db.begin_write().unwrap();
            tx.open_table(crate::storage::metadata::purge::PROGRESS)
                .unwrap()
                .insert(
                    progress.start_receipt.operation_id.as_bytes().as_slice(),
                    control::encode(&progress).unwrap().as_slice(),
                )
                .unwrap();
            tx.commit().unwrap();
        }
    }
    assert!(
        publisher
            .readiness_barrier(target, deadline())
            .await
            .unwrap()
            .ready
    );
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref(),
        target
    );
}
