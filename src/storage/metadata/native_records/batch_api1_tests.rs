//! Public-route regressions and separately labeled private-history evidence.
use super::*;

#[path = "batch_postcommit_tests.rs"]
mod postcommit;

async fn batch_http(api: &axum::Router, body: Value) -> (u16, Value) {
    use tower::ServiceExt;
    let response = api
        .clone()
        .oneshot(
            axum::http::Request::post("/memories/batch")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(
        response.into_body(),
        crate::daemon::protocol::MAX_MESSAGE_SIZE,
    )
    .await
    .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn api1_batch_public_mcp_native_unicode() {
    let (_d, f, a, provider, _clock) = setup().await;
    let result = crate::mcp::tools::dispatch_tool(&bridge(a), "store_memories",
        json!({"memories":[{"summary":"batch Unicode", "entities":["John Smith","José","Alice's"],"checkDuplicates":false}]})).await;
    crate::test_support::record_schema_result(
        "batch-native-unicode",
        "store_memories",
        &result,
        "actual API1 native insertion batch through public MCP dispatch; canonical Unicode and provider assertions unchanged",
    );
    let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
    let value: Value = serde_json::from_str(text).unwrap_or(json!({"error":text}));
    assert_eq!(value["stored"], 1, "{result:?}");
    assert_ne!(result.is_error, Some(true));
    let id: MemoryId = serde_json::from_value(value["results"][0]["id"].clone()).unwrap();
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .get_record(id)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .labels
            .entities,
        ["John Smith", "José", "Alice's"]
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn api1_batch_public_rest_native_unicode_timestamp() {
    let (d, f, a, provider, _clock) = setup().await;
    let api = router(&f, a, provider.clone());
    let (status,value)=batch_http(&api,json!({"memories":[{"namespace":"native","summary":"batch HTTP", "entities":["José"],"embedding":[2.,-0.,0.,0.],"embeddingProfileId":d.profile.profile_id().unwrap(),"createdAt":500,"initialStability":12.,"checkDuplicates":false}]})).await;
    assert_eq!(status, 201, "{value}");
    assert_eq!(value["data"]["total"], 1, "{value}");
    let id: MemoryId = serde_json::from_value(value["data"]["created"][0]["id"].clone()).unwrap();
    let record = f.storage.read().unwrap().get_record(id).unwrap().unwrap();
    assert_eq!(record.created_at, 500);
    assert_eq!(record.stability, 12.);
    assert_eq!(record.schema.unwrap().labels.entities, ["José"]);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

// This exporter runs ONLY on the recorded158 production source. It is private
// original1 history, not evidence that the prior public API1 route accepted it.
#[tokio::test]
#[ignore = "historical fixture generator: run only on recorded158 source"]
async fn api1_batch_private_before_timestamp_rejection_export() {
    let output = PathBuf::from(std::env::var("RECALLD_BATCH_API1_BEFORE").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let root = tempfile::tempdir_in(&output).unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, session.engine);
    readers::completion(&f).complete().await.unwrap();
    let provider = Arc::new(RecoveringProvider {
        profile: d.profile.clone(),
        unavailable: false.into(),
        calls: AtomicUsize::new(0),
        profiles: AtomicUsize::new(0),
        corrupt: Default::default(),
    });
    let a = adapter(&f, provider.clone(), clock, Default::default());
    let r = crate::storage::batch::BatchInput {
        request_context: crate::storage::request_context::RetainedRequestContext::from_contract(1)
            .unwrap(),
        expected_generation: None,
        result_version: 1,
        mutation_version: 3,
        kind: crate::storage::batch::BatchKind::Insert,
        items: vec![
            json!({"summary":"old private timestamp", "createdAt":500,"initialStability":12.,"checkDuplicates":false}),
        ],
        default_namespace: "native".into(),
        store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
        idempotency_key: Some("before-timestamp".into()),
        batch_operation_id: None,
    };
    let outcome = a.mutate_native_batch(r.clone()).await.unwrap();
    assert_eq!(outcome.outcome.failed, 1);
    assert_eq!(outcome.outcome.succeeded, 0);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let p = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .lookup_batch(r.store_id.unwrap(), outcome.outcome.batch_operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(p.original_contract().unwrap(), 1);
    let manifest = json!({"path":d.path,"namespace":d.ns,"parent":d.parent,"profile":d.profile,"request":r,"outcome":outcome,"progress":p});
    std::fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    drop(a);
    let _kept = f.close().keep();
}

#[tokio::test]
async fn api1_batch_genuine_private_timestamp_history_replay() {
    let extracted;
    let (artifact, source) = match std::env::var("RECALLD_BATCH_API1_READ_BEFORE") {
        Ok(root) => {
            let artifact = PathBuf::from(root);
            let saved: Value =
                serde_json::from_slice(&std::fs::read(artifact.join("manifest.json")).unwrap())
                    .unwrap();
            let source = artifact.join(
                Path::new(saved["path"].as_str().unwrap())
                    .strip_prefix("/evidence/artifact")
                    .unwrap(),
            );
            (artifact, source)
        }
        Err(_) => {
            let artifact = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/native-api1-batch-before158");
            extracted = tempfile::tempdir().unwrap();
            let source = extracted.path().join("source");
            let archive = artifact.join("store.zip");
            assert!(std::fs::metadata(&archive).unwrap().len() <= 2 * 1024 * 1024);
            let provenance: Value =
                serde_json::from_slice(&std::fs::read(artifact.join("PROVENANCE.json")).unwrap())
                    .unwrap();
            let expected = provenance["store_files_sha256"].as_object().unwrap();
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
            assert_eq!(zip.len(), expected.len());
            assert!(zip.len() <= 16);
            let mut total = 0_u64;
            let mut seen = std::collections::BTreeSet::new();
            for n in 0..zip.len() {
                let file = zip.by_index(n).unwrap();
                assert!(file.enclosed_name().is_some());
                assert!(file.name().len() <= 256);
                assert!(!file.is_dir());
                assert!(file.unix_mode().is_none_or(|m| m & 0o170000 != 0o120000));
                assert!(expected.contains_key(file.name()));
                assert!(seen.insert(file.name().to_owned()));
                total = total.checked_add(file.size()).unwrap();
                assert!(total <= 8 * 1024 * 1024);
            }
            use sha2::Digest;
            let hash = |bytes: Vec<u8>| {
                sha2::Sha256::digest(bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            };
            assert_eq!(
                hash(std::fs::read(archive).unwrap()),
                provenance["archive_sha256"].as_str().unwrap()
            );
            zip.extract(&source).unwrap();
            for (name, expected) in expected {
                assert_eq!(
                    hash(std::fs::read(source.join(name)).unwrap()),
                    expected.as_str().unwrap()
                );
            }
            (artifact, source)
        }
    };
    let saved: Value =
        serde_json::from_slice(&std::fs::read(artifact.join("manifest.json")).unwrap()).unwrap();
    let before_source = bytes(&source);
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("native");
    fn copy(source: &Path, target: &Path) {
        std::fs::create_dir(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let to = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &to)
            } else {
                std::fs::copy(entry.path(), to).unwrap();
            }
        }
    }
    copy(&source, &target);
    let d = Data {
        path: target,
        ns: serde_json::from_value(saved["namespace"].clone()).unwrap(),
        parent: serde_json::from_value(saved["parent"].clone()).unwrap(),
        profile: serde_json::from_value(saved["profile"].clone()).unwrap(),
    };
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(temp, session.engine);
    readers::completion(&f).complete().await.unwrap();
    let provider = Arc::new(RecoveringProvider {
        profile: d.profile.clone(),
        unavailable: false.into(),
        calls: AtomicUsize::new(0),
        profiles: AtomicUsize::new(0),
        corrupt: Default::default(),
    });
    let a = adapter(&f, provider.clone(), clock.clone(), Default::default());
    let r: crate::storage::batch::BatchInput =
        serde_json::from_value(saved["request"].clone()).unwrap();
    let old: crate::serialization::native_batch::NativeBatchOutcome =
        serde_json::from_value(saved["outcome"].clone()).unwrap();
    let baseline = bytes(&d.path);
    let time = clock.0.load(Ordering::SeqCst);
    let out = a
        .run_batch_with_surface(
            r.clone(),
            Some(crate::mcp::batch_budget::BatchBudget::STANDARD),
            Some(crate::serialization::batch_api1::Surface::Rest),
        )
        .await
        .unwrap();
    assert_eq!(out.failed, 1);
    assert_eq!(out.items[0].error, old.outcome.items[0].error);
    assert_eq!(
        out.reservation_receipt.as_ref().unwrap().operation_id,
        old.outcome.batch_operation_id
    );
    let retained = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .lookup_batch(r.store_id.unwrap(), out.batch_operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        canonical(&retained),
        canonical(
            &serde_json::from_value::<crate::storage::batch::BatchProgress>(
                saved["progress"].clone()
            )
            .unwrap()
        )
    );
    assert_eq!(bytes(&d.path), baseline);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(provider.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), time);
    let mut changed = r.clone();
    changed.items[0]["createdAt"] = json!(501);
    assert!(
        a.run_batch_with_surface(
            changed,
            Some(crate::mcp::batch_budget::BatchBudget::STANDARD),
            Some(crate::serialization::batch_api1::Surface::Rest)
        )
        .await
        .is_err()
    );
    assert_eq!(bytes(&d.path), baseline);
    // Historical replay above must remain valid without inventing old policy.
    // Only the fresh operation below needs explicit present-day adoption.
    let policy = crate::storage::native_lifecycle::NativePolicyInputs {
        global: crate::decay::DecayConfig::default(),
        multiplier: crate::config::RecalldConfig::default()
            .decay
            .decay_rate_multiplier,
    };
    {
        let engine = f.storage.read().unwrap();
        let namespace = engine.get_namespace(d.ns).unwrap().unwrap();
        assert!(namespace.decay_rate_multiplier.is_none());
        assert!(namespace.schema.as_ref().unwrap().inherited_decay.is_none());
        assert!(matches!(
            policy.resolve(&namespace),
            Err(StorageError::InvalidMutation(message))
                if message == "inherited policy is unknown; explicitly preview/adopt before dependent operations"
        ));
    }
    drop(a);
    let temp = f.close();
    let adoption = crate::test_support::explicitly_adopt_inherited_policy(
        &d.path,
        d.ns,
        policy.multiplier,
        2000,
    );
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(temp, session.engine);
    readers::completion(&f).complete().await.unwrap();
    let a = adapter(&f, provider.clone(), clock.clone(), Default::default());
    {
        let engine = f.storage.read().unwrap();
        let namespace = engine.get_namespace(d.ns).unwrap().unwrap();
        let binding = namespace
            .schema
            .as_ref()
            .unwrap()
            .inherited_decay
            .as_ref()
            .unwrap();
        assert!(binding.matches_receipt(&adoption));
        assert_eq!(
            policy
                .resolve(&namespace)
                .unwrap()
                .decay_rate_multiplier
                .to_bits(),
            (policy.multiplier as f32).to_bits()
        );
        let historical = engine
            .meta_store()
            .lookup_batch(r.store_id.unwrap(), old.outcome.batch_operation_id)
            .unwrap()
            .unwrap();
        assert_eq!(canonical(&historical), canonical(&retained));
    }
    assert_eq!(bytes(&source), before_source);
    let mut fresh = r;
    fresh.idempotency_key = Some("fresh-timestamp".into());
    // Historical replay above needs no retroactive certificate. Fresh writes
    // must still reject until the private copy undergoes the real remedy.
    let adopted_bytes = bytes(&d.path);
    let adopted_control = f.storage.read().unwrap().control_state().unwrap();
    let error = a
        .run_batch_with_surface(
            fresh.clone(),
            Some(crate::mcp::batch_budget::BatchBudget::STANDARD),
            Some(crate::serialization::batch_api1::Surface::Rest),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        crate::mcp::bridge_adapters::StoreWriteError::Canonical(
            StorageError::VectorCertificationRequired { namespace_id }
        ) if namespace_id == d.ns.get()
    ));
    assert_eq!(bytes(&d.path), adopted_bytes);
    assert_eq!(
        f.storage.read().unwrap().control_state().unwrap(),
        adopted_control
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(provider.profiles.load(Ordering::SeqCst), 0);
    drop(a);
    let temp = f.close();
    let adopted_path = d.path.clone();
    let closed_adopted_bytes = bytes(&adopted_path);
    let d = certified_positive_fixture::certify_positive(temp.path(), d);
    assert_eq!(bytes(&adopted_path), closed_adopted_bytes);
    assert_eq!(bytes(&source), before_source);
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(temp, session.engine);
    readers::completion(&f).rebuild_all().await.unwrap();
    let a = adapter(&f, provider.clone(), clock.clone(), Default::default());
    {
        let engine = f.storage.read().unwrap();
        assert_eq!(
            engine.control_state().unwrap().generation_ref().generation,
            adopted_control.generation_ref().generation.next().unwrap()
        );
        let historical = engine
            .meta_store()
            .lookup_batch(fresh.store_id.unwrap(), old.outcome.batch_operation_id)
            .unwrap()
            .unwrap();
        assert_eq!(canonical(&historical), canonical(&retained));
        let namespace = engine.get_namespace(d.ns).unwrap().unwrap();
        assert!(
            namespace
                .schema
                .as_ref()
                .unwrap()
                .inherited_decay
                .as_ref()
                .unwrap()
                .matches_receipt(&adoption)
        );
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(provider.profiles.load(Ordering::SeqCst), 0);
    let out = a
        .run_batch_with_surface(
            fresh,
            Some(crate::mcp::batch_budget::BatchBudget::STANDARD),
            Some(crate::serialization::batch_api1::Surface::Rest),
        )
        .await
        .unwrap();
    assert_eq!(out.succeeded, 1, "{out:?}");
    let id: MemoryId =
        serde_json::from_value(out.items[0].result.as_ref().unwrap()["id"].clone()).unwrap();
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .get_record(id)
            .unwrap()
            .unwrap()
            .created_at,
        500
    );
    assert_eq!(bytes(&source), before_source);
}

fn api1_request(f: &Fixture, items: Vec<Value>) -> crate::serialization::batch_api1::Request {
    crate::serialization::batch_api1::Request {
        surface: crate::serialization::batch_api1::Surface::Mcp,
        input: crate::storage::batch::BatchInput {
            request_context: None,
            expected_generation: None,
            result_version: 1,
            mutation_version: 1,
            kind: crate::storage::batch::BatchKind::Insert,
            items,
            default_namespace: "native".into(),
            store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
            idempotency_key: Some("api1-batch".into()),
            batch_operation_id: None,
        },
    }
}

#[tokio::test]
async fn api1_batch_final_barrier_failure_keeps_receipts_and_repair_replays() {
    use crate::serialization::batch_api1::{Reply, Surface};
    let (d, f, a, p, clock) = setup().await;
    let r = api1_request(
        &f,
        vec![
            json!({"summary":"barrier child","entities":["José"],"embedding":[2.,-0.,0.,0.],"embeddingProfileId":d.profile.profile_id().unwrap(),"checkDuplicates":false}),
        ],
    );
    a.fail_native_batch_after_children
        .store(1, Ordering::SeqCst);
    let Reply::Owned(Ok(out)) = a.store_batch_api1(r.clone()).await else {
        panic!("missing owned outcome")
    };
    assert_eq!(out.outcome.succeeded, 1);
    assert_eq!(out.outcome.failed, 0);
    assert!(!out.outcome.items[0].ready);
    let receipt = out.outcome.items[0].receipt.clone().unwrap();
    assert!(!out.readiness.as_ref().unwrap().ready);
    let (status, value) = Reply::Owned(Ok(out.clone())).response(Surface::Rest);
    assert_eq!(status, 207);
    assert_eq!(value["data"]["total"], 1);
    assert_eq!(value["batch"]["items"][0]["receipt"], canonical(&receipt));
    let mcp = Reply::Owned(Ok(out)).mcp();
    assert_eq!(mcp.is_error, Some(true));
    crate::test_support::record_schema_result(
        "batch-final-barrier",
        "store_memories",
        &mcp,
        "actual API1 final barrier failure after successful children, full retained receipts and same-key repair assertions unchanged",
    );
    readers::completion(&f).complete().await.unwrap();
    let base = bytes(&d.path);
    let time = clock.0.load(Ordering::SeqCst);
    let calls = p.calls.load(Ordering::SeqCst);
    let profiles = p.profiles.load(Ordering::SeqCst);
    let Reply::Owned(Ok(replay)) = a.store_batch_api1(r.clone()).await else {
        panic!()
    };
    assert!(replay.readiness.as_ref().unwrap().ready);
    assert!(replay.outcome.items[0].ready);
    let mut expected = receipt;
    expected.replayed = true;
    assert_eq!(replay.outcome.items[0].receipt.as_ref().unwrap(), &expected);
    assert_eq!(bytes(&d.path), base);
    assert_eq!(clock.0.load(Ordering::SeqCst), time);
    assert_eq!(p.calls.load(Ordering::SeqCst), calls);
    assert_eq!(p.profiles.load(Ordering::SeqCst), profiles);
    let manifest = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .lookup_batch(r.input.store_id.unwrap(), replay.outcome.batch_operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(manifest.original_contract().unwrap(), 1);
    assert_eq!(manifest.child(0).unwrap().context().contract_version, 1);
}

#[tokio::test]
async fn api1_batch_mixed_and_all_invalid_keep_every_index_and_no_false_rollback() {
    use crate::serialization::batch_api1::{Reply, Surface};
    let (d, f, a, p, _clock) = setup().await;
    let mut r = api1_request(
        &f,
        vec![
            json!({"summary":"one","entities":["José"],"checkDuplicates":false}),
            json!({"summary":false}),
            json!({"summary":"three","entities":[],"checkDuplicates":false}),
        ],
    );
    let Reply::Owned(Ok(out)) = a.store_batch_api1(r.clone()).await else {
        panic!()
    };
    assert_eq!(
        (
            out.outcome.succeeded,
            out.outcome.failed,
            out.outcome.unresolved
        ),
        (2, 1, 0)
    );
    assert_eq!(out.outcome.items.len(), 3);
    assert!(out.outcome.items[0].receipt.is_some());
    assert!(out.outcome.items[2].receipt.is_some());
    let id: MemoryId =
        serde_json::from_value(out.outcome.items[2].result.as_ref().unwrap()["id"].clone())
            .unwrap();
    assert!(
        f.storage
            .read()
            .unwrap()
            .get_record(id)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .labels
            .entities
            .is_empty()
    );
    let (status, value) = Reply::Owned(Ok(out.clone())).response(Surface::Rest);
    assert_eq!(status, 207);
    assert_eq!(value["data"]["failed"][0]["index"], 1);
    assert_eq!(
        value["batch"]["items"][2]["receipt"],
        canonical(out.outcome.items[2].receipt.as_ref().unwrap())
    );
    let mcp = Reply::Owned(Ok(out)).mcp();
    assert_eq!(mcp.is_error, Some(true));
    crate::test_support::record_schema_result(
        "batch-mixed",
        "store_memories",
        &mcp,
        "actual API1 mixed batch projection with complete successful and bookkeeping receipts",
    );
    r.input.idempotency_key = Some("all-invalid".into());
    r.input.items = vec![
        json!({"summary":false}),
        json!({"summary":"missing profile","embedding":[1.,0.,0.,0.]}),
    ];
    let calls = p.calls.load(Ordering::SeqCst);
    let Reply::Owned(Ok(out)) = a.store_batch_api1(r).await else {
        panic!()
    };
    assert_eq!((out.outcome.succeeded, out.outcome.failed), (0, 2));
    assert!(out.outcome.reservation_receipt.is_some());
    assert_eq!(Reply::Owned(Ok(out)).response(Surface::Rest).0, 422);
    assert_eq!(p.calls.load(Ordering::SeqCst), calls);
    assert!(
        f.storage
            .read()
            .unwrap()
            .get_record(d.parent)
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn api1_batch_actual_rpc_prework_frames_mode_and_remote() {
    use crate::serialization::batch_api1::{Reply, Surface};
    let (d, f, a, p, clock) = setup().await;
    let r = api1_request(
        &f,
        vec![json!({"summary":"RPC native","entities":["José"],"checkDuplicates":false})],
    );
    let baseline = bytes(&d.path);
    let time = clock.0.load(Ordering::SeqCst);
    let raw = crate::daemon::server::native_method_test_rpc(
        a.clone(),
        serde_json::to_value(&r.input).unwrap(),
        "mutate_batch",
    )
    .await;
    assert!(raw.error.is_some());
    let small = crate::daemon::server::native_method_test_rpc_with_limit(
        a.clone(),
        serde_json::to_value(&r).unwrap(),
        "store_batch_api1_v1",
        None,
    )
    .await;
    assert!(small.error.is_some());
    let mut bad = r.clone();
    bad.input.items[0]["createdAt"] = json!(500);
    let Reply::Owned(Err(_)) = a.store_batch_api1(bad).await else {
        panic!()
    };
    assert_eq!(bytes(&d.path), baseline);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(p.profiles.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), time);
    let response = crate::daemon::server::native_method_test_rpc(
        a.clone(),
        serde_json::to_value(&r).unwrap(),
        "store_batch_api1_v1",
    )
    .await;
    assert!(response.error.is_none(), "{response:?}");
    let reply = Reply::decode(response.result.unwrap(), &r);
    assert_eq!(reply.response(Surface::Rest).0, 201);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_batch_real_remote_public_mcp_unicode_reopen() {
    let (d, f, a, p, clock) = setup().await;
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        a.clone(),
        &socket.path().join("s.sock"),
    )
    .await;
    let mut b = StubBridge::default().into_bridge();
    b.storage = Arc::new(remote);
    b.default_namespace = "native".into();
    let result=crate::mcp::tools::dispatch_tool(&b,"store_memories",json!({"memories":[{"summary":"remote native batch","entities":["John Smith","José"],"checkDuplicates":false}]})).await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
    let value: Value = serde_json::from_str(text).unwrap();
    let id: MemoryId = serde_json::from_value(value["results"][0]["id"].clone()).unwrap();
    assert_eq!(value["stored"], 1);
    drop(b);
    task.await.unwrap();
    drop(a);
    let kept = f.close();
    let reopened = open(&d, clock);
    assert_eq!(
        reopened
            .engine
            .get_record(id)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .labels
            .entities,
        ["John Smith", "José"]
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    drop(reopened);
    drop(kept);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_batch_legacy_positive_shapes_and_explicit_profile_rejection() {
    let (_d, _native, _native_a, provider, clock) = setup().await;
    let f = Fixture::new();
    readers::completion(&f).complete().await.unwrap();
    let a = Arc::new(adapter(&f, provider.clone(), clock, Default::default()));
    let api = router(&f, a.clone(), provider.clone());
    let(status,value)=batch_http(&api,json!({"memories":[{"summary":"legacy one","embedding":[2.,0.,0.,0.],"entities":["Alice"],"createdAt":500,"initialStability":12.,"checkDuplicates":false}]})).await;
    assert_eq!(status, 201, "{value}");
    assert_eq!(value["data"]["total"], 1);
    assert!(value.get("batch").is_none());
    assert!(value["data"].get("failed").is_none());
    let id: MemoryId = serde_json::from_value(value["data"]["created"][0]["id"].clone()).unwrap();
    let record = f.storage.read().unwrap().get_record(id).unwrap().unwrap();
    assert_eq!(record.created_at, 500);
    assert_eq!(record.stability, 12.);
    assert!(record.schema.is_none());
    let before = f.storage.read().unwrap().control_state().unwrap();
    let(status,value)=batch_http(&api,json!({"memories":[{"summary":"explicit profile","embedding":[1.,0.,0.,0.],"embeddingProfileId":provider.profile.profile_id().unwrap()}]})).await;
    assert_eq!(status, 400, "{value}");
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    let mut b = bridge(a.clone());
    b.default_namespace = "default".into();
    let result=crate::mcp::tools::dispatch_tool(&b,"store_memories",json!({"memories":[{"summary":"legacy MCP supplied","embedding":[1.,0.,0.,0.],"checkDuplicates":false}]})).await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    crate::test_support::record_schema_result(
        "batch-legacy-success",
        "store_memories",
        &result,
        "actual public MCP durable legacy insertion batch with full receipts and no provider calls",
    );
    let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
    let value: Value = serde_json::from_str(text).unwrap();
    assert_eq!(value["stored"], 1);
    assert!(value["batch"]["items"][0]["receipt"].is_object());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn api1_batch_malformed_known_reply_preserves_receipts_and_unknown_ids() {
    use crate::serialization::batch_api1::{Reply, Surface};
    let (_d, f, a, _p, _clock) = setup().await;
    let r = api1_request(
        &f,
        vec![json!({"summary":"known reply","checkDuplicates":false})],
    );
    let Reply::Owned(Ok(out)) = a.store_batch_api1(r.clone()).await else {
        panic!()
    };
    let receipt = out.outcome.items[0].receipt.clone().unwrap();
    let original = canonical(&Reply::Owned(Ok(out.clone())));
    for mutate in 0..3 {
        let mut value = original.clone();
        match mutate {
            0 => value["result"]["Ok"]["readiness"] = json!(false),
            1 => value["result"]["Ok"]["items"][0]["result"] = json!({}),
            _ => value["result"]["Ok"]["succeeded"] = json!(99),
        };
        let (status, error) = Reply::decode(value, &r).response(Surface::Rest);
        assert_eq!(status, 503);
        assert_eq!(error["error"]["commitState"], "unknown");
        assert_eq!(
            error["receivedEvidence"]["items"][0]["receipt"],
            canonical(&receipt)
        );
        assert!(error.get("batch").is_none());
        assert!(error.get("readiness").is_none());
    }
    let mut unresolved = out;
    unresolved.outcome.items[0].status = crate::storage::batch::BatchStatus::Unknown;
    unresolved.outcome.items[0].receipt = None;
    unresolved.outcome.items[0].ready = false;
    unresolved.outcome.items[0].result = None;
    unresolved.outcome.recount();
    let op = unresolved.outcome.items[0].operation_id.unwrap();
    let (status, value) = Reply::Owned(Ok(unresolved)).response(Surface::Rest);
    assert_eq!(status, 503);
    assert_eq!(value["batch"]["items"][0]["operationId"], json!(op));
    assert_eq!(value["batch"]["unresolved"], 1);
}

#[tokio::test]
async fn api1_batch_fresh_budget_precedes_reservation_and_whole_projection_frames() {
    use crate::serialization::batch_api1::{Reply, Surface};
    let (d, f, a, p, clock) = setup().await;
    let r = api1_request(
        &f,
        vec![json!({"summary":"full budget","checkDuplicates":false}); 100],
    );
    let before = bytes(&d.path);
    let time = clock.0.load(Ordering::SeqCst);
    let Reply::Owned(Err(failure)) = a.store_batch_api1(r).await else {
        panic!("oversize admitted")
    };
    assert_eq!(failure.error.code, "RESPONSE_TOO_LARGE");
    assert!(failure.reservation_receipt.is_none());
    assert_eq!(bytes(&d.path), before);
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), time);
    let r = api1_request(
        &f,
        vec![json!({"summary":"valid frame","parentId":d.parent,"checkDuplicates":false})],
    );
    let Reply::Owned(Ok(mut out)) = a.store_batch_api1(r).await else {
        panic!()
    };
    // Actual canonical receipt, serializer-only worst advisory expansion. These
    // displayed strings are not represented as native persisted state/provenance.
    let matches = (0..3)
        .map(|n| crate::model::NearDuplicateMatch {
            id: MemoryId::from_uuid(uuid::Uuid::from_u128(n + 1)),
            score: 0.99999994,
            summary: "\u{1}".repeat(240),
        })
        .collect();
    let report = crate::model::NearDuplicateReport::from_matches(0.85, matches).unwrap();
    let result = out.outcome.items[0].result.as_mut().unwrap();
    result["nearDuplicates"] = canonical(&report);
    result["advisory"] = json!({"requested":true,"warnings":["ADVISORY_UNAVAILABLE"],"readGeneration":f.storage.read().unwrap().control_state().unwrap().generation_ref()});
    assert!(
        serde_json::to_vec(&result).unwrap().len() < 8 * 1024,
        "one bounded result exceeds original allowance"
    );
    let mcp = Reply::Owned(Ok(out.clone())).mcp();
    let (_, rest) = Reply::Owned(Ok(out.clone())).response(Surface::Rest);
    let daemon = crate::daemon::protocol::DaemonResponse::success(
        u64::MAX,
        canonical(&Reply::Owned(Ok(out))),
    );
    let packets = [
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":u64::MAX,"result":mcp})).unwrap(),
        serde_json::to_vec(&rest).unwrap(),
        serde_json::to_vec(&daemon).unwrap(),
    ];
    println!(
        "API1 whole projection bytes MCP={} REST={} daemon={}",
        packets[0].len(),
        packets[1].len(),
        packets[2].len()
    );
    for packet in packets {
        assert!(packet.len() < crate::daemon::protocol::MAX_MESSAGE_SIZE);
    }
}

#[tokio::test]
async fn api1_batch_generated_recipe_profile_errors_and_rest_null_controls() {
    use crate::serialization::batch_api1::{Reply, Surface};
    let (d, f, a, p, clock) = setup().await;
    drop(a);
    let spy = Arc::new(InputSpy {
        inner: p.clone(),
        texts: Default::default(),
    });
    let a = Arc::new(adapter(&f, spy.clone(), clock, Default::default()));
    let api = router(&f, a.clone(), spy.clone());
    let(status,value)=batch_http(&api,json!({"memories":[{"namespace":"native","summary":"header","fullText":"body","tags":["raw"],"entities":["José","John Smith"],"topics":["Rust"],"emotions":["Calm"],"createdAt":null,"initialStability":null,"checkDuplicates":false}]})).await;
    assert_eq!(status, 201, "{value}");
    assert_eq!(
        *spy.texts.lock().unwrap(),
        ["header\n\nbody raw entity/josé entity/john smith topic/rust emotion/calm"]
    );
    let mut wrong = d.profile.clone();
    wrong.model_id.push_str("-other");
    let r = api1_request(
        &f,
        vec![
            json!({"summary":"bad identity","embedding":[1.,0.,0.,0.],"embeddingProfileId":wrong.profile_id().unwrap()}),
            json!({"summary":"bad null","embedding":[1.,0.,0.,0.],"embeddingProfileId":null}),
            json!({"summary":"no identity","embedding":[1.,0.,0.,0.]}),
        ],
    );
    let calls = p.calls.load(Ordering::SeqCst);
    let Reply::Owned(Ok(out)) = a.store_batch_api1(r).await else {
        panic!()
    };
    assert_eq!((out.outcome.failed, out.outcome.succeeded), (3, 0));
    assert_eq!(
        out.outcome.items[0].error.as_ref().unwrap().code,
        "CONFLICT"
    );
    assert_eq!(Reply::Owned(Ok(out)).response(Surface::Rest).0, 422);
    assert_eq!(p.calls.load(Ordering::SeqCst), calls);
}

#[tokio::test]
async fn api1_batch_projection_enforces_display_limits_keeps_full_receipt() {
    use crate::serialization::batch_api1::{Reply, Surface};
    let (d, f, a, _p, _clock) = setup().await;
    let Reply::Owned(Ok(out)) = a
        .store_batch_api1(api1_request(
            &f,
            vec![json!({"summary":"bounded display","checkDuplicates":false})],
        ))
        .await
    else {
        panic!()
    };
    let receipt = out.outcome.items[0].receipt.clone().unwrap();
    for case in 0..10 {
        let mut bad = out.clone();
        let result = bad.outcome.items[0].result.as_mut().unwrap();
        match case {
            0 => result["namespace"] = json!("n".repeat(65)),
            1 => result["createdAt"] = json!("t".repeat(65)),
            2 => {
                result["nearDuplicates"] = json!({"threshold":0.85,"matches":[{"id":d.parent,"score":1.,"summary":"s".repeat(241)}]})
            }
            3 => {
                result["nearDuplicates"] = json!({"threshold":0.85,"matches":(1..=4).map(|n|json!({"id":uuid::Uuid::from_u128(n),"score":1.,"summary":"ok"})).collect::<Vec<_>>()})
            }
            4 => {
                result["advisory"] = json!({"requested":true,"readGeneration":null,"warnings":["ADVISORY_UNAVAILABLE","ADVISORY_UNAVAILABLE"]})
            }
            5 => {
                result["advisory"] =
                    json!({"requested":true,"readGeneration":null,"warnings":["x".repeat(513)]})
            }
            6 => {
                bad.outcome.items[0].error = Some(crate::storage::batch::BatchError {
                    code: "X".repeat(65),
                    field: None,
                })
            }
            7 => {
                bad.outcome.items[0].error = Some(crate::storage::batch::BatchError {
                    code: "INDEX_NOT_READY".into(),
                    field: Some("x".repeat(129)),
                })
            }
            8 => {
                result["supersedes"] =
                    json!({"target":d.parent,"status":"applied","unknown":"x".repeat(1000)})
            }
            _ => {
                result["nearDuplicates"] = json!({"threshold":0.85,"matches":[{"id":d.parent,"score":1.,"summary":"ok","unknown":"x".repeat(1000)}]})
            }
        }
        let (status, value) = Reply::owned(Ok(bad)).response(Surface::Rest);
        assert_eq!(status, 503, "case {case}");
        assert_eq!(value["error"]["commitState"], "unknown");
        assert_eq!(
            value["receivedEvidence"]["items"][0]["receipt"],
            canonical(&receipt)
        );
        assert!(
            value["receivedEvidence"]["items"][0]
                .get("result")
                .is_none()
        );
        assert_eq!(value["warnings"][0], "BATCH_DISPLAY_OMITTED");
    }
}

#[tokio::test]
async fn api1_batch_malformed_remote_evidence_never_claims_commit_or_counts() {
    use crate::serialization::batch_api1::{Reply, Surface};
    let (_d, f, a, _p, _clock) = setup().await;
    let r = api1_request(
        &f,
        vec![json!({"summary":"remote evidence", "checkDuplicates":false})],
    );
    let Reply::Owned(Ok(out)) = a.store_batch_api1(r.clone()).await else {
        panic!()
    };
    let original = canonical(&Reply::Owned(Ok(out.clone())));
    for case in 0..7 {
        let mut value = original.clone();
        match case {
            0 => value["result"]["Ok"]["succeeded"] = json!(99),
            1 => {
                value["result"]["Ok"]["items"][0]["receipt"]["storeId"] =
                    json!(uuid::Uuid::new_v4())
            }
            2 => value["result"]["Ok"]["items"][0]["receipt"]["receiptVersion"] = json!(999),
            3 => value["result"]["Ok"]["items"][0]["namespaceId"] = json!(999),
            4 => {
                value["result"]["Ok"]["reservationReceipt"]["outcome"]["caller_hash"] =
                    json!(vec![0u8; 32])
            }
            5 => {
                value["result"]["Ok"]["items"][0]["result"]["nearDuplicates"] =
                    json!({"threshold":0.85,"matches":[],"unknown":"x".repeat(20_000)})
            }
            _ => value["result"]["Ok"]["warnings"] = json!(["x".repeat(20_000)]),
        }
        let expected = value["result"]["Ok"]["items"][0]["receipt"].clone();
        let (status, error) = Reply::decode(value, &r).response(Surface::Rest);
        assert_eq!(status, 503, "case {case}: {error}");
        assert_eq!(error["error"]["commitState"], "unknown", "case {case}");
        crate::test_support::record_schema_value(
            &format!("batch-unverified-{case}"),
            "store_memories",
            &error,
            "actual decoder plus shared API1 failure projection (REST invocation) of authored malformed copies of genuine batch receipts; no canonical mutation",
        );
        assert_eq!(
            error["receivedEvidence"]["items"][0]["receipt"], expected,
            "case {case}"
        );
        assert!(error.get("batch").is_none());
        assert!(error.get("readiness").is_none());
        assert!(error["receivedEvidence"].get("succeeded").is_none());
        assert!(
            error["receivedEvidence"]["items"][0]
                .get("result")
                .is_none()
        );
        assert!(serde_json::to_vec(&error).unwrap().len() < 20_000);
    }
}

#[tokio::test]
async fn api1_batch_remote_request_binding_keeps_only_matching_reservation_committed() {
    use crate::serialization::{
        batch_api1::{Reply, Surface},
        native_batch::BatchFailure,
    };
    let (_d, f, a, _p, _clock) = setup().await;
    let r = api1_request(
        &f,
        vec![json!({"summary":"bound caller", "checkDuplicates":false})],
    );
    let Reply::Owned(Ok(out)) = a.store_batch_api1(r.clone()).await else {
        panic!()
    };
    let original = canonical(&Reply::Owned(Ok(out.clone())));
    assert_eq!(
        Reply::decode(original.clone(), &r)
            .response(Surface::Rest)
            .0,
        201
    );
    for case in 0..4 {
        let mut changed = r.clone();
        match case {
            0 => changed.input.items[0]["summary"] = json!("different caller"),
            1 => changed.input.items[0]["namespace"] = json!("other"),
            2 => changed.input.store_id = Some(uuid::Uuid::new_v4()),
            _ => changed.input.batch_operation_id = Some(uuid::Uuid::new_v4()),
        }
        let (status, value) = Reply::decode(original.clone(), &changed).response(Surface::Rest);
        assert_eq!(status, 503);
        assert_eq!(value["error"]["commitState"], "unknown");
        assert_eq!(
            value["receivedEvidence"]["reservationReceipt"],
            canonical(&out.outcome.reservation_receipt)
        );
    }
    let mut failure = BatchFailure::uncertain();
    failure.reservation_receipt = out.outcome.reservation_receipt.clone();
    let (status, valid) =
        Reply::decode(canonical(&Reply::Owned(Err(failure.clone()))), &r).response(Surface::Rest);
    assert_eq!(status, 503);
    assert_eq!(valid["error"]["commitState"], "committed");
    assert_eq!(valid["commitEvidenceScope"], "reservation");
    crate::test_support::record_schema_value(
        "batch-reservation-known",
        "store_memories",
        &valid,
        "actual API1 decoder/shared failure projection binds genuine parent caller hash; reservation evidence only",
    );
    assert!(valid.get("batch").is_none());
    let receipt = failure.reservation_receipt.as_mut().unwrap();
    receipt.receipt_version = 999;
    let expected = canonical(receipt);
    let (_, invalid) =
        Reply::decode(canonical(&Reply::Owned(Err(failure))), &r).response(Surface::Rest);
    assert_eq!(invalid["error"]["commitState"], "unknown");
    assert_eq!(invalid["reservationReceipt"], expected);
    crate::test_support::record_schema_value(
        "batch-reservation-unverified",
        "store_memories",
        &invalid,
        "actual API1 decoder/shared failure projection retains authored invalid-header receipt in original top-level field with unknown state and unverified warning",
    );
    assert!(invalid.get("readiness").is_none());
}
