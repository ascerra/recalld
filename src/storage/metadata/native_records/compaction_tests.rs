use super::*;
use crate::storage::ids::{IdKind, IdSource, SeededIds};
use crate::storage::{logical_export, validation};

#[test]
fn certified_compaction_actual_owned_dispatch_participates() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let mut engine = RedbStorageEngine::open_native(&d.path).unwrap();
    let before = engine.control_state().unwrap();
    let result = engine.compact_text_log();
    eprintln!("actual certified text selection result: {result:?}");
    if result.is_err() {
        assert_eq!(engine.control_state().unwrap(), before);
    }
    let result = result.expect(
        "native physical relocation must participate in the owned certification generation",
    );
    assert_eq!(
        result.receipt.committed_generation,
        before.committed_generation.next().unwrap()
    );
    let mut session = NativeLifecycleSession {
        engine,
        policy: NativePolicyInputs::default(),
        clock: Arc::new(FixedClock(AtomicUsize::new(0))),
    };
    control::take_validation_work();
    let intent = session.capture_insert(scope(&d), &input(&d)).unwrap();
    session
        .insert_prepared(
            &ctx(&session, &d, "after-compaction"),
            &intent,
            &generated(),
        )
        .unwrap();
    let work = control::take_validation_work();
    assert_eq!(work.full_validations, 0, "{work:?}");
    assert_eq!(work.certification_scans, 0, "{work:?}");
    drop(session);
    inspect_schema(&d.path).unwrap();
    RedbStorageEngine::open_native(&d.path).unwrap();
}

fn limits() -> crate::storage::canonical_vector_snapshot::SnapshotLimits {
    crate::storage::canonical_vector_snapshot::SnapshotLimits {
        max_records: 100,
        max_bytes: 1024 * 1024,
    }
}

// Exact retained bytes, including signed-zero f32 bits and actual document input.
// Only the physical offset is excluded; all logical tokens/provenance stay in
// the encoded record. This comparison is deliberately independent of the proof.
fn logical_bytes(engine: &RedbStorageEngine) -> std::collections::BTreeMap<MemoryId, Vec<u8>> {
    engine
        .scan_all()
        .unwrap()
        .into_iter()
        .map(|(id, mut row)| {
            let text = if row.text_offset != 0 || row.text_length != 0 {
                engine
                    .get_text(crate::storage::TextRef {
                        file_offset: row.text_offset,
                        length: row.text_length,
                    })
                    .unwrap()
            } else {
                None
            };
            let vector = engine
                .get_vector(NamespaceId::new(row.namespace_id), row.vector_slot)
                .unwrap()
                .map(|v| v.into_iter().map(f32::to_bits).collect::<Vec<_>>());
            let document =
                crate::storage::vector_certification::retained_document(&row, text.as_deref())
                    .unwrap()
                    .text;
            // Keep presence valid while normalizing location for comparison.
            row.text_offset = if text.is_some() { 48 } else { 0 };
            (
                id,
                control::encode(&(row.encode().unwrap(), text, vector, document)).unwrap(),
            )
        })
        .collect()
}

