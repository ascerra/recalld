//! Real legacy-owner reads plus explicitly labeled typed serializer specimens.
use crate::mcp::bridge::*;
use crate::mcp::protocol::*;
use crate::mcp::server::{
    McpServer,
    tests::{initialize_request, modern_request, plain_request},
};
use crate::model::{MemoryId, NamespaceId};
use crate::storage::StorageEngine as _;
use crate::test_support::{Fixture, MemorySpec, StubBridge};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Provider(AtomicUsize);

#[async_trait::async_trait]
impl crate::embedding::EmbeddingProvider for Provider {
    async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![1., 0., 0., 0.])
    }
    fn dimensions(&self) -> usize {
        4
    }
    fn model_name(&self) -> &str {
        "schema-fixture"
    }
}

fn bridge(f: &Fixture, provider: Arc<Provider>) -> McpBridge {
    use crate::search::adapters::*;
    let engine = crate::search::QueryEngine::new(
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
        Arc::new(StorageAccessRecorder::new(
            f.storage.clone(),
            f.cache.clone(),
        )),
    );
    let mut b = StubBridge::default().into_bridge();
    b.storage = Arc::new(f.storage_adapter());
    b.namespaces = Arc::new(f.namespace_adapter());
    b.search = Arc::new(crate::mcp::bridge_adapters::McpSearchAdapter::new(
        Arc::new(engine),
        provider,
        f.storage.clone(),
        f.graph.clone(),
        chrono_tz::UTC,
    ));
    b
}

