//! Exact page arithmetic and vector comparison. No storage generation is
//! produced here: the real canonical/vector capture and final guard wrap this.
use crate::{
    model::MemoryId,
    serialization::scan::{ExactScanRequest, MAX_PAIR_BUDGET, MAX_RESPONSE_PAIRS},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PairError {
    #[error("exact pair bounds overflow or exceed supported limits")]
    Bounds,
    #[error("cursor pair position is outside the captured eligible pairs")]
    InvalidPosition,
    #[error("exact scan response exceeds the transport byte budget; reduce pairBudget")]
    ResponseTooLarge,
    #[error("exact comparator requires ordered unique IDs and finite nonzero compatible vectors")]
    InvalidVectors,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairEdge {
    pub left_id: MemoryId,
    pub right_id: MemoryId,
    pub similarity: f64,
}
#[derive(Debug)]
pub(crate) struct ComparedPage {
    pub pairs: Vec<PairEdge>,
    pub total_pairs: u64,
    pub compared_this_page: u32,
    pub compared_so_far: u64,
    pub next_pair: Option<u64>,
}
pub(crate) fn total_pairs(n: u64) -> Result<u64, PairError> {
    if n < 2 {
        return Ok(0);
    }
    // Divide the even factor first; avoid an unnecessary intermediate overflow.
    let (a, b) = if n.is_multiple_of(2) {
        (n / 2, n - 1)
    } else {
        (n, (n - 1) / 2)
    };
    a.checked_mul(b).ok_or(PairError::Bounds)
}
pub(crate) fn compare_page(
    vectors: &[(MemoryId, Vec<f32>)],
    request: &ExactScanRequest,
    next: Option<u64>,
) -> Result<ComparedPage, PairError> {
    if !(1..=MAX_PAIR_BUDGET).contains(&request.pair_budget)
        || !request.threshold.is_finite()
        || !(0.0..=1.0).contains(&request.threshold)
    {
        return Err(PairError::Bounds);
    }
    let count = u64::try_from(vectors.len()).map_err(|_| PairError::Bounds)?;
    let total = total_pairs(count)?;
    if next.is_some_and(|n| n >= total) {
        return Err(PairError::InvalidPosition);
    }
    let dimension = vectors.first().map(|(_, v)| v.len()).unwrap_or(0);
    if dimension > 8192
        || vectors
            .windows(2)
            .any(|rows| rows[0].0.as_bytes() >= rows[1].0.as_bytes())
        || vectors.iter().any(|(_, v)| {
            v.is_empty()
                || v.len() != dimension
                || v.iter().any(|x| !x.is_finite())
                || v.iter().all(|x| *x == 0.0)
        })
    {
        return Err(PairError::InvalidVectors);
    }
    // Snapshot Available preserves original f32 bits; it is not a promise of
    // exact unit length. Even admitted Bound f32 vectors have rounding error.
    // Compute cosine in f64 without normalizing/mutating stored or copied bytes.
    // f32 products and sums for supported dimensions cannot overflow f64.
    let norms: Vec<f64> = vectors
        .iter()
        .map(|(_, vector)| {
            vector
                .iter()
                .map(|x| f64::from(*x).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .collect();
    let start = next.unwrap_or(0);
    let budget = u64::from(request.pair_budget).min(total - start);
    let mut remainder = start;
    let mut left = 0usize;
    while left + 1 < vectors.len() {
        let row_pairs = (vectors.len() - left - 1) as u64;
        if remainder < row_pairs {
            break;
        }
        remainder -= row_pairs;
        left += 1;
    }
    let mut right = left + 1 + usize::try_from(remainder).map_err(|_| PairError::Bounds)?;
    let mut pairs = Vec::new();
    for _ in 0..budget {
        let (left_id, a) = &vectors[left];
        let (right_id, b) = &vectors[right];
        let dot: f64 = a
            .iter()
            .zip(b)
            .map(|(x, y)| f64::from(*x) * f64::from(*y))
            .sum();
        let score = (dot / (norms[left] * norms[right])).clamp(-1.0, 1.0);
        if !score.is_finite() {
            return Err(PairError::InvalidVectors);
        }
        if score >= request.threshold {
            if pairs.len() == MAX_RESPONSE_PAIRS {
                return Err(PairError::ResponseTooLarge);
            }
            pairs.push(PairEdge {
                left_id: *left_id,
                right_id: *right_id,
                similarity: if score == 0.0 { 0.0 } else { score },
            });
        }
        right += 1;
        if right == vectors.len() {
            left += 1;
            right = left + 1;
        }
    }
    let compared = start.checked_add(budget).ok_or(PairError::Bounds)?;
    Ok(ComparedPage {
        pairs,
        total_pairs: total,
        compared_this_page: u32::try_from(budget).map_err(|_| PairError::Bounds)?,
        compared_so_far: compared,
        next_pair: (compared < total).then_some(compared),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;
    fn request(budget: u32, threshold: f64) -> ExactScanRequest {
        ExactScanRequest::from_http(
            &json!({"pairBudget":budget,"threshold":threshold}),
            "default",
        )
        .unwrap()
    }
    #[test]
    fn exact_pair_cosine_preserves_bits_and_handles_rounding_scaling_and_extremes() {
        let id = |n| MemoryId::from_uuid(Uuid::from_u128(n));
        let vectors = vec![(id(1), vec![1., 0.]), (id(2), vec![0.6_f32, 0.8_f32])];
        // These inputs satisfy the actual Bound norm tolerance, yet raw dot is
        // above true cosine. A threshold between them must not match.
        crate::storage::canonical_schema::validate_bound_vector(&vectors[1].1).unwrap();
        let raw_dot = f64::from(0.6_f32);
        let cosine = raw_dot / raw_dot.hypot(f64::from(0.8_f32));
        assert!(cosine < raw_dot);
        let midpoint = (raw_dot + cosine) / 2.;
        assert!(
            compare_page(&vectors, &request(1, midpoint), None)
                .unwrap()
                .pairs
                .is_empty()
        );
        let matched = compare_page(&vectors, &request(1, cosine - 1e-12), None).unwrap();
        assert!((matched.pairs[0].similarity - cosine).abs() < 1e-15);
        let scaled = vec![(id(1), vec![3., 4.]), (id(2), vec![0., 2.])];
        let before: Vec<_> = scaled
            .iter()
            .flat_map(|(_, v)| v.iter().map(|x| f32::to_bits(*x)))
            .collect();
        let page = compare_page(&scaled, &request(1, 0.79), None).unwrap();
        assert!((page.pairs[0].similarity - 0.8).abs() < 1e-15);
        assert!(
            compare_page(&scaled, &request(1, 0.81), None)
                .unwrap()
                .pairs
                .is_empty()
        );
        assert_eq!(
            before,
            scaled
                .iter()
                .flat_map(|(_, v)| v.iter().map(|x| x.to_bits()))
                .collect::<Vec<_>>()
        );
        // Pure comparator fixtures only: arbitrary scaling is not an attestation
        // of canonical Bound storage. Its stricter proof remains unchanged.
        let extremes = vec![
            (id(1), vec![f32::MAX, f32::MAX]),
            (id(2), vec![f32::from_bits(1), f32::from_bits(1)]),
            (id(3), vec![3., 3.]),
        ];
        let page = compare_page(&extremes, &request(3, 0.999), None).unwrap();
        assert_eq!(page.pairs.len(), 3);
        assert!(
            page.pairs
                .iter()
                .all(|p| p.similarity <= 1. && p.similarity.is_finite())
        );
    }
    #[test]
    fn exact_pair_dense_response_overflow_errors_without_losing_pairs_on_smaller_retry() {
        let vectors: Vec<_> = (1..=182)
            .map(|n| (MemoryId::from_uuid(Uuid::from_u128(n)), vec![1.]))
            .collect();
        assert!(total_pairs(vectors.len() as u64).unwrap() > MAX_RESPONSE_PAIRS as u64);
        assert_eq!(
            compare_page(&vectors, &request(MAX_PAIR_BUDGET, 1.), None).unwrap_err(),
            PairError::ResponseTooLarge
        );
        // Retry the same cursor with a smaller budget. No edge is dropped and
        // coverage counts compared pairs, not the failed request's partial work.
        let first = compare_page(&vectors, &request(MAX_RESPONSE_PAIRS as u32, 1.), None).unwrap();
        assert_eq!(first.pairs.len(), MAX_RESPONSE_PAIRS);
        assert_eq!(first.compared_so_far, MAX_RESPONSE_PAIRS as u64);
        let second = compare_page(
            &vectors,
            &request(MAX_RESPONSE_PAIRS as u32, 1.),
            first.next_pair,
        )
        .unwrap();
        assert_eq!(second.next_pair, None);
        assert_eq!(second.compared_so_far, first.total_pairs);
        let actual: Vec<_> = first
            .pairs
            .into_iter()
            .chain(second.pairs)
            .map(|p| (p.left_id, p.right_id))
            .collect();
        let mut oracle = Vec::new();
        for (i, (left, _)) in vectors.iter().enumerate() {
            for (right, _) in vectors.iter().skip(i + 1) {
                oracle.push((*left, *right));
            }
        }
        assert_eq!(actual, oracle);
    }
    #[test]
    fn exact_pair_pages_match_independent_dense_tied_disconnected_cross_200_oracle() {
        // Pure vector fixture; no native publication/ReadView claim. Four
        // disconnected one-hot groups produce exact known scores of 0 or 1.
        let vectors: Vec<_> = (0..205)
            .map(|i| {
                let mut v = vec![0.; 4];
                v[i % 4] = 1.;
                (MemoryId::from_uuid(Uuid::from_u128(i as u128 + 1)), v)
            })
            .collect();
        let mut oracle = Vec::new();
        for i in 0..vectors.len() {
            for j in i + 1..vectors.len() {
                if i % 4 == j % 4 {
                    oracle.push(PairEdge {
                        left_id: vectors[i].0,
                        right_id: vectors[j].0,
                        similarity: 1.,
                    });
                }
            }
        }
        assert!(
            oracle
                .iter()
                .any(|p| p.left_id == vectors[0].0 && p.right_id == vectors[204].0)
        );
        for budget in [1, 7, 101, MAX_PAIR_BUDGET] {
            let mut actual = Vec::new();
            let mut next = None;
            let mut compared = 0;
            loop {
                let page = compare_page(&vectors, &request(budget, 0.85), next).unwrap();
                assert!(page.compared_this_page <= budget);
                compared += u64::from(page.compared_this_page);
                assert_eq!(page.compared_so_far, compared);
                assert_eq!(page.total_pairs, 20_910);
                actual.extend(page.pairs);
                next = page.next_pair;
                if next.is_none() {
                    break;
                }
            }
            assert_eq!(compared, 20_910);
            assert_eq!(actual, oracle, "budget {budget}");
        }
    }
    #[test]
    fn exact_pair_empty_singleton_nonmatching_progress_invalid_positions_and_bounds() {
        for vectors in [
            vec![],
            vec![(MemoryId::from_uuid(Uuid::from_u128(1)), vec![1., 0.])],
        ] {
            let page = compare_page(&vectors, &request(1, 1.), None).unwrap();
            assert_eq!(page.total_pairs, 0);
            assert_eq!(page.compared_this_page, 0);
            assert!(page.next_pair.is_none());
            assert_eq!(
                compare_page(&vectors, &request(1, 1.), Some(0)).unwrap_err(),
                PairError::InvalidPosition
            );
        }
        let vectors = vec![
            (MemoryId::from_uuid(Uuid::from_u128(1)), vec![1., 0., 0.]),
            (MemoryId::from_uuid(Uuid::from_u128(2)), vec![0., 1., 0.]),
            (MemoryId::from_uuid(Uuid::from_u128(3)), vec![0., 0., 1.]),
        ];
        let page = compare_page(&vectors, &request(1, 1.), None).unwrap();
        assert!(page.pairs.is_empty());
        assert_eq!(page.next_pair, Some(1));
        assert_eq!(
            compare_page(&vectors, &request(1, 1.), Some(3)).unwrap_err(),
            PairError::InvalidPosition
        );
        let mut invalid = vectors.clone();
        invalid[1].1[0] = f32::NAN;
        assert_eq!(
            compare_page(&invalid, &request(1, 1.), None).unwrap_err(),
            PairError::InvalidVectors
        );
        invalid = vectors.clone();
        invalid.swap(0, 1);
        assert_eq!(
            compare_page(&invalid, &request(1, 1.), None).unwrap_err(),
            PairError::InvalidVectors
        );
        assert_eq!(total_pairs(u64::MAX), Err(PairError::Bounds));
        assert_eq!(total_pairs(100_000).unwrap(), 4_999_950_000);
    }
}
