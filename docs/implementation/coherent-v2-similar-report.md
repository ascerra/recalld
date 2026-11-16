# Coherent stored-vector similarity and legacy eligibility

Validated runtime/test commit: `0352d44a95f6088acc756749e6f3ef816efe3ea3`.

This slice joins published PR133 `27a6cd4c6fc89f19dcd8964c20ec3fbb8b24c36a`
and PR132 `5864c09050e532454c97ba8e73e00e7ba1670fb4` at `821e991`.
Their shared ancestor is PR119. No startup, moving wire, batch or performance
candidate parent is imported. This is an internal explicit similarity consumer
plus corrections to existing legacy similarity, not public API2 activation.

The existing same-namespace helper now loads canonical namespace records,
projects them once under one clock/policy context and excludes the source,
foreign records, Ghosts and Tombstones before exact vector top-k. It reuses the
existing vector comparator and dot-product scores. Legacy same-namespace calls
continue to support their existing profile lane and stored-version behavior;
they do not acquire strict-current replacement or a Bound-profile requirement.
The ghost-consumes-k defect is corrected for both semantics. Coherent, REST and
MCP callers share this helper. Canonical enumeration is an explicit cost, not a
performance improvement claim.

Selected rows are canonically re-read and validated with the same clock/policy
before observation. Missing rows or changed phase/scope/policy fail the whole
page, rather than silently dropping a hit. Namespace assertions retain existing
validation and ASCII case-insensitive comparison; canonical names, IDs and
receipt hashes are not rewritten.

The new Rust-only selector independently takes retrieval semantics, scope,
filters, threshold, access intent and original API number. It pins canonical and
derived views, one operation time, per-namespace lifecycle policies, and hydrates
only final returned IDs at the selected generation. Explicit current resolves
same-scope supersession chains and keeps the maximum admitted cosine per terminal;
history returns independently eligible stored versions. Hard filters precede score
admission. Resolution reuses the shared 64-traversed-hop and 12,800-examined-edge
limits; the examined successor establishing a hop-limit failure still consumes
work. Diagnostics separate canonical rows examined, eligible/scored/admitted
vectors, resolved results, final truncation and unresolved chains. Returned-ID
provenance does not expose excluded predecessors. Similarity performs no RIF or
associative graph expansion and does not replace cosine scoring with fusion.

Existing MCP `sameNamespace:false` now reaches the actual compatible-scope
consumer with V1 stored-version semantics. Source and target profiles must be
canonically Bound and equal; dimension-only equality and UnknownLegacy never
establish compatibility. Searched/excluded namespace observations remain internal
until A4 maps the explicit public surface. Default scope stays same namespace.
No provider invocation, profile attestation, serving activation or API2 envelope
is added. Unknown-profile cross-scope requests fail closed.

Returned-only associative exposure carries an opaque selection proof retaining
source identity/profile and selected target scopes. The existing observation
worker revalidates source visibility and every target before its first write,
then uses explicit per-namespace scopes in ascending namespace order. A foreign
source is validated separately rather than passed as a false same-namespace
assertion. There is no scope=None bypass or second observer. All groups share
the selection clock and original numeric API identity; each real writer captures
its current canonical state/policy. Selection G is not a claim that later
observations commit at that G. A later-group failure preserves earlier receipts
and awaits actual current publication; no atomic multi-namespace claim is made.
False and empty selection create no exposure writes.

## Validation provenance

Baseline manifest `9f525548a615b35596570d0a1e2ae86725f6bab7aff27902a488ce2048176c07`
records three actual assertion failures: canonical phase filtering after cap,
missing explicit current resolution, and ignored legacy false-scope flag. The
last unknown-profile fixture establishes ignored input only, not compatible-scope
correctness. Candidate fixtures instead use actual schema attestations and
lifecycle migration receipts, not fabricated policy rows.

Candidate1 failed compilation on two private-module imports; candidate2 passed
six focused tests. Candidate3 manifest
`fbb89f6ab18fcbd81b30071253532e9b267a5e0bfbbee640492005e870bed314`
passed nine tests and failed one new examination-count expectation: the 65-edge
case correctly examined 65 edges while traversing at most 64. Binary
`d531c7294e2c3024e1184ba8b6d2e090a8e01c3509074a44ef373f6d48a74532`
and the failed run are retained. The assertion now includes that examined edge;
no runtime budget change was made.

Candidate4 source manifest
`6c8e86ad2a6333e0895b96a451c31be5d1729371a390faa3279f5519441b0c4a`
contains 234 allowlisted files. Ten focused similarity tests and the strengthened
actual REST/MCP regression passed, but the full run ended at 1,039 passed and one
failed: `by_id_scope_and_record_policy_mismatches_fail_closed`. Reusing only the
enumeration map had eliminated the final canonical read where that regression
reveals malformed lifecycle provenance. Its exact binary is
`486dba894bd43a14f0a88f7122e8390d38dd3dd0b87ff93914abc426c8daf92f`;
Clippy was not reached. The correction restores canonical final revalidation
while keeping eligibility before k and failing closed on a missing/changed hit.