#[test]
fn certified_compaction_mixed_namespaces_preserve_bits_documents_and_exact_owner_without_query_scans()
 {
    fn assert_unknown_snapshot(engine: &RedbStorageEngine) {
        let snapshot = engine
            .canonical_vector_snapshot("uncertified", None, limits())
            .unwrap();
        assert_eq!(snapshot.rows.len(), 1);
        let schema = snapshot.namespace.schema.unwrap();
        assert!(matches!(
            schema.embedding_binding,
            crate::model::schema::EmbeddingBinding::UnknownLegacy
        ));
        assert!(schema.vector_certification.is_none());
    }
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture_with_seed(root.path(), |engine| {
        let mut ns = NamespaceConfig::default_namespace(0);
        ns.name = "uncertified".into();
        ns.embedding_dim = 4;
        let id = engine.create_namespace(&ns).unwrap();
        let memory = crate::storage::insertion::InsertMemory {
            id: None,
            summary: "retained peer é中🙂".into(),
            full_text: Some("unchanged full document\nsecond line".into()),
            tags: vec![crate::model::Tag::new("topic/retained").unwrap()],
            initial_stability: None,
            created_at: Some(123),
            embedding: Some(vec![0.6, -0.0, 0.8, 0.0]),
            parent_id: None,
            supersedes: None,
        };
        engine
            .insert_memory_with_receipt(&MutationContext::unkeyed(id), &memory, &[], 123)
            .unwrap();
    });
    let mut engine = RedbStorageEngine::open_native(&d.path).unwrap();
    let payloads = logical_bytes(&engine);
    assert_eq!(payloads.len(), 2);
    let namespaces = control::encode(&engine.list_namespaces().unwrap()).unwrap();
    for _ in 0..2 {
        let before = engine.control_state().unwrap();
        let result = engine.compact_text_log().unwrap();
        assert_eq!(
            result.receipt.committed_generation,
            before.committed_generation.next().unwrap()
        );
        assert_eq!(logical_bytes(&engine), payloads);
        assert_eq!(
            control::encode(&engine.list_namespaces().unwrap()).unwrap(),
            namespaces
        );
        control::take_validation_work();
        for _ in 0..3 {
            let snapshot = engine
                .canonical_vector_snapshot("native", None, limits())
                .unwrap();
            assert_eq!(
                snapshot.binding.at.generation,
                result.receipt.committed_generation
            );
            assert_eq!(snapshot.binding.at.store_id, before.store_id);
        }
        assert_eq!(
            control::take_validation_work(),
            control::ValidationWork::default()
        );
        // Physical relocation cannot certify an unrelated old namespace.
        assert_unknown_snapshot(&engine);
    }
    drop(engine);
    let old_receipts = mixed_history::receipts(&d.path);
    let stream = root.path().join("mixed.jsonl");
    let manifest = logical_export::export_store(&d.path, &stream).unwrap();
    let imported = root.path().join("mixed-imported");
    assert_eq!(
        logical_export::import_store(&stream, &imported).unwrap(),
        manifest
    );
    assert_eq!(mixed_history::receipts(&imported), old_receipts);
    let engine = RedbStorageEngine::open_native(&imported).unwrap();
    assert_eq!(logical_bytes(&engine), payloads);
    engine
        .canonical_vector_snapshot("native", None, limits())
        .unwrap();
    assert_unknown_snapshot(&engine);
}

#[test]
fn certified_compaction_unknown_commit_disables_owner_until_strict_reopen() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let mut engine = RedbStorageEngine::open_native(&d.path).unwrap();
    let before = engine.control_state().unwrap();
    let payloads = logical_bytes(&engine);
    crate::storage::metadata::text_generation::FAIL_AFTER_TEXT_COMMIT.with(|f| f.set(true));
    let StorageError::CommitOutcomeUnknown { operation_id, .. } =
        engine.compact_text_log().unwrap_err()
    else {
        panic!("unknown commit lost its classification")
    };
    assert!(
        engine
            .canonical_vector_snapshot("native", None, limits())
            .is_err()
    );
    assert!(engine.compact_text_log().is_err());
    drop(engine);
    let engine = RedbStorageEngine::open_native(&d.path).unwrap();
    let receipt = engine
        .lookup_operation(before.store_id, MutationScope::Catalog, operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt.committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativeTextGenerationSelected { .. }
    ));
    assert_eq!(logical_bytes(&engine), payloads);
    assert_eq!(
        engine
            .canonical_vector_snapshot("native", None, limits())
            .unwrap()
            .binding
            .at,
        engine.control_state().unwrap().generation_ref()
    );
}

#[test]
fn certified_compaction_process_crashes_reopen_exact_selection_and_certification() {
    for (stage, committed) in [
        ("EntriesWritten", false),
        ("FileSynced", false),
        ("DirectorySynced", false),
        ("Selected", true),
        ("Published", true),
        ("Cleanup", true),
        ("Unlinked", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let engine = RedbStorageEngine::open_native(&d.path).unwrap();
        let before = engine.control_state().unwrap();
        let payloads = logical_bytes(&engine);
        let selected = engine.meta_store().text_selection().unwrap().unwrap();
        drop(engine);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::engine::text_compaction::tests::compaction_native_process_child",
                "--nocapture",
            ])
            .env("RECALLD_NATIVE_TEXT_CRASH_PATH", &d.path)
            .env("RECALLD_NATIVE_TEXT_CRASH_STAGE", stage)
            .output()
            .unwrap();
        assert_eq!(
            child.status.code(),
            Some(73),
            "{stage}: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        for _ in 0..2 {
            let engine = RedbStorageEngine::open_native(&d.path).unwrap();
            let at = engine.control_state().unwrap().generation_ref();
            assert_eq!(
                at.generation,
                if committed {
                    before.committed_generation.next().unwrap()
                } else {
                    before.committed_generation
                }
            );
            assert_eq!(
                engine.meta_store().text_selection().unwrap().unwrap().id == selected.id,
                !committed
            );
            assert_eq!(logical_bytes(&engine), payloads);
            assert_eq!(
                engine
                    .canonical_vector_snapshot("native", None, limits())
                    .unwrap()
                    .binding
                    .at,
                at
            );
        }
    }
}

