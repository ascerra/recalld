use super::*;
use crate::graph::native_selection::capture;
use crate::model::EdgeType;

fn add(
    s: &mut NativeLifecycleSession,
    d: &Data,
    key: &str,
    score: f32,
    entities: Vec<String>,
    created: i64,
) -> MemoryId {
    let mut v = input(d);
    v.memory.parent_id = None;
    v.memory.created_at = Some(created);
    v.memory.embedding = Some(vec![score, (1. - score * score).sqrt(), 0., 0.]);
    v.embedding_profile = Some(d.profile.clone());
    v.entities = entities;
    let intent = s.capture_insert(scope(d), &v).unwrap();
    inserted(
        &s.insert_prepared(&ctx(s, d, key), &intent, &generated())
            .unwrap(),
    )
}
fn probe(s: &NativeLifecycleSession, d: &Data) -> NativeInsertIntent {
    let mut v = input(d);
    v.memory.parent_id = None;
    v.memory.embedding = Some(vec![2., 0., 0., 0.]);
    v.embedding_profile = Some(d.profile.clone());
    v.entities = vec!["Dr. John Smith".into()];
    s.capture_insert(scope(d), &v).unwrap()
}

#[test]
fn native_selection_canonical_scope_phase_labels_and_deterministic_caps() {
    let root = tempfile::tempdir().unwrap();
    let mut foreign = MemoryId::nil();
    let d = fixture_with_seed(root.path(), |engine| {
        let mut namespace = NamespaceConfig::default_namespace(0);
        namespace.name = "other".into();
        namespace.embedding_dim = 4;
        let ns = engine.create_namespace(&namespace).unwrap();
        let original = crate::storage::insertion::InsertMemory {
            id: None,
            summary: "foreign nearest".into(),
            full_text: None,
            tags: vec![],
            initial_stability: None,
            created_at: Some(1000),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        foreign = crate::storage::insertion::inserted_id(
            &engine
                .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &original, &[], 1000)
                .unwrap(),
        )
        .unwrap();
    });
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let entity = add(
        &mut s,
        &d,
        "entity",
        0.90,
        vec!["John Smith".into(), "Smith".into()],
        950,
    );
    let nearest = add(&mut s, &d, "nearest", 0.98, vec![], 990);
    let removed = add(&mut s, &d, "removed", 0.999, vec!["John Smith".into()], 999);
    let t = s.capture_tombstone(removed, None).unwrap();
    s.tombstone(&ctx(&s, &d, "remove"), &t).unwrap();
    let intent = probe(&s, &d);
    let before = s.metadata().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    let captured = capture(&s.engine, &intent, &generated()).unwrap();
    assert_eq!(captured.generation, before.generation_ref());
    assert_eq!(captured.namespace_id, d.ns);
    assert_eq!(captured.profile_id, d.profile.profile_id().unwrap());
    assert!(
        !captured
            .candidates
            .iter()
            .any(|c| [foreign, removed].contains(&c.id))
    );
    for candidate in &captured.candidates {
        assert_eq!(
            candidate.record_generation,
            s.engine
                .get_record(candidate.id)
                .unwrap()
                .unwrap()
                .schema
                .unwrap()
                .lifecycle
                .record_generation
        );
    }
    let settings = crate::config::GraphConfig {
        max_auto_links: 1,
        max_entity_links: 1,
        max_temporal_links: 2,
        temporal_window_ms: 100,
        ..Default::default()
    };
    let links = captured.static_links(&settings).unwrap();
    // The original migrated record has cosine1 and wins before the cap.
    assert_eq!(
        links
            .iter()
            .find(|e| e.edge_type == EdgeType::Associative)
            .unwrap()
            .target,
        d.parent
    );
    let linked = links
        .iter()
        .find(|e| e.edge_type == EdgeType::Entity)
        .unwrap();
    assert_eq!(linked.target, entity);
    assert_eq!(linked.weight, 1.);
    let temporal = captured.temporal_links(1000, &settings);
    assert_eq!(
        temporal.iter().map(|e| e.target).collect::<Vec<_>>(),
        vec![d.parent, nearest]
    );
    let advisory = captured.advisory(true).unwrap();
    assert_eq!(
        advisory.iter().map(|p| p.0).collect::<Vec<_>>(),
        vec![d.parent, nearest, entity]
    );
    assert!(captured.advisory(false).is_none());
    let mut reversed = captured.clone();
    reversed.candidates.reverse();
    assert_eq!(reversed.static_links(&settings).unwrap(), links);
    assert_eq!(reversed.temporal_links(1000, &settings), temporal);
    assert_eq!(reversed.advisory(true), Some(advisory));
    assert_eq!(before, s.metadata().control_state().unwrap());
    assert_eq!(calls, clock.0.load(Ordering::SeqCst));
    assert_eq!(
        intent.original.memory.embedding.as_ref().unwrap()[0].to_bits(),
        2f32.to_bits()
    );
    add(&mut s, &d, "new-after-capture", 1., vec![], 1000);
    assert!(matches!(
        capture(&s.engine, &intent, &generated()),
        Err(StorageError::LifecycleConflict(_))
    ));
}

