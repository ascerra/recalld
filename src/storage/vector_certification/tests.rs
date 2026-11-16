use super::*;
use crate::storage::{RedbStorageEngine, logical_export, offline_snapshot::OfflineSnapshot};
use redb::ReadableTable;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub(crate) struct Provider {
    profile: crate::model::schema::EmbeddingProfile,
    texts: Mutex<Vec<String>>,
    fail: bool,
}
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for Provider {
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        Some(self.profile.clone())
    }
    fn dimensions(&self) -> usize {
        self.profile.dimension as usize
    }
    fn model_name(&self) -> &str {
        &self.profile.model_id
    }
    async fn embed(&self, text: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.texts.lock().unwrap().push(text.into());
        if self.fail {
            return Err(crate::embedding::EmbeddingError::Unavailable(
                "test provider failure".into(),
            ));
        }
        Ok(vec![0., 1., 0., 0.])
    }
}
pub(crate) fn fixture(root: &Path) -> (PathBuf, ReembedVectors, Provider) {
    let export = root.join("genuine-before.jsonl");
    std::fs::write(
        &export,
        include_bytes!("../metadata/native_records/fixtures/document-recipe-legacy.jsonl"),
    )
    .unwrap();
    let path = root.join("old");
    logical_export::import_store(&export, &path).unwrap();
    let history: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../metadata/native_records/fixtures/document-recipe-legacy-receipts.json"
    ))
    .unwrap();
    let profile: crate::model::schema::EmbeddingProfile =
        serde_json::from_value(history["profile"].clone()).unwrap();
    let ns: NamespaceId = serde_json::from_value(history["namespaceId"].clone()).unwrap();
    let snapshot = OfflineSnapshot::open(&path).unwrap();
    let source = snapshot.control().generation_ref();
    let namespace = snapshot.namespaces().iter().find(|n| n.id == ns).unwrap();
    let request = ReembedVectors {
        source,
        namespace_id: ns,
        namespace_name: namespace.name.clone(),
        profile_id: profile.profile_id().unwrap(),
        context: MutationContext {
            store_id: Some(source.store_id),
            expected_generation: Some(source.generation),
            scope: MutationScope::Namespace(ns),
            contract_version: 1,
            idempotency_key: Some("explicit-reembedding".into()),
        },
    };
    (
        path,
        request,
        Provider {
            profile,
            texts: Mutex::new(vec![]),
            fail: false,
        },
    )
}
fn file_hashes(path: &Path) -> BTreeMap<PathBuf, [u8; 32]> {
    crate::storage::validation::canonical_files(path)
        .unwrap()
        .into_iter()
        .map(|p| {
            (
                p.strip_prefix(path).unwrap().to_owned(),
                hash_bytes(&std::fs::read(p).unwrap()),
            )
        })
        .collect()
}
fn receipts(snapshot: &OfflineSnapshot) -> BTreeMap<Vec<u8>, Vec<u8>> {
    snapshot
        .metadata
        .begin_read()
        .unwrap()
        .open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|row| {
            let (key, value) = row.unwrap();
            (key.value().to_vec(), value.value().to_vec())
        })
        .collect()
}
fn limits() -> crate::storage::canonical_vector_snapshot::SnapshotLimits {
    crate::storage::canonical_vector_snapshot::SnapshotLimits {
        max_records: 1000,
        max_bytes: 1024 * 1024,
    }
}

#[path = "admission_tests.rs"]
mod admission;

#[tokio::test]
async fn vector_certification_genuine_wrong_document_remedy_replay_export_reopen() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let before = file_hashes(&source);
    let old = OfflineSnapshot::open(&source).unwrap();
    let original_receipts = receipts(&old);
    let mut documents = vec![];
    old.for_each_record(|raw| {
        if raw.record.namespace_id == request.namespace_id.get() && raw.embedding.is_some() {
            documents.push(retained_document(&raw.record, raw.full_text.as_deref())?.text);
        }
        Ok(())
    })
    .unwrap();
    drop(old);
    let engine = RedbStorageEngine::open_native(&source).unwrap();
    assert!(
        engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap_err()
            .to_string()
            .contains("re-embedding")
    );
    drop(engine);
    // Opening the actual owned source may update redb bookkeeping; immutable
    // remedy assertion begins after that independently tested serving rejection.
    let before_remedy = file_hashes(&source);
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        before_remedy.keys().collect::<Vec<_>>()
    );
    let destination = root.path().join("certified");
    let receipt = reembed_fresh(&source, &destination, &request, &provider)
        .await
        .unwrap();
    assert_eq!(
        receipt.committed_generation,
        request.source.generation.next().unwrap()
    );
    assert_eq!(*provider.texts.lock().unwrap(), documents);
    assert!(
        documents
            .iter()
            .any(|text| text != "body" && text.contains("header"))
    );
    assert_eq!(file_hashes(&source), before_remedy);
    let snapshot = OfflineSnapshot::open(&destination).unwrap();
    let after_receipts = receipts(&snapshot);
    for (key, bytes) in original_receipts {
        assert_eq!(after_receipts.get(&key), Some(&bytes));
    }
    assert_eq!(snapshot.control().store_id, request.source.store_id);
    let ns = snapshot
        .namespaces()
        .iter()
        .find(|n| n.id == request.namespace_id)
        .unwrap();
    assert_eq!(
        crate::storage::native_records::profile_id(ns).unwrap(),
        request.profile_id
    );
    assert!(ns.schema.as_ref().unwrap().vector_certification.is_some());
    drop(snapshot);
    let replay = reembed_fresh_with_provider::<&dyn crate::embedding::EmbeddingProvider, _>(
        &source,
        &destination,
        &request,
        async { panic!("replay constructed fresh provider") },
    )
    .await
    .unwrap();
    assert!(replay.replayed);
    let replay_provider = Provider {
        fail: true,
        ..provider
    };
    let replay = reembed_fresh(&source, &destination, &request, &replay_provider)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(*replay_provider.texts.lock().unwrap(), documents);
    let engine = RedbStorageEngine::open_native(&destination).unwrap();
    let captured = engine
        .canonical_vector_snapshot(&request.namespace_name, None, limits())
        .unwrap();
    assert_eq!(captured.binding.at.generation, receipt.committed_generation);
    drop(engine);
    let export = root.path().join("certified.jsonl");
    logical_export::export_store(&destination, &export).unwrap();
    let imported = root.path().join("roundtrip");
    logical_export::import_store(&export, &imported).unwrap();
    let reopened = OfflineSnapshot::open(&imported).unwrap();
    assert_eq!(receipts(&reopened), after_receipts);
}

