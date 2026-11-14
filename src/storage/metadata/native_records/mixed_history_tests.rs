//! Real composed writer histories; no direct proof seeding for positive cases.
use super::*;
use crate::model::{DecayPhase, EdgeType};
use crate::storage::native_graph::{NativeEdgeInput, NativeGraphInput};
use crate::storage::native_maintenance::NativeDecayResult;
use crate::storage::native_store::{FrozenInsertLinks, NativeStoreInput, NativeStoreSettings};
use crate::storage::{logical_export, validation};

fn linked(s: &mut NativeLifecycleSession, d: &Data, key: &str) -> MemoryId {
    let mut original = input(d);
    original.memory.initial_stability = Some(1.);
    let original = NativeStoreInput {
        insert: original,
        check_duplicates: false,
    };
    let settings = NativeStoreSettings {
        autolink_enabled: true,
        auto_link_threshold_bits: 0.8_f64.to_bits(),
        max_auto_links: 1,
        max_entity_links: 1,
        temporal_window_ms: 0,
        max_temporal_links: 0,
    };
    let intent = s.capture_store(scope(d), &original, &settings).unwrap();
    let plan = FrozenInsertLinks {
        generation: intent.insert_intent().capture_generation(),
        profile_id: d.profile.profile_id().unwrap(),
        static_links: vec![(d.parent, EdgeType::Associative, 0.9)],
        temporal: vec![],
        advisory: None,
    };
    let prepared = s.prepare_store_links(&intent, &plan).unwrap();
    let receipt = s
        .store_prepared(&ctx(s, d, key), &prepared, &generated())
        .unwrap();
    match receipt.outcome {
        MutationOutcome::NativeMemoryInsertedWithLinks { transition } => transition.memory_id,
        _ => panic!("wrong creation proof"),
    }
}
fn graph(s: &mut NativeLifecycleSession, d: &Data, id: MemoryId) {
    let input = NativeGraphInput {
        edges: vec![NativeEdgeInput {
            source: id,
            target: d.parent,
            edge_type: EdgeType::Entity,
            weight: 0.75,
            auto_created: true,
        }],
    };
    s.put_graph(&ctx(s, d, "graph"), scope(d), &input)
        .unwrap()
        .unwrap();
    let key = ctx(s, d, "graph-noop");
    let receipt = s.put_graph(&key, scope(d), &input).unwrap().unwrap();
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativeGraphUnchanged { .. }
    ));
    let config = s.engine.get_namespace(d.ns).unwrap().unwrap();
    let mut replacement = crate::storage::catalog::NamespacePolicy::from(&config);
    replacement.initial_stability = 9.;
    s.update_policy(&ctx(s, d, "numeric"), scope(d), &replacement)
        .unwrap()
        .unwrap();
    let mut changed = input.clone();
    changed.edges[0].weight = 0.5;
    s.put_graph(&ctx(s, d, "graph-change"), scope(d), &changed)
        .unwrap()
        .unwrap();
    let before = s.engine.control_state().unwrap();
    let replay = s.put_graph(&key, scope(d), &input).unwrap().unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, receipt.outcome);
    assert_eq!(replay.committed_generation, receipt.committed_generation);
    assert_eq!(s.engine.control_state().unwrap(), before);
}
fn phases(s: &mut NativeLifecycleSession, d: &Data, id: MemoryId, count: usize) {
    let policy = NativePolicyInputs::default();
    let original = s.engine.get_record(id).unwrap().unwrap();
    for n in 0..count {
        let intent = s
            .metadata()
            .capture_native_decay(id, d.ns, &policy, 10_000 * 86_400_000_i64)
            .unwrap();
        assert!(matches!(
            s.engine
                .apply_native_decay(
                    &ctx(s, d, &format!("phase-{id}-{n}")),
                    &intent,
                    &policy,
                    None
                )
                .unwrap(),
            NativeDecayResult::Committed(_)
        ));
        if n == 1 {
            let ghost = s.engine.get_record(id).unwrap().unwrap();
            assert_eq!(ghost.phase, DecayPhase::Ghost);
            assert_eq!(
                ghost.schema.as_ref().unwrap().vector,
                original.schema.as_ref().unwrap().vector
            );
            assert!(ghost.schema.as_ref().unwrap().labels.is_empty());
        }
    }
}
fn purge(s: &mut NativeLifecycleSession, d: &Data) {
    let mut p = s
        .engine
        .begin_namespace_purge(&ctx(s, d, "purge"), true)
        .unwrap();
    while p.stage != crate::storage::purge::PurgeStage::Complete {
        p = s
            .engine
            .resume_namespace_purge(p.store_id, d.ns, p.start_receipt.operation_id, 1)
            .unwrap();
    }
    assert!(s.engine.get_namespace(d.ns).unwrap().is_none());
}
#[test]
fn mixed_history_harddelete_reverses_removed_edges_before_graph_noop_and_store() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let mut s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let id = linked(&mut s, &d, "linked");
    graph(&mut s, &d, id);
    phases(&mut s, &d, id, 3);
    assert!(s.engine.get_record(id).unwrap().is_none());
    drop(s);
    inspect_schema(&d.path).expect("valid graph/store history must survive hard deletion");
}
#[test]
fn mixed_history_purge_binds_retired_namespace_before_graph_and_store_validation() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let mut s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let id = linked(&mut s, &d, "linked");
    graph(&mut s, &d, id);
    phases(&mut s, &d, id, 2);
    purge(&mut s, &d);
    drop(s);
    inspect_schema(&d.path).expect("valid graph/store history must bind to retired namespace");
}
