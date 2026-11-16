//! Actual pre-certification binary produces pending multi-namespace history.
//! Current code only resumes original identities after explicit remedies.
use super::*;
use crate::storage::{
    StorageEngine as _,
    native_lifecycle::{NativeLifecycleSession, NativePolicyInputs},
};

fn genuine_pending(
    root: &Path,
) -> (
    PathBuf,
    ReembedVectors,
    Provider,
    crate::storage::batch::BatchProgress,
) {
    let (_, _, provider) = fixture(root);
    let extracted = root.join("pending-before");
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "fixtures/pending-before-certification.zip"
    )))
    .unwrap()
    .extract(&extracted)
    .unwrap();
    let request =
        serde_json::from_slice(&std::fs::read(extracted.join("remedy.json")).unwrap()).unwrap();
    let pending =
        serde_json::from_slice(&std::fs::read(extracted.join("pending.json")).unwrap()).unwrap();
    (extracted.join("store"), request, provider, pending)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vector_certification_pending_provider_race_rechecks_entire_policy_in_final_tx() {
    struct Latch {
        profile: crate::model::schema::EmbeddingProfile,
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        calls: AtomicUsize,
    }
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for Latch {
        fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
            Some(self.profile.clone())
        }
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> &str {
            &self.profile.model_id
        }
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.entered.notify_one();
            self.release.notified().await;
            Ok(vec![0., 1., 0., 0.])
        }
    }
    let root = tempfile::tempdir().unwrap();
    let (source, namespaces, provider, request, pending) = captured_case(root.path(), 2, 2, true);
    let snapshot = OfflineSnapshot::open_vector_remedy(
        crate::storage::ownership::StoreOwnership::acquire(&source).unwrap(),
    )
    .unwrap();
    let at = snapshot.control().generation_ref();
    drop(snapshot);
    let target = root.path().join("remedied");
    reembed_fresh(
        &source,
        &target,
        &ReembedVectors {
            source: at,
            namespace_id: namespaces[0],
            namespace_name: "native".into(),
            profile_id: provider.profile.profile_id().unwrap(),
            context: MutationContext {
                store_id: Some(at.store_id),
                expected_generation: Some(at.generation),
                scope: MutationScope::Namespace(namespaces[0]),
                contract_version: 1,
                idempotency_key: Some("race-cutover".into()),
            },
        },
        &provider,
    )
    .await
    .unwrap();
    let engine = RedbStorageEngine::open_native(&target).unwrap();
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let clock = Arc::new(CountClock(AtomicUsize::new(0)));
    let provider = Arc::new(Latch {
        profile: provider.profile,
        entered: Default::default(),
        release: Default::default(),
        calls: AtomicUsize::new(0),
    });
    let a = adapter(&f, provider.clone(), clock.clone()).await;
    let task = tokio::spawn(async move { a.run_batch_typed(request).await });
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        provider.entered.notified(),
    )
    .await
    .unwrap();
    // The lookup accepted the exact certification-only bridge before provider
    // work. A real concurrent policy writer makes that witness inapplicable.
    let receipt = {
        let _graph = f.graph.write().await;
        let mut engine = f.storage.write().unwrap();
        let ns = engine.get_namespace(namespaces[0]).unwrap().unwrap();
        let mut replacement = crate::storage::catalog::NamespacePolicy::from(&ns);
        replacement.desired_retention = 0.8;
        let policy = NativePolicyInputs::default();
        let intent = engine
            .meta_store()
            .capture_native_policy(
                crate::storage::native_lifecycle::LifecycleScope {
                    namespace_id: namespaces[0],
                    namespace_name: "native".into(),
                    source: None,
                },
                &replacement,
                &policy,
                None,
            )
            .unwrap();
        engine
            .apply_native_policy(
                &MutationContext {
                    store_id: Some(at.store_id),
                    scope: MutationScope::Namespace(namespaces[0]),
                    contract_version: 2,
                    expected_generation: None,
                    idempotency_key: Some("race-policy".into()),
                },
                &intent,
                &policy,
                None,
            )
            .unwrap()
            .unwrap()
    };
    let encode_rows = |rows: Vec<(crate::model::MemoryId, crate::model::DiskRecord)>| {
        rows.into_iter()
            .map(|(id, row)| (id, row.encode().unwrap()))
            .collect::<Vec<_>>()
    };
    let rows = encode_rows(f.storage.read().unwrap().scan_all().unwrap());
    let sampled = clock.0.load(Ordering::SeqCst);
    provider.release.notify_one();
    let outcome = task.await.unwrap().unwrap();
    assert_eq!(outcome.items[0].status, BatchStatus::Error);
    assert_eq!(
        outcome.items[0].error.as_ref().unwrap().code,
        "INVALID_INPUT"
    );
    assert!(outcome.items[0].receipt.is_none());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(clock.0.load(Ordering::SeqCst), sampled);
    let engine = f.storage.read().unwrap();
    assert_eq!(encode_rows(engine.scan_all().unwrap()), rows);
    let retained = engine
        .meta_store()
        .lookup_batch(at.store_id, pending.reservation_receipt.operation_id)
        .unwrap()
        .unwrap();
    let mut original_parent = pending.reservation_receipt.clone();
    original_parent.replayed = false; // Captured lookup response; canonical bytes never store replay=true.
    assert_eq!(retained.reservation_receipt, original_parent);
    for (old, new) in pending.items.iter().zip(&retained.items) {
        assert_eq!(old.operation_id, new.operation_id);
        assert_eq!(old.canonical_hash, new.canonical_hash);
        assert_eq!(old.defaults, new.defaults);
    }
    assert_eq!(retained.bookkeeping_receipts.len(), 1);
    assert_eq!(
        retained.bookkeeping_receipts[0].committed_generation,
        receipt.committed_generation.next().unwrap()
    );
    assert!(matches!(
        retained.items[1].state,
        BatchItemState::NotAttempted {}
    ));
}

