//! Evaluator-only source scoring, consumed by internal and serialized MCP runners.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::BufRead,
};

// Source IDs are evaluator-only. Deduplicate at first occurrence before DCG.
pub fn evidence_metrics(
    relevant: Option<&[usize]>,
    retained: &[usize],
    retrieved: &[usize],
) -> Value {
    let Some(relevant) = relevant else {
        return json!({"scored":false,"annotationStatus":"unknown","reason":"missing_annotations",
            "annotatedSourceUnits":null});
    };
    let gold: BTreeSet<_> = relevant.iter().copied().collect();
    if gold.is_empty() {
        return json!({"scored":false,"annotationStatus":"zero_label","reason":"zero_annotated_sources","annotatedSourceUnits":0});
    }
    let retained: BTreeSet<_> = retained.iter().copied().collect();
    let mut seen = BTreeSet::new();
    let ranked: Vec<_> = retrieved
        .iter()
        .copied()
        .filter(|s| seen.insert(*s))
        .collect();
    let recall =
        |k| ranked.iter().take(k).filter(|s| gold.contains(s)).count() as f64 / gold.len() as f64;
    let dcg: f64 = ranked
        .iter()
        .take(10)
        .enumerate()
        .filter(|(_, s)| gold.contains(s))
        .map(|(i, _)| 1.0 / ((i + 2) as f64).log2())
        .sum();
    let ideal: f64 = (0..gold.len().min(10))
        .map(|i| 1.0 / ((i + 2) as f64).log2())
        .sum();
    json!({"scored": true, "annotationStatus":"scored", "annotatedSourceUnits": gold.len(),
        "retrievedUniqueSourceUnits":ranked.len(),
        "extractionEvidenceRetention": gold.intersection(&retained).count() as f64 / gold.len() as f64,
        "retrievalRecallAt5": recall(5), "retrievalRecallAt10": recall(10),
        "retrievalRecallAt20": recall(20), "allRequiredEvidence": recall(usize::MAX) == 1.0,
        "binarySourceNdcgAt10": dcg / ideal,
        "propositionPreservation": "not measured", "answerQuality": "not measured"})
}

// Consume the append-only source ledger, including partial attempts on failure.
pub fn turn_coverage(reader: impl BufRead, eligible: usize) -> Result<Value> {
    let mut counts: BTreeMap<&str, usize> = [
        "seen",
        "attempted",
        "accepted",
        "no_op",
        "rejected",
        "truncated",
    ]
    .into_iter()
    .map(|status| (status, 0))
    .collect();
    let mut states = BTreeMap::<String, String>::new();
    let mut events = 0;
    for line in reader.lines() {
        let event: Value = serde_json::from_str(&line?)?;
        let id = event["sourceId"]
            .as_str()
            .context("missing source identity")?;
        let status = event["status"].as_str().context("missing status")?;
        ensure!(
            event["reason"].as_str().is_some_and(|r| !r.is_empty()),
            "missing reason"
        );
        let prior = states.get(id).map(String::as_str);
        ensure!(
            matches!(
                (prior, status),
                (None, "seen")
                    | (Some("seen"), "attempted" | "no_op" | "truncated")
                    | (
                        Some("attempted"),
                        "accepted" | "rejected" | "no_op" | "truncated"
                    )
            ),
            "invalid source event transition"
        );
        *counts.get_mut(status).context("unknown source status")? += 1;
        states.insert(id.to_owned(), status.to_owned());
        events += 1;
    }
    ensure!(
        counts["seen"] <= eligible,
        "more sources seen than eligible"
    );
    let processed = ["accepted", "no_op", "rejected", "truncated"]
        .iter()
        .map(|s| counts[s])
        .sum::<usize>();
    let ratio = |n: usize| {
        if eligible == 0 {
            Value::Null
        } else {
            json!(n as f64 / eligible as f64)
        }
    };
    Ok(
        json!({"ledgerVersion":1,"countUnit":"unique-source-turn","eligible":eligible.to_string(),
        "eventCount":events.to_string(),"statuses":counts.iter().map(|(k,v)| (*k,v.to_string())).collect::<BTreeMap<_,_>>(),
        "processed":processed.to_string(),"pending":(counts["seen"]-processed).to_string(),
        "unseen":(eligible-counts["seen"]).to_string(),"processedCoverage":ratio(processed),
        "acceptedMemoryCoverage":ratio(counts["accepted"]),"propositionPreservation":"not measured"}),
    )
}

