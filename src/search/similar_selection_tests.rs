// Included in read_view::tests: actual canonical migration/attestation and owner.
use super::super::similar_selection::{SimilarityExclusion, SimilarityScope, SimilaritySelection};
use std::sync::atomic::{AtomicUsize, Ordering};

struct SimilarClock(AtomicUsize);
impl crate::time::Clock for SimilarClock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        10_000
    }
}
struct SimilarFixture {
    f: Fixture,
    engine: QueryEngine,
    owner: DerivedCompletion,
    clock: Arc<SimilarClock>,
    source: MemoryId,
    old: MemoryId,
    terminal: MemoryId,
    live: MemoryId,
    foreign: MemoryId,
    other: NamespaceId,
    mismatch: NamespaceId,
    unknown: NamespaceId,
}

async fn similar_insert(
    owner: &DerivedCompletion,
    ns: NamespaceId,
    n: u128,
    v: [f32; 4],
    tag: &str,
    old: Option<MemoryId>,
) -> MemoryId {
    let id = fixed_id(n);
    owner
        .insert(
            MutationContext::unkeyed(ns),
            InsertMemory {
                id: Some(id),
                summary: format!("similar {n}"),
                full_text: Some(format!("text {n}")),
                tags: vec![crate::model::Tag::new(tag).unwrap()],
                initial_stability: None,
                created_at: Some(100),
                embedding: Some(v.to_vec()),
                parent_id: None,
                supersedes: old,
            },
            Vec::new().into(),
            100,
        )
        .await
        .unwrap();
    id
}

async fn similar_fixture() -> SimilarFixture {
    similar_fixture_with_chain(0).await
}