#[tokio::test]
async fn vector_certification_pending_compact_witness_rejects_cutoff_and_full_config_changes() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider, pending) = genuine_pending(root.path());
    let target = root.path().join("certified");
    reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap();
    let snapshot = OfflineSnapshot::open_vector_remedy(
        crate::storage::ownership::StoreOwnership::acquire(&target).unwrap(),
    )
    .unwrap();
    let (_, bindings) =
        validate_current_with_pending(&snapshot.metadata.begin_read().unwrap(), snapshot.control())
            .unwrap();
    let ns = snapshot
        .namespaces()
        .iter()
        .find(|n| n.id == request.namespace_id)
        .unwrap();
    let old = pending.items[0]
        .defaults
        .as_ref()
        .unwrap()
        .namespace_policy_hash;
    let hash = crate::storage::metadata::batch::policy_hash;
    let parent = GenerationRef {
        store_id: pending.reservation_receipt.store_id,
        generation: pending.reservation_receipt.committed_generation,
    };
    assert!(bindings.permits(ns, old, hash(ns).unwrap(), parent));
    assert!(!bindings.permits(
        ns,
        old,
        hash(ns).unwrap(),
        GenerationRef {
            generation: request.source.generation.next().unwrap(),
            ..parent
        }
    ));
    assert!(!bindings.permits(
        ns,
        old,
        hash(ns).unwrap(),
        GenerationRef {
            store_id: uuid::Uuid::from_u128(42),
            ..parent
        }
    ));
    for variant in 0..5 {
        let mut changed = ns.clone();
        match variant {
            0 => changed.name.push_str("-reused"),
            1 => changed.embedding_dim += 1,
            2 => changed.desired_retention = 0.8,
            3 => changed.id = NamespaceId::new(changed.id.get() + 1),
            4 => changed.schema.as_mut().unwrap().vector_certification = None,
            _ => unreachable!(),
        }
        assert!(
            !bindings.permits(&changed, old, hash(&changed).unwrap(), parent),
            "variant{variant}"
        );
    }
    assert!(!bindings.permits(ns, [0; 32], hash(ns).unwrap(), parent));
}