fn record(label: &str, tool: &str, schema: &Value, value: &Value, provenance: &str) {
    if let Some(dir) = std::env::var_os("RECALLD_LIVE_READ_SCHEMA_FIXTURES") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{label}-{tool}.json")),
            serde_json::to_vec(
                &json!({"tool":tool,"schema":schema,"value":value,"fixtureProvenance":provenance}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}

async fn catalog(server: &mut McpServer, modern: bool) -> Value {
    if !modern {
        assert!(
            server
                .handle_message(initialize_request(1, LEGACY_PROTOCOL_VERSION))
                .await
                .unwrap()
                .error
                .is_none()
        );
    }
    let msg = if modern {
        modern_request(2, "tools/list", json!({}))
    } else {
        plain_request(2, "tools/list", Some(json!({})))
    };
    server.handle_message(msg).await.unwrap().result.unwrap()
}

async fn call(server: &mut McpServer, modern: bool, tool: &str, args: Value) -> Value {
    let params = json!({"name":tool,"arguments":args});
    let msg = if modern {
        modern_request(3, "tools/call", params)
    } else {
        plain_request(3, "tools/call", Some(params))
    };
    let result = server.handle_message(msg).await.unwrap().result.unwrap();
    assert_ne!(result["isError"], true, "{tool}: {result}");
    let value: Value =
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    if modern && value.is_object() {
        assert_eq!(result["structuredContent"], value);
    } else {
        assert!(result.get("structuredContent").is_none());
    }
    value
}

fn schema(catalog: &Value, name: &str) -> Value {
    catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
        .unwrap()["outputSchema"]
        .clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_read_schemas_actual_readers_eras_counts_and_false_access() {
    let f = Fixture::new();
    let other = f.create_namespace("empty");
    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(
            f.insert_memory_with(MemorySpec {
                summary: "alpha",
                tags: &["entity/alice", "topic/tools", "emotion/calm"],
                embedding: Some(&[1., 0., 0., 0.]),
                is_permastore: true,
                ..MemorySpec::new(NamespaceId::new(1))
            })
            .await,
        );
    }
    let doomed = f.insert_memory().await;
    f.tombstone(doomed);
    f.completion().rebuild_all().await.unwrap();
    // Real graph-view edges for the legacy reader; this is not native publication proof.
    for (a, b) in [(0, 1), (1, 2), (0, 2)] {
        f.graph
            .write()
            .await
            .add_edge(
                ids[a],
                ids[b],
                crate::model::EdgeType::Associative,
                1.,
                false,
            )
            .unwrap();
    }
    let provider = Arc::new(Provider(AtomicUsize::new(0)));
    let b = Arc::new(bridge(&f, provider.clone()));
    ids.push(doomed);
    let before = f.storage.read().unwrap().control_state().unwrap();
    let records: Vec<_> = ids
        .iter()
        .map(|id| f.record(*id).unwrap().to_bytes())
        .collect();
    let mut schemas = std::collections::BTreeMap::new();
    for modern in [true, false] {
        let mut server = McpServer::new(b.clone());
        let tools = catalog(&mut server, modern).await;
        for name in ["list_memories", "recall_memories", "find_similar_memories"] {
            let s = schema(&tools, name);
            if modern {
                assert!(s.is_object());
                schemas.insert(name, s);
            } else {
                assert!(s.is_null());
            }
        }
        let similar = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "find_similar_memories")
            .unwrap();
        assert_eq!(similar["annotations"]["readOnlyHint"], false);
        assert!(
            similar["inputSchema"]["properties"]["sameNamespace"]["description"]
                .as_str()
                .unwrap()
                .contains("matching known embedding profile")
        );
        let n = provider.0.load(Ordering::SeqCst);
        for (label, args, total, len) in [
            ("first", json!({"limit":1}), 3, 1),
            ("offset", json!({"offset":1,"limit":1}), 3, 1),
            ("past", json!({"offset":3}), 3, 0),
            (
                "max",
                json!({"limit":crate::model::constants::MAX_LIST_LIMIT}),
                3,
                3,
            ),
            ("empty", json!({"namespace":"empty"}), 0, 0),
        ] {
            let value = call(&mut server, modern, "list_memories", args).await;
            assert_eq!(value["total"], total);
            assert_eq!(value["memories"].as_array().unwrap().len(), len);
            assert!(
                value["memories"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r["id"] != doomed.to_string() && r.get("labels").is_none())
            );
            record(
                &format!("reader-{modern}-{label}"),
                "list_memories",
                &schemas["list_memories"],
                &value,
                "actual legacy owner/storage reader",
            );
        }
        let namespaces = call(&mut server, modern, "list_namespaces", json!({})).await;
        assert!(
            namespaces
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["name"] == "empty")
        );
        assert!(schema(&tools, "list_namespaces").is_null());
        assert_eq!(
            provider.0.load(Ordering::SeqCst),
            n,
            "listing must not invoke provider"
        );
        for (label, args) in [
            (
                "default",
                json!({"query":"alpha","limit":1,"recordAccess":false}),
            ),
            (
                "full",
                json!({"query":"alpha","limit":1,"compact":false,"recordAccess":false}),
            ),
            (
                "full-all",
                json!({"query":"alpha","limit":3,"compact":false,"recordAccess":false}),
            ),
            (
                "empty-compact",
                json!({"query":"alpha","namespace":"empty","recordAccess":false}),
            ),
            (
                "empty-full",
                json!({"query":"alpha","namespace":"empty","compact":false,"recordAccess":false}),
            ),
        ] {
            let value = call(&mut server, modern, "recall_memories", args).await;
            if label.starts_with("empty") {
                assert_eq!(value, json!({"memories":[],"count":0}));
            } else {
                assert!(!value["memories"].as_array().unwrap().is_empty());
                assert_eq!(
                    value["count"].as_u64().unwrap() as usize,
                    value["memories"].as_array().unwrap().len()
                );
                if label == "default" {
                    assert!(value["memories"][0].get("score").is_none());
                }
                if label == "full" {
                    assert!(
                        !value["graphContext"]["neighbors"]
                            .as_array()
                            .unwrap()
                            .is_empty()
                    );
                }
            }
            record(
                &format!("reader-{modern}-{label}"),
                "recall_memories",
                &schemas["recall_memories"],
                &value,
                "actual legacy owner/query adapter with fixed test provider",
            );
        }
        for (label, args) in [
            (
                "single",
                json!({"id":ids[0].to_string(),"recordAccess":false}),
            ),
            ("scan", json!({"mode":"scan"})),
            ("scan-empty", json!({"mode":"scan","namespace":"empty"})),
        ] {
            let value = call(&mut server, modern, "find_similar_memories", args).await;
            if label == "single" {
                assert_eq!(value["count"], 2);
            } else {
                assert_eq!(value["coverage"]["countsAvailable"], true);
                assert_eq!(value["coverage"]["complete"], false);
                assert_eq!(
                    value["coverage"]["eligibleCount"],
                    if label == "scan" { 3 } else { 0 }
                );
                assert_eq!(value["coverage"]["truncated"], false);
            }
            record(
                &format!("reader-{modern}-{label}"),
                "find_similar_memories",
                &schemas["find_similar_memories"],
                &value,
                "actual legacy owner/vector adapter",
            );
        }
    }
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    for (id, bytes) in ids.iter().zip(records) {
        assert_eq!(f.record(*id).unwrap().to_bytes(), bytes);
    }
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .memories_in_namespace(other)
            .unwrap()
            .len(),
        0
    );
}

