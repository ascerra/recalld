//! Actual evaluator consumer for serialized synthetic MCP episodes. No engine owner.
#[path = "support/source_scoring.rs"]
mod source_scoring;
use anyhow::{Context, Result, ensure};
use clap::Parser;
use recalld::embedding::{EmbeddingProvider, OpenAIProvider, PrefixedProvider};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    profile: bool,
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long)]
    out: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Episode {
    profile: String,
    environment_hash: String,
    source_ids: Vec<String>,
    mappings: BTreeMap<String, Vec<String>>,
    relevant: BTreeMap<String, Vec<String>>,
    queries: Vec<Query>,
    source_events: String,
    source_events_sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Query {
    query_id: String,
    memory_ids: Vec<String>,
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn score(input: Episode) -> Result<Value> {
    ensure!(
        input.profile == "synthetic-unique-source-v1",
        "unsupported scorer"
    );
    ensure!(
        input.environment_hash.len() == 64
            && input
                .environment_hash
                .bytes()
                .all(|c| c.is_ascii_hexdigit()),
        "invalid environment hash"
    );
    ensure!(
        hash(input.source_events.as_bytes()) == input.source_events_sha256,
        "source event hash mismatch"
    );
    let ordinals: BTreeMap<_, _> = input
        .source_ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s, i))
        .collect();
    ensure!(
        ordinals.len() == input.source_ids.len(),
        "duplicate source identity"
    );
    let resolve = |ids: &[String]| -> Result<Vec<usize>> {
        ids.iter()
            .map(|s| ordinals.get(s).copied().context("unmapped source"))
            .collect()
    };
    let mut retained = Vec::new();
    for (memory, sources) in &input.mappings {
        ensure!(
            !uuid::Uuid::parse_str(memory)?.is_nil(),
            "invalid public memory UUID"
        );
        retained.extend(resolve(sources)?);
    }
    for labels in input.relevant.values() {
        resolve(labels)?;
    }
    let mut metrics = Vec::new();
    let mut trace = Vec::new();
    for query in input.queries {
        let retrieved = query
            .memory_ids
            .iter()
            .map(|id| {
                resolve(
                    input
                        .mappings
                        .get(id)
                        .context("returned UUID has no accepted mapping")?,
                )
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let relevant = input
            .relevant
            .get(&query.query_id)
            .map(|s| resolve(s))
            .transpose()?;
        let metric = source_scoring::evidence_metrics(relevant.as_deref(), &retained, &retrieved);
        trace.push(json!({"queryId":query.query_id,"metrics":metric,"retrievedSourceIds":retrieved.iter().map(|i| &input.source_ids[*i]).collect::<Vec<_>>()}));
        metrics.push(metric);
    }
    Ok(
        json!({"environmentHash":input.environment_hash,"profile":input.profile,
        "scoring":source_scoring::scoring_summary(&metrics),
        "turnCoverage":source_scoring::turn_coverage(input.source_events.as_bytes(), input.source_ids.len())?,
        "trace":trace,"semanticQualityClaim":false}),
    )
}
fn main() -> Result<()> {
    let args = Args::parse();
    if args.profile {
        ensure!(
            args.input.is_none() && args.out.is_none(),
            "profile mode does not read evaluator files"
        );
        // Read actual provider configuration identity without invoking the provider.
        let provider = OpenAIProvider::new(
            "synthetic-only".into(),
            "a6-public-synthetic-v1".into(),
            Some(4),
        );
        let provider = PrefixedProvider::new(
            Box::new(provider),
            "a6-document:".into(),
            "a6-query:".into(),
        );
        let profile = provider
            .profile()
            .context("configuration identity unavailable")?;
        println!(
            "{}",
            json!({"activeEmbedding":{"identity":"configuration","dimension":4,"profileId":profile.profile_id()?},
                "sourceCommit":option_env!("RECALLD_A6_SOURCE_COMMIT"),"sourceTreeSha256":option_env!("RECALLD_A6_SOURCE_TREE_SHA256"),
                "containerImageId":option_env!("RECALLD_A6_IMAGE_ID")})
        );
        return Ok(());
    }
    let input = fs::read(args.input.context("--input required")?)?;
    let result = score(serde_json::from_slice(&input)?)?;
    let out = args.out.context("--out required")?;
    // Preserve a prior result instead of overwriting it on retry.
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)?;
    file.write_all(&serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}
