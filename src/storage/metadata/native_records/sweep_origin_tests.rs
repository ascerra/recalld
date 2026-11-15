//! Actual request-owned manual and autonomous producers use the same runner.
use super::*;
use crate::api::FsrsEngine as _;
use crate::storage::control::{self, MutationContext};

fn adapter(f: &Fixture, c: Arc<CountingClock>) -> crate::api::adapters::FsrsEngineAdapter {
    crate::api::adapters::FsrsEngineAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        f.graph.clone(),
        Arc::new(Default::default()),
        false,
    )
    .with_indexes(
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
    )
    .with_clock(c)
    .with_native_lifecycle_policy(NativePolicyInputs::default())
}
fn last_native_hash(f: &Fixture, id: MemoryId, ns: NamespaceId, origin: u32) {
    let s = f.storage.read().unwrap();
    let read = s.meta_store().db.begin_read().unwrap();
    let latest = read
        .open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|r| control::validate_receipt(r.unwrap().1.value()).unwrap())
        .filter(|r| match &r.receipt.outcome {
            MutationOutcome::NativePhaseMaterialized { transition } => transition.memory_id == id,
            MutationOutcome::NativeMemoryHardDeleted { transition } => transition.memory_id == id,
            _ => false,
        })
        .max_by_key(|r| r.receipt.committed_generation)
        .unwrap();
    let mut ctx = MutationContext::unkeyed(ns);
    ctx.contract_version = origin;
    assert_eq!(
        latest.request_hash,
        control::request_hash(&ctx, "native_maintenance", &[id.as_bytes()])
    );
    ctx.contract_version = 3 - origin;
    assert_ne!(
        latest.request_hash,
        control::request_hash(&ctx, "native_maintenance", &[id.as_bytes()])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_origin_native_api_one_and_autonomous_two_are_real_producers() {
    for autonomous in [false, true] {
        let (f, d, id, c) = setup();
        completion(&f).rebuild_all().await.unwrap();
        c.set(10_000 * DAY);
        let original = row(&f, id);
        let sweep = runner(&f, c.clone(), false);
        let handle = autonomous.then(|| sweep.start());
        let manual = adapter(&f, c.clone());
        for expected in [Some(DecayPhase::Summary), Some(DecayPhase::Ghost), None] {
            let calls = c.calls.load(Ordering::SeqCst);
            if autonomous {
                let before = target(&f);
                sweep.trigger();
                let report = completion(&f)
                    .readiness_barrier(before, Instant::now() + Duration::from_secs(10))
                    .await
                    .unwrap();
                assert!(report.ready);
                assert_eq!(report.committed_generation, target(&f).generation);
            } else {
                let result = manual.trigger_sweep(None).await.unwrap();
                assert!(result.errors.is_empty(), "{:?}", result.errors);
                ready(&f).await;
            }
            assert_eq!(c.calls.load(Ordering::SeqCst), calls + 1);
            let current = f.storage.read().unwrap().get_record(id).unwrap();
            assert_eq!(current.as_ref().map(|r| r.phase), expected);
            if let Some(current) = current {
                retained(&original, &current);
            }
            last_native_hash(&f, id, d.ns, if autonomous { 2 } else { 1 });
        }
        if let Some(handle) = handle {
            sweep.shutdown();
            handle.await.unwrap();
        }
        drop((manual, sweep));
        let _root = f.close();
        inspect_schema(&d.path).unwrap();
        assert!(RedbStorageEngine::open(&d.path).is_err());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_origin_native_requires_configured_policy_and_rejects_unknown_before_clock() {
    let (f, _d, id, c) = setup_with_multiplier(0.);
    completion(&f).rebuild_all().await.unwrap();
    c.set(10_000 * DAY);
    let before = row(&f, id).encode().unwrap();
    let calls = c.calls.load(Ordering::SeqCst);
    let at = target(&f);
    let no_policy = crate::api::adapters::FsrsEngineAdapter::new(
        f.storage.clone(),
        f.cache.clone(),
        f.graph.clone(),
        Arc::new(Default::default()),
        false,
    )
    .with_indexes(
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
    )
    .with_clock(c.clone());
    assert!(no_policy.trigger_sweep(None).await.is_err());
    assert_eq!(c.calls.load(Ordering::SeqCst), calls);
    assert_eq!(target(&f), at);
    let manual = adapter(&f, c.clone());
    assert!(manual.trigger_sweep_with_origin(None, 3).await.is_err());
    assert_eq!(c.calls.load(Ordering::SeqCst), calls);
    assert_eq!(row(&f, id).encode().unwrap(), before);
    // Explicitly adopted native multiplier0 overrides the unrelated adapter legacy1.
    let manual = manual.with_native_lifecycle_policy(NativePolicyInputs {
        multiplier: 0.,
        ..Default::default()
    });
    let skipped = manual.trigger_sweep(None).await.unwrap();
    assert!(skipped.errors.is_empty());
    assert_eq!(skipped.full_to_summary, 0);
    assert_eq!(row(&f, id).encode().unwrap(), before);
    assert_eq!(target(&f), at);
    assert_eq!(c.calls.load(Ordering::SeqCst), calls + 1);
}

fn raw_receipts(f: &Fixture) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
    let s = f.storage.read().unwrap();
    raw_store_receipts(&s)
}
fn raw_store_receipts(s: &RedbStorageEngine) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
    s.meta_store()
        .db
        .begin_read()
        .unwrap()
        .open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|r| {
            let (k, v) = r.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}
fn last_legacy_hash(
    f: &Fixture,
    id: MemoryId,
    before: &DiskRecord,
    ns: &NamespaceConfig,
    origin: u32,
) {
    let s = f.storage.read().unwrap();
    let read = s.meta_store().db.begin_read().unwrap();
    let latest = read
        .open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|r| control::validate_receipt(r.unwrap().1.value()).unwrap())
        .filter(|r| match &r.receipt.outcome {
            MutationOutcome::LifecycleMaterialized { memory_id, .. } => *memory_id == id,
            MutationOutcome::GraphChanged {
                memories_removed, ..
            } => *memories_removed == 1,
            _ => false,
        })
        .max_by_key(|r| r.receipt.committed_generation)
        .unwrap();
    let hash = |origin| {
        let mut ctx = MutationContext::unkeyed(ns.id);
        ctx.contract_version = origin;
        if let Some(after) = s.get_record(id).unwrap() {
            control::request_hash(
                &ctx,
                "lifecycle_materialize_v1",
                &[
                    id.as_bytes(),
                    &before.encode().unwrap(),
                    &control::encode(ns).unwrap(),
                    &[after.phase.as_u8()],
                    &after.strength.to_bits().to_be_bytes(),
                    &after.decay_strength.to_bits().to_be_bytes(),
                ],
            )
        } else {
            let request = control::encode(&(
                Vec::<crate::storage::PersistedEdge>::new(),
                vec![id],
                true,
                true,
            ))
            .unwrap();
            control::request_hash(&ctx, "graph_effects", &[&request])
        }
    };
    assert_eq!(latest.request_hash, hash(origin));
    assert_ne!(latest.request_hash, hash(3 - origin));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_origin_legacy_manual_and_startup_support_phase_and_hard_delete() {
    for autonomous in [false, true] {
        let f = Fixture::new();
        let id = f.insert_memory().await;
        completion(&f).rebuild_all().await.unwrap();
        let ns = f
            .storage
            .read()
            .unwrap()
            .get_namespace(NamespaceId::new(1))
            .unwrap()
            .unwrap();
        let c = Arc::new(CountingClock::new(DAY));
        let manual = adapter(&f, c.clone());
        // Real compatibility1 strength materialization before origin2 operations.
        // Preserve these exact bytes; this is not a historical binary fixture.
        manual.trigger_sweep(None).await.unwrap();
        assert_eq!(row(&f, id).phase, DecayPhase::Full);
        let old1 = raw_receipts(&f);
        assert!(!old1.is_empty());
        c.set(10_000 * DAY);
        let sweep = runner(&f, c.clone(), autonomous);
        let mut handle = None;
        for (index, expected) in [Some(DecayPhase::Summary), Some(DecayPhase::Ghost), None]
            .into_iter()
            .enumerate()
        {
            let before = row(&f, id);
            let calls = c.calls.load(Ordering::SeqCst);
            if autonomous {
                let prior = target(&f);
                if index == 0 {
                    handle = Some(sweep.start());
                } else {
                    sweep.trigger();
                }
                let report = completion(&f)
                    .readiness_barrier(prior, Instant::now() + Duration::from_secs(10))
                    .await
                    .unwrap();
                assert!(report.ready);
                assert_eq!(report.committed_generation, target(&f).generation);
            } else {
                let result = manual.trigger_sweep(None).await.unwrap();
                assert!(result.errors.is_empty(), "{:?}", result.errors);
                ready(&f).await;
            }
            assert_eq!(c.calls.load(Ordering::SeqCst), calls + 1);
            let after = f.storage.read().unwrap().get_record(id).unwrap();
            assert_eq!(after.as_ref().map(|r| r.phase), expected);
            last_legacy_hash(&f, id, &before, &ns, if autonomous { 2 } else { 1 });
            let current = raw_receipts(&f);
            for (key, value) in &old1 {
                assert_eq!(current.get(key), Some(value));
            }
        }
        if let Some(handle) = handle {
            sweep.shutdown();
            handle.await.unwrap();
        }
        let final_rows = raw_receipts(&f);
        drop((manual, sweep));
        let root = f.close();
        crate::storage::validation::canonical_files(root.path()).unwrap();
        let s = RedbStorageEngine::open(root.path()).unwrap();
        assert!(s.get_record(id).unwrap().is_none());
        assert_eq!(raw_store_receipts(&s), final_rows);
        for (key, value) in old1 {
            assert_eq!(raw_store_receipts(&s).get(&key), Some(&value));
        }
    }
}

#[tokio::test]
async fn manual_origin_legacy_harddelete_unknown_origin_rejects_before_missing_lookup_or_write() {
    let f = Fixture::new();
    let id = f.insert_memory().await;
    let before = std::fs::read(f.path().join("meta.db")).unwrap();
    let state = f.storage.read().unwrap().control_state().unwrap();
    for target in [id, MemoryId::new()] {
        let error = f
            .storage
            .write()
            .unwrap()
            .delete_memory_with_edges_with_origin_work(target, &[], None, 3)
            .unwrap_err();
        assert!(matches!(error, StorageError::InvalidMutationContext(_)));
        assert_eq!(std::fs::read(f.path().join("meta.db")).unwrap(), before);
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), state);
    }
    assert!(f.record(id).is_some());
}
