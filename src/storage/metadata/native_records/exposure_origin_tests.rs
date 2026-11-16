use super::*;
use crate::model::AccessKind;
use crate::search::{RetrievalSemantics, VersionMode};

pub(super) fn assert_origin(
    f: &Fixture,
    ns: NamespaceId,
    origin: u32,
    source: Option<MemoryId>,
    accesses: &[(MemoryId, AccessKind)],
) {
    let storage = f.storage.read().unwrap();
    let state = storage.control_state().unwrap();
    let read = storage.meta_store().db.begin_read().unwrap();
    let table = read.open_table(control::RECEIPTS).unwrap();
    let stored = table
        .iter()
        .unwrap()
        .map(|r| control::validate_receipt(r.unwrap().1.value()).unwrap())
        .find(|s| {
            s.receipt.store_id == state.store_id
                && s.receipt.committed_generation == state.committed_generation
        })
        .unwrap();
    assert!(matches!(
        stored.receipt.outcome,
        MutationOutcome::LifecycleExposureRecorded { .. }
    ));
    let scope = crate::storage::native_lifecycle::LifecycleScope {
        namespace_id: ns,
        namespace_name: "native".into(),
        source,
    };
    let semantic = control::encode(&(scope, accesses, None::<u8>)).unwrap();
    let mut ctx = MutationContext::unkeyed(ns);
    ctx.contract_version = origin;
    assert_eq!(
        stored.request_hash,
        control::request_hash(&ctx, "native_exposure", &[&semantic])
    );
    ctx.contract_version = 3 - origin;
    assert_ne!(
        stored.request_hash,
        control::request_hash(&ctx, "native_exposure", &[&semantic])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exposure_origin_native_matrix_keeps_api_policy_semantics_independent() {
    for policy in [
        LifecycleVersion::LegacyAccessV1,
        LifecycleVersion::ExplicitFeedbackV2,
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut d = fixture(root.path());
        if policy == LifecycleVersion::LegacyAccessV1 {
            let inspection = inspect_schema(&d.path).unwrap();
            let s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
            let revision = s
                .engine
                .get_namespace(d.ns)
                .unwrap()
                .unwrap()
                .schema
                .unwrap()
                .policy_revision;
            drop(s);
            let dest = root.path().join("legacy-policy");
            migrate_lifecycle(
                &d.path,
                &dest,
                &LifecycleMigrationOptions {
                    expected_source: inspection.control.generation_ref(),
                    namespace_id: d.ns,
                    expected_policy_revision: revision,
                    target: policy,
                    idempotency_key: "back-to-v1".into(),
                },
            )
            .unwrap();
            d.path = dest;
        }
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let session = open(&d, clock.clone());
        let f = Fixture::from_open_engine(root, session.engine);
        let owner = completion(&f);
        owner.rebuild_all().await.unwrap();
        let (api, bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
        let initial = f.record(d.parent).unwrap();
        let query:SearchQuery=serde_json::from_value(json!({"namespace":"native","queryMode":"metadata_only","graphDepth":0,"filter":{"minStrength":0.0},"recordAccess":true})).unwrap();
        for origin in [1, 2] {
            for semantics in [
                RetrievalSemantics::V1,
                RetrievalSemantics::V2(VersionMode::Current),
            ] {
                let before = bytes(&d.path);
                let mut readonly = query.clone();
                readonly.record_access = false;
                let result = engine
                    .search_checked_with_origin_work(readonly, semantics, false, None, origin)
                    .await
                    .unwrap();
                assert_eq!(result.response.results.len(), 1);
                assert_eq!(bytes(&d.path), before);
                let calls = clock.0.load(Ordering::SeqCst);
                let result = engine
                    .search_checked_with_origin_work(query.clone(), semantics, false, None, origin)
                    .await
                    .unwrap();
                assert_eq!(clock.0.load(Ordering::SeqCst), calls + 1);
                let accesses: Vec<_> = result
                    .response
                    .results
                    .iter()
                    .map(|r| (r.memory_id, AccessKind::DirectRetrieval))
                    .collect();
                assert_origin(&f, d.ns, origin, None, &accesses);
                let after = f.record(d.parent).unwrap();
                let old = &initial.schema.as_ref().unwrap().lifecycle;
                let now = &after.schema.as_ref().unwrap().lifecycle;
                assert_eq!(now.lifecycle_version, policy);
                assert_eq!(after.stability, initial.stability);
                assert_eq!(now.reinforced_at, old.reinforced_at);
                assert_eq!(
                    now.decay_anchor_at,
                    if policy == LifecycleVersion::LegacyAccessV1 {
                        now.retrieved_at.unwrap()
                    } else {
                        old.decay_anchor_at
                    }
                );
            }
            engine
                .get_memory_with_origin_work(d.parent, true, None, origin)
                .await
                .unwrap()
                .unwrap();
            assert_origin(
                &f,
                d.ns,
                origin,
                Some(d.parent),
                &[(d.parent, AccessKind::DirectRetrieval)],
            );
        }
        // Existing Rust semantics2 convenience entry is still application1.
        engine
            .search_with_semantics(query, RetrievalSemantics::V2(VersionMode::Current))
            .await
            .unwrap();
        assert_origin(
            &f,
            d.ns,
            1,
            None,
            &[(d.parent, AccessKind::DirectRetrieval)],
        );
        drop((api, bridge, engine, owner));
        let _root = f.close();
        inspect_schema(&d.path).unwrap();
        let _reopened = open(&d, clock);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exposure_origin_native_similar_and_explicit_sample_use_original_version() {
    let (f, d, clock, id) = setup();
    completion(&f).rebuild_all().await.unwrap();
    let (_api, _bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
    for origin in [1, 2] {
        let before = bytes(&d.path);
        engine
            .similar_with_access_checked_with_origin_work(id, 10, None, false, false, None, origin)
            .await
            .unwrap();
        assert_eq!(bytes(&d.path), before);
        let result = engine
            .similar_with_access_checked_with_origin_work(id, 10, None, true, false, None, origin)
            .await
            .unwrap();
        assert!(!result.is_empty());
        let accesses: Vec<_> = result
            .iter()
            .map(|r| (r.memory_id, AccessKind::AssociativeRetrieval))
            .collect();
        assert_origin(&f, d.ns, origin, Some(id), &accesses);
        let calls = clock.0.load(Ordering::SeqCst);
        let accesses = [(id, AccessKind::DirectRetrieval)];
        engine
            .record_exposures_at_with_origin_work(
                &accesses,
                scope_for(&d),
                100_000_000,
                None,
                origin,
            )
            .await
            .unwrap();
        assert_origin(&f, d.ns, origin, Some(d.parent), &accesses);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert_eq!(
            f.record(id).unwrap().schema.unwrap().lifecycle.retrieved_at,
            Some(100_000_000)
        );
    }
}

#[tokio::test]
async fn exposure_origin_storage_recorder_preserves_both_versions_on_legacy_storage() {
    use crate::search::AccessRecorder;
    let f = Fixture::new();
    let id = f.insert_memory().await;
    struct RecorderClock(AtomicUsize);
    impl crate::time::Clock for RecorderClock {
        fn now_millis(&self) -> i64 {
            self.0.fetch_add(1, Ordering::SeqCst);
            1_900_000_000_007
        }
    }
    let clock = Arc::new(RecorderClock(AtomicUsize::new(0)));
    let recorder =
        StorageAccessRecorder::new(f.storage.clone(), f.cache.clone()).with_clock(clock.clone());
    let accesses = [(id, AccessKind::DirectRetrieval)];
    let scope = ExposureScope {
        namespace: "default".into(),
        source: Some(id),
    };
    for origin in [1, 2] {
        let result = recorder
            .record_access_batch_for_publication(
                &accesses,
                Some(&scope),
                Some(1_900_000_000_000),
                None,
                None,
                origin,
            )
            .await
            .unwrap();
        assert_eq!(result.receipts.len(), 1);
        assert_eq!(
            clock.0.load(Ordering::SeqCst),
            0,
            "explicit sampled time must not call recorder clock"
        );
        let s = f.storage.read().unwrap();
        let read = s.meta_store().db.begin_read().unwrap();
        use redb::ReadableTable;
        let stored = read
            .open_table(control::RECEIPTS)
            .unwrap()
            .iter()
            .unwrap()
            .map(|r| control::validate_receipt(r.unwrap().1.value()).unwrap())
            .find(|r| r.receipt.operation_id == result.receipts[0].operation_id)
            .unwrap();
        let mut ctx = MutationContext::unkeyed(NamespaceId::new(1));
        ctx.contract_version = origin;
        let encoded = control::encode(&accesses.to_vec()).unwrap();
        let assertion = control::encode(&scope).unwrap();
        assert_eq!(
            stored.request_hash,
            control::request_hash(&ctx, "record_exposure_batch", &[&encoded, &assertion])
        );
    }
    // Compare the retained pre-change API1 dispatch method with the new direct
    // publication helper. Both real transactions must sample exactly once.
    let old = recorder
        .record_access_batch_with_work(&accesses, Some(&scope), None)
        .await
        .unwrap();
    assert_eq!(old.len(), 1);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.record(id)
            .unwrap()
            .access_history
            .last()
            .unwrap()
            .timestamp,
        1_900_000_000_007
    );
    for origin in [1, 2] {
        let calls = clock.0.load(Ordering::SeqCst);
        let observed = recorder
            .record_access_batch_for_publication(&accesses, Some(&scope), None, None, None, origin)
            .await
            .unwrap();
        assert_eq!(observed.receipts.len(), 1);
        assert_eq!(
            clock.0.load(Ordering::SeqCst),
            calls + 1,
            "None dispatch must preserve the existing one-sample behavior"
        );
        assert_eq!(
            f.record(id)
                .unwrap()
                .access_history
                .last()
                .unwrap()
                .timestamp,
            1_900_000_000_007
        );
    }
    let before = f.record(id).unwrap().encode().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    assert!(
        recorder
            .record_access_batch_for_publication(&accesses, Some(&scope), None, None, None, 3)
            .await
            .is_err()
    );
    assert_eq!(f.record(id).unwrap().encode().unwrap(), before);
    assert_eq!(
        clock.0.load(Ordering::SeqCst),
        calls,
        "unsupported origin rejects before clock"
    );
    fn raw(s: &RedbStorageEngine) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
        use redb::ReadableTable;
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
    let (receipts, state) = {
        let s = f.storage.read().unwrap();
        (raw(&s), s.control_state().unwrap())
    };
    let empty = recorder
        .record_access_batch_for_publication(&[], Some(&scope), None, None, None, 3)
        .await
        .unwrap();
    assert!(empty.receipts.is_empty());
    assert_eq!(clock.0.load(Ordering::SeqCst), calls);
    {
        let s = f.storage.read().unwrap();
        assert_eq!(raw(&s), receipts);
        assert_eq!(s.control_state().unwrap(), state);
    }
    assert_eq!(f.record(id).unwrap().encode().unwrap(), before);
    drop(recorder);
    let root = f.close();
    crate::storage::validation::canonical_files(root.path()).unwrap();
    let reopened = RedbStorageEngine::open(root.path()).unwrap();
    assert_eq!(reopened.control_state().unwrap(), state);
    assert_eq!(
        raw(&reopened),
        receipts,
        "strict reopen must retain API1 and API2 receipt bytes"
    );
    assert_eq!(
        reopened.get_record(id).unwrap().unwrap().encode().unwrap(),
        before
    );
}