fn completion(f: &crate::test_support::Fixture) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}
fn files(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                visit(root, &p, out)
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().into(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut out = Default::default();
    visit(root, root, &mut out);
    out
}

#[test]
fn compaction_current_proof_hash_catalog_references_and_record_root_reject_immutably() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let mut s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let result = s.engine.compact_text_log().unwrap();
    drop(s);
    let pristine = files(&d.path);
    for case in [
        "api",
        "version",
        "missing",
        "before-owner",
        "after-seal",
        "after-refs",
        "record-root",
        "count",
    ] {
        let bad = root.path().join(case);
        std::fs::create_dir(&bad).unwrap();
        for (name, bytes) in &pristine {
            let p = bad.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }
        let db = redb::Database::open(bad.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let key = control::scoped_key(
                result.receipt.store_id,
                MutationScope::Catalog,
                result.receipt.operation_id.as_bytes(),
            );
            let mut table = tx.open_table(control::RECEIPTS).unwrap();
            let raw = table.get(key.as_slice()).unwrap().unwrap().value().to_vec();
            let mut value: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            let proof = &mut value["receipt"]["outcome"]["transition"];
            match case {
                "api" => {}
                "version" => proof["proofVersion"] = serde_json::json!(2),
                "missing" => {
                    proof.as_object_mut().unwrap().remove("source");
                }
                "before-owner" => proof["before"]["storeId"] = serde_json::json!(uuid::Uuid::nil()),
                "after-seal" => {
                    let n = proof["after"]["sealedLength"].as_u64().unwrap();
                    proof["after"]["sealedLength"] = serde_json::json!(n + 1);
                }
                "after-refs" => {
                    let n = proof["afterRefsSha256"][0].as_u64().unwrap();
                    proof["afterRefsSha256"][0] = serde_json::json!(n ^ 1);
                }
                "record-root" => {
                    let n = proof["recordsSha256"][0].as_u64().unwrap();
                    proof["recordsSha256"][0] = serde_json::json!(n ^ 1);
                }
                _ => {
                    let n = proof["recordCount"].as_u64().unwrap();
                    proof["recordCount"] = serde_json::json!(n + 1);
                }
            }
            if case == "api" {
                let MutationOutcome::NativeTextGenerationSelected { transition } =
                    &result.receipt.outcome
                else {
                    panic!()
                };
                let mut ctx = crate::storage::native_text::request_context(transition.source);
                ctx.contract_version = 2;
                value["request_hash"] =
                    serde_json::json!(control::request_hash(&ctx, "compactText", &[]));
            }
            let changed = serde_json::to_vec(&value).unwrap();
            assert_ne!(changed, raw);
            table.insert(key.as_slice(), changed.as_slice()).unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        let before = files(&bad);
        assert!(validation::canonical_files(&bad).is_err(), "{case}");
        assert!(
            RedbStorageEngine::open_with_ids(&bad, true, Box::new(crate::storage::ids::SystemIds))
                .is_err(),
            "{case}"
        );
        assert_eq!(files(&bad), before, "{case}");
    }
}

#[test]
fn compaction_owned_ids_logical_tokens_and_provenance_do_not_change() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let engine =
        RedbStorageEngine::open_with_ids(&d.path, true, Box::new(SeededIds::new([19; 32])))
            .unwrap();
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = NativeLifecycleSession {
        engine,
        policy: NativePolicyInputs::default(),
        clock: clock.clone(),
    };
    let original = input(&d);
    let intent = s.capture_insert(scope(&d), &original).unwrap();
    s.insert_prepared(&ctx(&s, &d, "compaction-native"), &intent, &generated())
        .unwrap();
    let old = s.engine.scan_all().unwrap();
    let expected =
        SeededIds::from_state(s.metadata().id_source().checkpoint().unwrap().unwrap()).unwrap();
    let generation = expected.uuid(IdKind::TextGeneration).unwrap();
    let operation = expected.uuid(IdKind::Operation).unwrap();
    let clock_before = clock.0.load(Ordering::SeqCst);
    let state = s.engine.control_state().unwrap();
    let result = s.engine.compact_text_log().unwrap();
    assert_eq!(result.receipt.operation_id, operation);
    assert_eq!(
        result.receipt.committed_generation,
        state.committed_generation.next().unwrap()
    );
    let MutationOutcome::NativeTextGenerationSelected { transition } = &result.receipt.outcome
    else {
        panic!()
    };
    assert_eq!(transition.after.id, generation);
    assert_eq!(
        s.metadata().id_source().checkpoint().unwrap(),
        expected.checkpoint().unwrap()
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), clock_before);
    for (id, before) in old {
        let mut after = s.engine.get_record(id).unwrap().unwrap();
        after.text_offset = before.text_offset;
        assert_eq!(before.encode().unwrap(), after.encode().unwrap());
    }
    let again = s.engine.compact_text_log().unwrap();
    assert_eq!(again.entries_removed, 0);
    assert_eq!(
        again.receipt.committed_generation,
        result.receipt.committed_generation.next().unwrap()
    );
    drop(s);
    inspect_schema(&d.path).unwrap();
}

