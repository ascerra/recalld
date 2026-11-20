use super::control::{Generation, MutationOutcome};
use super::control::{MutationContext, MutationScope};
use super::inherited_policy::*;
use super::native_lifecycle::{LifecycleScope, NativeLifecycleSession, NativePolicyInputs};
use super::offline_snapshot::OfflineSnapshot;
use super::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
use super::{RedbStorageEngine, StorageEngine, StorageError};
use crate::decay::DecayConfig;
use crate::model::{MemoryId, NamespaceConfig, NamespaceId};
use redb::ReadableTable;
use std::path::Path;
use std::sync::Arc;

struct Clock;
impl crate::time::Clock for Clock {
    fn now_millis(&self) -> i64 {
        2_592_000_000
    }
}
fn fixture(root: &Path) -> (std::path::PathBuf, NamespaceId, MemoryId) {
    let (path, ns, id, _) = fixture_with_peer(root, false);
    (path, ns, id)
}
fn fixture_with_peer(
    root: &Path,
    peer: bool,
) -> (std::path::PathBuf, NamespaceId, MemoryId, Option<MemoryId>) {
    fixture_with_phase(root, peer, false)
}
fn fixture_with_phase(
    root: &Path,
    peer: bool,
    ghost: bool,
) -> (std::path::PathBuf, NamespaceId, MemoryId, Option<MemoryId>) {
    fixture_with_binding(root, peer, ghost, false)
}
fn bound_fixture(root: &Path) -> (std::path::PathBuf, NamespaceId, MemoryId) {
    let (path, ns, id, _) = fixture_with_binding(root, false, false, true);
    (path, ns, id)
}
fn fixture_with_binding(
    root: &Path,
    peer: bool,
    ghost: bool,
    bound: bool,
) -> (std::path::PathBuf, NamespaceId, MemoryId, Option<MemoryId>) {
    let old = root.join("old");
    let new = root.join("native");
    let mut engine = RedbStorageEngine::open(&old).unwrap();
    let mut ns = NamespaceConfig::default_namespace(1);
    ns.embedding_dim = 4;
    // Genuine unknown inheritance: the actual legacy writer persists None.
    let id = engine.create_namespace(&ns).unwrap();
    let receipt = engine
        .insert_memory_with_receipt(
            &MutationContext::unkeyed(id),
            &super::insertion::InsertMemory {
                id: None,
                summary: "retained".into(),
                full_text: Some("retained long text".into()),
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
    let memory = super::insertion::inserted_id(&receipt).unwrap();
    let peer = peer.then(|| {
        super::insertion::inserted_id(
            &engine
                .insert_memory_with_receipt(
                    &MutationContext::unkeyed(id),
                    &super::insertion::InsertMemory {
                        id: None,
                        summary: "peer".into(),
                        full_text: None,
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
                .unwrap(),
        )
        .unwrap()
    });
    if ghost {
        let peer = peer.unwrap();
        engine
            .add_edge(
                memory,
                peer,
                crate::model::EdgeType::ParentChild,
                0.5,
                false,
                0,
            )
            .unwrap();
        let namespace = engine.get_namespace(id).unwrap().unwrap();
        for phase in [
            crate::model::DecayPhase::Summary,
            crate::model::DecayPhase::Ghost,
        ] {
            let before = engine.get_record(memory).unwrap().unwrap();
            assert!(
                engine
                    .apply_legacy_decay(memory, &before, &namespace, phase, 0.01, 0.01)
                    .unwrap()
            );
        }
    }
    engine.sync().unwrap();
    drop(engine);
    migrate_schema(
        &old,
        &new,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&old).unwrap().control.generation_ref(),
            attestations: if bound {
                vec![super::schema_migration::ProfileAttestationInput {
                    namespace_id: id,
                    namespace_name: "default".into(),
                    descriptor: crate::embedding::runtime::native_profile(
                        crate::model::schema::ProviderKind::Ollama,
                        "inherited-policy-fixture",
                        4,
                    ),
                }]
            } else {
                vec![]
            },
        },
    )
    .unwrap();
    (new, id, memory, peer)
}
fn request(path: &Path, id: NamespaceId, value: f64, key: &str) -> AdoptInheritedDecay {
    let source = inspect_schema(path).unwrap().control.generation_ref();
    let preview = preview_inherited_decay(path, id, value, source, &Clock).unwrap();
    AdoptInheritedDecay {
        context: MutationContext {
            store_id: Some(source.store_id),
            scope: MutationScope::Namespace(id),
            idempotency_key: Some(key.into()),
            expected_generation: Some(source.generation),
            contract_version: 1,
        },
        preview,
    }
}
fn session(path: &Path, multiplier: f64) -> NativeLifecycleSession {
    NativeLifecycleSession::open(
        path,
        NativePolicyInputs {
            multiplier,
            ..Default::default()
        },
        Arc::new(Clock),
    )
    .unwrap()
}
#[test]
fn inherited_policy_unknown_requires_real_adoption_replay_before_cas_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let (path, ns, id) = bound_fixture(root.path());
    let owner = session(&path, 1.);
    let before = owner.engine.get_record(id).unwrap().unwrap();
    assert!(
        owner
            .metadata()
            .capture_feedback(id, Some(ns), 3, &NativePolicyInputs::default())
            .unwrap_err()
            .to_string()
            .contains("unknown")
    );
    drop(owner);
    let req = request(&path, ns, 1., "adopt");
    assert!(req.preview.prior.is_none());
    assert_eq!(req.preview.record_count, 1);
    let receipt = adopt_inherited_decay(&path, &req).unwrap();
    let owner = session(&path, 1.);
    let after = owner.engine.get_record(id).unwrap().unwrap();
    let mut expected = before.clone();
    let fields = &mut expected.schema.as_mut().unwrap().lifecycle;
    fields.policy_revision = fields.policy_revision.next().unwrap();
    fields.record_generation = fields.record_generation.next().unwrap();
    assert_eq!(after.encode().unwrap(), expected.encode().unwrap());
    assert!(
        owner
            .metadata()
            .capture_feedback(id, Some(ns), 3, &NativePolicyInputs::default())
            .is_ok()
    );
    let ns_cfg = owner.engine.get_namespace(ns).unwrap().unwrap();
    assert!(
        resolve_bound(&ns_cfg, &DecayConfig::default(), 0.)
            .unwrap_err()
            .to_string()
            .contains("differs")
    );
    drop(owner);
    let later = request(&path, ns, 0., "adopt-zero");
    let changed = adopt_inherited_decay(&path, &later).unwrap();
    let replay = adopt_inherited_decay(&path, &req).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(
        inspect_schema(&path).unwrap().control.committed_generation,
        changed.committed_generation
    );
    let mut conflict = req.clone();
    conflict.preview = request(&path, ns, 0., "other").preview;
    conflict.context.expected_generation = Some(conflict.preview.source.generation);
    let bytes = std::fs::read(path.join("meta.db")).unwrap();
    assert!(adopt_inherited_decay(&path, &conflict).is_err());
    assert_eq!(std::fs::read(path.join("meta.db")).unwrap(), bytes);
}
#[test]
fn inherited_policy_keyed_noop_no_row_churn_and_some_to_none_binds_current_multiplier() {
    let root = tempfile::tempdir().unwrap();
    let (path, ns, id) = bound_fixture(root.path());
    adopt_inherited_decay(&path, &request(&path, ns, 1., "initial")).unwrap();
    let snapshot = OfflineSnapshot::open(&path).unwrap();
    let before = snapshot.record(id).unwrap().unwrap().record;
    drop(snapshot);
    let noop = request(&path, ns, 1. + f64::EPSILON, "noop");
    let receipt = adopt_inherited_decay(&path, &noop).unwrap();
    let MutationOutcome::InheritedDecayAdopted { transition } = receipt.outcome else {
        panic!()
    };
    assert!(!transition.changed());
    let snapshot = OfflineSnapshot::open(&path).unwrap();
    assert_eq!(
        snapshot
            .record(id)
            .unwrap()
            .unwrap()
            .record
            .encode()
            .unwrap(),
        before.encode().unwrap()
    );
    drop(snapshot);
    let mut owner = session(&path, 1.);
    let mut policy =
        super::catalog::NamespacePolicy::from(&owner.engine.get_namespace(ns).unwrap().unwrap());
    let scope = LifecycleScope {
        namespace_id: ns,
        namespace_name: owner.engine.get_namespace(ns).unwrap().unwrap().name,
        source: None,
    };
    policy.decay_rate_multiplier = Some(1.);
    owner
        .update_policy(&MutationContext::unkeyed(ns), scope.clone(), &policy)
        .unwrap();
    // Unused global changes do not affect an explicit override.
    assert!(
        resolve_bound(
            &owner.engine.get_namespace(ns).unwrap().unwrap(),
            &DecayConfig::default(),
            0.
        )
        .is_ok()
    );
    policy.decay_rate_multiplier = None;
    owner
        .update_policy(&MutationContext::unkeyed(ns), scope, &policy)
        .unwrap();
    // #237: returning to inheritance binds the owner's configured multiplier
    // (1.0 here) in the same receipt, so dependent operations keep working.
    let binding = owner
        .engine
        .get_namespace(ns)
        .unwrap()
        .unwrap()
        .schema
        .unwrap()
        .inherited_decay
        .unwrap();
    assert_eq!(binding.multiplier_bits, 1f32.to_bits());
    assert!(
        owner
            .metadata()
            .capture_feedback(id, Some(ns), 3, &NativePolicyInputs::default())
            .is_ok()
    );
    drop(owner);
    // A later configuration change still needs explicit adoption, from that binding.
    let next = request(&path, ns, 0., "after-clear");
    assert_eq!(next.preview.prior, Some(binding));
    adopt_inherited_decay(&path, &next).unwrap();
    assert!(
        session(&path, 0.)
            .metadata()
            .capture_feedback(
                id,
                Some(ns),
                3,
                &NativePolicyInputs {
                    multiplier: 0.,
                    ..Default::default()
                }
            )
            .is_ok()
    );
}
#[test]
fn inherited_policy_codec_and_preview_tampering_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let (path, ns, _) = fixture(root.path());
    let request = request(&path, ns, 1., "real");
    let old = crate::model::schema::NamespaceSchemaV2::from_legacy();
    let encoded = serde_json::to_value(&old).unwrap();
    assert!(encoded.get("inheritedDecay").is_none());
    let mut bad = encoded.clone();
    bad["inheritedDecay"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<crate::model::schema::NamespaceSchemaV2>(bad).is_err());
    let mut changed = request.clone();
    changed.preview.candidate.retained += 1;
    assert!(adopt_inherited_decay(&path, &changed).is_err());
    assert_eq!(
        inspect_schema(&path).unwrap().control.generation_ref(),
        request.preview.source
    );
    for value in [f64::NAN, f64::INFINITY, -1., f64::MIN_POSITIVE, f64::MAX] {
        assert!(normalize(value).is_err());
    }
    assert_eq!(normalize(-0.).unwrap(), 0);
    let mut custom = DecayConfig::default();
    custom.max_connection_bonus = 0.2;
    assert!(supported_inputs(&custom).is_err());
    adopt_inherited_decay(&path, &request).unwrap();
    let generation = inspect_schema(&path).unwrap().control.committed_generation;
    assert!(generation > Generation::default());
}

