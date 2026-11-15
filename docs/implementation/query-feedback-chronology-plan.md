# A3: prepared query exposure followed by public API1 feedback

Status: source-only plan for review. No join, implementation, build or new
executed result. Planning child starts on published PR166 only. PR166 stays at
`3f3ca05d160cad917b716758dd360412a652e9d9`; exact-head CI is still required before
its integration handoff. This document does not change that branch.

## Actual consumer gap and parents

Consume exactly PR166 plus frozen PR151 `2b2560516e1e8e4548c9511f296a72862110caa9`
(runtime `e8754931272dfd77d8762092924bd7b60351a958`) after plan approval.

- PR166 contains actual159 adoption and153 public API1 lifecycle. Its four cases
  establish unknown/adopted/override admission and separately labeled private
  keyed replay, not query exposure followed by feedback.
- PR151 contains actual146+88, including142/140/133/132. Its prepared MCP/REST
  handlers use the actual coherent selection, original2 recorder and completion
  barrier. It supplies the missing query consumer; no separate132 parent needed.
- Do not add156's already-covered phase/clock matrix,150/152/155/160 explanation
  or inventory branches,158 insertion,162 sweep clock, or a general checkpoint.
  Add a parent only for an identified actual API dependency, reported first.

Huygens confirms151 is frozen and he owns no active day0/day7/day8 composition.
He retains query/wire changes and public routing; his current batch work is
separate. Pascal owns this test composition and A3 lifecycle assertions. No
query-hunk edits are planned. Huygens reviewed this scope and confirmed the
existing-helper clock generalization is disjoint from his batch work. Coordinate substantive merge conflicts per hunk
with Huygens/Feynman; preserve159 unknown-binding proof and166 original1 behavior.

**Prepared API2 is not ordinary public API2 activation.** Invoke the actual
`mcp::tools::prepared_query` and `api::search_v2` inner handlers through their real
MCP/HTTP search adapters. Public version2 remains rejected by outer routing.
Day8 feedback uses actual public API1 MCP dispatch/HTTP router from166, original1
and old success shapes. No Remote prepared-query, keyed-query or API1-key promise.

## Source trace and fixture seam

PR151 `search/query_wire.rs::query_contract` resolves namespace identity, obtains
`prepare_coherent_payload_with_semantics`, computes final hydrated exposures,
projects/budgets the response, then invokes
`record_exposures_at_with_origin_work(..., prepared.evaluated_at, ..., 2)`.
The actual recorder calls the existing native coordinator with the query's
configured policy and sampled time. The accepted root finishes before the real
current completion barrier; unknown work is not declared drained. This path,
not a direct recorder call, is the experiment's day7 action.

Reuse166's native migration/attestation/adoption fixture and public lifecycle
adapters;151's query-wire tests show the existing prepared handler construction.
Keep one existing `ManualClock`, shared by QueryEngine, StorageAccessRecorder and
public feedback adapters. If needed, narrowly generalize existing test helper
clock arguments from FixedClock to the existing Clock trait and wrap ManualClock
only to count samples. Do not change production clocks, policies or constructors.
Configure QueryEngine and feedback from the same NativePolicyInputs/decay config;
never infer lifecycle version from retrieval semantics or original API number.

