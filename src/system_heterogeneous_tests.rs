//! Real BEFORE diagnostics; no production gate or publisher changes.
use super::*;
use crate::embedding::{EmbeddingError, EmbeddingProvider};
use crate::model::schema::{EmbeddingProfile, ProviderKind};
use crate::model::{NamespaceConfig, NamespaceId};
use crate::storage::TextRef;
use crate::storage::control::{GenerationRef, MutationContext, MutationScope};
use crate::storage::insertion::InsertMemory;
use crate::storage::offline_snapshot::OfflineSnapshot;
use crate::storage::schema_migration::{
    MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema,
};
use crate::storage::vector_certification::{ReembedVectors, reembed_fresh};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

struct Provider {
    descriptor: EmbeddingProfile,
    inputs: Mutex<Vec<String>>,
}
#[async_trait::async_trait]
impl EmbeddingProvider for Provider {
    fn profile(&self) -> Option<EmbeddingProfile> {
        Some(self.descriptor.clone())
    }
    fn dimensions(&self) -> usize {
        self.descriptor.dimension as usize
    }
    fn model_name(&self) -> &str {
        &self.descriptor.model_id
    }
    async fn embed(&self, text: &str) -> std::result::Result<Vec<f32>, EmbeddingError> {
        self.inputs.lock().unwrap().push(text.into());
        let mut result = vec![0.; self.dimensions()];
        result[1] = 1.;
        Ok(result)
    }
}

async fn fixture(root: &Path) -> (PathBuf, GenerationRef) {
    let legacy = root.join("legacy");
    let mut engine = RedbStorageEngine::open(&legacy).unwrap();
    let mut known = vec![];
    for (name, dimension, model) in [
        ("default", 4, Some("heterogeneous-default")),
        ("wide", 5, Some("heterogeneous-wide")),
        ("other-model", 4, Some("heterogeneous-other")),
        ("unknown", 4, None),
        ("old-bound", 4, Some("heterogeneous-default")),
    ] {
        let mut config = NamespaceConfig::default_namespace(1000);
        config.name = name.into();
        config.embedding_dim = dimension;
        config.decay_rate_multiplier = Some(1.);
        let ns = engine.create_namespace(&config).unwrap();
        let mut vector = vec![0.; dimension as usize];
        vector[0] = 1.;
        engine
            .insert_memory_with_receipt(
                &MutationContext::unkeyed(ns),
                &InsertMemory {
                    id: None,
                    summary: format!("retained summary {name}"),
                    full_text: Some(format!("retained body {name}")),
                    tags: vec![],
                    initial_stability: Some(3.),
                    created_at: Some(1000),
                    embedding: Some(vector),
                    parent_id: None,
                    supersedes: None,
                },
                &[],
                1000,
            )
            .unwrap();
        if let Some(model) = model {
            known.push(ProfileAttestationInput {
                namespace_id: ns,
                namespace_name: name.into(),
                descriptor: crate::embedding::runtime::native_profile(
                    ProviderKind::Ollama,
                    model,
                    dimension as usize,
                ),
            });
        }
    }
    engine.sync().unwrap();
    drop(engine);
    let mut source = root.join("schema");
    migrate_schema(
        &legacy,
        &source,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&legacy).unwrap().control.generation_ref(),
            attestations: known.clone(),
        },
    )
    .unwrap();
    for (index, attestation) in known.iter().take(3).enumerate() {
        let snapshot = OfflineSnapshot::open(&source).unwrap();
        let at = snapshot.control().generation_ref();
        drop(snapshot);
        let request = ReembedVectors {
            source: at,
            namespace_id: attestation.namespace_id,
            namespace_name: attestation.namespace_name.clone(),
            profile_id: attestation.descriptor.profile_id().unwrap(),
            context: MutationContext {
                store_id: Some(at.store_id),
                expected_generation: Some(at.generation),
                scope: MutationScope::Namespace(attestation.namespace_id),
                contract_version: 1,
                idempotency_key: Some(format!("heterogeneous-remedy-{index}")),
            },
        };
        let provider = Provider {
            descriptor: attestation.descriptor.clone(),
            inputs: Mutex::new(vec![]),
        };
        let destination = root.join(format!("certified-{index}"));
        let receipt = reembed_fresh(&source, &destination, &request, &provider)
            .await
            .unwrap();
        assert_eq!(receipt.committed_generation, at.generation.next().unwrap());
        assert_eq!(receipt.store_id, at.store_id);
        let inputs = provider.inputs.lock().unwrap();
        assert_eq!(inputs.len(), 1);
        assert!(inputs[0].contains(&format!("retained body {}", attestation.namespace_name)));
        source = destination;
    }
    let snapshot = OfflineSnapshot::open(&source).unwrap();
    assert_eq!(snapshot.namespaces().len(), 5);
    let mut rows = 0;
    snapshot
        .for_each_record(|row| {
            rows += 1;
            let ns = snapshot
                .namespaces()
                .iter()
                .find(|ns| ns.id.get() == row.record.namespace_id)
                .unwrap();
            let vector = row.embedding.as_ref().unwrap();
            assert_eq!(vector.len(), ns.embedding_dim as usize);
            let certified = !matches!(ns.name.as_str(), "unknown" | "old-bound");
            assert_eq!(
                ns.schema.as_ref().unwrap().vector_certification.is_some(),
                certified
            );
            assert_eq!(vector[usize::from(certified)].to_bits(), 1f32.to_bits());
            Ok(())
        })
        .unwrap();
    assert_eq!(rows, 5);
    let at = snapshot.control().generation_ref();
    eprintln!("VALIDATED_REAL_FIXTURE namespaces=5 rows=5 certified=3 widths=4,5 at={at:?}");
    (source, at)
}

