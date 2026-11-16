//! Score evidence captured by the actual v2 scorer, never used to select/rank.
use super::*;

pub const MAX_EXPLANATION_SOURCES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreFormula {
    Direct,
    Graph,
    EntityRecall,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinScoreDisposition {
    Passed,
    Rejected,
    Auxiliary,
}

/// No UUID: rejected direct evidence can describe only an ultimately returned
/// record. It is never admitted channel evidence or an exposure classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectScoreEvidence {
    pub vector: Option<f32>,
    pub fts: Option<f32>,
    pub fused: Option<f32>,
    pub min_score: f32,
    pub disposition: MinScoreDisposition,
}

impl DirectScoreEvidence {
    pub(super) fn capture(candidate: &Candidate, min_score: f32) -> Self {
        Self {
            vector: candidate.raw_vector_score,
            fts: candidate.raw_fts_score,
            fused: candidate.relevance_score,
            min_score,
            disposition: match candidate.relevance_score {
                Some(score) if score < min_score => MinScoreDisposition::Rejected,
                Some(_) => MinScoreDisposition::Passed,
                None => MinScoreDisposition::Auxiliary,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnScoreExplanation {
    pub formula: ScoreFormula,
    pub direct: DirectScoreEvidence,
    pub entity_overlap: f32,
    pub entity_recall: f32,
    pub fts_signal: f32,
    pub raw_retrievability: f32,
    pub effective_retrievability: f32,
    /// Applied to this origin's effective R; 1 for Off and returned Shadow scores.
    pub rif_multiplier: f32,
    pub activation: Option<f32>,
    pub activation_depth: Option<u8>,
    pub base_score: f32,
    pub temporal_factor: f32,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InheritedScoreExplanation {
    pub source: MemoryId,
    pub own: OwnScoreExplanation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoreExplanation {
    pub ranking_profile: &'static str,
    pub retrieval_semantics_version: u8,
    pub version: Option<VersionStatus>,
    /// The returned record's canonical/projected values, not its donor's.
    pub raw_retrievability: f32,
    pub effective_retrievability: f32,
    pub own: Option<OwnScoreExplanation>,
    pub rejected_direct: Option<DirectScoreEvidence>,
    pub inherited_floor: Option<f32>,
    /// Eligible scored origins only, descending score then UUID, at most 64.
    /// No intermediate chain nodes or rejected/foreign records are included.
    pub inherited_sources: Vec<InheritedScoreExplanation>,
    pub inherited_sources_omitted: usize,
    /// Applied to the returned record's effective R. Inherited origins carry
    /// their own multipliers; this is not a multiplier on the final score.
    pub rif_multiplier: f32,
    pub final_score: f32,
}

impl Candidate {
    pub(super) fn inherit_score_sources(&mut self, predecessor: &Candidate) {
        let own = predecessor
            .score_explanation
            .map(|own| (predecessor.record.id, own));
        for (id, own) in predecessor
            .inherited_score_sources
            .iter()
            .map(|(&id, &own)| (id, own))
            .chain(own)
        {
            self.inherited_score_sources
                .entry(id)
                .and_modify(|existing| {
                    if own.score > existing.score {
                        *existing = own;
                    }
                })
                .or_insert(own);
        }
    }

    pub(super) fn explain(&self, version: Option<VersionStatus>) -> ScoreExplanation {
        let mut sources: Vec<_> = self
            .inherited_score_sources
            .iter()
            .map(|(&source, &own)| InheritedScoreExplanation { source, own })
            .collect();
        sources.sort_by(|a, b| {
            b.own
                .score
                .total_cmp(&a.own.score)
                .then(a.source.cmp(&b.source))
        });
        let omitted = sources.len().saturating_sub(MAX_EXPLANATION_SOURCES);
        sources.truncate(MAX_EXPLANATION_SOURCES);
        ScoreExplanation {
            ranking_profile: "baseline-v1",
            retrieval_semantics_version: 2,
            version,
            raw_retrievability: self.record.strength,
            effective_retrievability: self.effective_r,
            own: self.score_explanation,
            rejected_direct: self.rejected_direct,
            inherited_floor: self.provenance.inherited_floor,
            inherited_sources: sources,
            inherited_sources_omitted: omitted,
            rif_multiplier: self.applied_rif_multiplier,
            final_score: self.composite_score,
        }
    }
}