Produce native schema with each actual lifecycle policy: legacy_access_v1 and
explicit_feedback_v2. Select/migrate using existing APIs, and actually adopt the
inherited multiplier before dependent positive operations. No raw policy rewrite,
new default, fabricated receipt, or adoption of genuine historic fixtures.
Creation uses a valid fixed T0 (the fixture's existing1000ms is sufficient), not
a wall-clock-derived event. Retain original observed_at and unknown feedback
provenance. Seed two eligible records through existing insertion before migration.
Day0 is that actual internal insertion/migration fixture, not a public store
request;151 does not include158 and no unused insertion parent is added. Retained
labels keep their existing provenance; do not upgrade them to NativeV2 merely
because subsequent query/feedback uses native writers.
Use existing logical export/import under closed ownership to make identical
synthetic day0 copies with the same record identities; strictly validate/open
both through existing explicit native entry. Rebuild and finish actual owned
publication on both imported copies before recording the baseline or advancing
time; assert the real completion state, not a synthetic ready stamp. No personal
data or live restore.
This is isolated paired test state, not a production multi-writer/cutover claim.

## Bounded checks

One chronological helper, two policy versions, two transport lanes (prepared MCP
query→public MCP feedback; prepared REST query→public HTTP feedback). No repeated
quality×phase×threshold×transport matrix.

1. **Day7 false control.** Advance the shared owned clock to T0+7days. Query
   metadata_only, limit1, depth0 over the two eligible rows, with recordAccess=false
   and RIF Off. Inspect the actual selected UUID rather than assuming generated
   UUID/tie ordering. Snapshot canonical record/namespace/control/ledger/receipt
   bytes and existing owned-fixture catalog-selected payload/vector/edge files
   around the action; pending/unresolved access work is zero after completion.
   No receipt, anchor/stability/timestamp/history change, or persistent loss.
   Cache accounting and accepted ticket sequence are not claimed immutable.
   Use the existing provider-that-fails-if-called to prove this metadata query
   and subsequent feedback do not call a provider. No fake search/observer.
2. **Day7 true action.** Repeat through the same actual prepared handler with
   recordAccess=true. Exactly the final returned UUID gets one exposure; the
   eligible but omitted record remains unchanged. Derive/deduplicate expected UUIDs
from the final returned hydrated payload, never a candidate pool or unreturned
neighbor list. Assert original2 exposure
   receipt, actual readGeneration before commit, exact query evaluatedAt in the
   transition and retrieved timestamp, current readiness and no pending/unresolved
   work. Count one sampled query time, with no independent recorder sample.
   In v2, anchor/stability/reinforced_at/observed_at are unchanged; in v1 the
   anchor resets to day7 without stability growth. Phase/payload stay retained.
   Preserve exact pre-existing receipt rows and assert the returned record's
canonical token/G transition while the omitted record's token stays unchanged.
A zero-result metadata filter then
   produces no exposure receipt or canonical write; this is not a durable keyed
   no-op. Depth0 does not claim graph duplicate-origin or all-namespace coverage.
3. **Day8 actual feedback.** Advance each paired instance's existing clock to
   T0+8days and submit quality3 to the actual public API1 handler for the same
   returned ID, in the observed and untouched no-query copies. Check original1
   receipt/hash, one clock sample, full receipt persistence and completed work.
   V2 resulting stability/difficulty match the no-query control despite day7
   retrieval; reinforced_at is day8, retrieved_at remains day7 only in the
   observed arm, observed_at is immutable. V1 preserves its day7 anchor input
   and differs from the no-query arm where the existing formula predicts it.
   Compare v1 against the existing shared planner's day1-vs-day8 elapsed inputs;
   that checks adapter/policy agreement, not an independent numerical oracle.
4. **Reopen.** Close all owners/adapters and strictly inspect/reopen both copies.
   Preserve original and new full receipts, anchors/timestamps, retained phase
   and payload. No cross-store generation ordering, full logical-state-hash,
   deterministic operation-ID or global idempotency assertion.

The same-clock checks must use the configured query capture and actual final
recorder call. A hardcoded direct exposure invocation is not an acceptable
substitute. If a composed consumer fails, retain the actual RED and consult its
owner before a runtime correction; do not bless unknown fixtures to pass.

## Acceptance reconciliation and deliberately separate work

| A3 requirement | Existing scope / next evidence |
| --- | --- |
| Exposure vs feedback | Main new gap selected here: actual prepared selection→observation→public feedback, paired v1/v2 chronology.151 and166 separately do not establish it. |
| Read-only and observation completeness | This adds actual prepared recall false/returned-only/empty controls. It does not replace all ID/batch/graph/replacement/cross-namespace conformance or duplicate Huygens/Curie consumers. |
| Clock determinism | One capture and chronological fields here. Cross-transport full state hashes and reproducible UUID/RNG/reset require the actual A6 ownership harness; no invented determinism claim. |
| Overrides, phase boundaries, permastore, terminal validation | Existing156 matrix/166 adoption and earlier native lifecycle/maintenance proofs remain separately cited. No repeated phase/quality/extreme-clock matrix or new phase writer. |
| No resurrection, deletion races, stale sweep | Existing atomic/ledger/native maintenance and sweep proofs remain separate. This query/feedback join cannot prove all restart/cache/backup loss invariants or replace concurrent sweep guards. |
| Retry/crash, migration, restore |166 preserves genuine historic replay, actual adoption and original receipts. Native fault/restore/rollback proofs retain their owners; this unkeyed query/public feedback test adds no retry protocol. |
| RIF boundary/permutation |151's actual ranked baseline is RIF Off. Hume confirms frozen150 provides actual Off/Shadow/Experimental false-only operator byte identity and Off/Shadow explanation identity, not true-observation/public-toggle proof.150 is not an ancestor here; cite its separate evidence, add no unused parent or duplicate harness. This plan makes no all-mode/permutation claim. |
| Promotion/quality/performance | No A3 completeness, ordinary native/public2 activation, default promotion, RL quality or A5 performance claim. Normative acceptance remains broader than a passing unit suite. |

Hume's precise separate RIF evidence is PR150
`d2e04cce5bdf62dab464cadafaa435c4c577806e` (actual146+102), runtime
`2e32a81b5f8fdc2c76864791dff6af75c5bb237a`; report
`/tmp/recalld-implementation/rif-score-explanations/docs/implementation/rif-score-explanations-report.md`.
Its21 focused /1064 library+2 CLI results are owner-reported here, not rerun by
this plan. Huygens retains the later actual experimental consumer join. There is
no justification to import150 merely to name the remaining RIF scope.

## Execution gate and resources

After review only: normal merge exact151 into this166 planning child, record actual
conflicts/owner reviews, add the narrow tests, freeze, then run new cases and
relevant prepared-query/public-lifecycle/adoption/history regressions. A real new
composition warrants one frozen full suite plus fmt/clippy, all offline Docker
Rust1.94.1, networknone, 2CPU/4GiB/jobs2. Preserve every failure and source/binary
provenance; no source changes during runs or automatic evidence-tail reruns.
Exact published-head CI remains required before integration.

Keep the single reusable /tmp/recalld-a3-matrix-target and private Cargo cache.
If switching source mounts, seal previous executable provenance then clean only
this package's outputs before a fresh compile; no dependency-cache clone. Use a
new bounded raw root only when approved validation starts. The completed166 raw
root was centrally deleted in batch40; never recreate it. Worktree/code/reports,
HOME compact results and Git stay preserved. No join/build was performed to write
this plan.
