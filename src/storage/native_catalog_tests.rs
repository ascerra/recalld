use super::*;
use crate::embedding::{EmbeddingError, EmbeddingProvider};
use crate::storage::RedbStorageEngine;
use crate::storage::control::MutationOutcome;
use crate::storage::native_lifecycle::{NativeLifecycleSession, NativePolicyInputs};
use crate::storage::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Clock(AtomicUsize);
impl crate::time::Clock for Clock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        2000
    }
}
struct Provider(bool);
#[async_trait::async_trait]
impl EmbeddingProvider for Provider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, EmbeddingError> {
        panic!("catalog called embedding")
    }
    fn dimensions(&self) -> usize {
        assert!(self.0, "replay consulted provider");
        4
    }
    fn model_name(&self) -> &str {
        "catalog-fixture"
    }
    fn profile(&self) -> Option<EmbeddingProfile> {
        assert!(self.0, "replay consulted provider");
        Some(profile())
    }
}
fn profile() -> EmbeddingProfile {
    crate::embedding::runtime::tests::profile()
}
fn input() -> NativeNamespaceInput {
    NativeNamespaceInput {
        name: "created".into(),
        embedding_dim: None,
        embedding_profile_id: None,
        initial_stability: Some(3.),
        desired_retention: None,
        decay_rate_multiplier: None,
    }
}
fn fixture(root: &Path) -> (PathBuf, Arc<Clock>, NativeLifecycleSession) {
    let source = root.join("legacy");
    let mut engine = RedbStorageEngine::open(&source).unwrap();
    let mut ns = NamespaceConfig::default_namespace(100);
    ns.embedding_dim = 4;
    engine.create_namespace(&ns).unwrap();
    engine.sync().unwrap();
    drop(engine);
    let target = root.join("native");
    migrate_schema(
        &source,
        &target,
        &MigrationOptions {
            migration_now: 1000,
            expected_source: inspect_schema(&source).unwrap().control.generation_ref(),
            attestations: vec![],
        },
    )
    .unwrap();
    let clock = Arc::new(Clock(AtomicUsize::new(0)));
    let session =
        NativeLifecycleSession::open(&target, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    (target, clock, session)
}
fn context(session: &NativeLifecycleSession, key: &str) -> MutationContext {
    let state = session.engine.control_state().unwrap();
    MutationContext {
        store_id: Some(state.store_id),
        scope: MutationScope::Catalog,
        idempotency_key: Some(key.into()),
        expected_generation: Some(state.committed_generation),
        contract_version: 2,
    }
}
fn bytes(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            out.extend(bytes(&entry.path()));
        } else {
            out.insert(entry.path(), std::fs::read(entry.path()).unwrap());
        }
    }
    out
}

#[test]
fn native_catalog_replay_before_provider_cas_and_real_record_reopen() {
    let root = tempfile::tempdir().unwrap();
    let (path, clock, mut session) = fixture(root.path());
    let ctx = context(&session, "create");
    let receipt = session
        .create_namespace(&ctx, &input(), &Provider(true))
        .unwrap();
    let MutationOutcome::NativeNamespaceCreated { transition } = &receipt.outcome else {
        panic!()
    };
    let id = transition.namespace_id;
    let ns = session.engine.get_namespace(id).unwrap().unwrap();
    assert_eq!(
        ns.schema.as_ref().unwrap().lifecycle_version,
        crate::model::schema::LifecycleVersion::LegacyAccessV1
    );
    assert!(matches!(
        ns.schema.as_ref().unwrap().embedding_binding,
        EmbeddingBinding::Bound {
            provenance: BindingProvenance::NativeV2,
            ..
        }
    ));
    assert!(path.join("created/vectors.dat").is_file());
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let replay = session
        .create_namespace(&ctx, &input(), &Provider(false))
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(replay.committed_generation, receipt.committed_generation);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let memory = crate::storage::native_records::NativeInsert {
        memory: crate::storage::insertion::InsertMemory {
            id: None,
            summary: "native retained text".into(),
            full_text: None,
            tags: vec![],
            initial_stability: None,
            created_at: None,
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        },
        observed_at: None,
        entities: vec!["日本語".into()],
        topics: vec![],
        emotions: vec![],
        embedding_profile: Some(profile()),
    };
    let intent = session
        .capture_insert(
            crate::storage::native_lifecycle::LifecycleScope {
                namespace_id: id,
                namespace_name: ns.name,
                source: None,
            },
            &memory,
        )
        .unwrap();
    let mut insert_ctx = context(&session, "insert");
    insert_ctx.scope = MutationScope::Namespace(id);
    session
        .insert_prepared(
            &insert_ctx,
            &intent,
            &crate::embedding::runtime::GeneratedEmbedding {
                values: vec![],
                profile: Some(profile()),
            },
        )
        .unwrap();
    session.engine.sync().unwrap();
    drop(session);
    let mut reopened =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
    assert!(
        reopened
            .create_namespace(&ctx, &input(), &Provider(false))
            .unwrap()
            .replayed
    );
    assert!(reopened.engine.get_namespace(id).unwrap().is_some());
    drop(reopened);
    let before = bytes(&path);
    assert!(RedbStorageEngine::open(&path).is_err());
    assert_eq!(bytes(&path), before);
}