#[test]
fn native_selection_invalid_vectors_and_temporal_extremes_do_not_mutate_or_sample_clock() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let s = open(&d, clock.clone());
    let mut intent = probe(&s, &d);
    let before = s.metadata().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    intent.original.memory.embedding = Some(vec![0.; 4]);
    assert!(matches!(
        capture(&s.engine, &intent, &generated()),
        Err(StorageError::InvalidMutation(_))
    ));
    intent.original.memory.embedding = None;
    intent.original.embedding_profile = None;
    let bad = GeneratedEmbedding {
        values: vec![f32::NAN; 4],
        profile: Some(d.profile.clone()),
    };
    assert!(matches!(
        capture(&s.engine, &intent, &bad),
        Err(StorageError::ProviderVectorUnavailable(_))
    ));
    let missing_profile = GeneratedEmbedding {
        values: vec![1., 0., 0., 0.],
        profile: None,
    };
    assert!(matches!(
        capture(&s.engine, &intent, &missing_profile),
        Err(StorageError::ProviderVectorUnavailable(_))
    ));
    let mut wrong_profile = d.profile.clone();
    wrong_profile.dimension = 2;
    let wrong_profile = GeneratedEmbedding {
        values: vec![1., 0., 0., 0.],
        profile: Some(wrong_profile),
    };
    assert!(matches!(
        capture(&s.engine, &intent, &wrong_profile),
        Err(StorageError::ProviderVectorUnavailable(_))
    ));
    let mut captured = capture(&s.engine, &probe(&s, &d), &generated()).unwrap();
    captured.candidates[0].created_at = i64::MIN;
    let settings = crate::config::GraphConfig {
        temporal_window_ms: u64::MAX,
        ..Default::default()
    };
    let links = captured.temporal_links(i64::MAX, &settings);
    assert_eq!(links[0].weight, 0.05);
    let zero = crate::config::GraphConfig {
        temporal_window_ms: 0,
        ..Default::default()
    };
    assert!(captured.temporal_links(i64::MAX, &zero).is_empty());
    assert_eq!(before, s.metadata().control_state().unwrap());
    assert_eq!(calls, clock.0.load(Ordering::SeqCst));
}

#[test]
fn native_selection_advisory_hydrates_frozen_ids_at_current_visibility_without_rediscovery() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock.clone());
    let original = add(&mut s, &d, "frozen-candidate", 0.99, vec![], 1000);
    let frozen = vec![(original, 0.99)];
    let captured_generation = s.metadata().control_state().unwrap().generation_ref();
    let t = s.capture_tombstone(original, None).unwrap();
    s.tombstone(&ctx(&s, &d, "remove-frozen"), &t).unwrap();
    let newcomer = add(&mut s, &d, "newer-neighbor", 1., vec![], 1000);
    let before = s.metadata().control_state().unwrap();
    let calls = clock.0.load(Ordering::SeqCst);
    let disabled = crate::mcp::native_advisory::hydrate(&s.engine, d.ns, None);
    let checked_empty = crate::mcp::native_advisory::hydrate(&s.engine, d.ns, Some(&[]));
    assert!(disabled.report.is_none() && checked_empty.report.is_none());
    assert!(disabled.read_generation.is_none());
    assert_eq!(checked_empty.read_generation, Some(before.generation_ref()));
    assert!(disabled.warnings.is_empty() && checked_empty.warnings.is_empty());
    let response = crate::mcp::native_advisory::hydrate(&s.engine, d.ns, Some(&frozen));
    assert!(response.report.is_none());
    assert_eq!(response.warnings, vec!["ADVISORY_CONTENT_UNAVAILABLE"]);
    assert_eq!(response.read_generation, Some(before.generation_ref()));
    assert_ne!(response.read_generation, Some(captured_generation));
    assert_ne!(newcomer, original);
    assert_eq!(frozen, vec![(original, 0.99)]);
    let current = crate::mcp::native_advisory::hydrate(&s.engine, d.ns, Some(&[(newcomer, 0.97)]));
    assert_eq!(current.report.unwrap().matches[0].score, 0.97);
    assert!(current.warnings.is_empty());
    let invalid =
        crate::mcp::native_advisory::hydrate(&s.engine, d.ns, Some(&[(newcomer, f32::NAN)]));
    assert!(invalid.report.is_none());
    assert_eq!(invalid.warnings, vec!["ADVISORY_UNAVAILABLE"]);
    assert_eq!(before, s.metadata().control_state().unwrap());
    assert_eq!(calls, clock.0.load(Ordering::SeqCst));
}

#[test]
fn native_selection_equal_scores_entities_and_times_use_uuid_ties_before_caps() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock);
    let a = add(&mut s, &d, "tie-a", 1., vec!["John Smith".into()], 1000);
    let b = add(&mut s, &d, "tie-b", 1., vec!["John Smith".into()], 1000);
    let mut selected = capture(&s.engine, &probe(&s, &d), &generated()).unwrap();
    selected.candidates.reverse();
    let settings = crate::config::GraphConfig {
        max_auto_links: 1,
        max_entity_links: 1,
        max_temporal_links: 1,
        ..Default::default()
    };
    let links = selected.static_links(&settings).unwrap();
    let expected = [a, b, d.parent].into_iter().min().unwrap();
    assert_eq!(
        links
            .iter()
            .find(|e| e.edge_type == EdgeType::Associative)
            .unwrap()
            .target,
        expected
    );
    assert_eq!(
        links
            .iter()
            .find(|e| e.edge_type == EdgeType::Entity)
            .unwrap()
            .target,
        a.min(b)
    );
    assert_eq!(selected.temporal_links(1000, &settings)[0].target, expected);
    let mut ids = vec![a, b, d.parent];
    ids.sort();
    assert_eq!(
        selected
            .advisory(true)
            .unwrap()
            .iter()
            .map(|m| m.0)
            .collect::<Vec<_>>(),
        ids
    );
}