#[tokio::test]
async fn vector_certification_provider_failure_leaves_source_and_destination_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, mut provider) = fixture(root.path());
    provider.fail = true;
    let before = file_hashes(&source);
    let destination = root.path().join("failed");
    let error = reembed_fresh(&source, &destination, &request, &provider)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("test provider failure"),
        "{error:?}"
    );
    assert_eq!(file_hashes(&source), before);
    assert!(!destination.exists());
    assert!(
        !root
            .path()
            .join("failed.recalld-vector-certification")
            .exists()
    );
}

#[tokio::test]
async fn vector_certification_publication_failure_and_unknown_ack_replay_exact_receipt() {
    for point in [
        "certification_before_commit",
        "certification_after_commit",
        "certification_published",
        "certification_commit_unknown",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (source, request, provider) = fixture(root.path());
        let before = file_hashes(&source);
        let target = root.path().join("result");
        crate::storage::catalog::FAILURE.with(|f| f.set(Some(point)));
        let error = reembed_fresh(&source, &target, &request, &provider)
            .await
            .unwrap_err();
        crate::storage::catalog::FAILURE.with(|f| f.set(None));
        assert_eq!(file_hashes(&source), before);
        let stage = root.path().join("result.recalld-vector-certification");
        if point == "certification_before_commit" {
            assert!(!target.exists() && !stage.exists());
            assert!(!matches!(
                error,
                StorageError::CommittedMutation { .. } | StorageError::CommitOutcomeUnknown { .. }
            ));
            continue;
        }
        let retained =
            OfflineSnapshot::open(if target.exists() { &target } else { &stage }).unwrap();
        let retained_receipt = receipts(&retained)
            .values()
            .find_map(|bytes| {
                let receipt = control::validate_receipt(bytes).unwrap().receipt;
                matches!(
                    receipt.outcome,
                    control::MutationOutcome::VectorsReembedded { .. }
                )
                .then_some(receipt)
            })
            .unwrap();
        drop(retained);
        match error {
            StorageError::CommittedMutation { receipt, .. } => {
                assert_eq!(*receipt, retained_receipt)
            }
            StorageError::CommitOutcomeUnknown { operation_id, .. } => {
                assert_eq!(point, "certification_commit_unknown");
                assert_eq!(operation_id, retained_receipt.operation_id);
            }
            other => panic!("lost known cutover state: {other:?}"),
        }
        let no_provider = Provider {
            profile: provider.profile,
            texts: Mutex::new(vec![]),
            fail: true,
        };
        let replay = reembed_fresh(
            &root.path().join("source-no-longer-needed"),
            &target,
            &request,
            &no_provider,
        )
        .await
        .unwrap();
        let mut expected = retained_receipt;
        expected.replayed = true;
        assert_eq!(replay, expected);
        assert!(no_provider.texts.lock().unwrap().is_empty());
        assert!(!stage.exists());
        let mut changed = request.clone();
        changed.context.contract_version = 2;
        assert!(matches!(
            reembed_fresh(&source, &target, &changed, &no_provider).await,
            Err(StorageError::IdempotencyConflict)
        ));
    }
}

#[tokio::test]
async fn vector_certification_provider_cancellation_cleans_only_uncommitted_private_stage() {
    struct PendingProvider(Provider);
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for PendingProvider {
        fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
            Some(self.0.profile.clone())
        }
        fn dimensions(&self) -> usize {
            self.0.profile.dimension as usize
        }
        fn model_name(&self) -> &str {
            &self.0.profile.model_id
        }
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            std::future::pending().await
        }
    }
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let before = file_hashes(&source);
    let target = root.path().join("cancelled");
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        reembed_fresh(&source, &target, &request, &PendingProvider(provider)),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(file_hashes(&source), before);
    assert!(!target.exists());
    assert!(
        !root
            .path()
            .join("cancelled.recalld-vector-certification")
            .exists()
    );
}

