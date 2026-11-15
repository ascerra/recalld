//! Actual paired-input mode of the public episode scorer, not a standalone harness.
use super::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

const PROFILE: &str = "synthetic-paired-grouped-unique-source-v1";
const METRICS: [&str; 6] = [
    "extractionEvidenceRetention",
    "retrievalRecallAt5",
    "retrievalRecallAt10",
    "retrievalRecallAt20",
    "binarySourceNdcgAt10",
    "allRequiredEvidenceRate",
];
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PairedInput {
    profile: String,
    pair_manifest_json: String,
    pair_manifest_sha256: String,
    contract_json: String,
    contract_sha256: String,
    arms: Vec<ArmInput>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArmInput {
    role: String,
    manifest_json: String,
    manifest_sha256: String,
    grouped_input_json: String,
    grouped_input_sha256: String,
    status_json: String,
    status_sha256: String,
}
fn policy() -> Value {
    json!({"uncertaintyVersion":1,"replicates":10000,"seed":7,"rng":"rand_chacha-0.3.1-ChaCha12Rng",
        "seedEncoding":"u64-le-zero-pad32","boundedIndex":"rand-0.8.6-u64-gen-range",
        "streams":{"train":0,"development":1,"test":2},"interval":"nearest-rank-2.5-97.5",
        "minimumEligibleGroups":2,"contrast":"candidate-minus-baseline",
        "population":"whole-history-groups-with-common-scored-queries"})
}
fn object(text: &str, expected_hash: &str) -> Result<Value> {
    ensure!(
        hash(text.as_bytes()) == expected_hash,
        "paired input hash mismatch"
    );
    Ok(serde_json::from_str(text)?)
}
#[derive(Clone)]
struct Group {
    count: usize,
    baseline: [f64; 6],
    candidate: [f64; 6],
}
fn paired_group(left: &[Value], right: &[Value]) -> Result<Group> {
    ensure!(left.len() == right.len(), "paired query counts differ");
    let mut group = Group {
        count: 0,
        baseline: [0.; 6],
        candidate: [0.; 6],
    };
    for (a, b) in left.iter().zip(right) {
        ensure!(
            a["annotationStatus"] == b["annotationStatus"],
            "outcome-dependent paired eligibility"
        );
        if a["annotationStatus"] != "scored" {
            continue;
        }
        group.count += 1;
        for (i, metric) in METRICS.iter().enumerate() {
            let value = |row: &Value| -> Result<f64> {
                if i == 5 {
                    Ok(
                        if row["allRequiredEvidence"]
                            .as_bool()
                            .context("missing all-required outcome")?
                        {
                            1.
                        } else {
                            0.
                        },
                    )
                } else {
                    row[*metric].as_f64().context("missing scored metric")
                }
            };
            group.baseline[i] += value(a)?;
            group.candidate[i] += value(b)?;
        }
    }
    Ok(group)
}
fn means(groups: &[Group], weights: &[u64]) -> ([f64; 6], [f64; 6], [f64; 6], [f64; 6]) {
    let (mut bm, mut cm, mut bg, mut cg) = ([0.; 6], [0.; 6], [0.; 6], [0.; 6]);
    let mut queries = 0.;
    let mut count = 0.;
    for (g, w) in groups.iter().zip(weights) {
        let w = *w as f64;
        queries += w * g.count as f64;
        count += w;
        for i in 0..6 {
            bm[i] += w * g.baseline[i];
            cm[i] += w * g.candidate[i];
            bg[i] += w * g.baseline[i] / g.count as f64;
            cg[i] += w * g.candidate[i] / g.count as f64;
        }
    }
    for i in 0..6 {
        bm[i] /= queries;
        cm[i] /= queries;
        bg[i] /= count;
        cg[i] /= count;
    }
    (bm, cm, bg, cg)
}
fn bootstrap(groups: &[Group], stream: u64) -> Value {
    let n = groups.len();
    if n == 0 {
        return json!({"status":"unscored","eligibleGroups":"0","requestedReplicates":"10000","executedReplicates":"0","point":null,"interval95":null,"drawsSha256":null,"qualityPromotionEligible":false});
    }
    let (bm, cm, bg, cg) = means(groups, &vec![1; n]);
    let mut point = BTreeMap::new();
    for (i, key) in METRICS.iter().enumerate() {
        point.insert(
            *key,
            json!({"queryMicro":{"baseline":bm[i],"candidate":cm[i],"effect":cm[i]-bm[i]},
            "groupMacro":{"baseline":bg[i],"candidate":cg[i],"effect":cg[i]-bg[i]}}),
        );
    }
    if n == 1 {
        return json!({"status":"insufficient_eligible_groups","eligibleGroups":"1","requestedReplicates":"10000","executedReplicates":"0","point":point,"interval95":null,"drawsSha256":null,"qualityPromotionEligible":false});
    }
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&7u64.to_le_bytes());
    let mut rng = ChaCha12Rng::from_seed(seed);
    rng.set_stream(stream);
    let mut draw_hash = Sha256::new();
    let mut micro: Vec<Vec<f64>> = (0..6).map(|_| Vec::with_capacity(10000)).collect();
    let mut macro_scores = micro.clone();
    for _ in 0..10000 {
        let mut weights = vec![0u64; n];
        for _ in 0..n {
            let index = rng.gen_range(0..n as u64);
            draw_hash.update(index.to_le_bytes());
            weights[index as usize] += 1;
        }
        // One whole-group draw vector, reused for both arms; never intersect outcomes.
        let (a, b, c, d) = means(groups, &weights);
        for i in 0..6 {
            micro[i].push(b[i] - a[i]);
            macro_scores[i].push(d[i] - c[i]);
        }
    }
    let interval = |values: &mut Vec<f64>| {
        values.sort_by(f64::total_cmp);
        // Nearest ranks ceil(.025*10000), ceil(.975*10000), converted to zero-based indices.
        json!({"lower":values[249],"upper":values[9749],"zeroObservedDispersion":values.first()==values.last()})
    };
    let mut intervals = BTreeMap::new();
    for (i, key) in METRICS.iter().enumerate() {
        intervals.insert(*key,json!({"queryMicro":interval(&mut micro[i]),"groupMacro":interval(&mut macro_scores[i])}));
    }
    json!({"status":"synthetic_diagnostic_only","eligibleGroups":n.to_string(),"requestedReplicates":"10000","executedReplicates":"10000",
        "point":point,"interval95":intervals,"drawsSha256":draw_hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),"qualityPromotionEligible":false})
}

