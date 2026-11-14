//! Actual private native writer, coordinator and transport consumers. No authored
//! canonical rows, placeholder backend or ordinary schema-2 serving activation.
use super::*;
use crate::api::adapters::*;
use crate::mcp::{bridge::McpBridge, bridge_adapters::*};
use crate::search::adapters::*;
use crate::test_support::Fixture;
use serde_json::{Value, json};

#[path = "feedback_consumer_tests.rs"]
mod feedback_consumer;
#[path = "forget_consumer_tests.rs"]
mod forget_consumer;
#[path = "graph_producer_tests.rs"]
mod graph_producer;
#[path = "replay_consumer_tests.rs"]
mod replay_consumer;

struct NoProvider;
#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for NoProvider {
    fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
        panic!("read consulted provider")
    }
    fn dimensions(&self) -> usize {
        panic!("read consulted provider dimensions")
    }
    fn model_name(&self) -> &str {
        "never-called"
    }
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        panic!("read called provider")
    }
}
fn completion(f: &Fixture) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}
fn readers(
    f: &Fixture,
    clock: Arc<FixedClock>,
) -> (axum::Router, McpBridge, Arc<crate::search::QueryEngine>) {
    let provider = Arc::new(NoProvider);
    let engine = Arc::new(
        crate::search::QueryEngine::new(
            Arc::new(StorageNamespaceResolver::new(f.storage.clone())),
            Arc::new(SingleEmbeddingRegistry::new(provider.clone())),
            Arc::new(SharedVectorIndexRegistry::new(f.vector_index.clone())),
            Arc::new(SharedFtsIndexRegistry::new(f.fts_index.clone())),
            Arc::new(SharedEntityIndexReader::new(f.entity_index.clone())),
            Arc::new(CacheManagerAdapter::new(f.cache.clone())),
            Arc::new(StorageMetadataAdapter::new(f.storage.clone())),
            Arc::new(SharedGraphReader::new(f.graph.clone())),
            Arc::new(RifProcessorAdapter::new(
                Arc::new(crate::rif::RifEngine::new(Default::default())),
                f.graph.clone(),
            )),
            Arc::new(
                StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
                    .with_clock(clock.clone()),
            ),
        )
        .with_lifecycle_inputs(clock.clone(), crate::decay::DecayConfig::default(), 1.0),
    );
    let config = Arc::new(crate::config::RecalldConfig::default());
    let router = crate::api::router(
        crate::api::AppState::new(
            Arc::new(SearchPipelineAdapter::new(
                engine.clone(),
                provider.clone(),
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
            )),
            Arc::new(
                StorageEngineAdapter::new(
                    f.storage.clone(),
                    f.cache.clone(),
                    f.vector_index.clone(),
                    f.fts_index.clone(),
                    f.entity_index.clone(),
                    f.graph.clone(),
                )
                .with_clock(clock.clone()),
            ),
            Arc::new(RecordCacheAdapter::new(f.cache.clone())),
            Arc::new(RelationshipGraphAdapter::new(
                f.graph.clone(),
                f.vector_index.clone(),
                f.entity_index.clone(),
                f.storage.clone(),
                f.cache.clone(),
                config.clone(),
            )),
            Arc::new(
                FsrsEngineAdapter::new(
                    f.storage.clone(),
                    f.cache.clone(),
                    f.graph.clone(),
                    config,
                    false,
                )
                .with_clock(clock),
            ),
            Arc::new(NamespaceRegistryAdapter::new(f.storage.clone())),
            Arc::new(NoopMetricsCollector),
        ),
        &crate::api::ApiConfig::default(),
    );
    let bridge = McpBridge {
        search: Arc::new(McpSearchAdapter::new(
            engine.clone(),
            provider,
            f.storage.clone(),
            f.graph.clone(),
            chrono_tz::UTC,
        )),
        storage: Arc::new(f.storage_adapter()),
        namespaces: Arc::new(f.namespace_adapter()),
        health: Arc::new(crate::test_support::StubBridge::default()),
        default_namespace: "different-default".into(),
        timezone: chrono_tz::UTC,
    };
    (router, bridge, engine)
}
async fn http(
    api: &axum::Router,
    method: &str,
    path: &str,
    body: Value,
) -> (axum::http::StatusCode, Value) {
    use tower::ServiceExt;
    let response = api
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            panic!(
                "{method} {path}: {status}: {e}: {}",
                String::from_utf8_lossy(&bytes)
            )
        }),
    )
}
async fn mcp(bridge: &McpBridge, name: &str, args: Value) -> Value {
    let result = crate::mcp::tools::dispatch_tool(bridge, name, args).await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let result = serde_json::to_value(result).unwrap();
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}
fn bytes(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, at: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, out)
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().into(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut out = Default::default();
    collect(path, path, &mut out);
    out
}
fn assert_labels(record: &Value, id: MemoryId, labels: &crate::model::schema::StructuredLabels) {
    assert_eq!(record["id"], json!(id));
    assert_eq!(record["labels"], json!(labels));
    if labels.entities.is_empty() {
        assert!(record.get("entities").is_none(), "{record}")
    } else if let Some(entities) = record.get("entities") {
        // MCP get retains its established shape (no top-level entity alias).
        // Its exact canonical labels are still required above.
        assert_eq!(*entities, json!(labels.entities));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_writer_publication_http_mcp_and_reopen_keep_exact_labels_and_read_only_bytes() {
    let mut root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let full = input(&d);
    let intent = session.capture_insert(scope(&d), &full).unwrap();
    let id = inserted(
        &session
            .insert_prepared(&ctx(&session, &d, "native-labels"), &intent, &generated())
            .unwrap(),
    );
    let mut empty = input(&d);
    empty.entities.clear();
    empty.topics.clear();
    empty.emotions.clear();
    empty.memory.tags = vec![crate::model::Tag::new("entity/do-not-infer").unwrap()];
    let intent = session.capture_insert(scope(&d), &empty).unwrap();
    let empty_id = inserted(
        &session
            .insert_prepared(&ctx(&session, &d, "native-empty"), &intent, &generated())
            .unwrap(),
    );
    let legacy = session
        .engine
        .get_record(d.parent)
        .unwrap()
        .unwrap()
        .schema
        .unwrap()
        .labels;
    assert_eq!(
        legacy.provenance,
        crate::model::schema::LabelProvenance::LegacyRetainedOnly
    );
    let expected = [
        (id, full.labels()),
        (empty_id, empty.labels()),
        (d.parent, legacy),
    ];
    let mut engine = Some(session.engine);
    for reopen in [false, true] {
        let f = Fixture::from_open_engine(root, engine.take().unwrap());
        let (api, bridge, query) = readers(&f, clock.clone());
        let publisher = completion(&f);
        if !reopen {
            let (status, _) = http(
                &api,
                "GET",
                &format!("/memories/{id}?recordAccess=false"),
                Value::Null,
            )
            .await;
            assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
            publisher.complete().await.unwrap();
        } else {
            publisher.rebuild_all().await.unwrap();
        }
        let before = bytes(&d.path);
        for (record_id, labels) in &expected {
            let (status, body) = http(
                &api,
                "GET",
                &format!("/memories/{record_id}?recordAccess=false"),
                Value::Null,
            )
            .await;
            assert!(status.is_success(), "{body}");
            assert_labels(&body["data"], *record_id, labels);
            let body = mcp(
                &bridge,
                "get_memory",
                json!({"id":record_id,"recordAccess":false}),
            )
            .await;
            assert_labels(&body, *record_id, labels);
        }
        let (status, list) = http(&api, "GET", "/memories?namespace=native", Value::Null).await;
        assert!(status.is_success(), "{list}");
        let mcp_list = mcp(&bridge, "list_memories", json!({"namespace":"native"})).await;
        for (record_id, labels) in &expected {
            for list in [&list["data"]["memories"], &mcp_list["memories"]] {
                let value = list
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["id"] == json!(record_id))
                    .unwrap();
                assert_labels(value, *record_id, labels);
            }
        }
        let (status, similar) = http(
            &api,
            "POST",
            &format!("/similar/{id}"),
            json!({"recordAccess":false}),
        )
        .await;
        assert!(status.is_success(), "{similar}");
        let hit = similar["hits"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["memory"]["id"] == json!(empty_id))
            .unwrap();
        assert_labels(&hit["memory"], empty_id, &empty.labels());
        let similar = mcp(
            &bridge,
            "find_similar_memories",
            json!({"id":id,"recordAccess":false}),
        )
        .await;
        let hit = similar["memories"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == json!(empty_id))
            .unwrap();
        assert_labels(hit, empty_id, &empty.labels());
        // The private policy-aware pipeline uses the same production backend.
        // Neither metadata-only nor supplied-vector selection consults a provider.
        for body in [
            json!({"namespace":"native","queryMode":"metadata_only","filter":{"labels":{"entities":[full.entities[0]]}},"recordAccess":false}),
            json!({"namespace":"native","text":"deliberately irrelevant","queryEmbedding":[1.,0.,0.,0.],"recordAccess":false}),
        ] {
            let result = query
                .search_with_semantics(
                    serde_json::from_value(body.clone()).unwrap(),
                    crate::search::RetrievalSemantics::V2(crate::search::VersionMode::Current),
                )
                .await
                .unwrap();
            assert!(result.response.results.iter().any(|r| r.memory_id == id));
            let target = f
                .storage
                .read()
                .unwrap()
                .meta_store()
                .publication_state()
                .unwrap()
                .canonical;
            let coherent = query
                .search_coherent_read_only(
                    &publisher,
                    Arc::new(crate::rif::RifEngine::new(Default::default())),
                    serde_json::from_value(body).unwrap(),
                    crate::search::RetrievalSemantics::V2(crate::search::VersionMode::Current),
                    std::time::Instant::now() + std::time::Duration::from_secs(10),
                )
                .await
                .unwrap();
            assert_eq!(coherent.read_generation, target);
            assert!(
                coherent
                    .response
                    .response
                    .results
                    .iter()
                    .any(|r| r.memory_id == id)
            );
            assert!(
                publisher
                    .readiness_barrier(
                        target,
                        std::time::Instant::now() + std::time::Duration::from_secs(10)
                    )
                    .await
                    .unwrap()
                    .ready
            );
        }
        assert_eq!(bytes(&d.path), before, "false-mode canonical bytes changed");
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .meta_store()
                .work_tracker()
                .status()
                .pending,
            0
        );
        if reopen {
            f.storage.read().unwrap().mark_unavailable();
            let result = query.search_coherent_read_only(
                &publisher, Arc::new(crate::rif::RifEngine::new(Default::default())),
                serde_json::from_value(json!({"namespace":"native", "queryEmbedding":[1.,0.,0.,0.], "recordAccess":false})).unwrap(),
                crate::search::RetrievalSemantics::V2(crate::search::VersionMode::Current),
                std::time::Instant::now() + std::time::Duration::from_secs(10),
            ).await;
            assert!(result.is_err());
            assert_eq!(
                bytes(&d.path),
                before,
                "poison rejection changed canonical bytes"
            );
        }
        drop((api, bridge, query, publisher));
        root = f.close();
        inspect_schema(&d.path).unwrap();
        assert!(RedbStorageEngine::open(&d.path).is_err());
        if !reopen {
            engine = Some(open(&d, clock.clone()).engine);
        }
    }
    // Corruption is a negative proof test, never fixture construction.
    let db = redb::Database::open(d.path.join("meta.db")).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx.open_table(META_TABLE).unwrap();
        let mut row = DiskRecord::from_bytes(
            table
                .get(id.as_bytes().as_slice())
                .unwrap()
                .unwrap()
                .value(),
        )
        .unwrap();
        row.schema.as_mut().unwrap().labels.entities[0] = "forged".into();
        table
            .insert(id.as_bytes().as_slice(), row.encode().unwrap().as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let before = bytes(&d.path);
    assert!(NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock).is_err());
    assert_eq!(bytes(&d.path), before);
    drop(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_forget_failed_publication_stays_blocked_until_real_retry_without_resurrection() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let intent = session.capture_insert(scope(&d), &input(&d)).unwrap();
    let created = session
        .insert_prepared(
            &ctx(&session, &d, "readable-before-forget"),
            &intent,
            &generated(),
        )
        .unwrap();
    let id = inserted(&created);
    let f = Fixture::from_open_engine(root, session.engine);
    let publisher = completion(&f);
    publisher.complete().await.unwrap();
    let (api, bridge, query) = readers(&f, clock.clone());
    let before = bytes(&d.path);
    let (status, body) = http(&api, "GET", &format!("/memories/{id}"), Value::Null).await;
    assert_eq!(
        status,
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "{body}"
    );
    assert_eq!(
        bytes(&d.path),
        before,
        "legacy observing route mutated native state"
    );
    let forgotten = {
        let mut engine = f.storage.write().unwrap();
        let policy = NativePolicyInputs::default();
        let intent = engine
            .meta_store()
            .capture_native_tombstone(id, None, &policy)
            .unwrap();
        let context = MutationContext {
            store_id: Some(created.store_id),
            scope: MutationScope::Namespace(d.ns),
            idempotency_key: Some("native-forget-reader".into()),
            expected_generation: None,
            contract_version: 2,
        };
        engine
            .tombstone_native_record(&context, &intent, &policy, clock.as_ref(), None)
            .unwrap()
    };
    let (status, _) = http(
        &api,
        "GET",
        &format!("/memories/{id}?recordAccess=false"),
        Value::Null,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
    let failure = publisher.complete().await.unwrap_err();
    let StorageError::CommittedMutation { receipt, .. } = failure else {
        panic!("publication failure lost full committed receipt: {failure:?}");
    };
    assert_eq!(*receipt, forgotten);
    assert!(
        !f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .effects_stamp()
            .is_err()
    );
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    publisher.complete().await.unwrap();
    let before = bytes(&d.path);
    let (status, body) = http(
        &api,
        "GET",
        &format!("/memories/{id}?recordAccess=false"),
        Value::Null,
    )
    .await;
    assert!(status.is_success(), "{body}");
    assert_eq!(body["data"]["summary"], json!(""));
    assert_eq!(body["data"]["labels"]["entities"], json!([]));
    assert!(body["data"].get("fullText").is_none_or(Value::is_null));
    let body = mcp(&bridge, "get_memory", json!({"id":id,"recordAccess":false})).await;
    assert_eq!(body["summary"], json!(""));
    assert_eq!(body["labels"]["entities"], json!([]));
    let list = mcp(&bridge, "list_memories", json!({"namespace":"native"})).await;
    assert!(
        !list["memories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == json!(id))
    );
    use crate::search::VectorIndex;
    assert!(f.vector_index.read().await.get_vector(id).is_none());
    assert!(f.vector_index.read().await.get_vector(d.parent).is_some());
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    assert_eq!(bytes(&d.path), before);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .status()
            .unresolved,
        0
    );
    drop((api, bridge, query, publisher));
    let _root = f.close();
    inspect_schema(&d.path).unwrap();
    let reopened = open(&d, clock);
    assert_eq!(
        reopened.engine.get_record(id).unwrap().unwrap().phase,
        DecayPhase::Tombstone
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_maintenance_accepted_publication_failure_replays_full_receipt_then_removes_content()
{
    use crate::storage::native_maintenance::NativeDecayResult;
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let intent = session.capture_insert(scope(&d), &input(&d)).unwrap();
    let key = ctx(&session, &d, "maintenance-reader-create");
    let created = session
        .insert_prepared(&key, &intent, &generated())
        .unwrap();
    let id = inserted(&created);
    let f = Fixture::from_open_engine(root, session.engine);
    let publisher = completion(&f);
    publisher.complete().await.unwrap();
    let (api, bridge, query) = readers(&f, clock.clone());
    let policy = NativePolicyInputs::default();
    let sampled = 10_000 * 86_400_000_i64;
    let capture = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .capture_native_decay(id, d.ns, &policy, sampled)
        .unwrap();
    let context = MutationContext {
        store_id: Some(created.store_id),
        scope: MutationScope::Namespace(d.ns),
        idempotency_key: Some("maintenance-reader-summary".into()),
        expected_generation: None,
        contract_version: 2,
    };
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let work = tracker.begin().unwrap();
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(8);
    let result = publisher
        .apply_native_decay_accepted(context.clone(), capture.clone(), policy.clone(), &work)
        .await;
    let Err(StorageError::CommittedMutation { receipt, .. }) = result else {
        panic!("expected full committed receipt")
    };
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::NativePhaseMaterialized { .. }
    ));
    let generation = receipt.committed_generation;
    let (status, _) = http(
        &api,
        "GET",
        &format!("/memories/{id}?recordAccess=false"),
        Value::Null,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .effects_stamp()
            .is_err()
    );
    *f.vector_index.write().await = crate::search::FlatVectorIndex::new(4);
    let NativeDecayResult::Committed(replay) = publisher
        .apply_native_decay_accepted(context, capture, policy.clone(), &work)
        .await
        .unwrap()
    else {
        panic!("expected receipt")
    };
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(replay.committed_generation, generation);
    work.finish();
    let (status, body) = http(
        &api,
        "GET",
        &format!("/memories/{id}?recordAccess=false"),
        Value::Null,
    )
    .await;
    assert!(status.is_success(), "{body}");
    assert_eq!(body["data"]["summary"], json!("native exact summary"));
    assert!(body["data"].get("fullText").is_none_or(Value::is_null));
    for (key, expected) in [
        ("maintenance-reader-ghost", Some(DecayPhase::Ghost)),
        ("maintenance-reader-delete", None),
    ] {
        let capture = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .capture_native_decay(id, d.ns, &policy, sampled)
            .unwrap();
        let work = tracker.begin().unwrap();
        let context = MutationContext {
            store_id: Some(created.store_id),
            scope: MutationScope::Namespace(d.ns),
            idempotency_key: Some(key.into()),
            expected_generation: None,
            contract_version: 2,
        };
        publisher
            .apply_native_decay_accepted(context, capture, policy.clone(), &work)
            .await
            .unwrap();
        work.finish();
        let row = f.storage.read().unwrap().get_record(id).unwrap();
        assert_eq!(row.as_ref().map(|r| r.phase), expected);
        if let Some(row) = row {
            assert!(row.schema.as_ref().unwrap().labels.is_empty());
            assert!(row.schema.as_ref().unwrap().vector.is_some());
        }
    }
    let (status, _) = http(
        &api,
        "GET",
        &format!("/memories/{id}?recordAccess=false"),
        Value::Null,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    drop((api, bridge, query, publisher));
    let _root = f.close();
    inspect_schema(&d.path).unwrap();
    let reopened = open(&d, clock);
    assert!(reopened.engine.get_record(id).unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_decay_cancelled_waiter_keeps_worker_publication_lease() {
    use std::time::Duration;
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut s = open(&d, clock);
    let intent = s.capture_insert(scope(&d), &input(&d)).unwrap();
    let key = ctx(&s, &d, "cancel-create");
    let created = s.insert_prepared(&key, &intent, &generated()).unwrap();
    let id = inserted(&created);
    let context = ctx(&s, &d, "cancel-decay");
    let policy = NativePolicyInputs::default();
    let capture = s
        .metadata()
        .capture_native_decay(id, d.ns, &policy, 10_000 * 86_400_000)
        .unwrap();
    let f = Fixture::from_open_engine(root, s.engine);
    let publisher = completion(&f);
    publisher.complete().await.unwrap();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let accepted = tracker.begin().unwrap();
    let gate = crate::derived_completion::publication_gate(&f.storage)
        .await
        .unwrap();
    let graph_block = f.graph.clone().write_owned().await;
    let task = tokio::spawn(async move {
        publisher
            .apply_native_decay_accepted(context, capture, policy, &accepted)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tracker.status().pending == 2 && gate.try_write().is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let storage = f.storage.clone();
    let (held_tx, held_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        let _guard = storage.write().unwrap();
        held_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    held_rx.await.unwrap();
    drop(graph_block);
    tokio::time::timeout(Duration::from_secs(5), async {
        while tracker.status().pending != 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let publication_held = gate.try_write().is_err();
    let graph_held = f.graph.try_write().is_err();
    // Always unblock the actual worker before assertions/fixture cleanup, even RED.
    release_tx.send(()).unwrap();
    blocker.await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while tracker.status().pending > 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let lease = tokio::time::timeout(Duration::from_secs(5), gate.write())
        .await
        .unwrap();
    drop(lease);
    assert!(graph_held, "worker graph guard was not retained");
    assert!(
        tracker.status().unresolved >= 1,
        "cancelled waiter must remain unresolved"
    );
    assert!(
        publication_held,
        "cancelled waiter released publication lease before blocked worker completed"
    );
}
