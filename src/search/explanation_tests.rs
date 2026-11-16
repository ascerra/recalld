use super::semantics_tests::{current, graph_engine, history};
use super::tests::{mid, ns, other_ns, query, rec};
use super::*;
use crate::model::{EdgeType, Tag};

pub(super) fn reconstruct(own: &OwnScoreExplanation) -> f32 {
    let fts_signal = own.direct.fts.map_or(0.0, |fts| {
        if fts > 0.5 {
            1.0 - (-fts * 0.5).exp()
        } else {
            0.0
        }
    });
    assert_eq!(fts_signal, own.fts_signal);
    let base = match own.formula {
        ScoreFormula::Direct => {
            assert_eq!(own.direct.disposition, MinScoreDisposition::Passed);
            0.90 * own.direct.fused.unwrap() + 0.05 * own.entity_overlap + 0.05 * fts_signal
        }
        ScoreFormula::Graph => 0.70 * own.activation.unwrap() + 0.30 * own.entity_overlap,
        ScoreFormula::EntityRecall => {
            0.40 * own.entity_recall
                + 0.20 * own.entity_overlap
                + 0.10 * own.effective_retrievability
                + 0.10 * fts_signal
        }
        ScoreFormula::Metadata => own.effective_retrievability * 0.1,
    };
    if own.formula != ScoreFormula::Direct {
        assert_eq!(own.direct.disposition, MinScoreDisposition::Auxiliary);
    }
    assert_eq!(base.to_bits(), own.base_score.to_bits());
    let score = base * own.temporal_factor;
    assert_eq!(score.to_bits(), own.score.to_bits());
    score
}