async fn similar_fixture_with_chain(chain_edges: usize) -> SimilarFixture {
    use crate::storage::lifecycle_migration::{LifecycleMigrationOptions, migrate_lifecycle};
    use crate::storage::schema_migration::{
        MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema,
    };
    let legacy = Fixture::new();
    let o = owner(&legacy);
    let other = legacy.create_namespace("other");
    let mismatch = legacy.create_namespace("mismatch");
    let unknown = legacy.create_namespace("unknown");
    let source = similar_insert(
        &o,
        NamespaceId::new(1),
        2001,
        [1., 0., 0., 0.],
        "source",
        None,
    )
    .await;
    let old = similar_insert(
        &o,
        NamespaceId::new(1),
        2002,
        [0.96, 0.28, 0., 0.],
        "old",
        None,
    )
    .await;
    let terminal = similar_insert(
        &o,
        NamespaceId::new(1),
        2003,
        [0., 1., 0., 0.],
        "terminal",
        Some(old),
    )
    .await;
    let live = similar_insert(
        &o,
        NamespaceId::new(1),
        2004,
        [0.8, 0.6, 0., 0.],
        "allow",
        None,
    )
    .await;
    let foreign = similar_insert(&o, other, 2005, [0.9, 0.4358899, 0., 0.], "allow", None).await;
    similar_insert(&o, mismatch, 2006, [1., 0., 0., 0.], "allow", None).await;
    similar_insert(&o, unknown, 2007, [1., 0., 0., 0.], "allow", None).await;
    if chain_edges > 0 {
        let mut previous = None;
        for step in 0..=chain_edges {
            let vector = if step == 0 {
                [0.6, 0.8, 0., 0.]
            } else {
                [0., 1., 0., 0.]
            };
            previous = Some(
                similar_insert(
                    &o,
                    NamespaceId::new(1),
                    3000 + step as u128,
                    vector,
                    "chain",
                    previous,
                )
                .await,
            );
        }
    }
    for _ in 0..2 {
        legacy
            .insert_memory_with(crate::test_support::MemorySpec {
                phase: DecayPhase::Ghost,
                embedding: Some(&[1., 0., 0., 0.]),
                tags: &["allow"],
                ..crate::test_support::MemorySpec::new(NamespaceId::new(1))
            })
            .await;
    }
    legacy.storage.write().unwrap().sync().unwrap();
    drop(o);
    let legacy = legacy.close();
    let canonical = tempfile::tempdir().unwrap();
    std::fs::remove_dir(canonical.path()).unwrap();
    let profile = crate::embedding::runtime::tests::profile();
    let mut wrong = profile.clone();
    wrong.model_id = "other-model".into();
    migrate_schema(
        legacy.path(),
        canonical.path(),
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(legacy.path())
                .unwrap()
                .control
                .generation_ref(),
            attestations: vec![
                ProfileAttestationInput {
                    namespace_id: NamespaceId::new(1),
                    namespace_name: "default".into(),
                    descriptor: profile.clone(),
                },
                ProfileAttestationInput {
                    namespace_id: other,
                    namespace_name: "other".into(),
                    descriptor: profile,
                },
                ProfileAttestationInput {
                    namespace_id: mismatch,
                    namespace_name: "mismatch".into(),
                    descriptor: wrong,
                },
            ],
        },
    )
    .unwrap();
    let native = tempfile::tempdir().unwrap();
    std::fs::remove_dir(native.path()).unwrap();
    migrate_lifecycle(
        canonical.path(),
        native.path(),
        &LifecycleMigrationOptions {
            expected_source: inspect_schema(canonical.path())
                .unwrap()
                .control
                .generation_ref(),
            namespace_id: other,
            expected_policy_revision: Default::default(),
            target: crate::model::schema::LifecycleVersion::ExplicitFeedbackV2,
            idempotency_key: "other-explicit".into(),
        },
    )
    .unwrap();
    let storage = crate::storage::RedbStorageEngine::open_native(native.path()).unwrap();
    let f = Fixture::from_open_engine(native, storage);
    let owner = owner(&f);
    owner.rebuild_all().await.unwrap();
    let clock = Arc::new(SimilarClock(AtomicUsize::new(0)));
    let mut engine = payload_engine(&f, &owner);
    engine.clock = clock.clone();
    engine.access_recorder = Arc::new(
        crate::search::adapters::StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
            .with_clock(Arc::new(crate::time::ManualClock::new(999_999)))
            .with_completion(owner.clone()),
    );
    SimilarFixture {
        f,
        engine,
        owner,
        clock,
        source,
        old,
        terminal,
        live,
        foreign,
        other,
        mismatch,
        unknown,
    }
}
fn similar_options(scope: SimilarityScope, mode: VersionMode, access: bool) -> SimilaritySelection {
    SimilaritySelection {
        scope,
        semantics: RetrievalSemantics::V2(mode),
        filter: Default::default(),
        limit: 10,
        min_score: None,
        record_access: access,
        include_embeddings: false,
        original_contract_version: 2,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_phase_refill_current_history_and_filtered_successor() {
    let f = similar_fixture().await;
    let mut o = similar_options(SimilarityScope::SameNamespace, VersionMode::Current, false);
    o.limit = 1;
    o.filter.require_tags = vec![crate::model::Tag::new("allow").unwrap()];
    let r = f
        .engine
        .similar_selected_with_work(f.source, None, o, None)
        .await
        .unwrap()
        .value;
    assert_eq!(
        r.rows
            .iter()
            .map(|r| r.result.memory_id)
            .collect::<Vec<_>>(),
        vec![f.live]
    );
    for mode in [VersionMode::Current, VersionMode::History] {
        let mut o = similar_options(SimilarityScope::SameNamespace, mode, false);
        o.limit = 1;
        let r = f
            .engine
            .similar_selected_with_work(f.source, None, o, None)
            .await
            .unwrap()
            .value;
        assert_eq!(
            r.rows[0].result.memory_id,
            if mode == VersionMode::Current {
                f.terminal
            } else {
                f.old
            }
        );
        if mode == VersionMode::Current {
            assert_eq!(r.rows[0].result.composite_score, Some(0.96));
            assert_eq!(r.rows[0].result.score, Some(0.0));
            let p = &r.diagnostics.provenance.as_ref().unwrap()[&f.terminal];
            assert_eq!(p.inherited_floor, Some(0.96));
            assert_eq!(p.own_vector_score, Some(0.));
        }
    }
    let mut o = similar_options(SimilarityScope::SameNamespace, VersionMode::Current, false);
    o.filter.exclude_tags = vec![crate::model::Tag::new("terminal").unwrap()];
    let r = f
        .engine
        .similar_selected_with_work(f.source, None, o, None)
        .await
        .unwrap()
        .value;
    assert!(
        !r.rows
            .iter()
            .any(|r| r.result.memory_id == f.old || r.result.memory_id == f.terminal)
    );
    assert_eq!(
        r.diagnostics.unresolved.get(&UnresolvedReason::Ineligible),
        Some(&1)
    );
    // V1 fixes phase-before-cap but retains stored-version cosine ranking.
    let old = f
        .engine
        .prepare_coherent_similar(f.source, None, 1, None, false, false, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old.results.len(), 1);
    assert_eq!(old.results[0].memory_id, f.old);
    assert_eq!(old.results[0].score.unwrap().to_bits(), 0.96f32.to_bits());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_bound_scope_exclusions_and_false_byte_identity() {
    let f = similar_fixture().await;
    f.f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f.f);
    let calls = f.clock.0.load(Ordering::SeqCst);
    let r = f
        .engine
        .similar_selected_with_work(
            f.source,
            Some("DEFAULT"),
            similar_options(SimilarityScope::AllCompatible, VersionMode::Current, false),
            None,
        )
        .await
        .unwrap();
    assert!(r.receipts.is_empty());
    assert_eq!(f.clock.0.load(Ordering::SeqCst), calls + 1);
    assert_eq!(
        r.value.coverage.searched,
        vec![NamespaceId::new(1), f.other]
    );
    assert_eq!(
        r.value.coverage.excluded.get(&f.mismatch),
        Some(&SimilarityExclusion::DifferentProfile)
    );
    assert_eq!(
        r.value.coverage.excluded.get(&f.unknown),
        Some(&SimilarityExclusion::UnknownProfile)
    );
    let row = r
        .value
        .rows
        .iter()
        .find(|r| r.result.memory_id == f.foreign)
        .unwrap();
    assert_eq!(row.namespace.id, f.other);
    assert_eq!(row.payload.record.namespace_id, f.other.get());
    assert_eq!(bytes(&f.f), before);
    let unknown_id = fixed_id(2007);
    for invalid in ["", "bad/name"] {
        let error = f
            .engine
            .similar_selected_with_work(
                f.source,
                Some(invalid),
                similar_options(SimilarityScope::AllCompatible, VersionMode::Current, true),
                None,
            )
            .await
            .err()
            .unwrap();
        assert!(
            matches!(error, SearchError::CanonicalRead(error) if matches!(*error, crate::storage::StorageError::InvalidNamespaceName { .. }))
        );
    }
    assert!(
        f.engine
            .similar_selected_with_work(
                unknown_id,
                None,
                similar_options(SimilarityScope::AllCompatible, VersionMode::Current, false),
                None
            )
            .await
            .is_err()
    );
    assert!(
        f.engine
            .similar_selected_with_work(
                f.source,
                Some("other"),
                similar_options(SimilarityScope::AllCompatible, VersionMode::Current, true),
                None
            )
            .await
            .is_err()
    );
    assert_eq!(bytes(&f.f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_actual_legacy_false_flag_uses_bound_scope_without_current_rewrite() {
    use crate::mcp::bridge::SearchPipeline;
    let f = similar_fixture().await;
    let adapter = crate::mcp::bridge_adapters::McpSearchAdapter::new(
        Arc::new(f.engine),
        Arc::new(crate::embedding::PassthroughProvider::new(4)),
        f.f.storage.clone(),
        f.f.graph.clone(),
        chrono_tz::UTC,
    );
    let same = adapter
        .find_similar(f.source, 10, None, true, false, None)
        .await
        .unwrap();
    assert!(!same.iter().any(|r| r.id == f.foreign.to_string()));
    let all = adapter
        .find_similar(f.source, 10, None, false, false, None)
        .await
        .unwrap();
    assert!(
        all.iter()
            .any(|r| r.id == f.foreign.to_string() && r.namespace == "other")
    );
    assert!(
        all.iter().any(|r| r.id == f.old.to_string()),
        "V1 history is not V2 current"
    );
    assert!(
        !all.iter()
            .any(|r| r.namespace == "unknown" || r.namespace == "mismatch")
    );
    // The actual MCP dispatcher must forward the explicit legacy flag too.
    let bridge = crate::mcp::bridge::McpBridge {
        search: Arc::new(adapter),
        storage: Arc::new(f.f.storage_adapter()),
        namespaces: Arc::new(f.f.namespace_adapter()),
        health: Arc::new(crate::test_support::StubBridge::default()),
        default_namespace: "default".into(),
        timezone: chrono_tz::UTC,
    };
    f.f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f.f);
    let response = crate::mcp::tools::dispatch_tool(
        &bridge,
        "find_similar_memories",
        serde_json::json!({
            "id": f.source.to_string(), "limit": 10, "sameNamespace": false, "recordAccess": false,
        }),
    )
    .await;
    assert_ne!(response.is_error, Some(true), "{response:?}");
    let wire = serde_json::to_value(response).unwrap();
    let body: serde_json::Value =
        serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(
        body["memories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == f.foreign.to_string() && row["namespace"] == "other")
    );
    assert_eq!(bytes(&f.f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_actual_returned_exposure_per_namespace_policy_clock_and_origin() {
    let f = similar_fixture().await;
    let before: std::collections::BTreeMap<_, _> = [f.source, f.old, f.terminal, f.live, f.foreign]
        .into_iter()
        .map(|id| (id, f.f.record(id).unwrap()))
        .collect();
    let calls = f.clock.0.load(Ordering::SeqCst);
    let r = f
        .engine
        .similar_selected_with_work(
            f.source,
            None,
            similar_options(SimilarityScope::AllCompatible, VersionMode::Current, true),
            None,
        )
        .await
        .unwrap();
    assert_eq!(r.receipts.len(), 2);
    assert_eq!(f.clock.0.load(Ordering::SeqCst), calls + 1);
    assert_eq!(r.value.evaluated_at, 10_000);
    let returned: HashSet<_> = r.value.rows.iter().map(|r| r.result.memory_id).collect();
    for (id, old) in before {
        let now = f.f.record(id).unwrap();
        assert_eq!(
            now.access_history.len(),
            old.access_history.len() + usize::from(returned.contains(&id))
        );
        if returned.contains(&id) {
            let event = now.access_history.last().unwrap();
            assert_eq!(event.kind, AccessKind::AssociativeRetrieval);
            assert_eq!(event.timestamp, 10_000);
            let a = &old.schema.as_ref().unwrap().lifecycle;
            let b = &now.schema.as_ref().unwrap().lifecycle;
            if id == f.foreign {
                assert_eq!(a.decay_anchor_at, b.decay_anchor_at);
            } else {
                assert_eq!(b.decay_anchor_at, 10_000);
            }
        } else {
            assert_eq!(old.encode().unwrap(), now.encode().unwrap());
        }
    }
    let first = &r.receipts[0];
    let second = &r.receipts[1];
    assert!(first.committed_generation < second.committed_generation);
    assert!(r.value.read_generation.generation < first.committed_generation);
    assert!(
        f.f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_source_revalidation_rejects_before_any_exposure() {
    let f = similar_fixture().await;
    let selected = f
        .engine
        .prepare_similar_selection(
            f.source,
            None,
            similar_options(SimilarityScope::AllCompatible, VersionMode::Current, true),
            None,
        )
        .await
        .unwrap();
    let policy = crate::storage::native_lifecycle::NativePolicyInputs::default();
    {
        let mut storage = f.f.storage.write().unwrap();
        let intent = storage
            .meta_store()
            .capture_native_tombstone(f.source, Some(NamespaceId::new(1)), &policy)
            .unwrap();
        let mut ctx = MutationContext::unkeyed(NamespaceId::new(1));
        ctx.contract_version = 2;
        storage
            .tombstone_native_record(
                &ctx,
                &intent,
                &policy,
                &crate::time::ManualClock::new(11_000),
                None,
            )
            .unwrap();
    }
    f.owner.complete().await.unwrap();
    f.f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f.f);
    let error = f
        .engine
        .record_selected_similarity(&selected, None)
        .await
        .err()
        .unwrap();
    let SearchError::Observation(error) = error else {
        panic!("unexpected error")
    };
    assert!(error.receipts.is_empty());
    assert_ne!(error.commit_state, "committed");
    assert_eq!(bytes(&f.f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_later_namespace_failure_keeps_earlier_receipt_and_publication() {
    let f = similar_fixture().await;
    let selected = f
        .engine
        .prepare_similar_selection(
            f.source,
            None,
            similar_options(SimilarityScope::AllCompatible, VersionMode::Current, true),
            None,
        )
        .await
        .unwrap();
    let before = f.f.record(f.foreign).unwrap().encode().unwrap();
    let first = f.f.record(f.terminal).unwrap().access_history.len();
    f.f.storage
        .read()
        .unwrap()
        .meta_store()
        .observation_fault
        .store(4, Ordering::SeqCst);
    let error = f
        .engine
        .record_selected_similarity(&selected, None)
        .await
        .err()
        .unwrap();
    let SearchError::Observation(error) = error else {
        panic!("lost observation error")
    };
    assert_eq!(error.commit_state, "committed");
    assert_eq!(error.receipts.len(), 1);
    assert!(error.unknown_operation_id.is_none());
    assert_eq!(
        f.f.record(f.terminal).unwrap().access_history.len(),
        first + 1
    );
    assert_eq!(f.f.record(f.foreign).unwrap().encode().unwrap(), before);
    assert!(
        f.f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_origin_hashes_and_reopen_keep_per_namespace_scope() {
    use crate::storage::control;
    use redb::ReadableTable;
    for origin in [1, 2] {
        let f = similar_fixture().await;
        let mut options =
            similar_options(SimilarityScope::AllCompatible, VersionMode::Current, true);
        options.original_contract_version = origin;
        let result = f
            .engine
            .similar_selected_with_work(f.source, None, options, None)
            .await
            .unwrap();
        let mut groups = std::collections::BTreeMap::<u32, Vec<_>>::new();
        for row in &result.value.rows {
            groups
                .entry(row.namespace.id.get())
                .or_default()
                .push((row.result.memory_id, AccessKind::AssociativeRetrieval));
        }
        let ids: HashSet<_> = result.receipts.iter().map(|r| r.operation_id).collect();
        drop(f.engine);
        drop(f.owner);
        let dir = f.f.close();
        let snapshot = crate::storage::offline_snapshot::OfflineSnapshot::open(dir.path()).unwrap();
        let read = snapshot.metadata.begin_read().unwrap();
        let table = read.open_table(control::RECEIPTS).unwrap();
        let receipts: Vec<_> = table
            .iter()
            .unwrap()
            .map(|row| control::validate_receipt(row.unwrap().1.value()).unwrap())
            .filter(|r| ids.contains(&r.receipt.operation_id))
            .collect();
        assert_eq!(receipts.len(), 2);
        for ((ns, accesses), receipt) in groups.into_iter().zip(result.receipts.iter()) {
            let stored = receipts
                .iter()
                .find(|s| s.receipt.operation_id == receipt.operation_id)
                .unwrap();
            let scope = crate::storage::native_lifecycle::LifecycleScope {
                namespace_id: NamespaceId::new(ns),
                namespace_name: if ns == 1 {
                    "default".into()
                } else {
                    "other".into()
                },
                source: (ns == 1).then_some(f.source),
            };
            let semantic = control::encode(&(scope, accesses, None::<u8>)).unwrap();
            let mut ctx = MutationContext::unkeyed(NamespaceId::new(ns));
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
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_target_removed_after_selection_fails_before_first_group() {
    let f = similar_fixture().await;
    let selected = f
        .engine
        .prepare_similar_selection(
            f.source,
            None,
            similar_options(SimilarityScope::AllCompatible, VersionMode::Current, true),
            None,
        )
        .await
        .unwrap();
    let policy = crate::storage::native_lifecycle::NativePolicyInputs::default();
    {
        let mut storage = f.f.storage.write().unwrap();
        let intent = storage
            .meta_store()
            .capture_native_tombstone(f.foreign, Some(f.other), &policy)
            .unwrap();
        let mut ctx = MutationContext::unkeyed(f.other);
        ctx.contract_version = 2;
        storage
            .tombstone_native_record(
                &ctx,
                &intent,
                &policy,
                &crate::time::ManualClock::new(11_000),
                None,
            )
            .unwrap();
    }
    f.owner.complete().await.unwrap();
    f.f.storage.write().unwrap().sync().unwrap();
    let before = bytes(&f.f);
    let error = f
        .engine
        .record_selected_similarity(&selected, None)
        .await
        .err()
        .unwrap();
    let SearchError::Observation(error) = error else {
        panic!("lost observation error")
    };
    assert!(error.receipts.is_empty());
    assert_eq!(bytes(&f.f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v1_canonical_prefilter_matches_cosine_oracle_and_uuid_ties() {
    for reverse in [false, true] {
        let f = Fixture::new();
        let owner = owner(&f);
        let source = payload_record(&owner, 5001, [1., 0., 0., 0.], None, None, true).await;
        let mut entries = vec![
            (5002, [0.8, 0.6, 0., 0.]),
            (5003, [0.8, 0.6, 0., 0.]),
            (5004, [-0.8, 0.6, 0., 0.]),
        ];
        if reverse {
            entries.reverse();
        }
        for &(n, v) in &entries {
            payload_record(&owner, n, v, None, None, true).await;
        }
        for _ in 0..5 {
            f.insert_memory_with(crate::test_support::MemorySpec {
                phase: DecayPhase::Ghost,
                embedding: Some(&[1., 0., 0., 0.]),
                ..crate::test_support::MemorySpec::new(NamespaceId::new(1))
            })
            .await;
        }
        owner.complete().await.unwrap();
        let engine = payload_engine(&f, &owner);
        f.storage.write().unwrap().sync().unwrap();
        let before = bytes(&f);
        for k in [1, 2, 3] {
            for threshold in [None, Some(0.0), Some(0.9)] {
                let result = engine
                    .prepare_coherent_similar(source, None, k, threshold, false, false, None)
                    .await
                    .unwrap()
                    .unwrap();
                let mut expected: Vec<_> = entries
                    .iter()
                    .map(|(n, v)| (fixed_id(*n), v[0]))
                    .filter(|(_, score)| threshold.is_none_or(|min| *score >= min))
                    .collect();
                expected.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                expected.truncate(k);
                assert_eq!(
                    result
                        .results
                        .iter()
                        .map(|r| (r.memory_id, r.score.unwrap().to_bits()))
                        .collect::<Vec<_>>(),
                    expected
                        .iter()
                        .map(|(id, score)| (*id, score.to_bits()))
                        .collect::<Vec<_>>()
                );
            }
        }
        assert_eq!(bytes(&f), before);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn similar_v2_actual_chain_64_resolves_65_is_unresolved_history_stays_raw() {
    for hops in [64, 65] {
        let f = similar_fixture_with_chain(hops).await;
        let mut options =
            similar_options(SimilarityScope::SameNamespace, VersionMode::Current, false);
        options.filter.require_tags = vec![crate::model::Tag::new("chain").unwrap()];
        options.min_score = Some(0.5);
        let result = f
            .engine
            .similar_selected_with_work(f.source, None, options, None)
            .await
            .unwrap()
            .value;
        if hops == 64 {
            assert_eq!(result.rows.len(), 1);
            assert_eq!(result.rows[0].result.memory_id, fixed_id(3064));
            assert_eq!(
                result.rows[0].result.composite_score.unwrap().to_bits(),
                0.6f32.to_bits()
            );
        } else {
            assert!(result.rows.is_empty());
            assert_eq!(
                result
                    .diagnostics
                    .unresolved
                    .get(&UnresolvedReason::HopLimit),
                Some(&1)
            );
            assert!(result.diagnostics.provenance.unwrap().is_empty());
        }
        // The untraversed 65th edge is examined to prove a hop-limited chain.
        assert_eq!(result.diagnostics.supersession_edges_examined, hops);
        let mut history =
            similar_options(SimilarityScope::SameNamespace, VersionMode::History, false);
        history.filter.require_tags = vec![crate::model::Tag::new("chain").unwrap()];
        history.min_score = Some(0.5);
        let result = f
            .engine
            .similar_selected_with_work(f.source, None, history, None)
            .await
            .unwrap()
            .value;
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].result.memory_id, fixed_id(3000));
        assert_eq!(
            result.rows[0].result.score.unwrap().to_bits(),
            0.6f32.to_bits()
        );
        assert!(
            result.diagnostics.provenance.as_ref().unwrap()[&fixed_id(3000)]
                .inherited_floor
                .is_none()
        );
        // Actual equal-cosine history rows use UUID order before the result cap.
        let mut ties = similar_options(SimilarityScope::SameNamespace, VersionMode::History, false);
        ties.filter.require_tags = vec![crate::model::Tag::new("chain").unwrap()];
        ties.limit = 3;
        let result = f
            .engine
            .similar_selected_with_work(f.source, None, ties, None)
            .await
            .unwrap()
            .value;
        assert_eq!(
            result
                .rows
                .iter()
                .map(|r| r.result.memory_id)
                .collect::<Vec<_>>(),
            vec![fixed_id(3000), fixed_id(3001), fixed_id(3002)]
        );
        assert_eq!(
            result.rows[1].result.score.unwrap().to_bits(),
            0.0f32.to_bits()
        );
        assert_eq!(
            result.rows[2].result.score.unwrap().to_bits(),
            0.0f32.to_bits()
        );
    }
}