#[tokio::test]
async fn vector_certification_pending_compact_heap_budget_rejects_before_collection() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider, _) = genuine_pending(root.path());
    let receipt = reembed_fresh(&source, &root.path().join("certified"), &request, &provider)
        .await
        .unwrap();
    let control::MutationOutcome::VectorsReembedded { mut transition } = receipt.outcome else {
        panic!()
    };
    let mut cutovers = BTreeMap::new();
    cutovers.insert(request.namespace_id.get(), transition.clone());
    PendingBindings::from_validated(&cutovers).unwrap();
    // Exercise the collector's incremental heap cap independently of history
    // authenticity. These synthetic size variants are never persisted or
    // admitted as canonical evidence; full history validation still comes first.
    let EmbeddingBinding::Bound {
        descriptor,
        profile_id,
        ..
    } = &mut transition.before_namespace.schema.embedding_binding
    else {
        panic!()
    };
    descriptor.document_preprocessing.prefix =
        "x".repeat(MAX_PROOF_BYTES as usize / 16 - 64 * 1024);
    *profile_id = descriptor.profile_id().unwrap();
    cutovers.insert(request.namespace_id.get(), transition.clone());
    PendingBindings::from_validated(&cutovers).unwrap();
    let EmbeddingBinding::Bound {
        descriptor,
        profile_id,
        ..
    } = &mut transition.before_namespace.schema.embedding_binding
    else {
        panic!()
    };
    descriptor.document_preprocessing.prefix = "x".repeat(MAX_PROOF_BYTES as usize / 16);
    *profile_id = descriptor.profile_id().unwrap();
    cutovers.insert(request.namespace_id.get(), transition);
    let error = PendingBindings::from_validated(&cutovers)
        .err()
        .expect("incremental witness cap must reject");
    assert!(
        error.to_string().contains("16 MiB heap budget"),
        "{error:?}"
    );
}

#[tokio::test]
async fn vector_certification_pending_cutover_process_child() {
    let Ok(source) = std::env::var("RECALLD_PENDING_CERT_SOURCE") else {
        return;
    };
    let target = PathBuf::from(std::env::var("RECALLD_PENDING_CERT_TARGET").unwrap());
    let request: ReembedVectors = serde_json::from_slice(
        &std::fs::read(std::env::var("RECALLD_PENDING_CERT_REQUEST").unwrap()).unwrap(),
    )
    .unwrap();
    let snapshot = OfflineSnapshot::open_vector_remedy(
        crate::storage::ownership::StoreOwnership::acquire(Path::new(&source)).unwrap(),
    )
    .unwrap();
    let namespace = snapshot
        .namespaces()
        .iter()
        .find(|n| n.id == request.namespace_id)
        .unwrap();
    let EmbeddingBinding::Bound { descriptor, .. } =
        &namespace.schema.as_ref().unwrap().embedding_binding
    else {
        panic!()
    };
    let provider = Provider {
        profile: descriptor.clone(),
        texts: Default::default(),
        fail: false,
    };
    drop(snapshot);
    reembed_fresh(Path::new(&source), &target, &request, &provider)
        .await
        .unwrap();
    panic!("cutover crash boundary not reached");
}

#[tokio::test]
async fn vector_certification_pending_cutover_process_exits_and_exact_receipt_retry() {
    for point in [
        "certification_before_commit",
        "certification_after_commit",
        "certification_published",
    ] {
        let root = tempfile::tempdir().unwrap();
        let (source, request, provider, pending) = genuine_pending(root.path());
        let bytes = raw_hashes(&source);
        let target = root.path().join("certified");
        let request_path = root.path().join("request.json");
        std::fs::write(&request_path, serde_json::to_vec(&request).unwrap()).unwrap();
        let status=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact",
            "storage::vector_certification::tests::admission::pending::vector_certification_pending_cutover_process_child","--nocapture"])
            .env("RECALLD_PENDING_CERT_SOURCE",&source).env("RECALLD_PENDING_CERT_TARGET",&target)
            .env("RECALLD_PENDING_CERT_REQUEST",&request_path).env("RECALLD_CATALOG_CRASH",point).status().unwrap();
        assert_eq!(status.code(), Some(89));
        assert_eq!(bytes, raw_hashes(&source));
        let stage = root.path().join("certified.recalld-vector-certification");
        if point == "certification_before_commit" {
            let before_stage = raw_hashes(&stage);
            let error = reembed_fresh(&source, &target, &request, &provider)
                .await
                .unwrap_err();
            eprintln!("actual precommit private stage retry rejection: {error:?}");
            assert!(!target.exists());
            let mut after_stage = raw_hashes(&stage);
            // First inspection creates only the legacy-compatible empty owner
            // lock. It must not repair or alter any staged canonical file.
            assert_eq!(
                after_stage.remove(Path::new("recalld.lock")),
                Some(hash_bytes(&[]))
            );
            assert_eq!(before_stage, after_stage);
            assert!(provider.texts.lock().unwrap().is_empty());
            continue; // No guessed receipt, destructive cleanup or automatic repair.
        }
        let receipt = reembed_fresh(&source, &target, &request, &provider)
            .await
            .unwrap();
        assert!(receipt.replayed);
        assert!(provider.texts.lock().unwrap().is_empty());
        let engine = RedbStorageEngine::open_native(&target).unwrap();
        assert_eq!(
            engine
                .meta_store()
                .lookup_batch(
                    request.source.store_id,
                    pending.reservation_receipt.operation_id
                )
                .unwrap()
                .unwrap(),
            pending
        );
        assert_eq!(
            engine.control_state().unwrap().committed_generation,
            receipt.committed_generation
        );
    }
}

