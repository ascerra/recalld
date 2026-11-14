//! Synthetic input-prefix builder for the actual native checkpoint runner.
//! Uses real legacy insertion and schema migration; no deterministic-genesis claim.
use anyhow::{Result, ensure};
use clap::Parser;
use recalld::model::{NamespaceConfig, schema::EmbeddingProfile};
use recalld::storage::{
    RedbStorageEngine, StorageEngine,
    control::MutationContext,
    insertion::{InsertMemory, inserted_id},
    schema_migration::{MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema},
};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Parser)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    root: PathBuf,
    #[arg(long)]
    report: PathBuf,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    fixture_version: u32,
    profile: EmbeddingProfile,
    attest: bool,
    sources: Vec<Source>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Source {
    source_id: String,
    namespace: String,
    summary: String,
    full_text: Option<String>,
    embedding: Vec<f32>,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let input: Input = serde_json::from_slice(&std::fs::read(&args.input)?)?;
    ensure!(
        input.fixture_version == 1 && input.sources.len() == 4,
        "unsupported synthetic fixture"
    );
    input.profile.validate().map_err(anyhow::Error::msg)?;
    ensure!(input.profile.dimension == 4, "synthetic dimension mismatch");
    ensure!(
        !args.root.exists() && !args.report.exists(),
        "fresh prefix/report required"
    );
    let parent = args
        .root
        .parent()
        .ok_or_else(|| anyhow::anyhow!("parent required"))?;
    let legacy = tempfile::Builder::new()
        .prefix("native-prefix-input-")
        .tempdir_in(parent)?;
    let mut engine = RedbStorageEngine::open(legacy.path())?;
    let mut namespaces = BTreeMap::new();
    let mut mappings = BTreeMap::new();
    let mut receipts = Vec::new();
    for source in input.sources {
        ensure!(
            !mappings.contains_key(&source.source_id),
            "duplicate source"
        );
        let namespace = if let Some(id) = namespaces.get(&source.namespace) {
            *id
        } else {
            let mut ns = NamespaceConfig::default_namespace(1_767_225_600_000);
            ns.name = source.namespace.clone();
            ns.embedding_dim = 4;
            let id = engine.create_namespace(&ns)?;
            namespaces.insert(source.namespace.clone(), id);
            id
        };
        let receipt = engine.insert_memory_with_receipt(
            &MutationContext::unkeyed(namespace),
            &InsertMemory {
                id: None,
                summary: source.summary,
                full_text: source.full_text,
                tags: vec![],
                initial_stability: None,
                created_at: Some(1_767_225_600_000),
                embedding: Some(source.embedding),
                parent_id: None,
                supersedes: None,
            },
            &[],
            1_767_225_600_000,
        )?;
        mappings.insert(source.source_id, inserted_id(&receipt)?.to_string());
        receipts.push(receipt);
    }
    ensure!(
        namespaces.contains_key("default") && namespaces.contains_key("foreign"),
        "fixture scopes required"
    );
    engine.sync()?;
    drop(engine);
    let attestations = if input.attest {
        namespaces
            .into_iter()
            .map(|(name, id)| ProfileAttestationInput {
                namespace_id: id,
                namespace_name: name,
                descriptor: input.profile.clone(),
            })
            .collect()
    } else {
        vec![]
    };
    let report = migrate_schema(
        legacy.path(),
        &args.root,
        &MigrationOptions {
            migration_now: 1_767_225_600_000,
            expected_source: inspect_schema(legacy.path())?.control.generation_ref(),
            attestations,
        },
    )?;
    let value = json!({"fixtureVersion":1,"scope":"synthetic-migrated-prefix-not-independent-genesis",
        "mappings":mappings,"sourceReceipts":receipts,"migration":report,"profile":input.profile});
    use std::io::Write;
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(args.report)?;
    out.write_all(&serde_json::to_vec_pretty(&value)?)?;
    out.sync_all()?;
    Ok(())
}