fn scope(request: &ReembedVectors) -> crate::storage::native_lifecycle::LifecycleScope {
    crate::storage::native_lifecycle::LifecycleScope {
        namespace_id: request.namespace_id,
        namespace_name: request.namespace_name.clone(),
        source: None,
    }
}
fn fresh_context(request: &ReembedVectors, key: &str) -> MutationContext {
    MutationContext {
        expected_generation: None,
        idempotency_key: Some(key.into()),
        ..request.context.clone()
    }
}
fn fresh_input() -> NativeInsert {
    NativeInsert {
        memory: InsertMemory {
            id: None,
            summary: "new certified header".into(),
            full_text: Some("actual certified body".into()),
            tags: vec![],
            initial_stability: None,
            created_at: None,
            embedding: None,
            parent_id: None,
            supersedes: None,
        },
        observed_at: None,
        entities: vec!["José".into()],
        topics: vec![],
        emotions: vec![],
        embedding_profile: None,
    }
}

#[tokio::test]
async fn vector_certification_owned_commits_and_queries_do_not_rescan_history() {
    use crate::storage::StorageEngine as _;
    use crate::storage::native_lifecycle::{NativeLifecycleSession, NativePolicyInputs};
    use std::sync::Arc;
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let destination = root.path().join("owned");
    reembed_fresh(&source, &destination, &request, &provider)
        .await
        .unwrap();
    crate::test_support::explicitly_adopt_inherited_policy(
        &destination,
        request.namespace_id,
        1.,
        100_000_000,
    );
    let mut session = NativeLifecycleSession::open(
        &destination,
        NativePolicyInputs::default(),
        Arc::new(crate::time::ManualClock::new(100_000_000)),
    )
    .unwrap();
    control::take_validation_work();
    for _ in 0..3 {
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap();
    }
    assert_eq!(
        control::take_validation_work(),
        control::ValidationWork::default()
    );
    let input = fresh_input();
    let receipt = session
        .insert_generated(
            &fresh_context(&request, "fresh-generated"),
            scope(&request),
            &input,
            &provider,
        )
        .await
        .unwrap();
    let work = control::take_validation_work();
    assert_eq!(work.full_validations, 0, "{work:?}");
    assert_eq!(work.certification_scans, 0, "{work:?}");
    assert_eq!(work.receipt_decodes, 1); // validate_existing checks this new effect's retained receipt.
    let pending = session.metadata().pending_effects().unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0].canonical_applied);
    assert_eq!(pending[0].receipt, receipt);
    let current = session
        .engine
        .canonical_vector_snapshot(&request.namespace_name, None, limits())
        .unwrap();
    assert_eq!(current.binding.at.generation, receipt.committed_generation);
    let ns = session
        .engine
        .get_namespace(request.namespace_id)
        .unwrap()
        .unwrap();
    let noop = session
        .update_policy(
            &fresh_context(&request, "keyed-policy-noop"),
            scope(&request),
            &crate::storage::catalog::NamespacePolicy::from(&ns),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        noop.committed_generation,
        receipt.committed_generation.next().unwrap()
    );
    assert_eq!(
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap()
            .binding
            .at
            .generation,
        noop.committed_generation
    );
    // Grow history while retaining the first insert's real unacknowledged outbox.
    // The second insert must validate exactly the old + new effect receipts;
    // the twelve unrelated retained policy receipts add no work. The existing
    // explicit supplied-UUID lifetime scan is a different, preserved path.
    for index in 0..12 {
        session
            .update_policy(
                &fresh_context(&request, &format!("history-noop-{index}")),
                scope(&request),
                &crate::storage::catalog::NamespacePolicy::from(&ns),
            )
            .unwrap()
            .unwrap();
    }
    control::take_validation_work();
    let second = session
        .insert_generated(
            &fresh_context(&request, "second-generated"),
            scope(&request),
            &fresh_input(),
            &provider,
        )
        .await
        .unwrap();
    assert_eq!(
        control::take_validation_work(),
        control::ValidationWork {
            receipt_decodes: 2,
            ..Default::default()
        }
    );
    let pending = session.metadata().pending_effects().unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().all(|p| p.canonical_applied));
    let actual: std::collections::HashSet<_> =
        pending.iter().map(|p| p.receipt.operation_id).collect();
    assert_eq!(
        actual,
        [receipt.operation_id, second.operation_id]
            .into_iter()
            .collect()
    );
    let batch_context = MutationContext {
        scope: MutationScope::Catalog,
        contract_version: 2,
        ..fresh_context(&request, "batch-reservation")
    };
    let error = crate::storage::batch::BatchError::new("INVALID_INPUT");
    let batch = session
        .metadata()
        .reserve_batch(
            &batch_context,
            crate::storage::batch::BatchKind::Insert,
            &[crate::storage::batch::BatchItemPlan::NativeInsert {
                request_hash: [17; 32],
                namespace: request.namespace_name.clone(),
                input: fresh_input(),
            }],
        )
        .unwrap();
    assert_eq!(
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap()
            .binding
            .at
            .generation,
        batch.reservation_receipt.committed_generation
    );
    let completed = session
        .metadata()
        .finalize_batch_error(&batch.child(0).unwrap(), error)
        .unwrap();
    assert!(
        matches!(&completed.items[0].state, crate::storage::batch::BatchItemState::Error { error } if error.code == "INVALID_INPUT")
    );
    let at = session.engine.control_state().unwrap().generation_ref();
    assert_eq!(
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap()
            .binding
            .at,
        at
    );
    let work = control::take_validation_work();
    assert_eq!(work.full_validations, 0, "{work:?}");
    assert_eq!(work.certification_scans, 0, "{work:?}");
    drop(session);
    OfflineSnapshot::open(&destination).unwrap();
}

