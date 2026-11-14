# Similarity V1 eligibility-before-cap decision

Source-only decision during the SSD cleanup hold. No runtime changes or new
validation accompany this note. Actual parents remain PR133 `27a6cd4` plus PR132
`5864c09`, joined at `821e991`; no moving wire/startup parent is needed.

## Contract and observed defect

A2 §4 requires canonical eligibility before candidate allowances, independently
of the strict-current, associative relay and RIF compatibility decisions in
§§6–8. CONTRACTS's “Namespace, embeddings and observation” and “Retrieval,
deletion and readiness” sections also separate namespace scope, lifecycle policy
and retrieval semantics. None grants V1 a phase-after-cap exception. Preserving the
ghost-consumes-k defect would therefore be the wrong compatibility boundary.

The actual baseline `similar_baseline_phase_before_cap` failed: two higher-score
ghosts consumed the `k + 1` pool and the existing coherent V1 response omitted a
lower-scoring eligible record. Its frozen source/binary and expected failure are
already recorded in the baseline evidence manifest. The separate predecessor
probe demonstrates the proposed V2 current behavior; it does not justify adding
strict-current replacement to legacy similarity.

`QueryEngine::similar_with_access_admitted` is the shared defect location. The
coherent prepared path calls it under pinned canonical/view leases; direct Rust,
REST and MCP wrappers also reach it. It currently searches `k + 1`, removes the
source, takes `k`, then loads records and rejects foreign/Ghost/Tombstone rows.
Fixing only the new explicit V2 consumer would leave these legacy callers wrong.

## Minimal implementation boundary

1. In that shared helper, resolve the source namespace as today and pin one
   `StrengthContext`. Read `namespace_records` once and project the namespace
   before filtering, including eligible donors for explicit lifecycle policy.
   Use `StrengthContext::pin` plus `project_namespace` here instead of calling
   `by_id_strength_context` and then repeating its explicit-policy namespace scan.
   Keep the original requested-access intent and sampled time.
2. Build canonical eligible records/IDs: same namespace, present, not the source,
   and neither Ghost nor Tombstone. Legacy similarity exposes no tag/strength
   filter parameters; do not invent filters or a new endpoint. The explicit
   selection consumer continues applying all of its supplied hard filters.
3. Call the existing `VectorIndexRegistry::search` with `k` and `Some(eligible_ids)`.
   Its production adapter already delegates to `FlatVectorIndex::search_eligible`,
   which checks membership before scoring/heap admission. No new trait or index
   implementation is needed. Retain its dot-product scores, signed-zero handling,
   score-descending/UUID comparator and the existing score threshold.
4. Build results from the same projected canonical record map, preserving vector
   order. Do not reread cache metadata as eligibility authority. Keep fallible
   hydration, returned-only exposure, original numeric API identity, deferred
   recording, worker scopes and existing read admission/readiness behavior.

Same-namespace V1 must not be routed through the new Bound-only all-compatible
consumer: that would introduce an unrelated provenance requirement on existing
same-namespace calls. V1 still returns independently stored versions without
strict-current collapse. Explicit V2 retains current/history selection. Default
scope stays same namespace; legacy `sameNamespace:false` is separately corrected
to require genuine compatible canonical profiles, never inferred compatibility
from dimension or UnknownLegacy. Existing V1 graph/payload/RIF behavior is outside
this narrow similarity correction.

The accurate compatibility claim is “preserves V1 cosine scoring and stored-version
behavior while fixing canonical eligibility-before-cap and the explicit cross-scope
flag,” not “all V1 behavior unchanged.” Namespace enumeration adds work on the legacy
lifecycle path; it is an explicit correctness cost, with no performance claim.

## Validation and coordination after the hold

The baseline RED remains immutable. The candidate test currently asserting an
empty legacy result must be replaced: the corrected V1 top result is the eligible
predecessor with its original cosine; explicit V2 current returns its terminal.
Reuse the real ghost fixture and compare eligible IDs and score bits against an
exhaustive oracle, including source exclusion, stale/missing/foreign entries,
thresholds, ties, false-mode byte identity and returned-only true exposures.
Exercise actual coherent V1 MCP/REST consumers, plus the explicit V2 and compatible
cross-scope fixtures. Canonical read failures must propagate rather than become
empty success. No larger overfetch factor is a substitute for prefiltering.

Hume owns the shared helper and regressions. Huygens confirmed no legacy wire
blocker, with existing result shapes, cosine scores, stored-version semantics,
default scope and access/error flow preserved. He also requested actual adapter/RPC
false-flag coverage, stable equal-score selection and false-mode immutability.
Strict public selectors/activation remain his ownership.
No new wire fields, ordinary native startup activation, graph ranking changes or
new dependency ancestry follow from this decision. Validation waits for explicit
END of the disk-pressure hold; earlier candidate2 six-test PASS predates this
decision and does not validate the proposed V1 correction.
