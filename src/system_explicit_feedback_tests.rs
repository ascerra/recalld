//! #250: under explicit feedback a recall leaves the decay clock alone, so a
//! following `reinforce_memory` grows stability from the real idle time.
use super::inherited_policy_tests::{embedding_stub, tool};
use super::*;
use crate::model::schema::LifecycleVersion;
use crate::model::{DiskRecord, MemoryId, NamespaceConfig};
use crate::storage::control::MutationContext;
use crate::storage::insertion::{InsertMemory, inserted_id};
use crate::storage::schema_migration::{MigrationOptions, inspect_schema, migrate_schema};
use crate::time::ManualClock;

const DAY: i64 = 86_400_000;
const NOW: i64 = 400 * DAY;

fn config(path: &std::path::Path, url: String) -> RecalldConfig {
    let mut config = RecalldConfig::default();
    config.storage.data_dir = path.to_str().unwrap().into();
    config.embedding.provider = crate::config::EmbeddingProvider::Ollama;
    config.embedding.model_name = "synthetic-explicit-feedback".into();
    config.embedding.dimensions = 4;
    config.embedding.base_url = url;
    config.embedding.api_key_env = "RECALLD_EXPLICIT_FEEDBACK_TEST_UNUSED_KEY".into();
    config.embedding.document_prefix.clear();
    config.embedding.query_prefix.clear();
    config.decay.disable_sweep = true;
    config.rif.enabled = false;
    config.graph.autolink_enabled = false;
    config
}

fn record(core: &Recalld, id: MemoryId) -> DiskRecord {
    core.storage()
        .read()
        .unwrap()
        .get_record(id)
        .unwrap()
        .unwrap()
}

fn anchor(record: &DiskRecord) -> i64 {
    record.schema.as_ref().unwrap().lifecycle.decay_anchor_at
}