#[tokio::test]
async fn vector_certification_complete_purge_retains_format2_anchor_without_placements() {
    use crate::storage::StorageEngine as _;
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let destination = root.path().join("purged");
    reembed_fresh(&source, &destination, &request, &provider)
        .await
        .unwrap();
    let mut engine = RedbStorageEngine::open_native(&destination).unwrap();
    let mut progress = engine
        .begin_namespace_purge(&fresh_context(&request, "purge-certified"), true)
        .unwrap();
    let original_parent = progress.start_receipt.operation_id;
    let mut steps = 0;
    while progress.stage != crate::storage::purge::PurgeStage::Complete {
        progress = engine
            .resume_namespace_purge(progress.store_id, request.namespace_id, original_parent, 1)
            .unwrap();
        steps += 1;
        assert!(steps < 20);
        if progress.stage == crate::storage::purge::PurgeStage::Complete {
            let at = engine.control_state().unwrap().generation_ref();
            assert_eq!(
                engine
                    .meta_store()
                    .with_generation_read_at(at, |view| Ok(view.at))
                    .unwrap(),
                at
            );
        } else {
            assert!(
                engine
                    .control_state()
                    .unwrap_err()
                    .to_string()
                    .contains("purge pending")
            );
        }
    }
    drop(engine);
    let snapshot = OfflineSnapshot::open(&destination).unwrap();
    assert!(
        snapshot
            .namespaces()
            .iter()
            .all(|n| n.id != request.namespace_id)
    );
    assert_eq!(snapshot.record_count().unwrap(), 0);
    drop(snapshot);
    let export = root.path().join("purged.jsonl");
    let summary = logical_export::export_store(&destination, &export).unwrap();
    let header: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(&export)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(header["data"]["exportFormatVersion"], 2);
    assert!(
        !std::fs::read_to_string(&export)
            .unwrap()
            .contains("vectorPlacement")
    );
    let imported = root.path().join("purged-import");
    assert_eq!(
        logical_export::import_store(&export, &imported).unwrap(),
        summary
    );
    OfflineSnapshot::open(&imported).unwrap();
}

#[tokio::test]
async fn vector_certification_restore_rejects_pre_cutover_target_pair_and_preserves_two_fork_origins()
 {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let config = crate::config::RecalldConfig::default();
    let old_backup =
        crate::backup::run_backup(&config, &root.path().join("old.zip"), Some(&source), false)
            .await
            .unwrap();
    let target = root.path().join("target");
    let certificate = reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap();
    let source_before = file_hashes(&source);
    let target_before = file_hashes(&target);
    let archive_before = std::fs::read(&old_backup).unwrap();
    let options = |archive: &Path, target: &Path| crate::backup::RestoreOptions {
        backup_path: archive.to_owned(),
        data_dir: target.to_owned(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    };
    let error = crate::backup::restore_from_backup(options(&old_backup, &target))
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("vector certification"),
        "{error:?}"
    );
    assert_eq!(file_hashes(&source), source_before);
    assert_eq!(file_hashes(&target), target_before);
    assert_eq!(std::fs::read(&old_backup).unwrap(), archive_before);
    assert!(!root.path().join("target.recalld-restore-pending").exists());
    let backup = crate::backup::run_backup(
        &config,
        &root.path().join("certified.zip"),
        Some(&target),
        false,
    )
    .await
    .unwrap();
    let mut origins = vec![request.source.store_id];
    for index in 0..2 {
        crate::backup::restore_from_backup(options(&backup, &target))
            .await
            .unwrap();
        let current = OfflineSnapshot::open(&target).unwrap();
        assert!(!origins.contains(&current.control().store_id));
        origins.push(current.control().store_id);
        let ns = current
            .namespaces()
            .iter()
            .find(|n| n.id == request.namespace_id)
            .unwrap();
        let binding = ns
            .schema
            .as_ref()
            .unwrap()
            .vector_certification
            .as_ref()
            .unwrap();
        assert_eq!(binding.origin.store_id, certificate.store_id);
        assert_eq!(binding.origin.generation, certificate.committed_generation);
        assert_eq!(binding.operation_id, certificate.operation_id);
        let rows = receipts(&current);
        assert!(
            rows.values()
                .any(|value| control::validate_receipt(value).unwrap().receipt == certificate)
        );
        drop(current);
        let export = root.path().join(format!("fork-{index}.jsonl"));
        logical_export::export_store(&target, &export).unwrap();
        logical_export::import_store(&export, &root.path().join(format!("fork-{index}-import")))
            .unwrap();
        let engine = RedbStorageEngine::open_native(&target).unwrap();
        engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap();
    }
}

