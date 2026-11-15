# A3 acceptance reconciliation: adopted policy through public API1 lifecycle

Status: implemented and validated in the bounded159+153 composition. See
`work-notes/adopted-public-lifecycle/REPORT.md` for exact source/results, preserved
failures and the outstanding published-head CI gate. The following is the
approved pre-implementation plan, retained with its original scope limits.
PR162 is separately published/frozen; do not edit it.
Owner: Pascal/A3, with Feynman policy-proof and Huygens wire ownership retained.

## Actual remaining gap and minimal parents

Proposed implementation consumes exactly:

- PR159 `b71793910b0fc5593bb9644317a1f514ee6a303e`, runtime
  `3e5fd80658e5d8ba2eec92a1c5e27bf8e45ae4a9`: actual inherited-policy binding,
  offline preview/adopt, strict proof and configured-input admission.
- PR153 `a2a8866dd898dff7eae01dd49b23d8cd18532980`, runtime
  `1364b27242dd3b0cf33a81eb9762a27a1f999cc8`: actual public API1 HTTP/MCP and
  negotiated, nonretrying daemon lifecycle adapters, original1 success/error shapes.

Planning worktree `/tmp/recalld-implementation/adopted-public-lifecycle`, branch
`impl/adopted-public-lifecycle`, starts on159 only. After plan review, merge the
exact153 parent normally; no cherry-pick duplication or blanket conflict choice.
Report any actual shared-core conflict or required runtime correction separately.
No extra156 matrix,162 HTTP clock,158 store,149/151 query, or integration checkpoint
parent merely for convenience. The159 ancestry already contains its actual143+135.

Evidence inspected:

- PR159 `src/system_inherited_policy_tests.rs::restart_case`: actual config loader,
 owned initializer, false get, unknown/mismatched binding rejection, real offline
 preview/adopt, manual sweep and direct DerivedCompletion feedback with context2.
 It does not invoke public API1 feedback/forget requests after adoption.
- PR159 `src/storage/inherited_policy_tests.rs`: unknown None direct structural
 forget, adoption token/history invariants, historical keyed adoption replay,
 import/fork and strict malformed-proof tests. These need no duplicate harness.
- PR153 `public_api1_tests.rs` and `wire_tests.rs`: real public lifecycle consumers,
 configured policy/original1, known-receipt failures and negotiated Remote routes.
 They predate adoption admission. Use their existing fixture/transport helpers.
- PR156 `public_matrix_tests.rs`: public feedback chronology/retained phases and
 direct recorder day7→public feedback day8. That explicit setup is not a public
 query exposure proof; do not reproduce its quality/phase/extreme-clock matrix.

Feynman confirms no active159+153 consumer edits; this public consumer proof is
available for Pascal. Huygens confirms public API1 query remains the inherited96
path; prepared API2 query151 is frozen, but later query coverage/explanation joins
are his work and not ready. Do not duplicate query activation acceptance. He has
no overlapping lifecycle/adoption test edits. This selects adoption→public
lifecycle as the concrete next gap; public query exposure→feedback remains a
separate future composition requiring his actual validated query consumer.

## Bounded executable cases

Use actual legacy insertion→schema migration→lifecycle migration to produce a
native explicit-feedback namespace with a genuine inherited None/unknown binding.
Do not replace None with Some(1), fabricate adoption receipts, or rewrite schema
rows to make a positive case pass. Preserve original observation/anchor/retained
payload before adoption. Use the existing owned clock and counting test wrapper,
and the same explicit NativePolicyInputs/config in each participating adapter.
No second runtime policy source or new public control. Quality3 suffices here;
this is admission/transport proof, not another formula/retention matrix.

1. **Unknown policy: feedback rejects; structural forget remains valid.**
   Through actual public MCP and HTTP, plus the existing negotiated Remote lane,
   a fresh feedback request must fail using the existing typed error mapping,
   with unchanged canonical record/control/receipt rows and zero mutation clock
   samples. A request may accept and finish an existing work ticket: do not claim
   no admission ticket merely from no canonical write. Then public forget on a
   retained record succeeds with original1, one mutation clock sample, a terminal
   ledger/receipt and completed work. The namespace remains unknown; forgetting
   does not adopt policy or project strength. Strict inspect/reopen must validate.

