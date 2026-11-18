//! Independent acceptance over a database written by the published v1.0.0 binary.
//! The mock below is loopback-only; no real provider, credential or personal data.
use super::*;
use crate::mcp::bridge::{StorageEngine as McpStorage, StoreInput};
use crate::mcp::bridge_adapters::McpStorageAdapter;
use crate::model::{
    MemoryId, NamespaceId,
    schema::{EmbeddingBinding, EmbeddingOrigin},
};
use crate::search::SearchQuery;
use crate::storage::{StorageEngine as _, TextRef};
use serde_json::{Value, json};
use std::sync::Mutex;

struct MockProvider {
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}
impl MockProvider {
    async fn start() -> Self {
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = requests.clone();
        let app = axum::Router::new().route(
            "/api/embed",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let captured = captured.clone();
                async move {
                    let count = body["input"].as_array().unwrap().len();
                    captured.lock().unwrap().push(body);
                    axum::Json(json!({"embeddings":vec![vec![1.,0.,0.,0.];count]}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            requests,
            task,
        }
    }
    fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
impl Drop for MockProvider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn directory_bytes(
    root: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn visit(
        root: &std::path::Path,
        path: &std::path::Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for item in std::fs::read_dir(path).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().into(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = Default::default();
    visit(root, root, &mut out);
    out
}

fn released_fixture() -> (tempfile::TempDir, std::path::PathBuf, Value) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("released-db");
    std::fs::create_dir(&path).unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "storage/fixtures/released-v1.zip"
    )))
    .unwrap()
    .extract(&path)
    .unwrap();
    let expected =
        serde_json::from_slice(include_bytes!("storage/fixtures/released-v1.json")).unwrap();
    (root, path, expected)
}
fn config(path: &std::path::Path, mock: &MockProvider) -> RecalldConfig {
    let mut config = RecalldConfig::default();
    config.storage.data_dir = path.to_str().unwrap().into();
    config.embedding.provider = crate::config::EmbeddingProvider::Ollama;
    config.embedding.model_name = "independent-release-mock".into();
    config.embedding.dimensions = 4;
    config.embedding.base_url = mock.url.clone();
    config.embedding.api_key_env = "RECALLD_RELEASE_TEST_UNUSED_CREDENTIAL".into();
    config.embedding.document_prefix = "doc: ".into();
    config.embedding.query_prefix = "query: ".into();
    config.decay.disable_sweep = true;
    config.decay.decay_rate_multiplier = 0.;
    config.rif.enabled = false;
    config.graph.autolink_enabled = false;
    config
}
fn adapter(system: &Recalld) -> McpStorageAdapter {
    McpStorageAdapter::new(
        system.storage.clone(),
        system.cache.clone(),
        system.embedding.clone(),
        system.vector_index.clone(),
        system.fts_index.clone(),
        system.entity_index.clone(),
        system.graph.clone(),
        system.config.clone(),
        chrono_tz::UTC,
    )
}
fn assert_old_data(system: &Recalld, expected: &Value) {
    let storage = system.storage.read().unwrap();
    assert_eq!(storage.control_state().unwrap().control_schema_version, 2);
    for (i, item) in expected["created"].as_array().unwrap().iter().enumerate() {
        let old = &item["data"];
        let id: MemoryId = serde_json::from_value(old["id"].clone()).unwrap();
        let record = storage.get_record(id).unwrap().unwrap();
        let name = old["namespace"].as_str().unwrap();
        let ns = storage.get_namespace_by_name(name).unwrap().unwrap();
        assert_eq!(record.version, 2);
        assert_eq!(
            u64::from(record.namespace_id),
            expected["namespaceIds"][name].as_u64().unwrap()
        );
        assert_eq!(
            ns.schema.as_ref().unwrap().embedding_binding,
            EmbeddingBinding::UnknownLegacy
        );
        assert!(
            ns.schema.as_ref().unwrap().vector_certification.is_none(),
            "unknown namespace was promoted"
        );
        assert!(
            record.schema.as_ref().unwrap().embedding_origin.is_none(),
            "old vector acquired invented producer evidence"
        );
        assert_eq!(record.summary, old["summary"].as_str().unwrap());
        assert_eq!(
            storage
                .get_text(TextRef {
                    file_offset: record.text_offset,
                    length: record.text_length
                })
                .unwrap()
                .unwrap(),
            old["fullText"].as_str().unwrap()
        );
        assert_eq!(
            record.tags.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
            old["tags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            storage
                .get_vector(NamespaceId::new(record.namespace_id), record.vector_slot)
                .unwrap()
                .unwrap(),
            [[1., 0., 0., 0.], [0.8, 0.6, 0., 0.], [0., 0., 1., 0.]][i]
        );
    }
    let edges = storage.edge_store().load_all_edges().unwrap();
    assert!(edges.iter().any(|edge| edge.source.to_string()
        == expected["created"][0]["data"]["id"].as_str().unwrap()
        && edge.target.to_string() == expected["created"][1]["data"]["id"].as_str().unwrap()));
}
fn query(namespace: &str, embedding: Option<Vec<f32>>, text: Option<&str>) -> SearchQuery {
    serde_json::from_value(
        json!({"namespace":namespace,"queryEmbedding":embedding,"text":text,
        "queryMode":"embedding_only","recordAccess":false,"limit":20,"minScore":0.,
        "includeGhosts":false,"graphDepth":0,"entities":[]}),
    )
    .unwrap()
}
async fn store(
    system: &Recalld,
    namespace: &str,
    summary: &str,
    embedding: Option<Vec<f32>>,
) -> MemoryId {
    let input = StoreInput {
        summary: summary.into(),
        full_text: Some(format!("{summary} full text")),
        tags: vec!["test/release-upgrade".into()],
        entities: vec![],
        topics: vec![],
        emotions: vec![],
        namespace: namespace.into(),
        embedding,
        initial_stability: None,
        parent_id: None,
        supersedes: None,
        check_duplicates: Some(false),
    };
    let reply = adapter(system)
        .store_api1(crate::serialization::store_api1::Request {
            input,
            profile_id: None,
            created_at: None,
            require_native: true,
        })
        .await;
    let crate::serialization::store_api1::Reply::Owned(reply) = reply else {
        panic!("not native: {reply:?}")
    };
    assert!(reply.error.is_none(), "native write failed: {reply:?}");
    serde_json::from_value(Value::String(reply.data.unwrap().id)).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn released_v1_new_schema_query_write_restart_preserves_unknown_provenance() {
    let (_root, path, expected) = released_fixture();
    let mock = MockProvider::start().await;
    let cfg = config(&path, &mock);
    let original_bytes = directory_bytes(&path);
    let system = Recalld::new(cfg.clone()).await.unwrap();
    assert_eq!(mock.calls(), 0, "upgrade must not reembed existing records");
    let backup = std::fs::read_dir(_root.path())
        .unwrap()
        .map(|e| e.unwrap().path().join("backup"))
        .find(|p| p.is_dir())
        .unwrap();
    assert_eq!(directory_bytes(&backup), original_bytes);
    assert_old_data(&system, &expected);
    let first: MemoryId =
        serde_json::from_value(expected["created"][0]["data"]["id"].clone()).unwrap();
    let other: MemoryId =
        serde_json::from_value(expected["created"][2]["data"]["id"].clone()).unwrap();
    let supplied = system
        .query_engine
        .search(query("default", Some(vec![1., 0., 0., 0.]), None))
        .await
        .unwrap();
    assert!(supplied.results.iter().any(|r| r.memory_id == first));
    assert!(supplied.results.iter().all(|r| r.memory_id != other));
    assert_eq!(mock.calls(), 0);
    let mut accessed = query("default", Some(vec![1., 0., 0., 0.]), None);
    accessed.record_access = true;
    assert!(
        !system
            .query_engine
            .search(accessed)
            .await
            .unwrap()
            .results
            .is_empty()
    );
    let generated = system
        .query_engine
        .search(query("default", None, Some("Release migration alpha")))
        .await
        .unwrap();
    assert!(generated.results.iter().any(|r| r.memory_id == first));
    assert_eq!(mock.calls(), 1);
    let inserted = store(
        &system,
        "default",
        "New generated compatibility memory",
        None,
    )
    .await;
    assert_eq!(mock.calls(), 2);
    let new_schema = system
        .storage
        .read()
        .unwrap()
        .get_record(inserted)
        .unwrap()
        .unwrap()
        .schema
        .unwrap();
    let Some(EmbeddingOrigin::Generated {
        descriptor,
        document_sha256,
    }) = &new_schema.embedding_origin
    else {
        panic!("missing actual generator evidence")
    };
    assert_eq!(descriptor.dimension, 4);
    let document = "New generated compatibility memory\n\nNew generated compatibility memory full text test/release-upgrade";
    use sha2::Digest;
    let expected_hash: [u8; 32] = sha2::Sha256::digest(document.as_bytes()).into();
    assert_eq!(*document_sha256, expected_hash);
    assert_eq!(
        mock.requests.lock().unwrap()[1]["input"][0],
        format!("doc: {document}")
    );
    // Canonical import validators must not accept invented producer evidence on
    // migrated rows, even when the descriptor itself is a valid current model.
    {
        let storage = system.storage.read().unwrap();
        let mut invented = storage.get_record(first).unwrap().unwrap();
        let namespace = storage.get_namespace_by_name("default").unwrap().unwrap();
        invented.schema.as_mut().unwrap().embedding_origin = new_schema.embedding_origin.clone();
        assert!(
            invented
                .schema
                .as_ref()
                .unwrap()
                .validate(&invented)
                .is_err()
        );
        assert!(
            crate::storage::native_records::validate_retained_creation(
                &invented, &namespace, None, None
            )
            .is_err()
        );
        // Claiming a native timestamp must not bypass the creation-proof check.
        invented.schema.as_mut().unwrap().lifecycle.timestamp_origin =
            crate::model::schema::TimestampOrigin::Native;
        assert!(
            crate::storage::native_records::validate_retained_creation(
                &invented, &namespace, None, None
            )
            .is_err()
        );
    }
    assert_old_data(&system, &expected);
    system.shutdown().await.unwrap();
    // Unknown old vectors are not proof that two known generators are compatible.
    let mut drifted = cfg.clone();
    drifted.embedding.model_name = "different-known-generator".into();
    let system = Recalld::new(drifted).await.unwrap();
    assert_eq!(mock.calls(), 2);
    assert!(
        system
            .query_engine
            .search(query("default", None, Some("drift")))
            .await
            .is_err(),
        "known generator mismatch must fail before provider calls"
    );
    assert_eq!(mock.calls(), 2);
    assert_old_data(&system, &expected);
    system.shutdown().await.unwrap();
    let system = Recalld::new(cfg.clone()).await.unwrap();
    assert_eq!(mock.calls(), 2, "restart must not reembed");
    assert_old_data(&system, &expected);
    assert_eq!(
        system
            .storage
            .read()
            .unwrap()
            .get_record(inserted)
            .unwrap()
            .unwrap()
            .schema
            .unwrap()
            .embedding_origin,
        new_schema.embedding_origin
    );
    let new_results = system
        .query_engine
        .search(query("default", Some(vec![1., 0., 0., 0.]), None))
        .await
        .unwrap();
    assert!(new_results.results.iter().any(|r| r.memory_id == inserted));
    // Native supplied compatibility must not be mislabeled as model-generated.
    let supplied_id = store(
        &system,
        "default",
        "New supplied compatibility memory",
        Some(vec![0., 1., 0., 0.]),
    )
    .await;
    assert_eq!(mock.calls(), 2);
    let supplied_schema = system
        .storage
        .read()
        .unwrap()
        .get_record(supplied_id)
        .unwrap()
        .unwrap()
        .schema
        .unwrap();
    assert_eq!(
        supplied_schema.embedding_origin,
        Some(EmbeddingOrigin::Supplied { profile_id: None }),
        "caller vector gained generator evidence"
    );
    system.shutdown().await.unwrap();
    let stream = _root.path().join("mixed.jsonl");
    let imported = _root.path().join("imported");
    let exported = crate::storage::logical_export::export_store(&path, &stream).unwrap();
    assert_eq!(
        crate::storage::logical_export::import_store(&stream, &imported).unwrap(),
        exported
    );
    let archive = crate::backup::run_backup(
        &cfg,
        &_root.path().join("mixed.zip"),
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
    let restored = Recalld::new(config(&imported, &mock)).await.unwrap();
    assert_old_data(&restored, &expected);
    for (id, origin) in [
        (inserted, new_schema.embedding_origin.clone()),
        (supplied_id, supplied_schema.embedding_origin.clone()),
    ] {
        assert_eq!(
            restored
                .storage
                .read()
                .unwrap()
                .get_record(id)
                .unwrap()
                .unwrap()
                .schema
                .unwrap()
                .embedding_origin,
            origin
        );
    }
    assert_eq!(mock.calls(), 2);
    restored.shutdown().await.unwrap();
    assert_eq!(directory_bytes(&backup), original_bytes);
    let system = Recalld::new(cfg.clone()).await.unwrap();
    // Actual generation evidence must survive proof validation and not
    // make the newly created record impossible to forget.
    // Match the public MCP/API1 route; delete_memory is the legacy-only bridge.
    let forgotten = adapter(&system)
        .api1_lifecycle(
            crate::serialization::lifecycle_api1::request(inserted, 3, Some("default")),
            crate::serialization::native_lifecycle::Operation::Forget,
        )
        .await;
    assert!(
        matches!(
            forgotten.value,
            Some(crate::serialization::lifecycle_api1::Value::Forget { deleted: true })
        ),
        "native API1 forget failed: {forgotten:?}"
    );
    assert_old_data(&system, &expected);
    system.shutdown().await.unwrap();
    let system = Recalld::new(cfg).await.unwrap();
    assert_eq!(mock.calls(), 2);
    assert_old_data(&system, &expected);
    assert_eq!(
        system
            .storage
            .read()
            .unwrap()
            .get_record(inserted)
            .unwrap()
            .unwrap()
            .phase,
        crate::model::DecayPhase::Tombstone
    );
    assert!(
        system
            .storage
            .read()
            .unwrap()
            .get_record(supplied_id)
            .unwrap()
            .is_some()
    );
    system.shutdown().await.unwrap();
    assert_eq!(directory_bytes(&backup), original_bytes);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_fixture_never_enables_cross_space_or_wrong_dimension_queries() {
    let (_root, path, expected) = released_fixture();
    let mock = MockProvider::start().await;
    let system = Recalld::new(config(&path, &mock)).await.unwrap();
    let before = system
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    assert!(
        system
            .query_engine
            .search(query("default", Some(vec![1., 0., 0.]), None))
            .await
            .is_err()
    );
    let first: MemoryId =
        serde_json::from_value(expected["created"][0]["data"]["id"].clone()).unwrap();
    let selection = system
        .query_engine
        .prepare_similar_selection(
            first,
            Some("default"),
            crate::search::SimilaritySelection {
                semantics: crate::search::RetrievalSemantics::V2(
                    crate::search::VersionMode::Current,
                ),
                scope: crate::search::SimilarityScope::AllCompatible,
                filter: Default::default(),
                limit: 20,
                min_score: None,
                record_access: false,
                include_embeddings: true,
                original_contract_version: 2,
            },
            None,
        )
        .await;
    assert!(
        selection.is_err(),
        "unknown source cannot claim compatibility across namespaces"
    );
    assert_eq!(mock.calls(), 0);
    assert_eq!(
        system
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .committed_generation,
        before
    );
    assert_old_data(&system, &expected);
    system.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mixed_unknown_receipts_replay_after_restart_without_reembedding() {
    let (_root, path, expected) = released_fixture();
    let mock = MockProvider::start().await;
    let cfg = config(&path, &mock);
    let system = Recalld::new(cfg.clone()).await.unwrap();
    let store_id = system
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .store_id;
    let mut retained = Vec::new();
    for (key, vector) in [
        ("generated", None),
        ("supplied", Some(vec![0., 1., 0., 0.])),
    ] {
        let mut body = json!({"namespace":"default", "storeId":store_id,
            "idempotencyKey":key,"summary":"Summary only, no invented full text", "checkDuplicates":false});
        if let Some(vector) = vector {
            body["embedding"] = json!(vector);
        }
        let request =
            crate::serialization::native_single::Request::parse(&body, true, None, "default")
                .unwrap();
        let reply = adapter(&system).store_native_wire(request.clone()).await;
        assert!(reply.error.is_none(), "{reply:?}");
        let receipt = reply.receipt.unwrap();
        retained.push((request, receipt));
    }
    assert_eq!(mock.calls(), 1);
    assert_old_data(&system, &expected);
    system.shutdown().await.unwrap();
    // Replay uses the original receipt even when today's configured model differs.
    let mut changed = cfg;
    changed.embedding.model_name = "different-model-for-replay".into();
    let system = Recalld::new(changed).await.unwrap();
    let before = system.storage.read().unwrap().control_state().unwrap();
    for (request, original) in retained {
        let replay = adapter(&system).store_native_wire(request).await;
        assert!(replay.error.is_none(), "{replay:?}");
        let receipt = replay.receipt.unwrap();
        assert!(receipt.replayed);
        assert_eq!(receipt.outcome, original.outcome);
        assert_eq!(receipt.committed_generation, original.committed_generation);
    }
    assert_eq!(mock.calls(), 1);
    assert_eq!(
        system.storage.read().unwrap().control_state().unwrap(),
        before
    );
    assert_old_data(&system, &expected);
    system.shutdown().await.unwrap();
}