#[tokio::test]
async fn inherited_policy_import_fork_preserves_original_binding_and_stripping_fails() {
    use super::logical_export::{export_store, import_store};
    use crate::backup::{RestoreOptions, restore_from_backup, run_backup};
    let root = tempfile::tempdir().unwrap();
    let (path, ns, _) = fixture(root.path());
    let req = request(&path, ns, 1., "origin");
    let receipt = adopt_inherited_decay(&path, &req).unwrap();
    let binding = inspect_schema(&path).unwrap().namespaces[0]
        .schema
        .as_ref()
        .unwrap()
        .inherited_decay
        .clone()
        .unwrap();
    let source_bytes = std::fs::read(path.join("meta.db")).unwrap();
    let stream = root.path().join("logical.jsonl");
    let imported = root.path().join("imported");
    assert_eq!(
        export_store(&path, &stream).unwrap(),
        import_store(&stream, &imported).unwrap()
    );
    assert_eq!(std::fs::read(path.join("meta.db")).unwrap(), source_bytes);
    for n in 0..2 {
        let archive = run_backup(
            &crate::config::RecalldConfig::default(),
            &root.path().join(format!("fork-{n}.zip")),
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
        let after = inspect_schema(&imported).unwrap();
        assert_ne!(after.control.store_id, receipt.store_id);
        assert_eq!(
            after.namespaces[0]
                .schema
                .as_ref()
                .unwrap()
                .inherited_decay
                .as_ref(),
            Some(&binding)
        );
    }
    // Deliberate corruption of the current field, with authoritative adoption
    // receipt unchanged. This is a negative, never a compatibility fixture setup.
    let db = redb::Database::open(imported.join("meta.db")).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx.open_table(super::metadata::NAMESPACE_TABLE).unwrap();
        let mut namespace =
            NamespaceConfig::decode(table.get(ns.get()).unwrap().unwrap().value()).unwrap();
        namespace.schema.as_mut().unwrap().inherited_decay = None;
        table
            .insert(
                ns.get(),
                super::control::encode(&namespace).unwrap().as_slice(),
            )
            .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let corrupt = std::fs::read(imported.join("meta.db")).unwrap();
    assert!(inspect_schema(&imported).is_err());
    assert!(RedbStorageEngine::open_native(&imported).is_err());
    assert_eq!(std::fs::read(imported.join("meta.db")).unwrap(), corrupt);
}

#[test]
fn inherited_policy_postcommit_errors_keep_exact_receipt_unknown_ack_keeps_allocated_identity() {
    for failure in [
        "before_metadata_commit",
        "after_metadata_commit",
        "commit_unknown",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (path, ns, _) = fixture(root.path());
        let req = request(&path, ns, 1., "fault");
        super::catalog::FAILURE.with(|f| f.set(Some(failure)));
        let result = adopt_inherited_decay(&path, &req);
        super::catalog::FAILURE.with(|f| f.set(None));
        let error = result.unwrap_err();
        let after = inspect_schema(&path).unwrap();
        if failure == "before_metadata_commit" {
            assert_eq!(after.control.generation_ref(), req.preview.source);
            assert!(
                after.namespaces[0]
                    .schema
                    .as_ref()
                    .unwrap()
                    .inherited_decay
                    .is_none()
            );
        } else {
            let replay = adopt_inherited_decay(&path, &req).unwrap();
            assert!(replay.replayed);
            match error {
                StorageError::CommittedMutation { receipt, .. }
                    if failure == "after_metadata_commit" =>
                {
                    let mut canonical = replay.clone();
                    canonical.replayed = false;
                    assert_eq!(*receipt, canonical);
                }
                StorageError::CommitOutcomeUnknown { operation_id, .. }
                    if failure == "commit_unknown" =>
                {
                    assert_eq!(operation_id, replay.operation_id)
                }
                e => panic!("wrong acknowledgement classification: {e}"),
            }
            assert_eq!(
                after.control.committed_generation,
                replay.committed_generation
            );
        }
    }
}
#[test]
fn inherited_policy_unknown_history_can_be_forgotten_without_adoption() {
    let root = tempfile::tempdir().unwrap();
    let (path, ns, id) = fixture(root.path());
    let mut owner = session(&path, 1.);
    let intent = owner.capture_tombstone(id, Some(ns)).unwrap();
    let mut ctx = MutationContext::unkeyed(ns);
    ctx.contract_version = 2;
    let receipt = owner.tombstone(&ctx, &intent).unwrap();
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativeMemoryTombstoned { .. }
    ));
    assert!(
        owner
            .engine
            .get_namespace(ns)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .inherited_decay
            .is_none()
    );
    assert_eq!(
        owner.engine.get_record(id).unwrap().unwrap().phase,
        crate::model::DecayPhase::Tombstone
    );
    drop(owner);
    inspect_schema(&path).unwrap();
}

