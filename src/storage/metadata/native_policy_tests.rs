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

// Import canonicalizes the unused physical slot of rows without vector ownership.
// Retained vector references, all f32 fields, payload and lifecycle bytes remain checked.
fn imported_rows(session: &NativeLifecycleSession) -> Vec<Vec<u8>> {
    let mut all = rows(session);
    for r in &mut all {
        if r.schema.as_ref().unwrap().vector.is_none() {
            r.vector_slot = 0;
        }
    }
    encoded(&all)
}

#[tokio::test]
async fn native_policy_offline_roundtrip_and_foreign_noop_proof_rejects_forged_policy() {
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    use crate::storage::logical_export::{export_store, import_store};
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let original = NamespacePolicy::from(&s.engine.get_namespace(id).unwrap().unwrap());
    let noop_key = ctx(&s, id, "original-no-change");
    let noop = s
        .update_policy(&noop_key, scope(&s, id), &original)
        .unwrap()
        .unwrap();
    let mut changed = original.clone();
    changed.initial_stability = 9.;
    s.update_policy(&ctx(&s, id, "changed"), scope(&s, id), &changed)
        .unwrap()
        .unwrap();
    let before_rows = imported_rows(&s);
    let config = control::encode(&s.engine.get_namespace(id).unwrap().unwrap()).unwrap();
    drop(s);
    let source_bytes = std::fs::read(path.join("meta.db")).unwrap();
    let stream = root.path().join("policy.jsonl");
    let imported = root.path().join("imported");
    assert_eq!(
        export_store(&path, &stream).unwrap(),
        import_store(&stream, &imported).unwrap()
    );
    assert_eq!(std::fs::read(path.join("meta.db")).unwrap(), source_bytes);
    let mut s = open(&imported);
    assert_eq!(imported_rows(&s), before_rows);
    assert_eq!(
        control::encode(&s.engine.get_namespace(id).unwrap().unwrap()).unwrap(),
        config
    );
    let replay = s
        .update_policy(&noop_key, scope(&s, id), &original)
        .unwrap()
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, noop.outcome);
    drop(s);
    let archive = run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("policy.zip"),
        Some(&imported),
        false,
    )
    .await
    .unwrap();
    restore_from_backup(RestoreOptions {
        backup_path: archive,
        data_dir: imported.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let mut s = open(&imported);
    assert_ne!(s.engine.control_state().unwrap().store_id, noop.store_id);
    assert_eq!(imported_rows(&s), before_rows);
    assert_eq!(
        control::encode(&s.engine.get_namespace(id).unwrap().unwrap()).unwrap(),
        config
    );
    assert!(matches!(
        s.update_policy(&noop_key, scope(&s, id), &original),
        Err(StorageError::StoreLineageMismatch)
    ));
    let receipt_key = control::scoped_key(
        noop.store_id,
        MutationScope::Namespace(id),
        noop.operation_id.as_bytes(),
    );
    let tx = s.metadata().db.begin_write().unwrap();
    let mut table = tx.open_table(control::RECEIPTS).unwrap();
    let mut stored: control::StoredReceipt =
        control::decode(table.get(receipt_key.as_slice()).unwrap().unwrap().value()).unwrap();
    assert_eq!(stored.receipt, noop);
    let MutationOutcome::NativeNamespacePolicyUnchanged { transition } =
        &mut stored.receipt.outcome
    else {
        panic!()
    };
    transition.policy = crate::storage::native_policy::policy_state(&changed).unwrap();
    table
        .insert(
            receipt_key.as_slice(),
            control::encode(&stored).unwrap().as_slice(),
        )
        .unwrap();
    drop(table);
    tx.commit().unwrap();
    drop(s);
    let corrupt = std::fs::read(imported.join("meta.db")).unwrap();
    assert!(
        NativeLifecycleSession::open(&imported, NativePolicyInputs::default(), Arc::new(NoClock))
            .is_err(),
        "foreign no-change policy forged to a later revision was admitted"
    );
    assert_eq!(std::fs::read(imported.join("meta.db")).unwrap(), corrupt);
}

#[test]
fn native_policy_version_migration_preserves_numeric_revision_chain() {
    use crate::storage::lifecycle_migration::{LifecycleMigrationOptions, migrate_lifecycle};
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let mut policy = NamespacePolicy::from(&s.engine.get_namespace(id).unwrap().unwrap());
    policy.desired_retention = 0.7;
    s.update_policy(&ctx(&s, id, "numeric-before"), scope(&s, id), &policy)
        .unwrap()
        .unwrap();
    let state = s.engine.control_state().unwrap();
    let ns = s.engine.get_namespace(id).unwrap().unwrap();
    drop(s);
    let target = root.path().join("explicit");
    migrate_lifecycle(
        &path,
        &target,
        &LifecycleMigrationOptions {
            expected_source: state.generation_ref(),
            namespace_id: id,
            expected_policy_revision: ns.schema.unwrap().policy_revision,
            target: crate::model::schema::LifecycleVersion::ExplicitFeedbackV2,
            idempotency_key: "switch".into(),
        },
    )
    .unwrap();
    let mut s = open(&target);
    policy.desired_retention = 0.6;
    s.update_policy(&ctx(&s, id, "numeric-after"), scope(&s, id), &policy)
        .unwrap()
        .unwrap();
    let noop = s
        .update_policy(&ctx(&s, id, "numeric-noop"), scope(&s, id), &policy)
        .unwrap()
        .unwrap();
    assert!(matches!(
        noop.outcome,
        MutationOutcome::NativeNamespacePolicyUnchanged { .. }
    ));
    drop(s);
    inspect_schema(&target).unwrap();
}

struct CreationClock;
impl crate::time::Clock for CreationClock {
    fn now_millis(&self) -> i64 {
        3000
    }
}
struct CatalogProvider;
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for CatalogProvider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        panic!("policy called provider")
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "policy-fixture"
    }
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        Some(crate::embedding::runtime::tests::profile())
    }
}
#[test]
fn native_policy_creation_proof_rewinds_numeric_changes_in_empty_namespace() {
    let root = tempfile::tempdir().unwrap();
    let (path, _, _) = fixture(root.path());
    let mut s = open(&path);
    s.clock = Arc::new(CreationClock);
    let mut create = ctx(&s, NamespaceId::new(1), "new-namespace");
    create.scope = MutationScope::Catalog;
    let receipt = s
        .create_namespace(
            &create,
            &crate::storage::native_catalog::NativeNamespaceInput {
                name: "new-policy".into(),
                embedding_dim: None,
                embedding_profile_id: None,
                initial_stability: None,
                desired_retention: None,
                decay_rate_multiplier: None,
            },
            &CatalogProvider,
        )
        .unwrap();
    let MutationOutcome::NativeNamespaceCreated { transition } = &receipt.outcome else {
        panic!()
    };
    let id = transition.namespace_id;
    s.clock = Arc::new(NoClock);
    let mut policy = NamespacePolicy::from(&s.engine.get_namespace(id).unwrap().unwrap());
    for (key, stability) in [("first", 5.), ("second", 7.)] {
        policy.initial_stability = stability;
        let receipt = s
            .update_policy(&ctx(&s, id, key), scope(&s, id), &policy)
            .unwrap()
            .unwrap();
        let MutationOutcome::NativeNamespacePolicyUpdated { transition } = &receipt.outcome else {
            panic!()
        };
        assert_eq!(transition.record_count, 0);
    }
    s.update_policy(&ctx(&s, id, "no-change"), scope(&s, id), &policy)
        .unwrap()
        .unwrap();
    drop(s);
    inspect_schema(&path).unwrap();
    let stream = root.path().join("created-policy.jsonl");
    let copy = root.path().join("copy");
    assert_eq!(
        crate::storage::logical_export::export_store(&path, &stream).unwrap(),
        crate::storage::logical_export::import_store(&stream, &copy).unwrap()
    );
    let s = open(&copy);
    assert_eq!(
        s.engine
            .get_namespace(id)
            .unwrap()
            .unwrap()
            .initial_stability,
        7.
    );
}

