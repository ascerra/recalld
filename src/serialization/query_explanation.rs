//! Projection of actual ranked-query evidence. This is not a similarity formula.
use crate::search::{
    DirectScoreEvidence, MinScoreDisposition, OwnScoreExplanation, ScoreExplanation, ScoreFormula,
};
use serde_json::{Value, json};

fn direct(e: &DirectScoreEvidence) -> Value {
    json!({"vector":e.vector,"fts":e.fts,"fused":e.fused,"minScore":e.min_score,
        "disposition":match e.disposition {MinScoreDisposition::Passed=>"passed",MinScoreDisposition::Rejected=>"rejected",MinScoreDisposition::Auxiliary=>"auxiliary"}})
}
fn own(e: &OwnScoreExplanation) -> Value {
    json!({"formula":match e.formula{ScoreFormula::Direct=>"direct",ScoreFormula::Graph=>"graph",ScoreFormula::EntityRecall=>"entityRecall",ScoreFormula::Metadata=>"metadata"},
        "direct":direct(&e.direct),"entityOverlap":e.entity_overlap,"entityRecall":e.entity_recall,"ftsSignal":e.fts_signal,
        "rawRetrievability":e.raw_retrievability,"effectiveRetrievability":e.effective_retrievability,"activation":e.activation,"activationDepth":e.activation_depth,
        "baseScore":e.base_score,"temporalFactor":e.temporal_factor,"rifMultiplier":e.rif_multiplier,"score":e.score})
}

pub(crate) fn similarity(e: &crate::search::SimilarityExplanation) -> Value {
    use crate::search::{SimilarityAlgorithm, SimilarityOwnAdmission};
    json!({"algorithm":match e.algorithm {SimilarityAlgorithm::CosineMax=>"cosineMax"},
        "ownCosine":e.own_cosine,"ownAdmission":match e.own_admission {
            SimilarityOwnAdmission::Missing=>"missing",SimilarityOwnAdmission::RejectedByMinScore=>"rejectedByMinScore",
            SimilarityOwnAdmission::NotAdmitted=>"notAdmitted",SimilarityOwnAdmission::Admitted=>"admitted"},
        "minScore":e.min_score,"admittedOwn":e.admitted_own,"inheritedFloor":e.inherited_floor,
        "origins":e.origins.iter().map(|o|json!({"originId":o.origin_id,"cosine":o.cosine})).collect::<Vec<_>>(),
        "omittedOrigins":e.omitted_origins,"finalSimilarity":e.final_similarity,"version":e.version.map(super::query_response::version)})
}
pub(crate) fn project(e: &ScoreExplanation) -> Value {
    json!({"rankingProfile":e.ranking_profile,"retrievalSemanticsVersion":e.retrieval_semantics_version,
        "version":e.version.map(super::query_response::version),"rawRetrievability":e.raw_retrievability,"effectiveRetrievability":e.effective_retrievability,
        "own":e.own.as_ref().map(own),"rejectedDirect":e.rejected_direct.as_ref().map(direct),"inheritedFloor":e.inherited_floor,
        "inheritedSources":e.inherited_sources.iter().map(|s|json!({"source":s.source,"own":own(&s.own)})).collect::<Vec<_>>(),
        "inheritedSourcesOmitted":e.inherited_sources_omitted,"rifMultiplier":e.rif_multiplier,"finalScore":e.final_score})
}