#[test]
fn inherited_policy_unknown_structural_edge_preserves_strength_and_final_guards() {
    use super::native_graph::{NativeEdgeInput, NativeGraphInput};
    let root = tempfile::tempdir().unwrap();
    let (path, ns, a, b) = fixture_with_peer(root.path(), true);
    let b = b.unwrap();
    let mut owner = session(&path, 1.);
    let scope = LifecycleScope {
        namespace_id: ns,
        namespace_name: owner.engine.get_namespace(ns).unwrap().unwrap().name,
        source: None,
    };
    let input = NativeGraphInput {
        edges: vec![NativeEdgeInput {
            source: a,
            target: b,
            edge_type: crate::model::EdgeType::ParentChild,
            weight: 0.5,
            auto_created: false,
        }],
    };
    let before = owner.engine.get_record(a).unwrap().unwrap();
    let intent = owner.capture_graph(scope.clone(), &input).unwrap();
    let mut ctx = MutationContext::unkeyed(ns);
    ctx.contract_version = 2;
    owner.put_graph_prepared(&ctx, &intent).unwrap().unwrap();
    let after = owner.engine.get_record(a).unwrap().unwrap();
    assert_eq!(before.strength.to_bits(), after.strength.to_bits());
    assert_eq!(
        before.decay_strength.to_bits(),
        after.decay_strength.to_bits()
    );
    assert!(
        owner
            .engine
            .get_namespace(ns)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .inherited_decay
            .is_none()
    );
    assert!(
        owner.put_graph_prepared(&ctx, &intent).is_err(),
        "stale row token must fail"
    );
    let intent = owner.capture_graph(scope.clone(), &input).unwrap();
    let mut wrong = ctx.clone();
    wrong.scope = MutationScope::Namespace(NamespaceId::new(99));
    assert!(owner.put_graph_prepared(&wrong, &intent).is_err());
    let mut policy =
        super::catalog::NamespacePolicy::from(&owner.engine.get_namespace(ns).unwrap().unwrap());
    policy.initial_stability += 1.;
    owner
        .update_policy(&MutationContext::unkeyed(ns), scope, &policy)
        .unwrap();
    assert!(
        owner.put_graph_prepared(&ctx, &intent).is_err(),
        "stale policy must fail"
    );
    // UnknownLegacy keeps runtime-compatible feedback without inventing adoption.
    let before_capture = owner.metadata().control_state().unwrap();
    owner
        .metadata()
        .capture_feedback(a, Some(ns), 3, &NativePolicyInputs::default())
        .unwrap();
    assert_eq!(owner.metadata().control_state().unwrap(), before_capture);
    let schema = owner
        .engine
        .get_namespace(ns)
        .unwrap()
        .unwrap()
        .schema
        .unwrap();
    assert!(schema.inherited_decay.is_none());
    assert!(matches!(
        schema.embedding_binding,
        crate::model::schema::EmbeddingBinding::UnknownLegacy
    ));
    drop(owner);
    inspect_schema(&path).unwrap();
}