#[tokio::test]
async fn vector_certification_root_omission_and_binding_corruption_reject_without_source_writes() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let target = root.path().join("valid");
    reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap();
    let export = root.path().join("valid.jsonl");
    logical_export::export_store(&target, &export).unwrap();
    for kind in [
        "omit-row",
        "strip-feature",
        "foreign-anchor",
        "omit-receipt",
    ] {
        let copy = root.path().join(kind);
        logical_export::import_store(&export, &copy).unwrap();
        let db = redb::Database::open(copy.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        if kind == "omit-row" {
            let mut table = tx.open_table(crate::storage::metadata::META_TABLE).unwrap();
            let id = table
                .iter()
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .0
                .value()
                .to_vec();
            table.remove(id.as_slice()).unwrap();
        } else if kind == "omit-receipt" {
            let mut table = tx.open_table(control::RECEIPTS).unwrap();
            let key = table
                .iter()
                .unwrap()
                .find_map(|row| {
                    let (key, value) = row.unwrap();
                    matches!(
                        control::validate_receipt(value.value())
                            .unwrap()
                            .receipt
                            .outcome,
                        control::MutationOutcome::VectorsReembedded { .. }
                    )
                    .then(|| key.value().to_vec())
                })
                .unwrap();
            table.remove(key.as_slice()).unwrap();
        } else {
            let mut table = tx
                .open_table(crate::storage::metadata::NAMESPACE_TABLE)
                .unwrap();
            let mut ns: NamespaceConfig = control::decode(
                table
                    .get(request.namespace_id.get())
                    .unwrap()
                    .unwrap()
                    .value(),
            )
            .unwrap();
            let schema = ns.schema.as_mut().unwrap();
            if kind == "strip-feature" {
                schema.required_features.clear();
            } else {
                schema
                    .vector_certification
                    .as_mut()
                    .unwrap()
                    .origin
                    .store_id = Uuid::new_v4();
            }
            table
                .insert(
                    request.namespace_id.get(),
                    control::encode(&ns).unwrap().as_slice(),
                )
                .unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        let bytes = std::fs::read(copy.join("meta.db")).unwrap();
        let result = OfflineSnapshot::open(&copy);
        let error = match result {
            Ok(_) => panic!("accepted {kind}"),
            Err(error) => error,
        };
        if kind == "omit-row" {
            assert!(
                error
                    .to_string()
                    .contains("cutover reverse inventory count"),
                "{error:?}"
            );
        }
        assert_eq!(
            std::fs::read(copy.join("meta.db")).unwrap(),
            bytes,
            "source changed for {kind}"
        );
        assert!(RedbStorageEngine::open_native(&copy).is_err());
        assert_eq!(
            std::fs::read(copy.join("meta.db")).unwrap(),
            bytes,
            "live preflight changed {kind}"
        );
    }
}

#[tokio::test]
async fn vector_certification_foreign_legacy_components_cannot_drift_from_retained_document() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let target = root.path().join("foreign");
    reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap();
    let backup = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("foreign.zip"),
        Some(&target),
        false,
    )
    .await
    .unwrap();
    crate::backup::restore_from_backup(crate::backup::RestoreOptions {
        backup_path: backup,
        data_dir: target.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let snapshot = OfflineSnapshot::open(&target).unwrap();
    let mut retained = None;
    snapshot
        .for_each_record(|row| {
            if row
                .record
                .schema
                .as_ref()
                .unwrap()
                .lifecycle
                .timestamp_origin
                == crate::model::schema::TimestampOrigin::LegacyInferred
            {
                retained = Some(row.record);
            }
            Ok(())
        })
        .unwrap();
    drop(snapshot);
    let record = retained.unwrap();
    let inventory = file_hashes(&target);
    for case in [
        "summary",
        "tags",
        "labels",
        "extensions",
        "text",
        "missing-text",
    ] {
        let altered = root.path().join(case);
        std::fs::create_dir(&altered).unwrap();
        for name in inventory.keys() {
            let destination = altered.join(name);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::copy(target.join(name), destination).unwrap();
        }
        let mut record = record.clone();
        let db = redb::Database::open(altered.join("meta.db")).unwrap();
        match case {
            "summary" => record.summary = "changed retained input without a writer receipt".into(),
            "tags" => record.tags.clear(),
            "labels" => record
                .schema
                .as_mut()
                .unwrap()
                .labels
                .entities
                .push("unproven".into()),
            "extensions" => {
                record
                    .schema
                    .as_mut()
                    .unwrap()
                    .extensions
                    .insert("unproven".into(), serde_json::json!(true));
            }
            "missing-text" => {
                record.text_offset = 0;
                record.text_length = 0;
            }
            "text" => {
                let read = db.begin_read().unwrap();
                let descriptor = crate::storage::metadata::text_generation::read_selection(&read)
                    .unwrap()
                    .unwrap();
                let mut text =
                    crate::storage::TextStore::open_generation(&altered, &descriptor, true)
                        .unwrap();
                let reference = text.append("changed actual retained full text").unwrap();
                text.sync_data().unwrap();
                record.text_offset = reference.file_offset;
                record.text_length = reference.length;
            }
            _ => unreachable!(),
        }
        if case == "tags" && record.tags.is_empty() {
            record.tags.push(crate::model::Tag::new("changed").unwrap());
        }
        let tx = db.begin_write().unwrap();
        tx.open_table(crate::storage::metadata::META_TABLE)
            .unwrap()
            .insert(record.id.as_slice(), record.encode().unwrap().as_slice())
            .unwrap();
        tx.commit().unwrap();
        drop(db);
        let bytes = std::fs::read(altered.join("meta.db")).unwrap();
        assert!(
            OfflineSnapshot::open(&altered).is_err(),
            "foreign cutover accepted changed retained {case}"
        );
        assert_eq!(std::fs::read(altered.join("meta.db")).unwrap(), bytes);
    }
    assert_eq!(file_hashes(&target), inventory);
}

#[tokio::test]
async fn vector_certification_foreign_legacy_phase_loss_and_deletion_remain_valid() {
    use crate::storage::{
        StorageEngine as _,
        native_lifecycle::{NativeLifecycleSession, NativePolicyInputs},
        native_maintenance::NativeDecayResult,
    };
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let target = root.path().join("loss");
    reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap();
    crate::test_support::explicitly_adopt_inherited_policy(
        &target,
        request.namespace_id,
        1.,
        100_000_000,
    );
    let archive = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("loss.zip"),
        Some(&target),
        false,
    )
    .await
    .unwrap();
    crate::backup::restore_from_backup(crate::backup::RestoreOptions {
        backup_path: archive,
        data_dir: target.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let snapshot = OfflineSnapshot::open(&target).unwrap();
    let original_receipts = receipts(&snapshot);
    let original_origin = snapshot
        .namespaces()
        .iter()
        .find(|n| n.id == request.namespace_id)
        .unwrap()
        .schema
        .as_ref()
        .unwrap()
        .vector_certification
        .clone();
    let mut legacy_id = None;
    snapshot
        .for_each_record(|row| {
            if row
                .record
                .schema
                .as_ref()
                .unwrap()
                .lifecycle
                .timestamp_origin
                == crate::model::schema::TimestampOrigin::LegacyInferred
            {
                legacy_id = Some(MemoryId::from_bytes(row.record.id));
            }
            Ok(())
        })
        .unwrap();
    drop(snapshot);
    let id = legacy_id.unwrap();
    for (key, phase) in [
        ("summary", Some(DecayPhase::Summary)),
        ("ghost", Some(DecayPhase::Ghost)),
        ("delete", None),
    ] {
        let mut session = NativeLifecycleSession::open(
            &target,
            NativePolicyInputs::default(),
            std::sync::Arc::new(crate::time::ManualClock::new(864_000_000_000)),
        )
        .unwrap();
        let context = MutationContext {
            store_id: Some(session.engine.control_state().unwrap().store_id),
            ..fresh_context(&request, key)
        };
        let NativeDecayResult::Committed(receipt) = session.maintain(&context, id).unwrap() else {
            panic!("phase not committed");
        };
        assert_eq!(
            session.engine.get_record(id).unwrap().map(|r| r.phase),
            phase
        );
        let at = session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap()
            .binding
            .at;
        assert_eq!(at.generation, receipt.committed_generation);
        drop(session);
        let snapshot = OfflineSnapshot::open(&target).unwrap();
        assert_eq!(
            snapshot
                .namespaces()
                .iter()
                .find(|n| n.id == request.namespace_id)
                .unwrap()
                .schema
                .as_ref()
                .unwrap()
                .vector_certification,
            original_origin
        );
        let actual_receipts = receipts(&snapshot);
        for (key, value) in &original_receipts {
            assert_eq!(actual_receipts.get(key), Some(value));
        }
        drop(snapshot);
        let export = root.path().join(format!("{key}.jsonl"));
        logical_export::export_store(&target, &export).unwrap();
        logical_export::import_store(&export, &root.path().join(format!("import-{key}"))).unwrap();
    }
}

#[tokio::test]
async fn vector_certification_unknown_record_commit_blocks_semantics_then_replays_once() {
    use crate::storage::{
        StorageEngine as _,
        native_lifecycle::{NativeLifecycleSession, NativePolicyInputs},
    };
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let target = root.path().join("unknown-record");
    reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap();
    crate::test_support::explicitly_adopt_inherited_policy(
        &target,
        request.namespace_id,
        1.,
        100_000_000,
    );
    let clock = std::sync::Arc::new(crate::time::ManualClock::new(100_000_000));
    let mut session =
        NativeLifecycleSession::open(&target, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let before = session.engine.control_state().unwrap().generation_ref();
    let context = fresh_context(&request, "unknown-record");
    let input = fresh_input();
    crate::storage::metadata::effects::FAIL_AFTER_RECORD_COMMIT.with(|flag| flag.set(true));
    let error = session
        .insert_generated(&context, scope(&request), &input, &provider)
        .await
        .unwrap_err();
    let StorageError::CommitOutcomeUnknown { operation_id, .. } = error else {
        panic!("{error:?}");
    };
    assert!(
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .is_err()
    );
    assert!(session.engine.control_state().is_err());
    drop(session);
    let mut session =
        NativeLifecycleSession::open(&target, NativePolicyInputs::default(), clock).unwrap();
    assert_eq!(
        session.engine.control_state().unwrap().committed_generation,
        before.generation.next().unwrap()
    );
    let provider = Provider {
        fail: true,
        ..provider
    };
    let calls = provider.texts.lock().unwrap().len();
    let replay = session
        .insert_generated(&context, scope(&request), &input, &provider)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, operation_id);
    assert_eq!(provider.texts.lock().unwrap().len(), calls);
    assert_eq!(
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap()
            .binding
            .at
            .generation,
        replay.committed_generation
    );
    drop(session);
    OfflineSnapshot::open(&target).unwrap();
}

#[tokio::test]
async fn vector_certification_retained_summary_remedy_preserves_absence_and_legacy_labels() {
    use crate::storage::{
        StorageEngine as _,
        native_lifecycle::{NativeLifecycleSession, NativePolicyInputs},
        native_maintenance::NativeDecayResult,
    };
    let root = tempfile::tempdir().unwrap();
    let (source, mut request, provider) = fixture(root.path());
    crate::test_support::explicitly_adopt_inherited_policy(
        &source,
        request.namespace_id,
        1.,
        100_000_000,
    );
    let mut session = NativeLifecycleSession::open(
        &source,
        NativePolicyInputs::default(),
        std::sync::Arc::new(crate::time::ManualClock::new(864_000_000_000)),
    )
    .unwrap();
    let legacy = session
        .engine
        .scan_all()
        .unwrap()
        .into_iter()
        .find(|(_, r)| {
            r.schema.as_ref().unwrap().lifecycle.timestamp_origin
                == crate::model::schema::TimestampOrigin::LegacyInferred
        })
        .unwrap()
        .0;
    assert!(matches!(
        session
            .maintain(&fresh_context(&request, "actual-summary"), legacy)
            .unwrap(),
        NativeDecayResult::Committed(_)
    ));
    let original = session.engine.get_record(legacy).unwrap().unwrap();
    assert_eq!(original.phase, DecayPhase::Summary);
    assert_eq!(original.text_length, 0);
    request.source = session.engine.control_state().unwrap().generation_ref();
    request.context.expected_generation = Some(request.source.generation);
    drop(session);
    let before = file_hashes(&source);
    let destination = root.path().join("summary-certified");
    let receipt = reembed_fresh(&source, &destination, &request, &provider)
        .await
        .unwrap();
    let control::MutationOutcome::VectorsReembedded { transition } = receipt.outcome else {
        panic!()
    };
    let evidence = &transition
        .records
        .iter()
        .find(|r| r.record.memory_id == legacy)
        .unwrap()
        .document;
    assert_eq!(evidence.phase, DecayPhase::Summary);
    assert_eq!(evidence.full_text, RetainedTextInput::Absent);
    assert_eq!(
        evidence.label_provenance,
        crate::model::schema::LabelProvenance::LegacyRetainedOnly
    );
    assert_eq!(file_hashes(&source), before);
    let snapshot = OfflineSnapshot::open(&destination).unwrap();
    let current = snapshot.record(legacy).unwrap().unwrap();
    assert_eq!(
        current.record.schema.as_ref().unwrap().labels,
        original.schema.as_ref().unwrap().labels
    );
    assert!(current.full_text.is_none());
    assert_eq!(current.record.phase, DecayPhase::Summary);
}

#[tokio::test]
async fn vector_certification_catalog_graph_lifecycle_and_real_derived_ack_carry_exact_generation()
{
    use crate::storage::native_graph::{NativeEdgeInput, NativeGraphInput};
    use crate::storage::{
        StorageEngine as _,
        native_lifecycle::{NativeLifecycleSession, NativePolicyInputs},
    };
    use std::sync::{Arc, RwLock};
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let target = root.path().join("writers");
    reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap();
    crate::test_support::explicitly_adopt_inherited_policy(
        &target,
        request.namespace_id,
        1.,
        100_000_000,
    );
    let mut session = NativeLifecycleSession::open(
        &target,
        NativePolicyInputs::default(),
        Arc::new(crate::time::ManualClock::new(100_000_000)),
    )
    .unwrap();
    let context = |key: &str| MutationContext {
        contract_version: 2,
        ..fresh_context(&request, key)
    };
    let catalog = MutationContext {
        scope: MutationScope::Catalog,
        ..context("create-certified")
    };
    let created = session
        .create_namespace(
            &catalog,
            &crate::storage::native_catalog::NativeNamespaceInput {
                name: "new-certified".into(),
                embedding_dim: Some(4),
                embedding_profile_id: None,
                initial_stability: None,
                desired_retention: None,
                decay_rate_multiplier: None,
            },
            &provider,
        )
        .unwrap();
    let control::MutationOutcome::NativeNamespaceCreated { transition } = &created.outcome else {
        panic!()
    };
    let new_ns = transition.namespace_id;
    let binding = transition.vector_certification.as_ref().unwrap();
    assert_eq!(binding.origin.generation, created.committed_generation);
    assert_eq!(binding.vector_count, 0);
    assert!(
        session
            .engine
            .canonical_vector_snapshot("new-certified", None, limits())
            .unwrap()
            .rows
            .is_empty()
    );
    let mut ids = vec![];
    for key in ["first", "second"] {
        let receipt = session
            .insert_generated(&context(key), scope(&request), &fresh_input(), &provider)
            .await
            .unwrap();
        let control::MutationOutcome::NativeMemoryInserted { transition } = &receipt.outcome else {
            panic!()
        };
        ids.push(transition.memory_id);
    }
    let graph = NativeGraphInput {
        edges: vec![NativeEdgeInput {
            source: ids[0],
            target: ids[1],
            edge_type: crate::model::EdgeType::Associative,
            weight: 0.5,
            auto_created: false,
        }],
    };
    let receipt = session
        .put_graph(&context("edge"), scope(&request), &graph)
        .unwrap()
        .unwrap();
    assert_eq!(
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap()
            .binding
            .at
            .generation,
        receipt.committed_generation
    );
    let noop = session
        .put_graph(&context("edge-noop"), scope(&request), &graph)
        .unwrap()
        .unwrap();
    assert_eq!(
        noop.committed_generation,
        receipt.committed_generation.next().unwrap()
    );
    assert_eq!(
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap()
            .binding
            .at
            .generation,
        noop.committed_generation
    );
    let feedback = session
        .capture_feedback(ids[0], Some(request.namespace_id), 3)
        .unwrap();
    let receipt = session
        .apply_feedback(&context("feedback"), &feedback)
        .unwrap();
    assert_eq!(
        session
            .engine
            .canonical_vector_snapshot(&request.namespace_name, None, limits())
            .unwrap()
            .binding
            .at
            .generation,
        receipt.committed_generation
    );
    let exposure = session
        .capture_exposure(
            scope(&request),
            &[(ids[0], crate::model::AccessKind::DirectRetrieval)],
        )
        .unwrap();
    if let Some(receipt) = session
        .record_exposure(&context("exposure"), &exposure)
        .unwrap()
    {
        assert_eq!(
            session
                .engine
                .canonical_vector_snapshot(&request.namespace_name, None, limits())
                .unwrap()
                .binding
                .at
                .generation,
            receipt.committed_generation
        );
    } else {
        panic!("actual exposure did not commit");
    }
    // The composed physical writer carries the same owned certification through
    // exactly one canonical generation, without changing its namespace binding.
    let before = session.engine.control_state().unwrap().generation_ref();
    let selected = session.engine.compact_text_log().unwrap().receipt;
    assert!(matches!(
        selected.outcome,
        control::MutationOutcome::NativeTextGenerationSelected { .. }
    ));
    let after = GenerationRef {
        generation: before.generation.next().unwrap(),
        ..before
    };
    assert_eq!(selected.store_id, after.store_id);
    assert_eq!(selected.committed_generation, after.generation);
    assert_eq!(
        session
            .engine
            .lookup_operation(
                selected.store_id,
                MutationScope::Catalog,
                selected.operation_id
            )
            .unwrap(),
        Some(selected)
    );
    assert_eq!(
        session.engine.control_state().unwrap().generation_ref(),
        after
    );
    assert_eq!(
        session
            .engine
            .canonical_vector_snapshot("new-certified", None, limits())
            .unwrap()
            .binding
            .at,
        after
    );
    std::fs::create_dir(root.path().join("fts")).unwrap();
    let owner = crate::derived_completion::DerivedCompletion {
        storage: Arc::new(RwLock::new(session.engine)),
        cache: Arc::new(crate::cache::CacheManager::new(
            crate::cache::CacheConfig {
                max_capacity_bytes: 1024 * 1024,
                ..Default::default()
            },
            None,
        )),
        graph: Arc::new(tokio::sync::RwLock::new(
            crate::graph::RelationshipGraph::new(),
        )),
        vectors: Arc::new(tokio::sync::RwLock::new(
            crate::search::FlatVectorIndex::new(4),
        )),
        fts: Arc::new(tokio::sync::Mutex::new(
            crate::search::FtsIndex::new(&root.path().join("fts")).unwrap(),
        )),
        entities: Arc::new(tokio::sync::RwLock::new(crate::search::EntityIndex::new())),
    };
    owner.rebuild_all().await.unwrap();
    {
        let engine = owner.storage.read().unwrap();
        assert!(engine.meta_store().pending_effects().unwrap().is_empty());
        assert_eq!(engine.control_state().unwrap().generation_ref(), after);
        let publication = engine.meta_store().publication_state().unwrap();
        assert_eq!(publication.canonical, after);
        assert!(publication.matches_canonical());
        assert_eq!(
            engine
                .canonical_vector_snapshot("new-certified", None, limits())
                .unwrap()
                .binding
                .at,
            after
        );
        assert_eq!(
            engine
                .get_namespace(new_ns)
                .unwrap()
                .unwrap()
                .schema
                .unwrap()
                .vector_certification
                .as_ref(),
            Some(binding)
        );
    }
    drop(owner);
    OfflineSnapshot::open(&target).unwrap();
}
