# Original application contract through exposure

Validated implementation on direct PR119 (`8ab79b45cf53f277f96c587808615273b0055882`).
No feedback126, manual107, compound catalog/purge, or public routing parent.

The existing AccessRecorder publication method and accepted native coordinator
receive a required final `original_contract_version: u32`. The real storage
recorder carries that value to either existing physical writer. Native and legacy
local API-only restrictions are removed while MutationContext::validate, namespace
checks, physical writer admission and native ownership remain. The legacy shared
scope helper also admits standalone feedback2 without changing retention math.
Its positive/conflict regressions are included; no public feedback2 activation.

QueryEngine's existing Rust convenience entries explicitly mean application1,
including search_with_semantics(V2). New private explicit entries append the
number to the existing arguments:

- search_checked_with_origin_work
- get_memory_with_origin_work
- similar_with_access_checked_with_origin_work
- record_exposures_with_origin_work
- record_exposures_at_with_origin_work
- record_similar_accesses_with_origin_work

No SearchQuery field, mutable engine request default, or second public selector
is added. Huygens passes the existing ContractVersion parser's number at the
public routing join. Current API1/MCP/daemon compatibility callers reach the same
actual writer with1. Application2 query routing remains separately gated.
Pinned read-engine internals defer every exposure and keep RejectExposure; the
outer query's original number accompanies the final selected set. Their unused
compatibility number cannot select or authorize a mutation.

Old-only AccessRecorder implementations forward1 using the original sampled/None
method dispatch and reject unsupported2 before invoking a mutation. False-mode
queries never invoke the observer. Actual StorageAccessRecorder preserves both
origins, configured policy, accepted parent, partial receipts/unknown identity,
and the existing sampled clock rule. Native inferred samples are forwarded;
legacy inferred samples retain the old fallback. Explicit `_at` preserves time.
Native publication keeps the current full fallback; Hume's optional delta join
is separate. No receipt generation substitutes for pre-exposure readGeneration.

Validation covers actual native policy1/2 × application1/2 × retrieval
semantics1/2, raw request-hash discrimination, get/similar/false behavior,
legacy1/2 storage receipts, old-only dispatch, existing partial/unknown and
publication proofs, strict reopen, full suite/fmt/Clippy. Final5 origin-focused
and9 native-exposure-focused tests passed; full1021 library+2CLI passed,4doctests
ignored. Formatting passed; Clippy exit0 with701library/773test warnings
(682duplicates). All228 source/Cargo/test/toolchain inputs were sealed and
verified unchanged. Two prior failed fixture runs are retained: missing required
metadata filter (EmptyQuery), then single-thread Tokio used with the real blocking
read handle. Only the new test filter/runtime annotation changed; runtime code
was unchanged across those runs. Selected UUID hashing remains internal exposure intent, not public
keyed-query semantics or deterministic ID/RNG support.

## A2 selection-context overlap

Hume's source-only coherent-v2-selection child also replaces the body of
search_checked_with_work with a selection-context helper. At the actual join,
keep the existing compatibility wrapper selecting1 and thread the required final
number through search_checked_with_origin_work → search_checked_with_selection
→ search_admitted. Preserve his validated query/namespace/strength/eligible
selection context and the original recordAccess intent. That context does not
supply or infer the application origin. No A2 source is merged for convenience.


## Legacy clock and reopen review checks

The real legacy recorder test now compares the retained pre-change
record_access_batch_with_work(None) method against publication(None, original1):
both must sample the injected recorder clock exactly once. Explicit samples must
sample it zero times; unsupported nonempty origin3 must reject before clock.
The same test closes the real owner, runs strict canonical inventory validation,
reopens the legacy engine and compares the exact API1/API2 receipt bytes, control
state and record bytes. These additions passed on the final validated source; the
retained old method is a comparison path, not a historical daemon/binary run.

The source regressions also pass unsupported origin3 through false-mode query/get
and empty observation paths: no observer invocation, receipt, control change or
recorder clock sample is permitted when the origin is unused. This is an internal
no-observation check; public transport selectors remain independently validated.

## Transport activation prerequisite

Hume/Huygens own final post-supplemental transport selection. HTTP hit sets and
MCP neighbor sets can differ: only the final returned UUIDs, deduplicated and
grouped by namespace, may drive the actual exposure call. These private origin
entries carry the caller number alongside that set; they do not select it or
certify whole-query exposure parity. Native delta remains None/full fallback.

## Sealed evidence

Runtime `4f7d2e667829388b84f0f1feb1802618b20b25bc`; evidence `96bef33e1cd1c4189112938541e8a06a900c0d99`.
See `work-notes/native-exposure-origin/REPORT.md` for all three source runs.
Actual HOME copy: `/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-exposure-origin-96bef33`.
COPY.json SHA256 `cec0444fb61de19c4bae5d5f34f9da3d43a1c00abc325789f16df9c9ccb2b0b9`.
All44 files/5,628,094 bytes were individually hash-verified; the existing research
notebook was appended. No executable or cache was copied.