// These are typed DTO/actual MCP serializer specimens, not native persistence or
// serving evidence. No schema rows are injected into a legacy store.
struct Specimen(SearchResponse);

#[async_trait::async_trait]
impl SearchPipeline for Specimen {
    async fn search(&self, _: SearchInput) -> Result<SearchResponse, BridgeError> {
        Ok(self.0.clone())
    }
    async fn find_similar(
        &self,
        _: MemoryId,
        _: usize,
        _: Option<f32>,
        _: bool,
        _: bool,
        _: Option<&str>,
    ) -> Result<Vec<SearchHit>, BridgeError> {
        Ok(self.0.hits.clone())
    }
    async fn scan_duplicates(
        &self,
        _: &str,
        _: f32,
        _: usize,
    ) -> Result<Vec<DuplicateCluster>, BridgeError> {
        Ok(vec![DuplicateCluster {
            memories: self
                .0
                .hits
                .iter()
                .map(|h| DuplicateEntry {
                    id: h.id.clone(),
                    summary: h.summary.clone(),
                })
                .collect(),
            max_similarity: 0.9,
        }])
    }
}

#[tokio::test]
async fn live_read_schemas_http_typed_label_and_old_coverage_specimens() {
    use crate::model::schema::{LabelProvenance, StructuredLabels};
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    for (label, labels) in [
        ("absent", None),
        (
            "authoritative-empty",
            Some(StructuredLabels {
                entities: vec![],
                topics: vec![],
                emotions: vec![],
                provenance: LabelProvenance::NativeV2,
            }),
        ),
        (
            "unicode-native",
            Some(StructuredLabels {
                entities: vec!["John Smith".into(), "José".into(), "Alice's".into()],
                topics: vec!["日本語".into()],
                emotions: vec!["très calme".into()],
                provenance: LabelProvenance::NativeV2,
            }),
        ),
        (
            "retained",
            Some(StructuredLabels {
                entities: vec!["alice".into()],
                topics: vec![],
                emotions: vec![],
                provenance: LabelProvenance::LegacyRetainedOnly,
            }),
        ),
    ] {
        let hit = SearchHit {
            labels: labels.clone(),
            id: MemoryId::new().to_string(),
            summary: "évidence".into(),
            full_text: Some("line\n\"quoted\"".into()),
            score: 1.25,
            namespace: "default".into(),
            tags: vec![],
            entities: vec![],
            topics: vec![],
            emotions: vec![],
            phase: "Full".into(),
            strength: 0.75,
            created_at: "1970-01-01T00:00:00Z".into(),
            last_accessed_at: "1970-01-01T00:00:00Z".into(),
            related: vec![RelatedMemory {
                id: MemoryId::new().to_string(),
                edge_type: "Associative".into(),
                weight: 0.5,
            }],
        };
        let neighbor = NeighborMemory {
            labels: labels.clone(),
            id: MemoryId::new().to_string(),
            summary: "neighbor".into(),
            full_text: None,
            topics: vec!["tools".into()],
            emotions: vec!["calm".into()],
            edge_type: "Associative".into(),
            weight: 0.5,
            connected_to: hit.id.clone(),
        };
        let mut b = StubBridge::default().into_bridge();
        b.search = Arc::new(Specimen(SearchResponse {
            hits: vec![hit.clone()],
            neighbors: vec![neighbor],
        }));
        let b = Arc::new(b);
        let mut server = McpServer::new(b.clone());
        let tools = catalog(&mut server, true).await;
        let app = crate::mcp::http_transport::mcp_router(b, Default::default());
        for (mode, tool, args) in [
            (
                "compact",
                "recall_memories",
                json!({"query":"q","recordAccess":false}),
            ),
            (
                "full",
                "recall_memories",
                json!({"query":"q","compact":false,"recordAccess":false}),
            ),
            (
                "single",
                "find_similar_memories",
                json!({"id":hit.id,"recordAccess":false}),
            ),
            ("old-scan", "find_similar_memories", json!({"mode":"scan"})),
        ] {
            let request = modern_request(1, "tools/call", json!({"name":tool,"arguments":args}));
            let response = app
                .clone()
                .oneshot(
                    Request::post("/mcp")
                        .header("content-type", "application/json")
                        .header("MCP-Protocol-Version", PROTOCOL_VERSION)
                        .header("MCP-Method", "tools/call")
                        .header("MCP-Name", tool)
                        .body(Body::from(serde_json::to_vec(&request).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let response: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                    .unwrap();
            let result = &response["result"];
            assert_ne!(result["isError"], true, "{response}");
            let value: Value =
                serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(result["structuredContent"], value);
            if mode == "old-scan" {
                assert_eq!(value["coverage"]["countsAvailable"], false);
                for field in [
                    "eligibleCount",
                    "sampledCount",
                    "skippedVectorCount",
                    "truncated",
                ] {
                    assert!(value["coverage"][field].is_null());
                }
            } else if let Some(labels) = &labels {
                assert_eq!(
                    value["memories"][0]["labels"],
                    serde_json::to_value(labels).unwrap()
                );
            } else {
                assert!(value["memories"][0].get("labels").is_none());
            }
            record(
                &format!("specimen-{label}-{mode}"),
                tool,
                &schema(&tools, tool),
                &value,
                "authored typed DTO through actual HTTP MCP serializer; not native persistence/serving",
            );
        }
        let entry = ListMemoryEntry {
            labels,
            id: hit.id,
            summary: hit.summary,
            entities: vec![],
            topics: vec![],
            created_at: hit.created_at,
        };
        let result = ToolCallResult::json(&ListMemoriesResponse {
            memories: vec![entry],
            total: 1,
            offset: 0,
            limit: 1,
        })
        .unwrap();
        let result = serde_json::to_value(result).unwrap();
        let value: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(result["structuredContent"], value);
        record(
            &format!("specimen-{label}"),
            "list_memories",
            &schema(&tools, "list_memories"),
            &value,
            "authored typed list DTO through actual ToolCallResult serializer; not native persistence/serving",
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_read_schemas_actual_dense_scan_reports_sample_truncation() {
    let f = Fixture::new();
    for _ in 0..201 {
        f.insert_memory_with(MemorySpec {
            embedding: Some(&[1., 0., 0., 0.]),
            is_permastore: true,
            ..MemorySpec::new(NamespaceId::new(1))
        })
        .await;
    }
    f.completion().rebuild_all().await.unwrap();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let mut server = McpServer::new(Arc::new(bridge(
        &f,
        Arc::new(Provider(AtomicUsize::new(0))),
    )));
    let tools = catalog(&mut server, true).await;
    let value = call(
        &mut server,
        true,
        "find_similar_memories",
        json!({"mode":"scan","threshold":1.0}),
    )
    .await;
    assert_eq!(
        value["coverage"],
        json!({"algorithm":"sampledNearestNeighbors","eligibleCount":201,"sampledCount":200,
        "skippedVectorCount":0,"sampleCap":200,"neighborCap":11,"complete":false,"countsAvailable":true,"truncated":true})
    );
    assert!(!value["clusters"].as_array().unwrap().is_empty());
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    record(
        "reader-dense-ties",
        "find_similar_memories",
        &schema(&tools, "find_similar_memories"),
        &value,
        "actual legacy owner/sample scan; not exhaustive pair coverage",
    );
}