pub(super) fn assert_explanation(response: &SemanticsResponse) {
    let evidence = response.diagnostics.provenance.as_ref().unwrap();
    assert_eq!(evidence.len(), response.response.results.len());
    for row in &response.response.results {
        let e = evidence[&row.memory_id].explanation.as_ref().unwrap();
        assert_eq!(e.ranking_profile, "baseline-v1");
        assert_eq!(e.retrieval_semantics_version, 2);
        assert_eq!(
            e.version,
            response.diagnostics.versions.get(&row.memory_id).copied()
        );
        assert!(e.version.is_some());
        assert_eq!(e.raw_retrievability, row.retrievability);
        assert_eq!(e.effective_retrievability, row.effective_r);
        assert_eq!(e.rif_multiplier, 1.0);
        let own = e.own.as_ref().map_or(0.0, reconstruct);
        let inherited = e
            .inherited_sources
            .iter()
            .map(|s| reconstruct(&s.own))
            .fold(0.0, f32::max);
        // Highest-score-first projection always retains the winning source,
        // including when lower contributors exceed the detail allowance.
        assert_eq!(inherited, e.inherited_floor.unwrap_or(0.0));
        assert!((own.max(inherited) - e.final_score).abs() < 1e-6);
        assert_eq!(Some(e.final_score), row.composite_score);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn score_explanations_reconstruct_all_formulas_and_temporal_factor() {
    use crate::search::{FtsIndex, adapters::SharedFtsIndexRegistry};
    let mut seen = Vec::new();
    for formula in [
        ScoreFormula::Direct,
        ScoreFormula::EntityRecall,
        ScoreFormula::Metadata,
        ScoreFormula::Graph,
    ] {
        let mut records: Vec<_> = (1..=10).map(|n| rec(mid(n))).collect();
        for record in &mut records {
            record.strength = 0.6;
            record.decay_strength = 0.8;
        }
        records[0].entities = vec!["Alice".into()];
        let vectors = if matches!(formula, ScoreFormula::Direct | ScoreFormula::Graph) {
            vec![(mid(1), 0.8)]
        } else {
            vec![]
        };
        let edges = if formula == ScoreFormula::Graph {
            vec![(mid(1), mid(2), EdgeType::Associative)]
        } else {
            vec![]
        };
        let (mut engine, _) = graph_engine(records, &vectors, &edges);
        engine.clock = Arc::new(crate::time::ManualClock::new(1_700_000_001_000));
        let directory = tempfile::tempdir().unwrap();
        if formula == ScoreFormula::Direct {
            let index = FtsIndex::new(directory.path()).unwrap();
            for n in 1..=10 {
                index
                    .add(
                        ns(),
                        mid(n),
                        if n == 1 { "keyword" } else { "filler" },
                        None,
                        &[],
                    )
                    .unwrap();
            }
            engine.fts_index = Arc::new(SharedFtsIndexRegistry::new(Arc::new(
                tokio::sync::Mutex::new(index),
            )));
        }
        let result = engine.search_with_semantics(query(serde_json::json!({
            "text":"keyword", "limit":10, "recordAccess":false,
            "filter":{"minStrength":0.0},
            "entities":if matches!(formula, ScoreFormula::Direct | ScoreFormula::EntityRecall) { vec!["Alice", "Bob"] } else { vec![] },
            "queryMode":if formula == ScoreFormula::Metadata {"metadata_only"} else {"embedding_plus_metadata"},
            "graphDepth":if formula == ScoreFormula::Graph {1} else {0},
            "timeRangeStart":1_699_999_999_000i64,"timeRangeEnd":1_700_000_001_000i64
        })), current()).await.unwrap();
        assert_explanation(&result);
        let id = if formula == ScoreFormula::Graph {
            mid(2)
        } else {
            mid(1)
        };
        let own = result.diagnostics.provenance.as_ref().unwrap()[&id]
            .explanation
            .as_ref()
            .unwrap()
            .own
            .unwrap();
        assert_eq!(own.formula, formula);
        assert_eq!(own.temporal_factor, 1.5);
        assert_eq!(own.raw_retrievability, 0.6);
        assert_eq!(own.effective_retrievability, 0.8);
        if formula == ScoreFormula::Direct {
            assert!(own.fts_signal > 0.0); // Exercise the actual FTS saturation branch.
            assert_eq!(own.direct.vector, Some(0.8));
        }
        if formula == ScoreFormula::Graph {
            assert_eq!(own.activation_depth, Some(1));
        }
        seen.push(own.formula);
    }
    assert_eq!(seen.len(), 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn score_explanations_bound_deduplicated_sources_and_separate_rejected_own() {
    let mut previous = None;
    for reverse in [false, true] {
        let mut records: Vec<_> = (1..=70)
            .map(|n| rec(mid(n)))
            .chain([rec(mid(1000))])
            .collect();
        let mut vectors: Vec<_> = (1..=70)
            .map(|n| (mid(n), 0.9))
            .chain([(mid(1000), 0.1)])
            .collect();
        let mut edges: Vec<_> = (1..=70)
            .map(|n| (mid(1000), mid(n), EdgeType::Supersedes))
            .collect();
        if reverse {
            records.reverse();
            vectors.reverse();
            edges.reverse();
        }
        let (mut engine, _) = graph_engine(records, &vectors, &edges);
        engine.clock = Arc::new(crate::time::ManualClock::new(123));
        let result = engine
            .search_with_semantics(
                query(serde_json::json!({"text":"q","limit":100,
            "minScore":0.8,"graphDepth":1})),
                current(),
            )
            .await
            .unwrap();
        assert_explanation(&result);
        assert_eq!(result.response.results.len(), 1);
        let p = &result.diagnostics.provenance.as_ref().unwrap()[&mid(1000)];
        let e = p.explanation.as_ref().unwrap();
        assert!(e.own.is_none());
        assert_eq!(
            e.rejected_direct.unwrap().disposition,
            MinScoreDisposition::Rejected
        );
        assert_eq!(e.rejected_direct.unwrap().vector, Some(0.1));
        assert!(!p.own_channels.contains(&RetrievalChannel::Vector));
        assert_eq!(e.inherited_sources.len(), 64);
        assert_eq!(e.inherited_sources_omitted, 6);
        assert_eq!(
            e.inherited_sources
                .iter()
                .map(|s| s.source)
                .collect::<Vec<_>>(),
            (1..=64).map(mid).collect::<Vec<_>>()
        );
        if let Some(before) = previous {
            assert_eq!(e, &before);
        }
        previous = Some(e.clone());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn score_explanations_do_not_leak_paths_or_ineligible_origins_and_history_has_no_inheritance()
{
    let mut records: Vec<_> = (1..=7).map(|n| rec(mid(n))).collect();
    for r in &mut records {
        r.tags = vec![Tag::new("allow").unwrap()];
    }
    records[1].phase = DecayPhase::Tombstone;
    records[4].namespace_id = other_ns();
    records[5].tags.clear();
    records[6].namespace_id = other_ns();
    let (engine, _) = graph_engine(
        records,
        &[
            (mid(1), 0.9),
            (mid(3), 0.1),
            (mid(4), 0.95),
            (mid(6), 0.99),
            (mid(7), 0.99),
        ],
        &[
            (mid(2), mid(1), EdgeType::Supersedes),
            (mid(3), mid(2), EdgeType::Supersedes),
            (mid(5), mid(4), EdgeType::Supersedes),
            (mid(3), mid(5), EdgeType::Supersedes),
        ],
    );
    let q = serde_json::json!({"text":"q","minScore":0.8,"filter":{"requireTags":["allow"]},"graphDepth":0});
    let current = engine
        .search_with_semantics(query(q.clone()), current())
        .await
        .unwrap();
    assert_explanation(&current);
    let evidence = current.diagnostics.provenance.as_ref().unwrap();
    assert_eq!(evidence.keys().copied().collect::<Vec<_>>(), vec![mid(3)]);
    let e = evidence[&mid(3)].explanation.as_ref().unwrap();
    assert_eq!(
        e.inherited_sources
            .iter()
            .map(|s| s.source)
            .collect::<Vec<_>>(),
        vec![mid(1)]
    );
    assert!(e.own.is_none());
    let retained = engine
        .search_with_semantics(query(q), history())
        .await
        .unwrap();
    assert_explanation(&retained);
    assert_eq!(
        retained
            .response
            .results
            .iter()
            .map(|r| r.memory_id)
            .collect::<Vec<_>>(),
        vec![mid(4), mid(1)]
    );
    for p in retained.diagnostics.provenance.as_ref().unwrap().values() {
        let e = p.explanation.as_ref().unwrap();
        assert!(e.inherited_sources.is_empty());
        assert!(e.inherited_floor.is_none());
        assert!(e.own.is_some());
    }
}