#[tokio::test]
async fn compaction_caller_retains_receipt_on_real_publication_failure_and_repairs_same_g() {
    publication_failure(false).await;
}
#[tokio::test]
async fn certified_compaction_caller_retains_new_receipt_and_repairs_same_g() {
    publication_failure(true).await;
}
async fn publication_failure(certified: bool) {
    let root = tempfile::tempdir().unwrap();
    let d = if certified {
        certified_fixture(root.path())
    } else {
        fixture(root.path())
    };
    let engine =
        RedbStorageEngine::open_with_ids(&d.path, true, Box::new(SeededIds::new([20; 32])))
            .unwrap();
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let c = completion(&f);
    c.complete().await.unwrap();
    let result = c.compact_text_with_work(None).await.unwrap();
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(5);
    let error = c.compact_text_with_work(None).await.unwrap_err();
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("lost receipt: {error}")
    };
    assert_eq!(
        receipt.committed_generation,
        result.receipt.committed_generation.next().unwrap()
    );
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .lookup_operation(
                receipt.store_id,
                MutationScope::Catalog,
                receipt.operation_id
            )
            .unwrap(),
        Some(*receipt.clone())
    );
    let state = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .publication_state()
        .unwrap();
    assert!(!state.matches_canonical());
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    c.complete().await.unwrap();
    let repaired = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .publication_state()
        .unwrap();
    assert!(repaired.matches_canonical());
    assert_eq!(repaired.canonical, state.canonical);
}

#[tokio::test]
async fn compaction_prior_phase_failure_keeps_receipt_and_allocates_nothing() {
    prior_failure(false).await;
}
#[tokio::test]
async fn certified_compaction_prior_phase_failure_keeps_receipt_and_allocates_nothing() {
    prior_failure(true).await;
}
async fn prior_failure(certified: bool) {
    let root = tempfile::tempdir().unwrap();
    let d = if certified {
        certified_fixture(root.path())
    } else {
        fixture(root.path())
    };
    let engine =
        RedbStorageEngine::open_with_ids(&d.path, true, Box::new(SeededIds::new([21; 32])))
            .unwrap();
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let c = completion(&f);
    c.complete().await.unwrap();
    let prior = {
        let mut engine = f.storage.write().unwrap();
        let policy = NativePolicyInputs::default();
        let intent = engine
            .meta_store()
            .capture_native_decay(d.parent, d.ns, &policy, 10_000 * 86_400_000_i64)
            .unwrap();
        let ctx = MutationContext {
            store_id: Some(engine.control_state().unwrap().store_id),
            scope: MutationScope::Namespace(d.ns),
            idempotency_key: Some("prior-phase".into()),
            expected_generation: None,
            contract_version: 2,
        };
        let crate::storage::native_maintenance::NativeDecayResult::Committed(receipt) = engine
            .apply_native_decay(&ctx, &intent, &policy, None)
            .unwrap()
        else {
            panic!()
        };
        receipt
    };
    let (before, ids, selected) = {
        let engine = f.storage.read().unwrap();
        (
            engine.control_state().unwrap(),
            engine.meta_store().id_source().checkpoint().unwrap(),
            engine.meta_store().text_selection().unwrap(),
        )
    };
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(5);
    let error = c.compact_text_with_work(None).await.unwrap_err();
    let StorageError::CommittedMutation { receipt, .. } = error else {
        panic!("{error}")
    };
    assert_eq!(*receipt, prior);
    {
        let engine = f.storage.read().unwrap();
        assert_eq!(engine.control_state().unwrap(), before);
        assert_eq!(engine.meta_store().id_source().checkpoint().unwrap(), ids);
        assert_eq!(engine.meta_store().text_selection().unwrap(), selected);
    }
    *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(4);
    c.complete().await.unwrap();
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    let result = c.compact_text_with_work(None).await.unwrap();
    assert_eq!(
        result.receipt.committed_generation,
        before.committed_generation.next().unwrap()
    );
    assert!(matches!(
        result.receipt.outcome,
        MutationOutcome::NativeTextGenerationSelected { .. }
    ));
}

