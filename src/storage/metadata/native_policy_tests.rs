use super::*;
use crate::model::{DecayPhase, MemoryId};
use crate::storage::native_lifecycle::{
    LifecycleScope, NativeLifecycleSession, NativePolicyInputs,
};
use crate::storage::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
use crate::storage::{RedbStorageEngine, StorageEngine};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct NoClock;
impl crate::time::Clock for NoClock {
    fn now_millis(&self) -> i64 {
        panic!("numeric policy sampled clock")
    }
}
fn open(path: &Path) -> NativeLifecycleSession {
    NativeLifecycleSession::open(path, NativePolicyInputs::default(), Arc::new(NoClock)).unwrap()
}
fn fixture(root: &Path) -> (PathBuf, NamespaceId, Vec<MemoryId>) {
    let source = root.join("legacy");
    let target = root.join("native");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let mut ns = NamespaceConfig::default_namespace(100);
    ns.embedding_dim = 4;
    let id = engine.create_namespace(&ns).unwrap();
    ns.id = id;
    let mut ids = Vec::new();
    for n in 0..4 {
        let receipt = engine
            .insert_memory_with_receipt(
                &MutationContext::unkeyed(id),
                &crate::storage::insertion::InsertMemory {
                    id: None,
                    summary: format!("retained summary {n}"),
                    full_text: Some(format!("retained full text {n}")),
                    tags: vec![],
                    initial_stability: None,
                    created_at: Some(1000),
                    embedding: Some(vec![1., 0., 0., 0.]),
                    parent_id: None,
                    supersedes: None,
                },
                &[],
                1000,
            )
            .unwrap();
        ids.push(crate::storage::insertion::inserted_id(&receipt).unwrap());
    }
    for index in [1, 2] {
        let before = engine.get_record(ids[index]).unwrap().unwrap();
        assert!(
            engine
                .apply_legacy_decay(ids[index], &before, &ns, DecayPhase::Summary, 0.2, 0.2)
                .unwrap()
        );
    }
    let before = engine.get_record(ids[2]).unwrap().unwrap();
    assert!(
        engine
            .apply_legacy_decay(ids[2], &before, &ns, DecayPhase::Ghost, 0.05, 0.05)
            .unwrap()
    );
    engine
        .tombstone_memory_with_receipt(&MutationContext::unkeyed(id), ids[3])
        .unwrap();
    engine.sync().unwrap();
    drop(engine);
    migrate_schema(
        &source,
        &target,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    (target, id, ids)
}
fn scope(s: &NativeLifecycleSession, id: NamespaceId) -> LifecycleScope {
    LifecycleScope {
        namespace_id: id,
        namespace_name: s.engine.get_namespace(id).unwrap().unwrap().name,
        source: None,
    }
}
fn ctx(s: &NativeLifecycleSession, id: NamespaceId, key: &str) -> MutationContext {
    let state = s.engine.control_state().unwrap();
    MutationContext {
        store_id: Some(state.store_id),
        scope: MutationScope::Namespace(id),
        idempotency_key: Some(key.into()),
        expected_generation: Some(state.committed_generation),
        contract_version: 2,
    }
}
fn rows(s: &NativeLifecycleSession) -> Vec<DiskRecord> {
    s.metadata()
        .scan_all()
        .unwrap()
        .into_iter()
        .map(|(_, r)| r)
        .collect()
}
fn encoded(rows: &[DiskRecord]) -> Vec<Vec<u8>> {
    rows.iter().map(|r| r.encode().unwrap()).collect()
}

#[test]
fn native_policy_keyed_noop_binds_lifetime_key_before_cas_and_fresh_policy() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let original = NamespacePolicy::from(&s.engine.get_namespace(id).unwrap().unwrap());
    let before = encoded(&rows(&s));
    let ns_before = s.engine.get_namespace(id).unwrap().unwrap();
    let key = ctx(&s, id, "keyed-noop");
    let receipt = s
        .update_policy(&key, scope(&s, id), &original)
        .unwrap()
        .unwrap();
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativeNamespacePolicyUnchanged { .. }
    ));
    assert_eq!(
        receipt.committed_generation,
        key.expected_generation.unwrap().next().unwrap()
    );
    assert_eq!(encoded(&rows(&s)), before);
    assert_eq!(
        control::encode(&s.engine.get_namespace(id).unwrap().unwrap()).unwrap(),
        control::encode(&ns_before).unwrap()
    );
    let mut replacement = original.clone();
    replacement.initial_stability = 7.;
    let changed = s
        .update_policy(&ctx(&s, id, "other"), scope(&s, id), &replacement)
        .unwrap()
        .unwrap();
    let after = encoded(&rows(&s));
    let state = s.engine.control_state().unwrap();
    s.policy.multiplier = f64::NAN;
    let replay = s
        .update_policy(&key, scope(&s, id), &original)
        .unwrap()
        .unwrap();
    let mut expected = receipt.clone();
    expected.replayed = true;
    assert_eq!(replay, expected);
    assert_eq!(s.engine.control_state().unwrap(), state);
    assert_eq!(encoded(&rows(&s)), after);
    assert_eq!(
        s.engine
            .get_namespace(id)
            .unwrap()
            .unwrap()
            .initial_stability,
        7.
    );
    assert!(matches!(
        s.update_policy(&key, scope(&s, id), &replacement),
        Err(StorageError::IdempotencyConflict)
    ));
    assert!(changed.committed_generation > receipt.committed_generation);
    drop(s);
    let mut s = open(&path);
    assert_eq!(
        s.update_policy(&key, scope(&s, id), &original)
            .unwrap()
            .unwrap(),
        expected
    );
    let mut unkeyed = ctx(&s, id, "unused");
    unkeyed.idempotency_key = None;
    let state = s.engine.control_state().unwrap();
    assert!(
        s.update_policy(&unkeyed, scope(&s, id), &replacement)
            .unwrap()
            .is_none()
    );
    assert_eq!(s.engine.control_state().unwrap(), state);
}