#[test]
fn inherited_policy_preview_ghost_neighbor_uses_real_maintenance_bonus_and_retains_payload_loss() {
    let root = tempfile::tempdir().unwrap();
    let (path, ns, ghost, _) = fixture_with_phase(root.path(), true, true);
    let before = {
        let owner = RedbStorageEngine::open_native(&path).unwrap();
        owner.get_record(ghost).unwrap().unwrap()
    };
    assert_eq!(before.phase, crate::model::DecayPhase::Ghost);
    assert!(before.summary.is_empty());
    assert_eq!(before.text_length, 0);
    assert!(before.schema.as_ref().unwrap().vector.is_some());
    let request = request(&path, ns, 1., "ghost-policy");
    assert_eq!(request.preview.record_count, 2);
    // The preview uses the actual anchor/stability, not the legacy materialized
    // 0.01 written during fixture phase loss. At this clock the Ghost is retained.
    assert_eq!(request.preview.candidate.ghost_to_delete, 0);
    adopt_inherited_decay(&path, &request).unwrap();
    let owner = RedbStorageEngine::open_native(&path).unwrap();
    let mut after = owner.get_record(ghost).unwrap().unwrap();
    let prior = before.schema.as_ref().unwrap();
    let lifecycle = &mut after.schema.as_mut().unwrap().lifecycle;
    assert_eq!(
        lifecycle.record_generation,
        prior.lifecycle.record_generation.next().unwrap()
    );
    assert_eq!(
        lifecycle.policy_revision,
        prior.lifecycle.policy_revision.next().unwrap()
    );
    lifecycle.record_generation = prior.lifecycle.record_generation;
    lifecycle.policy_revision = prior.lifecycle.policy_revision;
    assert_eq!(after.encode().unwrap(), before.encode().unwrap());
}

#[test]
fn inherited_policy_copy_to_live_open_retains_the_same_source_owner() {
    let root = tempfile::tempdir().unwrap();
    let (path, _, _) = fixture(root.path());
    let original = super::ownership::StoreOwnership::acquire(&path).unwrap();
    let snapshot = OfflineSnapshot::open_owned(original).unwrap();
    assert!(matches!(
        super::ownership::StoreOwnership::acquire(&path),
        Err(StorageError::DatabaseLocked)
    ));
    let owner = snapshot.into_owner();
    assert!(matches!(
        super::ownership::StoreOwnership::acquire(&path),
        Err(StorageError::DatabaseLocked)
    ));
    let engine =
        RedbStorageEngine::open_owned_with_ids(owner, true, Box::new(super::ids::SystemIds))
            .unwrap();
    assert!(matches!(
        super::ownership::StoreOwnership::acquire(&path),
        Err(StorageError::DatabaseLocked)
    ));
    drop(engine);
    assert!(super::ownership::StoreOwnership::acquire(&path).is_ok());
}