#[tokio::test]
async fn compaction_caller_rejects_active_purge_without_resuming_or_allocating() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let engine =
        RedbStorageEngine::open_with_ids(&d.path, true, Box::new(SeededIds::new([22; 32])))
            .unwrap();
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let c = completion(&f);
    c.complete().await.unwrap();
    let (progress, ids) = {
        let mut engine = f.storage.write().unwrap();
        let ctx = MutationContext {
            store_id: Some(engine.control_state().unwrap().store_id),
            scope: MutationScope::Namespace(d.ns),
            idempotency_key: Some("unfinished-purge".into()),
            expected_generation: None,
            contract_version: 2,
        };
        let progress = engine.begin_namespace_purge(&ctx, true).unwrap();
        assert_ne!(progress.stage, crate::storage::purge::PurgeStage::Complete);
        (
            progress,
            engine.meta_store().id_source().checkpoint().unwrap(),
        )
    };
    let before = files(&d.path);
    let error = c.compact_text_with_work(None).await.unwrap_err();
    assert!(
        matches!(error, StorageError::RecoveryRequired(_)),
        "{error}"
    );
    let engine = f.storage.read().unwrap();
    assert_eq!(engine.meta_store().id_source().checkpoint().unwrap(), ids);
    let after = engine
        .lookup_namespace_purge(progress.store_id, d.ns, progress.start_receipt.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        control::encode(&after).unwrap(),
        control::encode(&progress).unwrap()
    );
    assert_eq!(files(&d.path), before);
}

async fn fork(root: &Path) -> PathBuf {
    let d = certified_fixture(root);
    let mut s = open(&d, Arc::new(FixedClock(AtomicUsize::new(0))));
    let id = mixed_history::linked(&mut s, &d, "compaction-linked");
    mixed_history::graph(&mut s, &d, id);
    s.engine.compact_text_log().unwrap();
    mixed_history::phases(&mut s, &d, id, 2);
    s.engine.compact_text_log().unwrap();
    drop(s);
    mixed_history::export_import_fork(root, &d).await
}

#[tokio::test]
async fn compaction_history_survives_phase_loss_export_import_forks_and_purge() {
    let root = tempfile::tempdir().unwrap();
    let path = fork(root.path()).await;
    let original = mixed_history::receipts(&path);
    let inspection = inspect_schema(&path).unwrap();
    assert_eq!(inspection.control.restore_lineage_version, Some(2));
    let before = root.path().join("before.jsonl");
    logical_export::export_store(&path, &before).unwrap();
    let mut s = NativeLifecycleSession::open(
        &path,
        NativePolicyInputs::default(),
        Arc::new(FixedClock(AtomicUsize::new(0))),
    )
    .unwrap();
    s.engine.compact_text_log().unwrap();
    drop(s);
    let after = root.path().join("after.jsonl");
    logical_export::export_store(&path, &after).unwrap();
    assert_eq!(
        mixed_history::payloads(&before),
        mixed_history::payloads(&after)
    );
    for (key, value) in original {
        assert_eq!(mixed_history::receipts(&path).get(&key), Some(&value));
    }
    let stream = root.path().join("root-preserved.jsonl");
    let exported = logical_export::export_store(&path, &stream).unwrap();
    let imported = root.path().join("second-import");
    assert_eq!(
        logical_export::import_store(&stream, &imported).unwrap(),
        exported
    );
    let archive = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("second.zip"),
        Some(&imported),
        false,
    )
    .await
    .unwrap();
    crate::backup::restore_from_backup(crate::backup::RestoreOptions {
        backup_path: archive,
        data_dir: imported.clone(),
        force: true,
        no_stop_daemon: true,
        historical: false,
    })
    .await
    .unwrap();
    let mut s = NativeLifecycleSession::open(
        &imported,
        NativePolicyInputs::default(),
        Arc::new(FixedClock(AtomicUsize::new(0))),
    )
    .unwrap();
    let ns = s.engine.list_namespaces().unwrap()[0].id;
    let d = Data {
        path: imported.clone(),
        ns,
        parent: MemoryId::nil(),
        profile: crate::embedding::runtime::tests::profile(),
    };
    mixed_history::purge(&mut s, &d);
    let empty = s.engine.compact_text_log().unwrap();
    assert_eq!(empty.entries_kept, 0);
    drop(s);
    inspect_schema(&imported).unwrap();
    logical_export::export_store(&imported, &root.path().join("purged.jsonl")).unwrap();
}