pub(super) fn score_paired(input: PairedInput) -> Result<Value> {
    ensure!(input.profile == PROFILE, "unsupported paired scorer");
    let manifest = object(&input.pair_manifest_json, &input.pair_manifest_sha256)?;
    let shared = object(&input.contract_json, &input.contract_sha256)?;
    ensure!(
        manifest["track"] == "serialized-mcp-paired-grouped-synthetic-diagnostic-v1"
            && manifest["pairVersion"] == 1
            && manifest["comparisonProfile"] == PROFILE
            && manifest["uncertaintyPolicy"] == policy(),
        "unsupported paired manifest/policy"
    );
    ensure!(
        manifest["pairContract"]["sha256"] == input.contract_sha256
            && shared["pairContractVersion"] == 1
            && shared["fixture"] == "paired-public-limit-synthetic-v1"
            && shared["uncertaintyPolicy"] == policy(),
        "paired contract/policy mismatch"
    );
    ensure!(
        shared["armPolicies"]
            == json!({"baseline":{"retrievalLimit":1,"recordAccess":false,"depth":0,"namespace":"default"},
        "candidate":{"retrievalLimit":2,"recordAccess":false,"depth":0,"namespace":"default"}}),
        "unsupported arm policy"
    );
    let entries = manifest["arms"]
        .as_array()
        .context("missing manifest arm roster")?;
    ensure!(
        entries.len() == 2 && input.arms.len() == 2,
        "incomplete paired arm roster"
    );
    let mut expected = BTreeMap::new();
    for entry in entries {
        let role = entry["role"].as_str().context("missing arm role")?;
        ensure!(
            ["baseline", "candidate"].contains(&role) && expected.insert(role, entry).is_none(),
            "duplicate/unknown manifest arm"
        );
    }
    let mut evaluated = BTreeMap::new();
    for arm in input.arms {
        let declared = expected
            .get(arm.role.as_str())
            .context("unknown scored arm")?;
        ensure!(
            declared["manifest"]["sha256"] == arm.manifest_sha256
                && declared["manifest"]["file"] == format!("{}/manifest.json", arm.role),
            "arm manifest binding mismatch"
        );
        let arm_manifest = object(&arm.manifest_json, &arm.manifest_sha256)?;
        ensure!(
            arm_manifest["track"] == "serialized-mcp-paired-grouped-arm-v1"
                && arm_manifest["pairedArm"]
                    == json!({"role":arm.role,
            "retrievalLimit": if arm.role=="baseline" {1} else {2},"pairContractSha256":input.contract_sha256,"fixtureVersion":1}),
            "arm role/policy mismatch"
        );
        for key in [
            "manifestVersion",
            "sourceMappingVersion",
            "traceSchemaVersion",
            "versions",
            "capabilities",
            "sourceCommit",
            "sourceFilesSha256",
            "containerImageId",
            "limits",
            "executables",
            "activeEmbedding",
            "scoringProfile",
        ] {
            ensure!(
                !manifest[key].is_null() && arm_manifest[key] == manifest[key],
                "paired build/profile differs"
            );
        }
        for key in ["sourceRegistry", "groupMap", "splitMap"] {
            ensure!(
                arm_manifest[key]["sha256"] == shared[format!("{key}Sha256")],
                "paired map/source differs"
            );
        }
        let members = arm_manifest["episodes"]
            .as_array()
            .context("missing arm members")?;
        let shared_members = shared["episodes"]
            .as_object()
            .context("missing shared source/label roster")?;
        ensure!(
            members.len() == shared_members.len(),
            "paired source/label roster size differs"
        );
        let mut ids = std::collections::BTreeSet::new();
        for member in members {
            let id = member["episodeId"]
                .as_str()
                .context("missing episode identity")?;
            ensure!(
                shared_members.contains_key(id) && ids.insert(id),
                "unknown/duplicate paired source member"
            );
            for key in [
                "actorInputs",
                "evaluatorLabels",
                "replayCache",
                "evaluatorSchedule",
            ] {
                ensure!(
                    member["artifacts"][key]["sha256"] == shared_members[id][key],
                    "paired shared labels/query/cache/source differs"
                );
            }
        }
        let raw = object(&arm.grouped_input_json, &arm.grouped_input_sha256)?;
        let grouped: GroupedInput = serde_json::from_value(raw)?;
        ensure!(
            grouped.environment_hash == arm.manifest_sha256
                && grouped.group_map_sha256 == shared["groupMapSha256"]
                && grouped.split_map_sha256 == shared["splitMapSha256"],
            "grouped scores belong to another arm"
        );
        let status = object(&arm.status_json, &arm.status_sha256)?;
        let count = grouped.episode_roster.len() as u64;
        ensure!(
            status["status"] == "passed"
                && status["environmentHash"] == grouped.environment_hash
                && status["planned"] == count
                && status["attempted"] == count
                && status["succeeded"] == count
                && status["failed"] == 0
                && status["unattempted"] == 0,
            "failed/incomplete paired arm"
        );
        let statuses = status["episodes"]
            .as_array()
            .context("missing status roster")?;
        ensure!(
            statuses.len() == grouped.episode_roster.len(),
            "incomplete status member roster"
        );
        let mut seen = std::collections::BTreeSet::new();
        for row in statuses {
            ensure!(row["status"] == "passed", "failed paired member");
            let id = row["episodeId"]
                .as_str()
                .context("missing status identity")?;
            ensure!(seen.insert(id), "duplicate status member");
            let member = grouped
                .episode_roster
                .iter()
                .find(|v| v.episode_id == id)
                .context("unknown status member")?;
            let mut actual = row.clone();
            actual
                .as_object_mut()
                .context("invalid status row")?
                .remove("status");
            ensure!(
                actual == serde_json::to_value(member)?,
                "paired status/query roster differs"
            );
        }
        let result = evaluate_grouped(grouped)?;
        ensure!(
            evaluated.insert(arm.role, result).is_none(),
            "duplicate scored arm"
        );
    }
    ensure!(evaluated.len() == 2, "missing scored arm");
    let left = &evaluated["baseline"];
    let right = &evaluated["candidate"];
    ensure!(
        left.signatures == right.signatures
            && left.assignments == right.assignments
            && left.metrics.keys().eq(right.metrics.keys()),
        "paired source/labels/query/split universe differs"
    );
    let mut splits = BTreeMap::new();
    for (stream, split) in ["train", "development", "test"].iter().enumerate() {
        let mut groups = Vec::new();
        let mut eligible = Vec::new();
        let mut excluded = BTreeMap::new();
        for (id, values) in &left.metrics {
            if left.assignments[id] != *split {
                continue;
            }
            let group = paired_group(values, &right.metrics[id])?;
            if group.count == 0 {
                excluded.insert(id,json!({"reason":"no_common_scored_queries","baseline":left.summary["splits"][split]["groups"][id],"candidate":right.summary["splits"][split]["groups"][id]}));
            } else {
                eligible.push(id);
                groups.push(group);
            }
        }
        splits.insert(*split,json!({"eligibleGroupIds":eligible,"excludedGroups":excluded,"baseline":left.summary["splits"][split],
            "candidate":right.summary["splits"][split],"paired":bootstrap(&groups,stream as u64)}));
    }
    Ok(
        json!({"profile":PROFILE,"pairManifestSha256":input.pair_manifest_sha256,"pairContractSha256":input.contract_sha256,
        "policy":policy(),"splits":splits,"semanticQualityClaim":false,"qualityPromotionEligible":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn group(count: usize, baseline: f64, candidate: f64) -> Group {
        Group {
            count,
            baseline: [baseline * count as f64; 6],
            candidate: [candidate * count as f64; 6],
        }
    }
    #[test]
    fn hand_calculated_paired_micro_macro_and_same_draws() {
        let groups = vec![group(3, 0., 1.), group(1, 1., 0.)];
        let value = bootstrap(&groups, 0);
        assert_eq!(
            value["point"]["retrievalRecallAt5"]["queryMicro"]["effect"],
            0.5
        );
        assert_eq!(
            value["point"]["retrievalRecallAt5"]["groupMacro"]["effect"],
            0.
        );
        assert_eq!(
            value["interval95"]["retrievalRecallAt5"]["queryMicro"]["lower"],
            -1.
        );
        assert_eq!(
            value["interval95"]["retrievalRecallAt5"]["queryMicro"]["upper"],
            1.
        );
        assert_eq!(bootstrap(&groups, 0), value);
        assert_ne!(bootstrap(&groups, 1)["drawsSha256"], value["drawsSha256"]);
        let equal = bootstrap(&[group(1, 0., 0.), group(3, 1., 1.)], 0);
        assert_eq!(
            equal["interval95"]["retrievalRecallAt5"]["queryMicro"],
            json!({"lower":0.,"upper":0.,"zeroObservedDispersion":true})
        );
    }
    #[test]
    fn insufficient_groups_are_not_fake_intervals() {
        let zero = bootstrap(&[], 0);
        assert_eq!(zero["status"], "unscored");
        assert!(zero["point"].is_null());
        assert!(zero["interval95"].is_null());
        let one = bootstrap(&[group(1, 0., 1.)], 0);
        assert_eq!(one["status"], "insufficient_eligible_groups");
        assert_eq!(one["executedReplicates"], "0");
        assert!(one["interval95"].is_null());
        assert_eq!(
            one["point"]["retrievalRecallAt5"]["queryMicro"]["effect"],
            1.
        );
    }
    #[test]
    fn unknown_zero_and_mismatched_eligibility_remain_explicit() {
        let unknown = source_scoring::evidence_metrics(None, &[1], &[1]);
        let zero = source_scoring::evidence_metrics(Some(&[]), &[1], &[1]);
        assert_eq!(
            paired_group(
                &[unknown.clone(), zero.clone()],
                &[unknown.clone(), zero.clone()]
            )
            .unwrap()
            .count,
            0
        );
        assert!(paired_group(&[unknown], &[zero]).is_err());
        let partial = source_scoring::evidence_metrics(Some(&[1, 2]), &[1], &[1]);
        let perfect = source_scoring::evidence_metrics(Some(&[1, 2]), &[1, 2], &[1, 1, 2]);
        let value = paired_group(&[partial], &[perfect]).unwrap();
        assert_eq!(value.baseline[1], 0.5);
        assert_eq!(value.candidate[1], 1.);
    }
}
