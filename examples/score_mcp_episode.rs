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
    let mut accepted = std::collections::BTreeSet::new();
    for line in input.source_events.lines() {
        let event: Value = serde_json::from_str(line)?;
        let source = event["sourceId"].as_str().context("missing event source")?;
        ensure!(
            ordinals.contains_key(&source.to_owned()),
            "unknown event source"
        );
        if event["status"] == "accepted" {
            accepted.insert((
                event["memoryId"]
                    .as_str()
                    .context("accepted event missing public UUID")?
                    .to_owned(),
                source.to_owned(),
            ));
        }
    }
    let mapped: std::collections::BTreeSet<_> = input
        .mappings
        .iter()
        .flat_map(|(id, sources)| {
            sources
                .iter()
                .map(move |source| (id.clone(), source.clone()))
        })
        .collect();
    ensure!(
        accepted == mapped,
        "accepted events and source mappings differ"
    );
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
    let query_ids: std::collections::BTreeSet<_> =
        input.queries.iter().map(|q| q.query_id.as_str()).collect();
    ensure!(
        query_ids.len() == input.queries.len()
            && input
                .relevant
                .keys()
                .all(|q| query_ids.contains(q.as_str())),
        "duplicate/unknown evaluator query"
    );
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
            json!({"activeEmbedding":{"identity":"configuration","dimension":4,"profileId":profile.profile_id().map_err(anyhow::Error::msg)?},
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

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Episode {
        let events = ["seen", "attempted", "accepted"].iter().map(|status| json!({"sourceId":"turn-0","status":status,"reason":"synthetic-test","memoryId":"00000000-0000-0000-0000-000000000001"}).to_string()).collect::<Vec<_>>().join("\n");
        serde_json::from_value(json!({"profile":"synthetic-unique-source-v1","environmentHash":"a".repeat(64),
            "sourceIds":["turn-0"],"mappings":{"00000000-0000-0000-0000-000000000001":["turn-0"]},
            "relevant":{"q0":["turn-0"]},"queries":[{"queryId":"q0","memoryIds":["00000000-0000-0000-0000-000000000001"]}],
            "sourceEvents":events,"sourceEventsSha256":hash(events.as_bytes())})).unwrap()
    }
    #[test]
    fn actual_episode_projection_requires_mapped_public_ids_and_known_queries() {
        assert_eq!(
            score(input()).unwrap()["scoring"]["means"]["retrievalRecallAt5"],
            1.0
        );
        let mut unknown = input();
        unknown.queries[0].memory_ids[0] = "unmapped".into();
        assert!(score(unknown).is_err());
        let mut duplicate = input();
        duplicate.queries.push(Query {
            query_id: "q0".into(),
            memory_ids: vec![],
        });
        assert!(score(duplicate).is_err());
        let mut foreign = input();
        foreign.relevant.insert("foreign".into(), vec![]);
        assert!(score(foreign).is_err());
    }
}
