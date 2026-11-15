use super::*;

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
            ctx.expected_generation = Some(f.storage.read().unwrap().control_state().unwrap().committed_generation);
            let mut input = document_recipe_input(&d);
            if !generated {
                input.insert.memory.embedding = Some(vec![2., -0., 0., 0.]);
            }
            let profile_id = (!generated).then(|| d.profile.profile_id().unwrap());
            let result = a.store_native_single_by_profile_id(ctx.clone(), input.clone(), profile_id.clone(), NativePolicyInputs::default(), None).await.unwrap();
            assert_eq!(p.calls.load(Ordering::SeqCst), usize::from(generated));
            if generated {
                assert_eq!(*p.texts.lock().unwrap(), vec!["header\n\nbody raw entity/alice raw entity/bob topic/rust emotion/calm"]);
            }
            let before = f.storage.read().unwrap().control_state().unwrap();
            let rows = receipt_rows(&f);
            let clock_before = clock.0.load(Ordering::SeqCst);
            let mut unavailable = provider(&d);
            Arc::get_mut(&mut unavailable).unwrap().profile = None;
            let mut config = crate::config::RecalldConfig::default();
            config.graph.auto_link_threshold = f64::NAN;
            let replay_adapter = adapter(&f, unavailable.clone(), clock.clone(), config);
            let replay = replay_adapter.store_native_single_by_profile_id(ctx.clone(), input.clone(), profile_id.clone(), NativePolicyInputs::default(), None).await.unwrap();
            assert_eq!(replay.receipt, expected_replay(&result.receipt));
            ctx.contract_version = 3 - contract_version;
            assert!(matches!(replay_adapter.store_native_single_by_profile_id(ctx, input, profile_id, NativePolicyInputs::default(), None).await, Err(StorageError::IdempotencyConflict)));
            assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
            assert_eq!(receipt_rows(&f), rows);
            assert_eq!(clock.0.load(Ordering::SeqCst), clock_before);
            assert_eq!(unavailable.profiles.load(Ordering::SeqCst), 0);
            assert_eq!(unavailable.calls.load(Ordering::SeqCst), 0);
        }
    }
}
