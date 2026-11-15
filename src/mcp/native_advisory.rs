//! Hydrate only IDs already fixed by a native store outcome. Never rediscover
//! candidates or put text in receipts. The eventual native writer calls this
//! after readiness; this module alone does not enable requested advisory.
use crate::model::{DecayPhase, MemoryId, NamespaceId, NearDuplicateMatch, NearDuplicateReport};
use crate::storage::control::GenerationRef;
use crate::storage::{RedbStorageEngine, StorageEngine, StorageError};

pub(crate) struct NativeAdvisoryRead {
    pub report: Option<NearDuplicateReport>,
    pub read_generation: Option<GenerationRef>,
    pub warnings: Vec<&'static str>,
}

/// Failure is enrichment-unavailable on an already committed store, not a
/// failed write or an invented empty successful duplicate scan. Caller holds
/// engine read ownership and retains the immutable original receipt separately.
pub(crate) fn hydrate(
    engine: &RedbStorageEngine,
    namespace: NamespaceId,
    candidates: Option<&[(MemoryId, f32)]>,
) -> NativeAdvisoryRead {
    let Some(candidates) = candidates else {
        return NativeAdvisoryRead {
            report: None,
            read_generation: None,
            warnings: vec![],
        };
    };
    let result = (|| {
        engine.ensure_available()?;
        let generation = engine.control_state()?.generation_ref();
        if candidates.len() > crate::model::constants::MAX_NEAR_DUPLICATE_MATCHES {
            return Err(StorageError::InvalidControl(
                "oversized frozen native advisory".into(),
            ));
        }
        let mut matches = Vec::new();
        let mut omitted = false;
        let mut seen = std::collections::BTreeSet::new();
        for &(id, score) in candidates {
            if !score.is_finite()
                || !((crate::model::constants::DUPLICATE_SIMILARITY_THRESHOLD as f32)..=1.0)
                    .contains(&score)
                || !seen.insert(id)
            {
                return Err(StorageError::InvalidControl(
                    "invalid frozen native advisory".into(),
                ));
            }
            let Some(record) = engine.get_record(id)? else {
                omitted = true;
                continue;
            };
            if record.namespace_id != namespace.get()
                || !matches!(record.phase, DecayPhase::Full | DecayPhase::Summary)
            {
                omitted = true;
                continue;
            }
            matches.push(NearDuplicateMatch {
                id,
                score,
                summary: crate::model::truncate_summary(
                    &record.summary,
                    crate::model::constants::NEAR_DUPLICATE_SUMMARY_MAX_BYTES,
                ),
            });
        }
        engine.ensure_available()?;
        if engine.control_state()?.generation_ref() != generation {
            return Err(StorageError::LifecycleConflict(
                "native advisory visibility changed".into(),
            ));
        }
        Ok(NativeAdvisoryRead {
            report: NearDuplicateReport::from_matches(
                crate::model::constants::DUPLICATE_SIMILARITY_THRESHOLD,
                matches,
            ),
            read_generation: Some(generation),
            warnings: if omitted {
                vec!["ADVISORY_CONTENT_UNAVAILABLE"]
            } else {
                vec![]
            },
        })
    })();
    result.unwrap_or_else(|_: StorageError| NativeAdvisoryRead {
        report: None,
        read_generation: None,
        warnings: vec!["ADVISORY_UNAVAILABLE"],
    })
}