#[tokio::test]
async fn vector_certification_pending_cutover_unknown_ack_preserves_original_operation() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider, pending) = genuine_pending(root.path());
    let before = raw_hashes(&source);
    let target = root.path().join("unknown");
    crate::storage::catalog::FAILURE.with(|f| f.set(Some("certification_commit_unknown")));
    let error = reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap_err();
    crate::storage::catalog::FAILURE.with(|f| f.set(None));
    let StorageError::CommitOutcomeUnknown { operation_id, .. } = error else {
        panic!("{error:?}")
    };
    let calls = provider.texts.lock().unwrap().len();
    let receipt = reembed_fresh(&source, &target, &request, &provider)
        .await
        .unwrap();
    assert_eq!(receipt.operation_id, operation_id);
    assert!(receipt.replayed);
    assert_eq!(provider.texts.lock().unwrap().len(), calls);
    assert_eq!(before, raw_hashes(&source));
    let engine = RedbStorageEngine::open_native(&target).unwrap();
    assert_eq!(
        engine
            .meta_store()
            .lookup_batch(
                request.source.store_id,
                pending.reservation_receipt.operation_id
            )
            .unwrap()
            .unwrap(),
        pending
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vector_certification_original_pending_child_unknown_reopens_once_and_forks_history() {
    struct UnknownClock;
    impl crate::time::Clock for UnknownClock {
        fn now_millis(&self) -> i64 {
            crate::storage::metadata::effects::FAIL_AFTER_RECORD_COMMIT.with(|flag| flag.set(true));
            100_000_000
        }
    }
    let root = tempfile::tempdir().unwrap();
    let (source, remedy, provider, pending) = genuine_pending(root.path());
    let target = root.path().join("certified");
    reembed_fresh(&source, &target, &remedy, &provider)
        .await
        .unwrap();
    let engine = RedbStorageEngine::open_native(&target).unwrap();
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let provider = Arc::new(provider);
    let clock = Arc::new(CountClock(AtomicUsize::new(0)));
    let a = adapter(&f, provider.clone(), clock.clone())
        .await
        .with_clock(Arc::new(UnknownClock));
    let request = input(&remedy, &provider);
    let calls = provider.texts.lock().unwrap().len();
    let outcome = a.run_batch_typed(request.clone()).await.unwrap();
    assert_eq!(outcome.items[0].status, BatchStatus::Unknown);
    assert_eq!(
        outcome.items[0].operation_id,
        Some(pending.items[0].operation_id)
    );
    assert_eq!(
        outcome.items[0].error.as_ref().unwrap().code,
        "COMMIT_OUTCOME_UNKNOWN"
    );
    assert!(outcome.items[0].receipt.is_none());
    assert!(
        f.storage
            .read()
            .unwrap()
            .canonical_vector_snapshot(&remedy.namespace_name, None, limits())
            .is_err()
    );
    drop(a);
    let root = f.close();
    let engine = RedbStorageEngine::open_native(&target).unwrap();
    let committed = engine.control_state().unwrap();
    let f = crate::test_support::Fixture::from_open_engine(root, engine);
    let a = adapter(&f, provider.clone(), clock.clone()).await;
    let replay = a.run_batch_typed(request).await.unwrap();
    assert!(replay.all_ready());
    assert_eq!(
        replay.items[0].receipt.as_ref().unwrap().operation_id,
        pending.items[0].operation_id
    );
    assert!(replay.items[0].receipt.as_ref().unwrap().replayed);
    assert_eq!(
        f.storage.read().unwrap().control_state().unwrap(),
        committed
    );
    assert_eq!(provider.texts.lock().unwrap().len(), calls);
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    drop(a);
    let root = f.close();
    let before = OfflineSnapshot::open(&target).unwrap();
    let original_receipts = receipts(&before);
    let certificate = before
        .namespaces()
        .iter()
        .find(|n| n.id == remedy.namespace_id)
        .unwrap()
        .schema
        .as_ref()
        .unwrap()
        .vector_certification
        .clone();
    drop(before);
    let backup = crate::backup::run_backup(
        &crate::config::RecalldConfig::default(),
        &root.path().join("completed.zip"),
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
    let fork = OfflineSnapshot::open(&target).unwrap();
    assert_ne!(fork.control().store_id, committed.store_id);
    for (key, value) in original_receipts {
        assert_eq!(receipts(&fork).get(&key), Some(&value));
    }
    assert_eq!(
        fork.namespaces()
            .iter()
            .find(|n| n.id == remedy.namespace_id)
            .unwrap()
            .schema
            .as_ref()
            .unwrap()
            .vector_certification,
        certificate
    );
    drop(fork);
    let export = root.path().join("fork.jsonl");
    logical_export::export_store(&target, &export).unwrap();
    let imported = root.path().join("fork-imported");
    logical_export::import_store(&export, &imported).unwrap();
    RedbStorageEngine::open_native(&imported).unwrap();
}

fn multi_source(root: &Path) -> (PathBuf, Vec<NamespaceId>, Provider) {
    use crate::storage::schema_migration::{
        MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema,
    };
    let legacy = root.join("legacy");
    let mut engine = RedbStorageEngine::open(&legacy).unwrap();
    let profile = crate::embedding::runtime::tests::profile();
    let mut namespaces = Vec::new();
    let mut attestations = Vec::new();
    for name in ["native", "second"] {
        let mut namespace = NamespaceConfig::default_namespace(0);
        namespace.name = name.into();
        namespace.embedding_dim = 4;
        let id = engine.create_namespace(&namespace).unwrap();
        let row = InsertMemory {
            id: None,
            summary: format!("retained {name}"),
            full_text: Some(format!("body {name}")),
            tags: vec![],
            initial_stability: None,
            created_at: Some(1000),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        engine
            .insert_memory_with_receipt(&MutationContext::unkeyed(id), &row, &[], 1000)
            .unwrap();
        namespaces.push(id);
        attestations.push(ProfileAttestationInput {
            namespace_id: id,
            namespace_name: name.into(),
            descriptor: profile.clone(),
        });
    }
    engine.sync().unwrap();
    drop(engine);
    let mut current = root.join("schema");
    migrate_schema(
        &legacy,
        &current,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&legacy).unwrap().control.generation_ref(),
            attestations,
        },
    )
    .unwrap();
    for id in &namespaces {
        let target = root.join(format!("lifecycle-{}", id.get()));
        crate::storage::lifecycle_migration::migrate_lifecycle(
            &current,
            &target,
            &crate::storage::lifecycle_migration::LifecycleMigrationOptions {
                expected_source: inspect_schema(&current).unwrap().control.generation_ref(),
                namespace_id: *id,
                expected_policy_revision: Generation::default(),
                target: crate::model::schema::LifecycleVersion::ExplicitFeedbackV2,
                idempotency_key: format!("lifecycle-{}", id.get()),
            },
        )
        .unwrap();
        current = target;
    }
    for id in &namespaces {
        crate::test_support::explicitly_adopt_inherited_policy(&current, *id, 1., 3000);
    }
    (
        current,
        namespaces,
        Provider {
            profile,
            texts: Default::default(),
            fail: false,
        },
    )
}

fn before_case(
    root: &Path,
    version: u32,
    original_contract: u32,
    partial: bool,
    generated: bool,
) -> (
    PathBuf,
    Vec<NamespaceId>,
    Provider,
    BatchInput,
    crate::storage::batch::BatchProgress,
) {
    let before_binary = std::env::var("RECALLD_CERTIFICATION_BEFORE_BINARY")
        .expect("capture requires genuine BEFORE binary");
    let (source, namespaces, provider) = multi_source(root);
    let state = OfflineSnapshot::open(&source).unwrap().control().clone();
    let mut request = BatchInput {
        request_context: crate::storage::request_context::RetainedRequestContext::from_contract(
            original_contract,
        )
        .unwrap(),
        expected_generation: None,
        mutation_version: version,
        result_version: 1,
        kind: BatchKind::Insert,
        default_namespace: "native".into(),
        store_id: Some(state.store_id),
        idempotency_key: Some("pending-aba".into()),
        batch_operation_id: None,
        items: vec![],
    };
    for (index, namespace) in ["native", "second", "native"].into_iter().enumerate() {
        let mut value = serde_json::json!({"summary":format!("pending child {index}"),"namespace":namespace,"checkDuplicates":false});
        if !generated {
            value["embedding"] = serde_json::json!([2., 0., 0., 0.]);
            value["embeddingProfile"] = serde_json::to_value(&provider.profile).unwrap();
        }
        request.items.push(value);
    }
    let request_file = root.join("before-request.json");
    std::fs::write(&request_file, serde_json::to_vec(&request).unwrap()).unwrap();
    let context_before = std::env::var("RECALLD_CERTIFICATION_CONTEXT_BEFORE_BINARY").ok();
    let selected_binary = if original_contract == 1 {
        context_before
            .as_ref()
            .expect("API1 pending proof requires genuine159+158 before binary")
    } else {
        &before_binary
    };
    let mut command = std::process::Command::new(selected_binary);
    command.args([
        "--exact",
        if version == 2 {
            "storage::metadata::native_records::tests::batches::native_batch_crash_child"
        } else {
            "storage::metadata::native_records::tests::store_batches::linked_batch_process_child"
        },
        "--nocapture",
    ]);
    if version == 2 {
        command
            .env("RECALLD_NATIVE_BATCH_CHILD", &source)
            .env("RECALLD_NATIVE_BATCH_REQUEST", &request_file);
    } else {
        command
            .env("RECALLD_LINKED_BATCH_PATH", &source)
            .env("RECALLD_LINKED_BATCH_REQUEST", &request_file);
    }
    let result = command
        .env(
            "RECALLD_TEST_EFFECT_CRASH",
            if partial {
                "after_metadata_commit"
            } else {
                "after_batch_commit"
            },
        )
        .status()
        .unwrap();
    assert_eq!(
        result.code(),
        Some(91),
        "genuine before hook version{version}/contract{original_contract}"
    );
    // Actual owned startup repairs only its real interrupted write and applies
    // exact pending effects; it must not execute later missing child payloads.
    let engine = RedbStorageEngine::open_native(&source).unwrap();
    let plans: Vec<_> = if version == 2 {
        crate::mcp::batches::prepare(&request).unwrap()
    } else {
        crate::mcp::batches::prepare_native_store(&request).unwrap()
    }
    .into_iter()
    .map(|p| p.plan)
    .collect();
    let pending = engine
        .meta_store()
        .reserve_batch_version_with_work(
            &request.context(),
            version,
            request.kind,
            &plans,
            None,
            None,
            || panic!("original parent replay consulted settings"),
        )
        .unwrap();
    assert_eq!(pending.unresolved(), if partial { 2 } else { 3 });
    drop(engine);
    let current = source;
    if let Ok(output) = std::env::var("RECALLD_CERTIFICATION_MULTI_BEFORE_CAPTURE") {
        use std::io::Write;
        let output = PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        let file = std::fs::File::create(output.join(format!(
            "pending-v{version}-api{original_contract}-generated{generated}.zip"
        )))
        .unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for relative in raw_hashes(&current).keys() {
            zip.start_file(format!("store/{}", relative.to_str().unwrap()), options)
                .unwrap();
            std::io::copy(
                &mut std::fs::File::open(current.join(relative)).unwrap(),
                &mut zip,
            )
            .unwrap();
        }
        for (name, value) in [
            ("request.json", serde_json::to_vec(&request).unwrap()),
            ("pending.json", serde_json::to_vec(&pending).unwrap()),
            ("namespaces.json", serde_json::to_vec(&namespaces).unwrap()),
            (
                "profile.json",
                serde_json::to_vec(&provider.profile).unwrap(),
            ),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(&value).unwrap();
        }
        zip.finish().unwrap();
    }
    (current, namespaces, provider, request, pending)
}

fn captured_case(
    root: &Path,
    version: u32,
    original_contract: u32,
    generated: bool,
) -> (
    PathBuf,
    Vec<NamespaceId>,
    Provider,
    BatchInput,
    crate::storage::batch::BatchProgress,
) {
    let bytes: &[u8] = match (version, original_contract, generated) {
        (2, 2, false) => include_bytes!("fixtures/pending-v2-api2-generatedfalse.zip"),
        (2, 2, true) => include_bytes!("fixtures/pending-v2-api2-generatedtrue.zip"),
        (3, 1, false) => include_bytes!("fixtures/pending-v3-api1-generatedfalse.zip"),
        (3, 2, true) => include_bytes!("fixtures/pending-v3-api2-generatedtrue.zip"),
        _ => panic!("unsupported genuine BEFORE case"),
    };
    let extracted = root.join("captured");
    zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .unwrap()
        .extract(&extracted)
        .unwrap();
    let read = |name| std::fs::read(extracted.join(name)).unwrap();
    let provider = Provider {
        profile: serde_json::from_slice(&read("profile.json")).unwrap(),
        texts: Default::default(),
        fail: false,
    };
    (
        extracted.join("store"),
        serde_json::from_slice(&read("namespaces.json")).unwrap(),
        provider,
        serde_json::from_slice(&read("request.json")).unwrap(),
        serde_json::from_slice(&read("pending.json")).unwrap(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vector_certification_old_binary_pending_aba_partial_and_generated_resume() {
    for (version, original_contract, partial, generated) in [
        (2, 2, true, false),
        (2, 2, false, true),
        (3, 1, true, false),
        (3, 2, false, true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (mut current, namespaces, provider, request, pending) =
            if std::env::var_os("RECALLD_CERTIFICATION_MULTI_BEFORE_CAPTURE").is_some() {
                before_case(root.path(), version, original_contract, partial, generated)
            } else {
                captured_case(root.path(), version, original_contract, generated)
            };
        let store_id = pending.reservation_receipt.store_id;
        let mut prefix = pending.items[0].clone();
        let mut has_prefix = partial;
        let mut remaining_generated = if generated { 3 } else { 0 };
        let mut cutovers = Vec::new();
        for (index, id) in namespaces.iter().enumerate() {
            let engine = RedbStorageEngine::open_native(&current).unwrap();
            let source_ref = engine.control_state().unwrap().generation_ref();
            let namespace = engine.get_namespace(*id).unwrap().unwrap();
            drop(engine);
            let remedy = ReembedVectors {
                source: source_ref,
                namespace_id: *id,
                namespace_name: namespace.name,
                profile_id: provider.profile.profile_id().unwrap(),
                context: MutationContext {
                    store_id: Some(source_ref.store_id),
                    expected_generation: Some(source_ref.generation),
                    scope: MutationScope::Namespace(*id),
                    contract_version: 1,
                    idempotency_key: Some(format!("remedy-{index}")),
                },
            };
            let bytes = raw_hashes(&current);
            let next = root.path().join(format!("remedied-{index}"));
            let receipt = reembed_fresh(&current, &next, &remedy, &provider)
                .await
                .unwrap();
            assert_eq!(bytes, raw_hashes(&current));
            assert!(
                OfflineSnapshot::open(&next).is_err(),
                "ordinary offline must retain pending rejection"
            );
            cutovers.push(receipt);
            current = next;
            if index == 0 && generated {
                // A is now certified, B is deliberately still old/uncertified.
                // Real resubmission may commit A, but may not skip or finalize B.
                let engine = RedbStorageEngine::open_native(&current).unwrap();
                let f = crate::test_support::Fixture::from_open_engine(
                    tempfile::tempdir().unwrap(),
                    engine,
                );
                let intermediate_provider = Arc::new(Provider {
                    profile: provider.profile.clone(),
                    texts: Default::default(),
                    fail: false,
                });
                let a = adapter(
                    &f,
                    intermediate_provider.clone(),
                    Arc::new(CountClock(AtomicUsize::new(0))),
                )
                .await;
                let intermediate = a.run_batch_typed(request.clone()).await.unwrap();
                assert_eq!(intermediate.succeeded, 1);
                assert_eq!(intermediate.unresolved, 2);
                assert_eq!(intermediate.items[1].status, BatchStatus::NotAttempted);
                assert_eq!(
                    intermediate.items[1].error.as_ref().unwrap().code,
                    "UNAVAILABLE"
                );
                assert!(
                    !intermediate.items[1].retryable && intermediate.items[1].receipt.is_none()
                );
                assert_eq!(intermediate_provider.texts.lock().unwrap().len(), 1);
                prefix = f
                    .storage
                    .read()
                    .unwrap()
                    .meta_store()
                    .lookup_batch(store_id, pending.reservation_receipt.operation_id)
                    .unwrap()
                    .unwrap()
                    .items[0]
                    .clone();
                has_prefix = true;
                remaining_generated = 2;
                drop(a);
                drop(f.close());
            }
        }
        let clock = Arc::new(CountClock(AtomicUsize::new(0)));
        let mut session =
            NativeLifecycleSession::open(&current, NativePolicyInputs::default(), clock.clone())
                .unwrap();
        let namespace = session
            .engine
            .get_namespace(namespaces[0])
            .unwrap()
            .unwrap();
        for index in 0..12 {
            session
                .update_policy(
                    &MutationContext {
                        store_id: Some(store_id),
                        scope: MutationScope::Namespace(namespaces[0]),
                        contract_version: 2,
                        expected_generation: None,
                        idempotency_key: Some(format!("unrelated-noop-{index}")),
                    },
                    crate::storage::native_lifecycle::LifecycleScope {
                        namespace_id: namespaces[0],
                        namespace_name: "native".into(),
                        source: None,
                    },
                    &crate::storage::catalog::NamespacePolicy::from(&namespace),
                )
                .unwrap()
                .unwrap();
        }
        let f = crate::test_support::Fixture::from_open_engine(root, session.engine);
        let provider = Arc::new(provider);
        let a = adapter(&f, provider.clone(), clock.clone()).await;
        let calls = provider.texts.lock().unwrap().len();
        control::take_validation_work();
        let outcome = a.run_batch_typed(request.clone()).await.unwrap();
        assert!(outcome.all_ready(), "{outcome:?}");
        let work = control::take_validation_work();
        assert_eq!(work.certification_scans, 0, "{work:?}");
        assert_eq!(work.full_validations, 0, "{work:?}");
        for (old, new) in pending.items.iter().zip(&outcome.items) {
            assert_eq!(Some(old.operation_id), new.operation_id);
        }
        let completed = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .lookup_batch(store_id, pending.reservation_receipt.operation_id)
            .unwrap()
            .unwrap();
        if has_prefix {
            assert_eq!(completed.items[0], prefix);
        }
        assert_eq!(
            provider.texts.lock().unwrap().len() - calls,
            remaining_generated
        );
        let at = f.storage.read().unwrap().control_state().unwrap();
        let sampled = clock.0.load(Ordering::SeqCst);
        let calls = provider.texts.lock().unwrap().len();
        assert!(a.run_batch_typed(request).await.unwrap().all_ready());
        assert_eq!(at, f.storage.read().unwrap().control_state().unwrap());
        assert_eq!(sampled, clock.0.load(Ordering::SeqCst));
        assert_eq!(calls, provider.texts.lock().unwrap().len());
        drop(a);
        let root = f.close();
        OfflineSnapshot::open(&current).unwrap();
        let export = root.path().join("completed.jsonl");
        logical_export::export_store(&current, &export).unwrap();
        let imported = root.path().join("imported");
        logical_export::import_store(&export, &imported).unwrap();
        RedbStorageEngine::open_native(&imported).unwrap();
    }
}