#[test]
fn native_policy_all_terminal_rows_participate_without_payload_or_timestamp_churn() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let before = rows(&s);
    assert_eq!(before.len(), 4);
    for phase in [
        DecayPhase::Full,
        DecayPhase::Summary,
        DecayPhase::Ghost,
        DecayPhase::Tombstone,
    ] {
        assert!(before.iter().any(|r| r.phase == phase));
    }
    let mut policy = NamespacePolicy::from(&s.engine.get_namespace(id).unwrap().unwrap());
    policy.default_difficulty = 7.;
    let intent = s.capture_policy(scope(&s, id), &policy).unwrap();
    let receipt = s
        .apply_policy(&ctx(&s, id, "change"), &intent)
        .unwrap()
        .unwrap();
    let MutationOutcome::NativeNamespacePolicyUpdated { transition } = &receipt.outcome else {
        panic!()
    };
    assert_eq!(transition.record_count, 4);
    for mut after in rows(&s) {
        let old = before.iter().find(|r| r.id == after.id).unwrap();
        let fields = &mut after.schema.as_mut().unwrap().lifecycle;
        let original = &old.schema.as_ref().unwrap().lifecycle;
        assert_eq!(
            fields.record_generation,
            original.record_generation.next().unwrap()
        );
        assert_eq!(
            fields.policy_revision,
            original.policy_revision.next().unwrap()
        );
        fields.record_generation = original.record_generation;
        fields.policy_revision = original.policy_revision;
        assert_eq!(after.encode().unwrap(), old.encode().unwrap());
    }
    let stale = ctx(&s, id, "stale");
    assert!(matches!(
        s.apply_policy(&stale, &intent),
        Err(StorageError::LifecycleConflict(_))
    ));
    drop(s);
    inspect_schema(&path).unwrap();
    open(&path);
}

#[test]
fn native_policy_commit_faults_preserve_original_keyed_noop_outcome() {
    for point in [
        "before_metadata_commit",
        "after_metadata_commit",
        "commit_unknown",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (path, id, _) = fixture(root.path());
        let mut s = open(&path);
        let policy = NamespacePolicy::from(&s.engine.get_namespace(id).unwrap().unwrap());
        let key = ctx(&s, id, "noop-fault");
        let before = encoded(&rows(&s));
        crate::storage::catalog::FAILURE.with(|v| v.set(Some(point)));
        let result = s.update_policy(&key, scope(&s, id), &policy);
        crate::storage::catalog::FAILURE.with(|v| v.set(None));
        let committed = if point == "after_metadata_commit" {
            let Err(StorageError::CommittedMutation { receipt, .. }) = result else {
                panic!("{result:?}")
            };
            Some(*receipt)
        } else {
            assert!(result.is_err());
            None
        };
        drop(s);
        let mut s = open(&path);
        let receipt = s
            .update_policy(&key, scope(&s, id), &policy)
            .unwrap()
            .unwrap();
        assert_eq!(receipt.replayed, point != "before_metadata_commit");
        assert_eq!(encoded(&rows(&s)), before);
        assert_eq!(
            receipt.committed_generation,
            key.expected_generation.unwrap().next().unwrap()
        );
        if let Some(mut original) = committed {
            original.replayed = true;
            assert_eq!(receipt, original);
        }
    }
}