#[tokio::test]
async fn compaction_foreign_anchor_omissions_and_malformed_proofs_reject_immutably() {
    use crate::storage::lineage::LINEAGE;
    let root = tempfile::tempdir().unwrap();
    let path = fork(root.path()).await;
    let pristine = files(&path);
    for case in [
        "omit-one",
        "omit-all",
        "hash",
        "origin",
        "proof",
        "missing",
        "null",
        "version",
        "downgrade",
        "count",
    ] {
        let bad = root.path().join(format!("bad-{case}"));
        std::fs::create_dir(&bad).unwrap();
        for (name, bytes) in &pristine {
            let p = bad.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }
        let db = redb::Database::open(bad.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx.open_table(control::RECEIPTS).unwrap();
            let rows: Vec<_> = table
                .iter()
                .unwrap()
                .filter_map(|row| {
                    let (k, v) = row.unwrap();
                    let stored = control::validate_receipt(v.value()).unwrap();
                    matches!(
                        stored.receipt.outcome,
                        MutationOutcome::NativeTextGenerationSelected { .. }
                    )
                    .then(|| (k.value().to_vec(), v.value().to_vec()))
                })
                .collect();
            assert_eq!(rows.len(), 2);
            if case.starts_with("omit") {
                for (key, _) in rows.iter().take(if case == "omit-one" { 1 } else { 2 }) {
                    table.remove(key.as_slice()).unwrap();
                }
            } else if matches!(case, "hash" | "origin" | "proof") {
                let (key, bytes) = &rows[0];
                let mut value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                match case {
                    "hash" => value["request_hash"][0] = serde_json::json!(255),
                    "origin" => {
                        value["receipt"]["outcome"]["transition"]["source"]["generation"] =
                            serde_json::json!("0")
                    }
                    _ => {
                        value["receipt"]["outcome"]["transition"]["recordsSha256"][0] =
                            serde_json::json!(255)
                    }
                }
                table
                    .insert(
                        key.as_slice(),
                        serde_json::to_vec(&value).unwrap().as_slice(),
                    )
                    .unwrap();
            }
        }
        if matches!(case, "missing" | "null" | "version" | "downgrade" | "count") {
            let mut table = tx.open_table(LINEAGE).unwrap();
            let mut row: serde_json::Value =
                serde_json::from_slice(table.get("state").unwrap().unwrap().value()).unwrap();
            match case {
                "missing" => {
                    row.as_object_mut().unwrap().remove("nativeTextAnchor");
                }
                "null" => row["nativeTextAnchor"] = serde_json::Value::Null,
                "version" => row["nativeTextAnchor"]["version"] = serde_json::json!(2),
                "count" => row["nativeTextAnchor"]["count"] = serde_json::json!(0),
                _ => {
                    row["version"] = serde_json::json!(1);
                    row.as_object_mut().unwrap().remove("nativeTextAnchor");
                    let mut state = control::read_control(&tx).unwrap();
                    state.restore_lineage_version = Some(1);
                    tx.open_table(control::CONTROL)
                        .unwrap()
                        .insert("state", control::encode(&state).unwrap().as_slice())
                        .unwrap();
                }
            }
            table
                .insert("state", serde_json::to_vec(&row).unwrap().as_slice())
                .unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        let before = files(&bad);
        assert!(inspect_schema(&bad).is_err(), "{case}");
        assert!(validation::canonical_files(&bad).is_err(), "{case}");
        assert!(
            logical_export::export_store(&bad, &root.path().join(format!("{case}.jsonl"))).is_err(),
            "{case}"
        );
        assert!(
            RedbStorageEngine::open_with_ids(&bad, true, Box::new(crate::storage::ids::SystemIds))
                .is_err(),
            "{case}"
        );
        assert_eq!(files(&bad), before, "source mutated: {case}");
    }
}
