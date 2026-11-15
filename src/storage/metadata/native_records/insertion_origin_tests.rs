use super::*;
use crate::storage::request_context::RetainedRequestContext;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_origin_single_actual_1_and_2_keep_identity_and_replay_before_provider() {
    for contract_version in [1, 2] {
        for generated in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let d = fixture(root.path());
            let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
            let s = open(&d, clock.clone());
            let f = Fixture::from_open_engine(root, s.engine);
            completion(&f).rebuild_all().await.unwrap();
            let p = provider(&d);
            let a = adapter(&f, p.clone(), clock.clone(), Default::default());
            let mut ctx = single_context(&f, &d, "origin");
            ctx.contract_version = contract_version;
            ctx.expected_generation = Some(
                f.storage
                    .read()
                    .unwrap()
                    .control_state()
                    .unwrap()
                    .committed_generation,
            );
            let mut input = document_recipe_input(&d);
            if !generated {
                input.insert.memory.embedding = Some(vec![2., -0., 0., 0.]);
            }
            let profile_id = (!generated).then(|| d.profile.profile_id().unwrap());
            let result = a
                .store_native_single_by_profile_id(
                    ctx.clone(),
                    input.clone(),
                    profile_id.clone(),
                    NativePolicyInputs::default(),
                    None,
                )
                .await
                .unwrap();
            assert_eq!(p.calls.load(Ordering::SeqCst), usize::from(generated));
            if generated {
                assert_eq!(
                    *p.texts.lock().unwrap(),
                    vec!["header\n\nbody raw entity/alice raw entity/bob topic/rust emotion/calm"]
                );
            }
            let before = f.storage.read().unwrap().control_state().unwrap();
            let rows = receipt_rows(&f);
            let clock_before = clock.0.load(Ordering::SeqCst);
            let mut unavailable = provider(&d);
            Arc::get_mut(&mut unavailable).unwrap().profile = None;
            let mut config = crate::config::RecalldConfig::default();
            config.graph.auto_link_threshold = f64::NAN;
            let replay_adapter = adapter(&f, unavailable.clone(), clock.clone(), config);
            let replay = replay_adapter
                .store_native_single_by_profile_id(
                    ctx.clone(),
                    input.clone(),
                    profile_id.clone(),
                    NativePolicyInputs::default(),
                    None,
                )
                .await
                .unwrap();
            assert_eq!(replay.receipt, expected_replay(&result.receipt));
            ctx.contract_version = 3 - contract_version;
            assert!(matches!(
                replay_adapter
                    .store_native_single_by_profile_id(
                        ctx,
                        input,
                        profile_id,
                        NativePolicyInputs::default(),
                        None
                    )
                    .await,
                Err(StorageError::IdempotencyConflict)
            ));
            assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
            assert_eq!(receipt_rows(&f), rows);
            assert_eq!(clock.0.load(Ordering::SeqCst), clock_before);
            assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
            assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_origin_batch_actual_runner_replay_and_cross_contract_conflict() {
    for version in [2, 3] {
        for contract in [1, 2] {
            let root = tempfile::tempdir().unwrap();
            let d = fixture(root.path());
            let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
            let s = open(&d, clock.clone());
            let f = Fixture::from_open_engine(root, s.engine);
            completion(&f).rebuild_all().await.unwrap();
            let p = provider(&d);
            let mut config = crate::config::RecalldConfig::default();
            if version == 2 {
                config.graph.autolink_enabled = false;
            }
            let a = adapter(&f, p.clone(), clock.clone(), config.clone());
            let mut r = request(&f, "batch-origin");
            r.mutation_version = version;
            r.request_context = RetainedRequestContext::from_contract(contract).unwrap();
            r.items = vec![
                json!({"summary":"header", "fullText":"body", "tags":["raw"], "entities":["José"], "checkDuplicates":false}),
                json!({"summary":false}),
            ];
            if version == 3 {
                r.expected_generation = Some(
                    f.storage
                        .read()
                        .unwrap()
                        .control_state()
                        .unwrap()
                        .committed_generation,
                );
            }
            let result = a.run_batch(r.clone()).await.unwrap();
            assert_eq!(result.succeeded, 1, "{result:?}");
            assert_eq!(result.failed, 1, "{result:?}");
            assert_eq!(result.unresolved, 0);
            assert!(result.items[0].ready);
            assert_eq!(
                *p.texts.lock().unwrap(),
                vec!["header\n\nbody raw entity/josé"]
            );
            let parent = result.reservation_receipt.as_ref().unwrap();
            let manifest = f
                .storage
                .read()
                .unwrap()
                .meta_store()
                .lookup_batch(parent.store_id, parent.operation_id)
                .unwrap()
                .unwrap();
            assert_eq!(manifest.original_contract().unwrap(), contract);
            assert_eq!(
                manifest.child(0).unwrap().context().contract_version,
                contract
            );
            assert_eq!(
                manifest.reservation_expected_generation().unwrap(),
                r.expected_generation
            );
            let before = f.storage.read().unwrap().control_state().unwrap();
            let rows = receipt_rows(&f);
            let clock_before = clock.0.load(Ordering::SeqCst);
            let mut unavailable = provider(&d);
            Arc::get_mut(&mut unavailable).unwrap().profile = None;
            config.graph.auto_link_threshold = f64::NAN;
            let retry = adapter(&f, unavailable.clone(), clock.clone(), config);
            for by_operation in [false, true] {
                let mut same = r.clone();
                if by_operation {
                    same.idempotency_key = None;
                    same.batch_operation_id = Some(parent.operation_id);
                }
                let replay = retry.run_batch(same.clone()).await.unwrap();
                assert_eq!(replay.reservation_receipt, Some(expected_replay(parent)));
                assert_eq!(
                    replay.items[0].receipt,
                    Some(expected_replay(result.items[0].receipt.as_ref().unwrap()))
                );
                assert_eq!(replay.items[1].error, result.items[1].error);
                same.request_context = RetainedRequestContext::from_contract(3 - contract).unwrap();
                assert!(matches!(
                    retry.run_batch_typed(same).await,
                    Err(crate::mcp::bridge_adapters::StoreWriteError::Canonical(
                        StorageError::IdempotencyConflict
                    ))
                ));
            }
            assert_eq!(receipt_rows(&f), rows);
            assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
            assert_eq!(clock.0.load(Ordering::SeqCst), clock_before);
            assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
            assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_origin_api1_publication_failures_keep_exact_receipts_then_recover() {
    batch_publication_origin(1).await;
    single_publication_origin(1).await;
}