#[test]
fn native_context_api1_policy_keyed_nochange_replay_after_other_policy() {
    let root = tempfile::tempdir().unwrap();
    let (path, id, _) = fixture(root.path());
    let mut s = open(&path);
    let original = crate::storage::catalog::NamespacePolicy::from(
        &s.engine.get_namespace(id).unwrap().unwrap(),
    );
    let mut key = ctx(&s, id, "api1-nochange");
    key.contract_version = 1;
    let first = s
        .update_policy(&key, scope(&s, id), &original)
        .unwrap()
        .unwrap();
    assert!(matches!(
        first.outcome,
        MutationOutcome::NativeNamespacePolicyUnchanged { .. }
    ));
    let mut changed = original.clone();
    changed.desired_retention = 0.8;
    let mut fresh = ctx(&s, id, "api1-changed");
    fresh.contract_version = 1;
    s.update_policy(&fresh, scope(&s, id), &changed)
        .unwrap()
        .unwrap();
    let before = rows(&s).iter().map(|r| r.to_bytes()).collect::<Vec<_>>();
    let state = s.engine.control_state().unwrap();
    let mut expected = first;
    expected.replayed = true;
    assert_eq!(
        s.update_policy(&key, scope(&s, id), &original)
            .unwrap()
            .unwrap(),
        expected
    );
    let mut other = key.clone();
    other.contract_version = 2;
    assert!(matches!(
        s.update_policy(&other, scope(&s, id), &original),
        Err(StorageError::IdempotencyConflict)
    ));
    assert!(matches!(
        s.update_policy(&key, scope(&s, id), &changed),
        Err(StorageError::IdempotencyConflict)
    ));
    assert_eq!(
        rows(&s).iter().map(|r| r.to_bytes()).collect::<Vec<_>>(),
        before
    );
    assert_eq!(s.engine.control_state().unwrap(), state);
    drop(s);
    let mut s = open(&path);
    assert_eq!(
        s.update_policy(&key, scope(&s, id), &original)
            .unwrap()
            .unwrap(),
        expected
    );
}
