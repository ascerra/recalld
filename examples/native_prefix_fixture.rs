//! Synthetic input-prefix builder for the actual native checkpoint runner.
//! Uses real legacy insertion and schema migration; no deterministic-genesis claim.
use anyhow::{Result, ensure};
use clap::Parser;
use recalld::model::{NamespaceConfig, schema::EmbeddingProfile};
use recalld::storage::{
    RedbStorageEngine, StorageEngine,
    control::{MutationContext, MutationScope},
    inherited_policy::{AdoptInheritedDecay, adopt_inherited_decay, preview_inherited_decay},
    insertion::{InsertMemory, inserted_id},
    offline_snapshot::OfflineSnapshot,
    schema_migration::{MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema},
};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Parser)]
struct Args {
    #[arg(
        long,
        required_unless_present = "inspect_existing",
        conflicts_with = "inspect_existing"
    )]
    input: Option<PathBuf>,
    /// Read a closed fixture through the existing offline snapshot/export APIs.
    #[arg(long)]
    inspect_existing: bool,
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
    namespace_decay_multipliers: Option<BTreeMap<String, f32>>,
    inherited_decay_adoptions: Option<BTreeMap<String, f64>>,
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
    if args.inspect_existing {
        ensure!(!args.report.exists(), "fresh report required");
        return write_report(&args.report, &closed_inventory(&args.root)?);
    }
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(
        args.input.as_ref().expect("clap requires input"),
    )?)?;
    ensure!(
        raw["fixtureVersion"] != 1 || raw.get("namespaceDecayMultipliers").is_none(),
        "fixture v1 does not support namespace policy fields"
    );
    ensure!(
        raw["fixtureVersion"] == 3 || raw.get("inheritedDecayAdoptions").is_none(),
        "old fixtures do not support inherited policy adoption"
    );
    let input: Input = serde_json::from_value(raw)?;
    ensure!(
        input.sources.len() == 4
            && match input.fixture_version {
                1 =>
                    input.namespace_decay_multipliers.is_none()
                        && input.inherited_decay_adoptions.is_none(),
                2 | 3 =>
                    input.namespace_decay_multipliers.as_ref()
                        == Some(&BTreeMap::from([("foreign".to_string(), 0.0)]))
                        && (if input.fixture_version == 3 {
                            input.inherited_decay_adoptions.as_ref()
                                == Some(&BTreeMap::from([("default".to_string(), 1.0)]))
                        } else {
                            input.inherited_decay_adoptions.is_none()
                        })
                        && input
                            .sources
                            .iter()
                            .filter(|s| s.namespace == "foreign")
                            .count()
                            == 1
                        && input
                            .sources
                            .iter()
                            .filter(|s| s.namespace == "default")
                            .count()
                            == 3,
                _ => false,
            },
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
            ns.decay_rate_multiplier = input
                .namespace_decay_multipliers
                .as_ref()
                .and_then(|v| v.get(&source.namespace))
                .copied();
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
            .iter()
            .map(|(name, id)| ProfileAttestationInput {
                namespace_id: *id,
                namespace_name: name.clone(),
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
    let mut adoptions = Vec::new();
    if let Some(requested) = input.inherited_decay_adoptions {
        let before_receipts = receipt_rows(&args.root)?;
        let before = OfflineSnapshot::open(&args.root)?;
        let foreign = before
            .namespaces()
            .iter()
            .find(|ns| ns.name == "foreign")
            .unwrap()
            .clone();
        ensure!(
            foreign.decay_rate_multiplier == Some(0.0),
            "foreign override required"
        );
        drop(before);
        let clock = recalld::time::ManualClock::new(1_767_225_600_000);
        for (name, multiplier) in requested {
            let source = inspect_schema(&args.root)?.control.generation_ref();
            let namespace = namespaces[&name];
            let preview =
                preview_inherited_decay(&args.root, namespace, multiplier, source, &clock)?;
            ensure!(
                preview.prior.is_none(),
                "fixture must start with unknown inheritance"
            );
            let request = AdoptInheritedDecay {
                context: MutationContext {
                    store_id: Some(source.store_id),
                    scope: MutationScope::Namespace(namespace),
                    expected_generation: Some(source.generation),
                    idempotency_key: Some(format!("native-fixture-v3-adopt-{name}")),
                    contract_version: 1,
                },
                preview,
            };
            let receipt = adopt_inherited_decay(&args.root, &request)?;
            let after = inspect_schema(&args.root)?.control.generation_ref();
            ensure!(
                after.store_id == source.store_id
                    && after.generation == receipt.committed_generation,
                "adoption generation mismatch"
            );
            adoptions.push(json!({"request":request,"receipt":receipt,"after":after}));
        }
        let after_receipts = receipt_rows(&args.root)?;
        ensure!(
            after_receipts.len() == before_receipts.len() + adoptions.len(),
            "unexpected receipt count"
        );
        ensure!(
            before_receipts
                .iter()
                .all(|(key, value)| after_receipts.get(key) == Some(value)),
            "historical receipt bytes changed"
        );
        let after = OfflineSnapshot::open(&args.root)?;
        ensure!(
            serde_json::to_value(
                after
                    .namespaces()
                    .iter()
                    .find(|ns| ns.name == "foreign")
                    .unwrap()
            )? == serde_json::to_value(&foreign)?,
            "foreign override changed"
        );
        adoptions.push(json!({"historicalReceiptRows":before_receipts.iter().collect::<Vec<_>>(),"historicalReceiptBytesPreserved":true,"foreignNamespaceUnchanged":true}));
    }
    let mut value = json!({"fixtureVersion":input.fixture_version,"scope":"synthetic-migrated-prefix-not-independent-genesis",
        "mappings":mappings,"sourceReceipts":receipts,"migration":report,"profile":input.profile});
    if !adoptions.is_empty() {
        value["inheritedPolicyAdoptions"] = json!(adoptions);
    }
    if let Some(policy) = input.namespace_decay_multipliers {
        value["namespaceDecayMultipliers"] = json!(policy);
    }
    write_report(&args.report, &value)
}

fn write_report(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    use std::io::Write;
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    out.write_all(&serde_json::to_vec_pretty(value)?)?;
    out.sync_all()?;
    Ok(())
}

// Compare canonical values, including raw record and retained receipt bytes,
// separately from redb/SQLite open/close file bookkeeping. The export's random
// transport envelope is excluded; every ordered chunk item is retained.
fn closed_inventory(root: &std::path::Path) -> Result<serde_json::Value> {
    let scratch = tempfile::tempdir()?;
    let export = scratch.path().join("canonical.jsonl");
    let summary = recalld::storage::logical_export::export_store(root, &export)?;
    let mut items = Vec::new();
    for line in std::fs::read_to_string(export)?.lines() {
        let line: serde_json::Value = serde_json::from_str(line)?;
        if line["kind"] == "chunk" {
            items.push(line["data"]["item"].clone());
        }
    }
    ensure!(items.len() as u64 == summary.chunks, "missing export item");
    let snapshot = OfflineSnapshot::open(root)?;
    ensure!(
        snapshot.control().generation_ref() == summary.source,
        "offline inventory source changed"
    );
    let mut records = Vec::new();
    snapshot.for_each_record(|record| {
        records.push(json!({"recordBytes":record.record.to_bytes(),
            "fullText":record.full_text,
            "embeddingBits":record.embedding.map(|v| v.into_iter().map(f32::to_bits).collect::<Vec<_>>())}));
        Ok(())
    })?;
    Ok(json!({"inventoryVersion":1,"source":summary.source,
        "logicalSha256":summary.logical_sha256,"control":snapshot.control(),
        "namespaces":snapshot.namespaces(),"records":records,"items":items}))
}

// Inspect actual retained receipt key/value bytes through the existing logical
// exporter; its random transport header is not canonical fixture state.
fn receipt_rows(root: &std::path::Path) -> Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    let scratch = tempfile::tempdir()?;
    let export = scratch.path().join("receipts.jsonl");
    recalld::storage::logical_export::export_store(root, &export)?;
    let mut selected = false;
    let mut found = false;
    let mut rows = BTreeMap::new();
    for line in std::fs::read_to_string(export)?.lines() {
        let line: serde_json::Value = serde_json::from_str(line)?;
        if line["kind"] != "chunk" {
            continue;
        }
        let item = &line["data"]["item"];
        match item["kind"].as_str() {
            Some("table") => {
                selected = item["data"]["name"] == "mutation_receipts";
                found |= selected;
            }
            Some("row") if selected => {
                let key = serde_json::from_value(item["data"]["key"].clone())?;
                let value = serde_json::from_value(item["data"]["value"].clone())?;
                ensure!(
                    rows.insert(key, value).is_none(),
                    "duplicate receipt export key"
                );
            }
            Some("row") => {}
            _ => selected = false,
        }
    }
    ensure!(found, "missing retained receipt table");
    Ok(rows)
}