pub fn scoring_summary(metrics: &[Value]) -> Value {
    let scored: Vec<_> = metrics.iter().filter(|m| m["scored"] == true).collect();
    let count = |status: &str| {
        metrics
            .iter()
            .filter(|m| m["annotationStatus"] == status)
            .count()
            .to_string()
    };
    let means: BTreeMap<_, _> = [
        "extractionEvidenceRetention",
        "retrievalRecallAt5",
        "retrievalRecallAt10",
        "retrievalRecallAt20",
        "binarySourceNdcgAt10",
    ]
    .into_iter()
    .map(|name| {
        let mean = if scored.is_empty() {
            Value::Null
        } else {
            json!(
                scored
                    .iter()
                    .map(|m| m[name].as_f64().unwrap())
                    .sum::<f64>()
                    / scored.len() as f64
            )
        };
        (name, mean)
    })
    .collect();
    json!({"profile":"synthetic-unique-source-v1","denominatorUnit":"measured-query-event",
        "queryEvents":metrics.len().to_string(),"scored":count("scored"),"zeroLabelUnscored":count("zero_label"),
        "unknownAnnotations":count("unknown"),"means":means,"allRequiredEvidenceRate": if scored.is_empty() { Value::Null } else {
            json!(scored.iter().filter(|m| m["allRequiredEvidence"] == true).count() as f64 / scored.len() as f64)
        },"semanticQualityClaim":false})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_scorer_hand_calculated_cases_and_denominators() {
        let unknown = evidence_metrics(None, &[1], &[1]);
        let zero = evidence_metrics(Some(&[]), &[1], &[1]);
        assert_eq!(unknown["annotationStatus"], "unknown");
        assert_eq!(zero["annotationStatus"], "zero_label");
        assert!(unknown.get("retrievalRecallAt5").is_none());
        assert!(zero.get("binarySourceNdcgAt10").is_none());
        let empty = evidence_metrics(Some(&[1, 2]), &[1], &[]);
        assert_eq!(empty["retrievalRecallAt5"], 0.0);
        assert_eq!(empty["binarySourceNdcgAt10"], 0.0);
        assert_eq!(empty["allRequiredEvidence"], false);
        let partial = evidence_metrics(Some(&[1, 2]), &[1], &[9, 1, 1]);
        // DCG = 1/log2(3); ideal = 1 + 1/log2(3), hand-calculated binary nDCG.
        assert!(
            (partial["binarySourceNdcgAt10"].as_f64().unwrap() - 0.38685280723454163).abs() < 1e-12
        );
        assert_eq!(partial["retrievalRecallAt5"], 0.5);
        assert_eq!(partial["extractionEvidenceRetention"], 0.5);
        let perfect = evidence_metrics(Some(&[1, 1, 2]), &[1, 1, 2], &[1, 1, 2, 2]);
        assert_eq!(perfect["annotatedSourceUnits"], 2);
        assert_eq!(perfect["retrievedUniqueSourceUnits"], 2);
        assert_eq!(perfect["binarySourceNdcgAt10"], 1.0);
        assert_eq!(perfect["retrievalRecallAt20"], 1.0);
        let cut = evidence_metrics(Some(&[1]), &[], &[9, 9, 8, 7, 6, 5, 1]);
        assert_eq!(cut["retrievalRecallAt5"], 0.0);
        assert_eq!(cut["retrievalRecallAt10"], 1.0);
        assert_eq!(cut["allRequiredEvidence"], true);
        let summary = scoring_summary(&[unknown.clone(), zero.clone(), empty, partial, perfect]);
        assert_eq!(summary["queryEvents"], "5");
        assert_eq!(summary["scored"], "3");
        assert_eq!(summary["unknownAnnotations"], "1");
        assert_eq!(summary["zeroLabelUnscored"], "1");
        assert_eq!(summary["means"]["retrievalRecallAt5"], 0.5);
        assert_eq!(
            scoring_summary(&[unknown, zero])["means"]["retrievalRecallAt5"],
            Value::Null
        );
    }

    #[test]
    fn ledger_counts_no_memory_rejection_truncation_and_partial_attempts() {
        let rows = [
            ("a", "seen"),
            ("a", "attempted"),
            ("a", "accepted"),
            ("b", "seen"),
            ("b", "no_op"),
            ("c", "seen"),
            ("c", "attempted"),
            ("c", "rejected"),
            ("d", "seen"),
            ("d", "truncated"),
            ("e", "seen"),
            ("e", "attempted"),
        ];
        let lines = rows
            .iter()
            .map(|(id, status)| {
                json!({"sourceId":id,"status":status,"reason":"synthetic-test"}).to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let summary = turn_coverage(lines.as_bytes(), 6).unwrap();
        assert_eq!(summary["eventCount"], "12");
        assert_eq!(summary["statuses"]["seen"], "5");
        assert_eq!(summary["statuses"]["accepted"], "1");
        assert_eq!(summary["statuses"]["no_op"], "1");
        assert_eq!(summary["statuses"]["rejected"], "1");
        assert_eq!(summary["statuses"]["truncated"], "1");
        assert_eq!(summary["pending"], "1");
        assert_eq!(summary["unseen"], "1");
        assert_eq!(summary["processedCoverage"], 4.0 / 6.0);
        assert_eq!(summary["acceptedMemoryCoverage"], 1.0 / 6.0);
        let invalid = format!(
            "{lines}\n{}",
            json!({"sourceId":"a","status":"accepted","reason":"duplicate"})
        );
        assert!(turn_coverage(invalid.as_bytes(), 6).is_err());
        assert_eq!(
            turn_coverage("".as_bytes(), 0).unwrap()["processedCoverage"],
            Value::Null
        );
    }
}
