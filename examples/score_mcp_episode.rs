//! Actual evaluator consumer for serialized synthetic MCP episodes. No engine owner.
#[path = "support/paired_scoring.rs"]
mod paired_scoring;
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
    #[arg(long, conflicts_with = "input")]
    grouped_input: Option<PathBuf>,
    #[arg(long, conflicts_with_all = ["input", "grouped_input"])]
    paired_input: Option<PathBuf>,
    #[arg(long)]
    out: Option<PathBuf>,
}
#[derive(Deserialize, serde::Serialize)]
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
#[derive(Deserialize, serde::Serialize)]
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
const GROUPED_PROFILE: &str = "synthetic-grouped-unique-source-v1";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GroupedInput {
    profile: String,
    group_map_version: u32,
    split_map_version: u32,
    environment_hash: String,
    group_map_json: String,
    group_map_sha256: String,
    split_map_json: String,
    split_map_sha256: String,
    episode_roster: Vec<GroupMember>,
    episodes: Vec<GroupEpisode>,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GroupMember {
    episode_id: String,
    group_id: String,
    split: String,
    query_ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GroupEpisode {
    episode_id: String,
    input_json: String,
    input_sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GroupMap {
    group_map_version: u32,
    dedup_method: String,
    groups: BTreeMap<String, Vec<String>>,
    episodes: BTreeMap<String, String>,
    history_sha256_by_episode: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SplitMap {
    split_map_version: u32,
    method: String,
    rounding: String,
    ratios: Vec<u32>,
    groups: BTreeMap<String, String>,
    counts: BTreeMap<String, usize>,
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn episode_environment(
    cohort: &str,
    group_hash: &str,
    split_hash: &str,
    member: &GroupMember,
) -> String {
    let context = json!({"cohortEnvironmentHash":cohort,"episodeId":member.episode_id,
        "groupId":member.group_id,"split":member.split,"queryIds":member.query_ids,
        "groupMapSha256":group_hash,"splitMapSha256":split_hash});
    hash((serde_json::to_string_pretty(&context).unwrap() + "\n").as_bytes())
}
struct GroupEvaluation {
    summary: Value,
    metrics: BTreeMap<String, Vec<Value>>,
    signatures: BTreeMap<String, Value>,
    assignments: BTreeMap<String, String>,
}
fn score_grouped(input: GroupedInput) -> Result<Value> {
    Ok(evaluate_grouped(input)?.summary)
}
fn evaluate_grouped(input: GroupedInput) -> Result<GroupEvaluation> {
    ensure!(
        input.profile == GROUPED_PROFILE
            && input.group_map_version == 1
            && input.split_map_version == 1
            && valid_hash(&input.environment_hash),
        "unsupported grouped scorer/version"
    );
    ensure!(
        hash(input.group_map_json.as_bytes()) == input.group_map_sha256
            && hash(input.split_map_json.as_bytes()) == input.split_map_sha256,
        "group/split map hash mismatch"
    );
    let groups: GroupMap = serde_json::from_str(&input.group_map_json)?;
    let splits: SplitMap = serde_json::from_str(&input.split_map_json)?;
    ensure!(
        groups.group_map_version == 1
            && groups.dedup_method == "exact-role-namespace-content-v1"
            && splits.split_map_version == 1
            && splits.method == "a6-split-v1"
            && splits.rounding == "floor-train-dev-remainder-test"
            && splits.ratios == [60, 20, 20],
        "unsupported grouping policy"
    );
    ensure!(
        !groups.groups.is_empty() && groups.groups.keys().eq(splits.groups.keys()),
        "group/split identities differ"
    );
    let mut mapped = BTreeMap::new();
    for (group, members) in &groups.groups {
        ensure!(
            valid_hash(group) && !members.is_empty(),
            "invalid group identity/members"
        );
        for member in members {
            ensure!(
                !member.is_empty() && mapped.insert(member.clone(), group.clone()).is_none(),
                "duplicate group member"
            );
        }
    }
    ensure!(
        mapped == groups.episodes
            && mapped.keys().eq(groups.history_sha256_by_episode.keys())
            && groups
                .history_sha256_by_episode
                .values()
                .all(|v| valid_hash(v)),
        "incomplete source group map"
    );
    let mut ordered: Vec<_> = groups.groups.keys().collect();
    ordered.sort_by_key(|group| (hash(format!("a6-split-v1{group}").as_bytes()), *group));
    let count = ordered.len();
    let mut expected_counts = BTreeMap::from([
        ("train".to_owned(), 0),
        ("development".to_owned(), 0),
        ("test".to_owned(), 0),
    ]);
    for (i, group) in ordered.iter().enumerate() {
        let split = if i < count * 60 / 100 {
            "train"
        } else if i < count * 60 / 100 + count * 20 / 100 {
            "development"
        } else {
            "test"
        };
        ensure!(
            splits.groups[*group] == split,
            "split assignment differs from frozen whole-group policy"
        );
        *expected_counts.get_mut(split).unwrap() += 1;
    }
    ensure!(splits.counts == expected_counts, "incorrect split counts");
    let mut roster = BTreeMap::new();
    for member in input.episode_roster {
        ensure!(
            !member.query_ids.is_empty()
                && member.query_ids.iter().all(|id| !id.is_empty())
                && member.query_ids.windows(2).all(|pair| pair[0] < pair[1]),
            "query roster must be sorted and unique"
        );
        ensure!(
            groups.episodes.get(&member.episode_id) == Some(&member.group_id)
                && splits.groups.get(&member.group_id) == Some(&member.split),
            "episode roster group/split mismatch"
        );
        ensure!(
            roster.insert(member.episode_id.clone(), member).is_none(),
            "duplicate episode roster member"
        );
    }
    ensure!(
        roster.keys().eq(groups.episodes.keys()),
        "missing episode roster member"
    );
    let mut episodes = BTreeMap::new();
    for episode in input.episodes {
        ensure!(
            roster.contains_key(&episode.episode_id),
            "unknown scored episode"
        );
        ensure!(
            hash(episode.input_json.as_bytes()) == episode.input_sha256,
            "episode score input hash mismatch"
        );
        ensure!(
            episodes
                .insert(episode.episode_id.clone(), episode)
                .is_none(),
            "duplicate scored episode"
        );
    }
    ensure!(
        episodes.keys().eq(roster.keys()),
        "missing or failed scored episode"
    );
    let mut metrics: BTreeMap<String, Vec<Value>> = groups
        .groups
        .keys()
        .map(|g| (g.clone(), Vec::new()))
        .collect();
    let mut signatures = BTreeMap::new();
    for (identity, episode) in episodes {
        let raw: Episode = serde_json::from_str(&episode.input_json)?;
        ensure!(
            raw.environment_hash
                == episode_environment(
                    &input.environment_hash,
                    &input.group_map_sha256,
                    &input.split_map_sha256,
                    &roster[&identity]
                ),
            "episode score input belongs to another cohort/member"
        );
        let mut actual_queries: Vec<_> = raw.queries.iter().map(|q| q.query_id.as_str()).collect();
        actual_queries.sort();
        ensure!(
            actual_queries
                == roster[&identity]
                    .query_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            "missing or duplicate scored query"
        );
        signatures.insert(
            identity.clone(),
            json!({"sourceIds":raw.source_ids,"relevant":raw.relevant,"queryIds":actual_queries}),
        );
        let value = score(raw)?;
        let mut rows = value["trace"]
            .as_array()
            .context("missing query metrics")?
            .clone();
        rows.sort_by(|a, b| a["queryId"].as_str().cmp(&b["queryId"].as_str()));
        metrics
            .get_mut(&roster[&identity].group_id)
            .unwrap()
            .extend(rows.into_iter().map(|v| v["metrics"].clone()));
    }
    let mut results = BTreeMap::new();
    for split in ["train", "development", "test"] {
        let mut all_metrics = Vec::new();
        let mut summaries = BTreeMap::new();
        for (group, values) in &metrics {
            if splits.groups[group] == split {
                all_metrics.extend(values.iter().cloned());
                summaries.insert(group.clone(), source_scoring::scoring_summary(values));
            }
        }
        let scored: Vec<_> = summaries
            .values()
            .filter(|v| v["scored"].as_str() != Some("0"))
            .collect();
        let mut means = BTreeMap::new();
        for key in [
            "extractionEvidenceRetention",
            "retrievalRecallAt5",
            "retrievalRecallAt10",
            "retrievalRecallAt20",
            "binarySourceNdcgAt10",
        ] {
            means.insert(
                key,
                if scored.is_empty() {
                    Value::Null
                } else {
                    json!(
                        scored
                            .iter()
                            .map(|v| v["means"][key].as_f64().unwrap())
                            .sum::<f64>()
                            / scored.len() as f64
                    )
                },
            );
        }
        let all_required = if scored.is_empty() {
            Value::Null
        } else {
            json!(
                scored
                    .iter()
                    .map(|v| v["allRequiredEvidenceRate"].as_f64().unwrap())
                    .sum::<f64>()
                    / scored.len() as f64
            )
        };
        results.insert(split, json!({"queryMicro":source_scoring::scoring_summary(&all_metrics),
            "groupMacro":{"denominatorUnit":"scored-history-group", "groups":summaries.len().to_string(),
                "scoredGroups":scored.len().to_string(), "unscoredGroups":(summaries.len()-scored.len()).to_string(),
                "means":means,"allRequiredEvidenceRate":all_required},"groups":summaries}));
    }
    Ok(GroupEvaluation {
        summary: json!({"profile":GROUPED_PROFILE,"environmentHash":input.environment_hash,
        "groupMapVersion":1,"splitMapVersion":1,"groupMapSha256":input.group_map_sha256,
        "splitMapSha256":input.split_map_sha256,"episodes":roster.len().to_string(),
        "splits":results,"bootstrap":null,"semanticQualityClaim":false}),
        metrics,
        signatures,
        assignments: splits.groups,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.profile {
        ensure!(
            args.input.is_none()
                && args.grouped_input.is_none()
                && args.paired_input.is_none()
                && args.out.is_none(),
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
    let result = if let Some(path) = args.paired_input {
        paired_scoring::score_paired(serde_json::from_slice(&fs::read(path)?)?)?
    } else if let Some(path) = args.grouped_input {
        score_grouped(serde_json::from_slice(&fs::read(path)?)?)?
    } else {
        score(serde_json::from_slice(&fs::read(
            args.input.context("--input or --grouped-input required")?,
        )?)?)?
    };
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
    fn grouped_fixture() -> Value {
        let mut groups: Vec<_> = (0..5)
            .map(|i| hash(format!("synthetic-group-{i}").as_bytes()))
            .collect();
        groups.sort_by_key(|group| hash(format!("a6-split-v1{group}").as_bytes()));
        let mut group_members = BTreeMap::new();
        let mut by_episode = BTreeMap::new();
        let mut fingerprints = BTreeMap::new();
        let mut assignments = BTreeMap::new();
        let mut roster = Vec::new();
        let mut episodes = Vec::new();
        for (i, group) in groups.iter().enumerate() {
            let identity = format!("episode-{i}");
            let split = if i < 3 {
                "train"
            } else if i == 3 {
                "development"
            } else {
                "test"
            };
            group_members.insert(group, vec![identity.clone()]);
            by_episode.insert(identity.clone(), group);
            fingerprints.insert(identity.clone(), hash(identity.as_bytes()));
            assignments.insert(group, split);
            roster.push(json!({"episodeId":identity,"groupId":group,"split":split,"queryIds":if i == 0 { vec!["q0", "q1", "q2"] } else { vec!["q0"] }}));
            let mut episode = input();
            if i == 0 {
                episode.queries = (0..3)
                    .map(|q| Query {
                        query_id: format!("q{q}"),
                        memory_ids: vec!["00000000-0000-0000-0000-000000000001".into()],
                    })
                    .collect();
                episode.relevant = (0..3)
                    .map(|q| (format!("q{q}"), vec!["turn-0".into()]))
                    .collect();
            } else if i == 1 {
                episode.queries[0].memory_ids.clear();
            } else if i == 3 {
                episode.relevant.insert("q0".into(), vec![]);
            } else {
                episode.relevant.clear();
            }
            let raw = serde_json::to_string(&episode).unwrap();
            episodes.push(
                json!({"episodeId":identity,"inputSha256":hash(raw.as_bytes()),"inputJson":raw}),
            );
        }
        let group_map = json!({"groupMapVersion":1,"dedupMethod":"exact-role-namespace-content-v1",
            "groups":group_members,"episodes":by_episode,"historySha256ByEpisode":fingerprints})
        .to_string();
        let split_map = json!({"splitMapVersion":1,"method":"a6-split-v1","rounding":"floor-train-dev-remainder-test",
            "ratios":[60,20,20],"groups":assignments,"counts":{"train":3,"development":1,"test":1}}).to_string();
        for (value, member) in episodes.iter_mut().zip(&roster) {
            let member: GroupMember = serde_json::from_value(member.clone()).unwrap();
            let mut raw: Value =
                serde_json::from_str(value["inputJson"].as_str().unwrap()).unwrap();
            raw["environmentHash"] = episode_environment(
                &"a".repeat(64),
                &hash(group_map.as_bytes()),
                &hash(split_map.as_bytes()),
                &member,
            )
            .into();
            let text = raw.to_string();
            value["inputSha256"] = hash(text.as_bytes()).into();
            value["inputJson"] = text.into();
        }
        json!({"profile":GROUPED_PROFILE,"groupMapVersion":1,"splitMapVersion":1,"environmentHash":"a".repeat(64),
            "groupMapSha256":hash(group_map.as_bytes()),"groupMapJson":group_map,
            "splitMapSha256":hash(split_map.as_bytes()),"splitMapJson":split_map,
            "episodeRoster":roster,"episodes":episodes})
    }
    #[test]
    fn grouped_actual_scorer_distinguishes_query_micro_group_macro_and_unscored() {
        let fixture = grouped_fixture();
        let result = score_grouped(serde_json::from_value(fixture.clone()).unwrap()).unwrap();
        let train = &result["splits"]["train"];
        assert_eq!(train["queryMicro"]["means"]["retrievalRecallAt5"], 0.75);
        assert_eq!(train["groupMacro"]["means"]["retrievalRecallAt5"], 0.5);
        assert_eq!(train["groupMacro"]["allRequiredEvidenceRate"], 0.5);
        assert_eq!(train["groupMacro"]["groups"], "3");
        assert_eq!(train["groupMacro"]["scoredGroups"], "2");
        assert_eq!(train["groupMacro"]["unscoredGroups"], "1");
        assert_eq!(train["queryMicro"]["scored"], "4");
        assert_eq!(train["queryMicro"]["unknownAnnotations"], "1");
        assert!(
            result["splits"]["development"]["groupMacro"]["means"]["retrievalRecallAt5"].is_null()
        );
        assert_eq!(
            result["splits"]["development"]["queryMicro"]["zeroLabelUnscored"],
            "1"
        );
        assert!(result["splits"]["test"]["groupMacro"]["allRequiredEvidenceRate"].is_null());
        let mut reversed = fixture;
        reversed["episodeRoster"].as_array_mut().unwrap().reverse();
        reversed["episodes"].as_array_mut().unwrap().reverse();
        for episode in reversed["episodes"].as_array_mut().unwrap() {
            let mut raw: Value =
                serde_json::from_str(episode["inputJson"].as_str().unwrap()).unwrap();
            raw["queries"].as_array_mut().unwrap().reverse();
            let text = raw.to_string();
            episode["inputSha256"] = hash(text.as_bytes()).into();
            episode["inputJson"] = text.into();
        }
        assert_eq!(
            score_grouped(serde_json::from_value(reversed).unwrap()).unwrap(),
            result
        );
    }
    #[test]
    fn grouped_scorer_rejects_missing_duplicate_unbound_and_rehashed_split_inputs() {
        let fixture = grouped_fixture();
        let mut missing = fixture.clone();
        missing["episodes"].as_array_mut().unwrap().pop();
        assert!(
            score_grouped(serde_json::from_value(missing).unwrap())
                .unwrap_err()
                .to_string()
                .contains("missing or failed")
        );
        let mut duplicate = fixture.clone();
        let first = duplicate["episodes"][0].clone();
        duplicate["episodes"].as_array_mut().unwrap().push(first);
        assert!(score_grouped(serde_json::from_value(duplicate).unwrap()).is_err());
        let mut roster = fixture.clone();
        roster["episodeRoster"].as_array_mut().unwrap().pop();
        assert!(score_grouped(serde_json::from_value(roster).unwrap()).is_err());
        let mut bad_hash = fixture.clone();
        bad_hash["episodes"][0]["inputSha256"] = "0".repeat(64).into();
        assert!(score_grouped(serde_json::from_value(bad_hash).unwrap()).is_err());
        let mut changed = fixture.clone();
        let group = changed["episodeRoster"][0]["groupId"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut splits: Value =
            serde_json::from_str(changed["splitMapJson"].as_str().unwrap()).unwrap();
        splits["groups"][group] = "test".into();
        let raw = splits.to_string();
        changed["splitMapSha256"] = hash(raw.as_bytes()).into();
        changed["splitMapJson"] = raw.into();
        assert!(
            score_grouped(serde_json::from_value(changed).unwrap())
                .unwrap_err()
                .to_string()
                .contains("split assignment")
        );
        let mut swapped = fixture.clone();
        swapped["episodes"][0]["inputJson"] = fixture["episodes"][1]["inputJson"].clone();
        swapped["episodes"][0]["inputSha256"] = fixture["episodes"][1]["inputSha256"].clone();
        assert!(
            score_grouped(serde_json::from_value(swapped).unwrap())
                .unwrap_err()
                .to_string()
                .contains("another cohort/member")
        );
        let mut missing_query = fixture.clone();
        let mut raw: Value =
            serde_json::from_str(missing_query["episodes"][2]["inputJson"].as_str().unwrap())
                .unwrap();
        raw["queries"] = json!([]);
        let text = raw.to_string();
        missing_query["episodes"][2]["inputSha256"] = hash(text.as_bytes()).into();
        missing_query["episodes"][2]["inputJson"] = text.into();
        assert!(
            score_grouped(serde_json::from_value(missing_query).unwrap())
                .unwrap_err()
                .to_string()
                .contains("missing or duplicate scored query")
        );
        let mut version = fixture;
        version["groupMapVersion"] = 2.into();
        assert!(score_grouped(serde_json::from_value(version).unwrap()).is_err());
    }
}
