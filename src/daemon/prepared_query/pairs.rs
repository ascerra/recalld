use crate::search::exact_scan::{
    ExactCoverage, ExactPairAlgorithm, ExactScanData, ExactScanError, ExactScanPage,
};
use crate::serialization::scan::ExactScanRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Page {
    namespace: String,
    namespace_id: crate::model::NamespaceId,
    data: Data,
    warnings: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Data {
    pairs: Vec<Pair>,
    coverage: Coverage,
    next_cursor: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pair {
    left_id: crate::model::MemoryId,
    right_id: crate::model::MemoryId,
    similarity_bits: String,
}

pub(super) fn encode(page: &ExactScanPage) -> Result<Value, crate::mcp::bridge::BridgeError> {
    let mut value = serde_json::to_value(page).map_err(|_| {
        crate::mcp::bridge::BridgeError::Internal("exact page serialization failed".into())
    })?;
    // Obtain every bit from the original PairEdge before any decimal parse.
    for (value, pair) in value["data"]["pairs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(&page.data.pairs)
    {
        value.as_object_mut().unwrap().remove("similarity");
        value["similarityBits"] =
            Value::String(crate::serialization::scan::scalar_bits(pair.similarity));
    }
    Ok(value)
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Coverage {
    algorithm: String,
    read_generation: crate::storage::control::GenerationRef,
    embedding_profile_id: String,
    eligible_count: u64,
    excluded_count: u64,
    total_pairs: u64,
    compared_this_page: u32,
    compared_so_far: u64,
    complete: bool,
    phase_counts: BTreeMap<String, u64>,
    exclusions: BTreeMap<String, u64>,
}
fn names(
    map: BTreeMap<String, u64>,
    allowed: &[&'static str],
) -> Result<BTreeMap<&'static str, u64>, ExactScanError> {
    map.into_iter()
        .map(|(key, value)| {
            allowed
                .iter()
                .find(|name| **name == key)
                .map(|name| (*name, value))
                .ok_or(ExactScanError::Unavailable)
        })
        .collect()
}
pub(super) fn decode(
    value: &Value,
    request: &ExactScanRequest,
) -> Result<ExactScanPage, ExactScanError> {
    let page: Page =
        serde_json::from_value(value.clone()).map_err(|_| ExactScanError::Unavailable)?;
    let encoded: Value = serde_json::from_slice(
        &serde_json::to_vec(&page).map_err(|_| ExactScanError::Unavailable)?,
    )
    .map_err(|_| ExactScanError::Unavailable)?;
    let c = page.data.coverage;
    if &encoded != value
        || page.namespace_id.is_unset()
        || crate::model::validation::validate_namespace_name(&page.namespace).is_err()
        || request
            .namespace
            .as_ref()
            .is_some_and(|n| !n.eq_ignore_ascii_case(&page.namespace))
        || c.algorithm != "exactPairs"
        || c.read_generation.store_id.is_nil()
        || !crate::embedding::profile::valid_profile_id(&c.embedding_profile_id)
        || c.compared_this_page > request.pair_budget
        || c.compared_so_far < c.compared_this_page as u64
        || c.compared_so_far > c.total_pairs
        || page.data.pairs.len() > c.compared_this_page as usize
        || page.data.pairs.len() > crate::serialization::scan::MAX_RESPONSE_PAIRS
        || c.total_pairs
            != crate::search::exact_pairs::total_pairs(c.eligible_count)
                .map_err(|_| ExactScanError::Unavailable)?
        || c.complete != (page.data.next_cursor.is_none() && c.excluded_count == 0)
        || page.data.next_cursor.is_none() != (c.compared_so_far == c.total_pairs)
    {
        return Err(ExactScanError::Unavailable);
    }
    let phases = names(c.phase_counts, &["full", "summary", "ghost", "tombstone"])?;
    let exclusions = names(
        c.exclusions,
        &[
            "missingVector",
            "dimensionMismatch",
            "nonFinite",
            "zeroNorm",
        ],
    )?;
    let excluded = exclusions
        .values()
        .try_fold(0u64, |n, v| n.checked_add(*v))
        .ok_or(ExactScanError::Unavailable)?;
    let live = ["full", "summary", "ghost"]
        .iter()
        .try_fold(0u64, |n, key| {
            phases.get(key).and_then(|v| n.checked_add(*v))
        })
        .ok_or(ExactScanError::Unavailable)?;
    if phases.len() != 4
        || excluded != c.excluded_count
        || c.eligible_count.checked_add(excluded) != Some(live)
    {
        return Err(ExactScanError::Unavailable);
    }
    // The strict private roundtrip above contains bit strings, never f64 scores.
    let pairs = page
        .data
        .pairs
        .into_iter()
        .map(|p| {
            let similarity = crate::serialization::scan::scalar_from_bits(&p.similarity_bits)
                .ok_or(ExactScanError::Unavailable)?;
            Ok(crate::search::exact_pairs::PairEdge {
                left_id: p.left_id,
                right_id: p.right_id,
                similarity,
            })
        })
        .collect::<Result<Vec<_>, ExactScanError>>()?;
    let mut previous = None;
    for p in &pairs {
        let pair = (p.left_id, p.right_id);
        if p.left_id.as_bytes() == &[0; 16]
            || p.left_id >= p.right_id
            || previous.is_some_and(|old| old >= pair)
            || !p.similarity.is_finite()
            || !(-1.0..=1.0).contains(&p.similarity)
            || p.similarity < request.threshold
        {
            return Err(ExactScanError::Unavailable);
        }
        previous = Some(pair);
    }
    if let Some(cursor) = &page.data.next_cursor {
        // Existing decoder validates its version and actual declared binding;
        // the next server call still revalidates canonical generation/policy.
        let next = crate::serialization::pair_cursor::PairCursor::decode(cursor, request)
            .map_err(|_| ExactScanError::Unavailable)?;
        if next.at != c.read_generation
            || next.namespace_id != page.namespace_id
            || next.profile_id != c.embedding_profile_id
            || next.threshold_bits != request.threshold.to_bits()
            || next.next_pair != c.compared_so_far
        {
            return Err(ExactScanError::Unavailable);
        }
    }
    let previous = match &request.cursor {
        Some(cursor) => {
            let previous = crate::serialization::pair_cursor::PairCursor::decode(cursor, request)
                .map_err(|_| ExactScanError::Unavailable)?;
            if previous.at != c.read_generation
                || previous.namespace_id != page.namespace_id
                || previous.profile_id != c.embedding_profile_id
            {
                return Err(ExactScanError::Unavailable);
            }
            previous.next_pair
        }
        None => 0,
    };
    if previous.checked_add(c.compared_this_page as u64) != Some(c.compared_so_far) {
        return Err(ExactScanError::Unavailable);
    }
    let expected = if excluded > 0 {
        vec!["Some live namespace records lack comparable vectors; terminal coverage cannot be complete.".to_string()]
    } else {
        vec![]
    };
    if page.warnings != expected {
        return Err(ExactScanError::Unavailable);
    }
    Ok(ExactScanPage {
        namespace: page.namespace,
        namespace_id: page.namespace_id,
        warnings: page.warnings,
        diagnostics: None,
        data: ExactScanData {
            pairs,
            next_cursor: page.data.next_cursor,
            coverage: ExactCoverage {
                algorithm: ExactPairAlgorithm::ExactPairs,
                read_generation: c.read_generation,
                embedding_profile_id: c.embedding_profile_id,
                eligible_count: c.eligible_count,
                excluded_count: excluded,
                total_pairs: c.total_pairs,
                compared_this_page: c.compared_this_page,
                compared_so_far: c.compared_so_far,
                complete: c.complete,
                phase_counts: phases,
                exclusions,
            },
        },
    })
}