2. **Actual adoption and changed configured global.**
   Close all fixture owners, capture the real preview and apply its keyed offline
   adoption, then reopen the same canonical store and construct the public
   adapters from the configured inputs. Adoption changes revision/tokens only;
   assert preserved anchors, timestamps, phase and retained payload before the
   public request. Public unkeyed feedback succeeds with original1, one mutation
   clock sample and the old API1 success shape; inspect its persisted transition
   and receipt, and require accepted work completion.
   Reopen with a different effective inherited multiplier: a fresh feedback
   rejects without mutation/clock, while structural forget stays available.
   On a retained peer, close ownership, perform a real new preview/adopt and
   reopen; the same public consumer can then apply fresh feedback under the new
   canonical revision. Preserve prior receipt bytes across adoption and reopen.
   Count preview sampling separately from per-request mutation clock sampling.

3. **Explicit namespace override control.**
   A namespace created with actual Some(1) needs no inherited adoption. Changing
   the unused configured global multiplier must not change its resolved policy
   or silently create an inherited binding/revision. A public feedback request
   still works, using the explicit override and original1. Compare only relevant
   persisted numerical/anchor fields with the shared planner under that override;
   this proves policy/adapter agreement, not an independent formula oracle.

4. **Historic keyed replay only on an existing keyed API.**
   Public API1 success requests are unkeyed; never invent an API1 idempotency key
   or call repeated public success a replay. Reuse the existing prepared private
   lifecycle request/RPC alias with explicit original1 and a supported key for one
   complementary replay case. First commit with adopted inputs, then change the
   configured global (and, in the strongest case, explicitly re-adopt a new
   revision offline). Retry the original prepared request and expected G: return
   its same full historical receipt/operation/G before current policy, fresh CAS
   or clock checks; do not mutate anchors/history or allocate another operation.
   Different quality with that key still conflicts. Label this private keyed
   contract proof separately from public API1. Existing offline-adoption replay
   tests remain separately authoritative; no new global/cross-copy key claim.

Use a few focused tests around these cases, reusing153's actual HTTP router,
MCP dispatcher and one-connection Remote fixture. Avoid the full Cartesian
layout×quality×phase×transport matrix from156. Native explicit-feedback is the
main admission case; legacy disk is not subject to this native binding and its
already-tested API1 compatibility is an affected regression check, not a new
migration fixture. No public2 routing or ordinary native startup enablement.

## Integration, validation and scope limits

First inspect the exact159+153 diff and compile candidate. Preserve any real RED
before a runtime correction; missing coverage alone is not a demonstrated defect.
Tests should live in a narrow child beside the existing native_records public
consumer tests. Reuse fixture constructors with parameters only where needed;
do not copy whole adapters or create a parallel harness. Feynman retains shared
binding/history codec changes; coordinate any concrete seam defect before editing.
Huygens retains public schemas/routing/capabilities. Do not change their semantics
merely to satisfy this test join.

After implementation approval, freeze source and run new focused cases plus
existing public API1, legacy lifecycle decoder, inherited-policy and historic
receipt proof tests in offline Rust1.94.1 Docker (2CPU/4GiB/jobs2/networknone).
An actual new two-parent composition warrants one source-specific full suite,
fmt and clippy; preserve failure attribution and avoid redundant full runs for
artifact-only tails. Verify source/executable provenance. No heavy job begins
for this source-only plan.

Retain one protected private Cargo cache and `/tmp/recalld-a3-matrix-target`.
Use a fresh bounded raw root, never recreate cleanup-released roots. Clean only
this package's target outputs when changing mounted source to avoid stale mtime
artifacts; retain dependency cache. Once a resulting PR and results are recorded,
release raw runs through central cleanup; no raw HOME archive.

This proves only adoption admission and actual public lifecycle composition.
It does not complete A3's public query exposure→feedback, permutation/RIF/loss
acceptance, all-transport deterministic state traces, startup writer inventory,
profile certification, public2 activation, default promotion or RL quality gates.
Those require their actual remaining owners/consumers; do not infer completion
from a passing isolated consumer join.