Candidate5 source manifest
`b2a2e09400ba1ee975193009d00eb77474f3f4871ae2ac44754f62a02bf5a27e`
is the corrected frozen source. Final validation passed **13 focused checks,
1,041 library tests and two CLI tests**, with no failures. The library run took
498.08 seconds; CLI tests took 3.04 and 1.46 seconds. Formatting passed. All-target
Clippy exited zero with warnings (707 library, 776 library-test including 685
duplicates, and one binary warning). These are 11 new library tests plus existing
coverage, not 13 new independent tests. No doctest claim follows from all-targets.

All 234 snapshot files, worktree files and staged source files matched before
the runtime commit. The container exited zero without OOM and was removed.
Library-test executable SHA256:
`a4edfcb2c32e364c733b749a94da93cffe599404cda4fcc6931a0459f6e74cdd`.
Validation log SHA256:
`5dcfbdfd2927354ce958fd96f54f4c55673198c9690c4273bf97a1937e2172c6`.
Exact image, scripts, source files and six executable hashes are in
`evidence/coherent-v2-similar/candidate-5/verification.json` and adjacent manifests.
Earlier failures remain separate entries; they were not overwritten or relabeled.

Tests cover actual Bound matching/mismatch/unbound scope, canonical phase/filter
refill, legacy cosine score bits and UUID ties under reversed insertion order,
negative cosine and thresholds, current/history and filtered successors,
64/65-edge chains, false canonical byte identity, actual MCP false-flag dispatch,
per-namespace lifecycle projection/exposure, one sampled clock, original API1/2
receipt hashes and strict reopen, source/target loss before any exposure, and a
later-group failure retaining an earlier real receipt and completed publication.
Existing shared resolver tests cover additional cycle/branch/missing/foreign and
query-work bounds; this slice does not claim a new exhaustive transport matrix
for every such graph shape.

Runtime validation uses `recalld-ci:1.94`, Rust1.94.1, `--network none --cpus 2
--memory 4g`, two build jobs and no home, credentials or socket mounts. Candidate
sources are immutable read-only allowlisted snapshots. After switching source,
only this agent's recalld package is cleaned in its single target; dependencies
are reused. Pre/post source and executable hashes distinguish each attempt.
No earlier released target/cache root was recreated.

Changed source: `search/similar_selection.rs`, `search/pipeline.rs`,
`search/read_view.rs` and `search/mod.rs` implement selection; `decay/similarity_scope.rs`,
`decay/observation.rs`, `decay/mod.rs`, `derived_completion/native_exposure.rs` and
`search/adapters.rs` extend the existing observer; `mcp/bridge_adapters.rs` connects
the explicit legacy flag. `search/similar_selection_tests.rs`, `search/strength_tests.rs`
and `daemon/read_only_tests.rs` supply the regressions.

## Remaining owned/integration scope

A4 still owns strict public selectors, coverage/error envelopes and actual wire
activation after native startup prerequisites. This consumer does not activate
ordinary native serving, change get behavior or certify public API2 readiness.
Hume retains the A2 selection/conformance ownership during that join; Huygens owns
mapping and Pascal original-origin/observation dispatch. Supplemental graph
coverage from PR133 must remain reflected in the final public coverage, separately
from ranked totalMatches. V1 graph representation sensitivity is unchanged; no
V1 graph determinism, RIF quality promotion, full-A2 completion or performance
promotion is claimed. A5/A6 measurements and broader public conformance remain.

Raw scratch stays outside Git. Under the user's retention override, after the
resulting PR and results are recorded, owned raw data/cache/source copies can be
released to the central cleanup owner without HOME archival. Tracked source,
reports and small provenance manifests are retained. No raw archive upload is
part of this handoff.

At handoff there are no owned running containers. Active/protected paths are
`/tmp/recalld-implementation/coherent-v2-similar/.validation/cargo`, the single
`target-active`, and unpublished candidate evidence under the same `.validation`.
Previously released `target-baseline` is never recreated. Central cleanup alone
owns deletion. Seven additional completed PR71 archive parts (145,349,318 bytes)
were explicitly released via `/tmp/recalld-hume-additional-raw-releases.json`,
with tracked manifests/reports preserved; this is released volume, not a claim
that this agent deleted or verified reclamation of those bytes. Actual notebook:
`/Users/caleb.evans/work-notes/work/Experiments/Recalld correctness and retrieval research.md`.
No new HOME raw archive or COPY manifest was created under the retention override.