/// Recall (recording access) after `idle`, then reinforce with quality 3.
/// Returns the record before, after the recall and after the reinforcement.
async fn recall_then_reinforce(
    core: &Recalld,
    clock: &ManualClock,
    id: MemoryId,
    query: &str,
    at: i64,
) -> [DiskRecord; 3] {
    let before = record(core, id);
    clock.advance_to(at).unwrap();
    let recalled = tool(core, "recall_memories", serde_json::json!({"query": query})).await;
    assert!(
        recalled["memories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == id.to_string().as_str()),
        "{recalled}"
    );
    let after_recall = record(core, id);
    tool(
        core,
        "reinforce_memory",
        serde_json::json!({"id": id.to_string(), "quality": 3}),
    )
    .await;
    [before, after_recall, record(core, id)]
}

fn assert_feedback_only(states: &[DiskRecord; 3], at: i64) {
    let [before, recalled, reinforced] = states;
    // The recall was recorded, but it moved neither the clock nor stability.
    assert_eq!(
        recalled.schema.as_ref().unwrap().lifecycle.retrieved_at,
        Some(at)
    );
    assert_eq!(anchor(recalled), anchor(before));
    assert_eq!(recalled.stability.to_bits(), before.stability.to_bits());
    // Reinforcement measured the real idle time, so stability grew a lot.
    assert!(
        reinforced.stability > 5. * before.stability,
        "{} -> {}",
        before.stability,
        reinforced.stability
    );
    assert_eq!(anchor(reinforced), at);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_store_uses_explicit_feedback_and_recall_then_reinforce_grows_stability() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("db");
    let (url, stub) = embedding_stub().await;
    let clock = Arc::new(ManualClock::new(NOW));
    let core = Recalld::with_clock(config(&path, url), clock.clone())
        .await
        .unwrap();
    // `create_namespace` takes the same native path; native_catalog_tests checks it.
    let namespaces = core.storage().read().unwrap().list_namespaces().unwrap();
    assert_eq!(namespaces.len(), 1);
    assert_eq!(
        namespaces[0].schema.as_ref().unwrap().lifecycle_version,
        LifecycleVersion::ExplicitFeedbackV2
    );
    let stored = tool(
        &core,
        "store_memory",
        serde_json::json!({"summary": "explicit feedback fixture", "checkDuplicates": false}),
    )
    .await;
    let id = MemoryId::from(uuid::Uuid::parse_str(stored["id"].as_str().unwrap()).unwrap());
    let at = NOW + 60 * DAY;
    let states = recall_then_reinforce(&core, &clock, id, "explicit feedback fixture", at).await;
    assert_eq!(anchor(&states[0]), NOW);
    assert_feedback_only(&states, at);
    core.shutdown().await.unwrap();
    stub.abort();
    // The creation receipt replays to the same lifecycle on reopen.
    assert!(
        inspect_schema(&path).unwrap().namespaces.iter().all(|ns| ns
            .schema
            .as_ref()
            .unwrap()
            .lifecycle_version
            == LifecycleVersion::ExplicitFeedbackV2)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lifecycle_cli_migrates_a_legacy_store_copy_and_the_copy_serves_feedback() {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join("legacy");
    let source = root.path().join("source");
    let destination = root.path().join("migrated");
    let (url, stub) = embedding_stub().await;
    let config = config(&destination, url);
    // A 1.x-style store: legacy lifecycle, no recorded inherited multiplier.
    let mut engine = RedbStorageEngine::open(&legacy).unwrap();
    let mut namespace = NamespaceConfig::default_namespace(DAY);
    namespace.embedding_dim = 4;
    let ns = engine.create_namespace(&namespace).unwrap();
    let id = inserted_id(
        &engine
            .insert_memory_with_receipt(
                &MutationContext::unkeyed(ns),
                &InsertMemory {
                    id: None,
                    summary: "legacy feedback fixture".into(),
                    full_text: Some("legacy feedback fixture text".into()),
                    tags: vec![],
                    initial_stability: None,
                    created_at: Some(DAY),
                    embedding: Some(vec![1., 0., 0., 0.]),
                    parent_id: None,
                    supersedes: None,
                },
                &[],
                DAY,
            )
            .unwrap(),
    )
    .unwrap();
    engine.sync().unwrap();
    drop(engine);
    migrate_schema(
        &legacy,
        &source,
        &MigrationOptions {
            migration_now: 2 * DAY,
            expected_source: inspect_schema(&legacy).unwrap().control.generation_ref(),
            // As the automatic 2.x upgrade does: vectors keep unknown provenance.
            attestations: vec![],
        },
    )
    .unwrap();
    let before = inspect_schema(&source).unwrap();
    let schema = before.namespaces[0].schema.as_ref().unwrap();
    assert_eq!(schema.lifecycle_version, LifecycleVersion::LegacyAccessV1);
    assert!(schema.inherited_decay.is_none());

    let names = vec!["default".to_string()];
    let preview = crate::cli::lifecycle::preview(&config, &source, &names).unwrap();
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].record_count, 1);
    std::fs::create_dir(&destination).unwrap();
    assert!(
        crate::cli::lifecycle::migrate(&source, &destination, &names, 1., &ManualClock::new(NOW))
            .is_err(),
        "an existing destination is refused"
    );
    std::fs::remove_dir(&destination).unwrap();
    crate::cli::lifecycle::migrate(&source, &destination, &names, 1., &ManualClock::new(NOW))
        .unwrap();
    // The source is untouched and no work directory is left behind.
    let after = inspect_schema(&source).unwrap();
    assert_eq!(after.control, before.control);
    assert_eq!(
        serde_json::to_value(&after.namespaces).unwrap(),
        serde_json::to_value(&before.namespaces).unwrap()
    );
    assert!(std::fs::read_dir(root.path()).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".recalld-lifecycle-")
    }));
    let migrated = inspect_schema(&destination).unwrap();
    let schema = migrated.namespaces[0].schema.as_ref().unwrap();
    assert_eq!(
        schema.lifecycle_version,
        LifecycleVersion::ExplicitFeedbackV2
    );
    assert_eq!(
        schema.inherited_decay.as_ref().unwrap().multiplier_bits,
        1f32.to_bits()
    );

    // Ordinary startup serves the migrated copy; #250 is fixed there.
    let clock = Arc::new(ManualClock::new(NOW));
    let core = Recalld::with_clock(config, clock.clone()).await.unwrap();
    let at = NOW + DAY;
    let states = recall_then_reinforce(&core, &clock, id, "legacy feedback fixture", at).await;
    assert_eq!(anchor(&states[0]), DAY);
    assert_feedback_only(&states, at);
    core.shutdown().await.unwrap();
    stub.abort();
}