fn retained(engine: &RedbStorageEngine) -> BTreeMap<String, Vec<u8>> {
    let mut bytes = BTreeMap::new();
    for ns in engine.list_namespaces().unwrap() {
        bytes.insert(
            format!("ns/{}", ns.id.get()),
            crate::storage::control::encode(&ns).unwrap(),
        );
    }
    for (id, record) in engine.scan_all().unwrap() {
        bytes.insert(format!("record/{id}"), record.to_bytes());
        let vector = engine
            .get_vector(NamespaceId::new(record.namespace_id), record.vector_slot)
            .unwrap()
            .unwrap();
        bytes.insert(
            format!("vector/{id}"),
            vector
                .iter()
                .flat_map(|v| v.to_bits().to_le_bytes())
                .collect(),
        );
        let text = engine
            .get_text(TextRef {
                file_offset: record.text_offset,
                length: record.text_length,
            })
            .unwrap()
            .unwrap();
        bytes.insert(format!("text/{id}"), text.into_bytes());
    }
    bytes
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heterogeneous_startup_preserves_namespace_bindings() {
    let root = tempfile::tempdir().unwrap();
    let (path, at) = fixture(root.path()).await;
    let mut config = RecalldConfig::default();
    config.storage.data_dir = path.to_str().unwrap().into();
    config.embedding.provider = crate::config::EmbeddingProvider::Ollama;
    config.embedding.model_name = "heterogeneous-default".into();
    config.embedding.dimensions = 4;
    config.embedding.base_url = "http://127.0.0.1:1".into();
    config.embedding.document_prefix.clear();
    config.embedding.query_prefix.clear();
    config.decay.disable_sweep = true;
    config.rif.enabled = false;
    config.graph.autolink_enabled = false;
    let engine = RedbStorageEngine::open_native(&path).unwrap();
    let before = retained(&engine);
    let result = Recalld::with_owned_storage(
        config,
        Arc::new(crate::time::ManualClock::new(3000)),
        engine,
    )
    .await;
    let system = result.unwrap();
    system.shutdown().await.unwrap();
    let engine = RedbStorageEngine::open_native(&path).unwrap();
    assert_eq!(engine.control_state().unwrap().generation_ref(), at);
    assert_eq!(retained(&engine), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heterogeneous_independent_publisher_covers_all_namespaces() {
    let root = tempfile::tempdir().unwrap();
    let (path, at) = fixture(root.path()).await;
    let engine = RedbStorageEngine::open_native(&path).unwrap();
    let before = retained(&engine);
    let vector_owner =
        crate::search::VectorIndexOwner::from_namespaces(&engine.list_namespaces().unwrap())
            .unwrap();
    let fts_path = root.path().join("derived-fts");
    std::fs::create_dir(&fts_path).unwrap();
    let owner = crate::derived_completion::DerivedCompletion {
        storage: Arc::new(std::sync::RwLock::new(engine)),
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
        vectors: Arc::new(tokio::sync::RwLock::new(vector_owner)),
        fts: Arc::new(tokio::sync::Mutex::new(
            crate::search::FtsIndex::new(&fts_path).unwrap(),
        )),
        entities: Arc::new(tokio::sync::RwLock::new(crate::search::EntityIndex::new())),
    };
    let result = owner.rebuild_all().await;
    eprintln!("ACTUAL_INDEPENDENT_REBUILD_RESULT {result:?}");
    let engine = owner.storage.read().unwrap();
    assert_eq!(engine.control_state().unwrap().generation_ref(), at);
    assert_eq!(retained(&engine), before);
    let publication = engine.meta_store().publication_state().unwrap();
    eprintln!("ACTUAL_PUBLICATION_STATE {publication:?}");
    assert!(publication.matches_canonical());
    assert!(
        result.is_ok(),
        "valid heterogeneous publisher failed: {result:?}"
    );
    let rows = engine.scan_all().unwrap();
    let wide = engine.get_namespace_by_name("wide").unwrap().unwrap();
    drop(engine);
    {
        let vectors = owner.vectors.read().await;
        assert_eq!(vectors.len(), 5);
        for (id, _) in &rows {
            let expected = before.get(&format!("vector/{id}")).unwrap();
            let actual: Vec<u8> = vectors
                .get_vector(*id)
                .unwrap()
                .iter()
                .flat_map(|v| v.to_bits().to_le_bytes())
                .collect();
            assert_eq!(&actual, expected);
        }
    }
    // The real native purge removes one width partition, then publishes all others.
    let work = owner.accept_work(None).unwrap();
    let progress = owner
        .purge_native_accepted(MutationContext::unkeyed(wide.id), true, &work)
        .await
        .unwrap();
    owner.finish_known_work(work);
    assert_eq!(progress.stage, crate::storage::purge::PurgeStage::Complete);
    assert_eq!(progress.removed, 1);
    assert_eq!(owner.vectors.read().await.len(), 4);
    {
        let engine = owner.storage.read().unwrap();
        let publication = engine.meta_store().publication_state().unwrap();
        assert_eq!(
            publication.canonical,
            engine.control_state().unwrap().generation_ref()
        );
        assert!(publication.matches_canonical());
        assert!(engine.get_namespace(wide.id).unwrap().is_none());
    }
    owner.rebuild_all().await.unwrap();
    assert_eq!(owner.vectors.read().await.len(), 4);
}

#[tokio::test]
async fn heterogeneous_corrupt_referenced_file_rejects_without_source_repair() {
    let root = tempfile::tempdir().unwrap();
    let (path, _) = fixture(root.path()).await;
    let vector_file = path.join("wide/vectors.dat");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&vector_file)
        .unwrap()
        .set_len(32)
        .unwrap();
    let meta = std::fs::read(path.join("meta.db")).unwrap();
    let corrupt = std::fs::read(&vector_file).unwrap();
    assert!(RedbStorageEngine::open_native(&path).is_err());
    assert_eq!(std::fs::read(path.join("meta.db")).unwrap(), meta);
    assert_eq!(std::fs::read(vector_file).unwrap(), corrupt);
}