#[test]
fn native_catalog_rejections_do_not_allocate_or_publish() {
    let root = tempfile::tempdir().unwrap();
    let (path, clock, mut session) = fixture(root.path());
    let ctx = context(&session, "bad");
    let control = session.engine.control_state().unwrap();
    for dim in [0, 5, 8193] {
        let mut bad = input();
        bad.embedding_dim = Some(dim);
        assert!(
            session
                .create_namespace(&ctx, &bad, &Provider(true))
                .is_err()
        );
        assert_eq!(session.engine.control_state().unwrap(), control);
        assert!(!path.join("created").exists());
    }
    let mut wrong = input();
    wrong.embedding_profile_id = Some(format!("ep1:{}", "0".repeat(64)));
    assert!(
        session
            .create_namespace(&ctx, &wrong, &Provider(true))
            .is_err()
    );
    let receipt = session
        .create_namespace(&ctx, &input(), &Provider(true))
        .unwrap();
    let committed = session.engine.control_state().unwrap();
    let mut changed = input();
    changed.initial_stability = Some(4.);
    assert!(matches!(
        session.create_namespace(&ctx, &changed, &Provider(false)),
        Err(StorageError::IdempotencyConflict)
    ));
    let mut foreign = ctx.clone();
    foreign.store_id = Some(uuid::Uuid::new_v4());
    assert!(matches!(
        session.create_namespace(&foreign, &input(), &Provider(false)),
        Err(StorageError::StoreLineageMismatch)
    ));
    assert_eq!(session.engine.control_state().unwrap(), committed);
    assert_eq!(committed.committed_generation, receipt.committed_generation);
    assert!(clock.0.load(Ordering::SeqCst) >= 1);
}

#[test]
fn native_catalog_publication_fault_reconciles_receipt_on_reopen() {
    for point in [
        "before_stage",
        "after_payload_publish",
        "before_metadata_commit",
        "after_metadata_commit",
        "before_handle_publish",
        "after_handle_publish",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (path, clock, mut session) = fixture(root.path());
        let ctx = context(&session, "fault");
        crate::storage::catalog::FAILURE.with(|f| f.set(Some(point)));
        let result = session.create_namespace(&ctx, &input(), &Provider(true));
        crate::storage::catalog::FAILURE.with(|f| f.set(None));
        assert!(result.is_err(), "{point}");
        drop(session);
        let mut reopened =
            NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
        let receipt = reopened
            .create_namespace(&ctx, &input(), &Provider(true))
            .unwrap();
        assert_eq!(
            receipt.replayed,
            matches!(
                point,
                "after_metadata_commit" | "before_handle_publish" | "after_handle_publish"
            ),
            "{point}"
        );
        assert!(path.join("created/vectors.dat").is_file());
    }
}

#[test]
fn forged_native_catalog_configuration_is_rejected_immutably() {
    let root = tempfile::tempdir().unwrap();
    let (path, _, mut session) = fixture(root.path());
    let ctx = context(&session, "create");
    let receipt = session
        .create_namespace(&ctx, &input(), &Provider(true))
        .unwrap();
    let MutationOutcome::NativeNamespaceCreated { transition } = receipt.outcome else {
        panic!()
    };
    drop(session);
    let db = redb::Database::open(path.join("meta.db")).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx
            .open_table(crate::storage::metadata::NAMESPACE_TABLE)
            .unwrap();
        let mut ns = NamespaceConfig::decode(
            table
                .get(transition.namespace_id.get())
                .unwrap()
                .unwrap()
                .value(),
        )
        .unwrap();
        ns.initial_stability += 1.;
        table
            .insert(ns.id.get(), control::encode(&ns).unwrap().as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let before = bytes(&path);
    assert!(inspect_schema(&path).is_err());
    assert_eq!(bytes(&path), before);
}
