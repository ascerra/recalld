# Original application contract in native feedback

Parent: published PR126 (`179edf0d14d383466253a62b892c3741b5d4d9b1`), no
compound codec/catalog/purge parent. Local native lifecycle restriction removal
reuses the existing MutationContext::validate1|2 and unchanged native ownership,
namespace, policy/token, receipt replay and terminal guards.

The private normalized Request now stores `original_contract_version: u32`.
`parse_private` requires an explicit1|2 selector through the existing shared
ContractVersion parser; `.wire()` forwards that exact number. The private daemon
aliases call this parser. The common executor constructs the actual context with
the saved number. It never selects application version from schema or lifecycle.
`parse` remains strict prepared-public2; existing public routing remains gated.
No public API1 native activation or public2 serving claim is made.

A matching original request replays before new CAS/policy/clock checks. Same-key
1↔2 conflicts. Native feedback/exposure core accepts both domains while preserving
one sampled clock and canonical transaction. Forget1 still needs Curie's narrow
native_records context child; this branch does not change that writer or pretend
a legacy wrapper provides native semantics.

## Evidence currently available

Before runtime edits, an isolated unchanged126 writer authored real API2 feedback
and forget receipts through actual coordinators. The same binary proved API1
rejected before clock/record changes. Its embedded32,657-byte synthetic ZIP
contains strict canonical inventory and exact raw receipt expectations. That
fixture is an actual historical writer output, not a hand-authored proof row.
Before executable SHA256:
`620aed3c1b48b84128379a3b1a91ef5278ae9a00c2f56a7c67a9b9da7dd0a8f6`.
Fixture SHA256:
`b1205839f62ebc7abb383296a137be1de4ce3dc4c9f14a6a8c0a5ec91a58040b`.

The completed frozen run in `.test-scratch/final/` passed1140 library tests and2
CLI tests, with4 doctests ignored. fmt passed; Clippy exit0 with890 library and954
test warnings (865duplicates). All274 inputs were hash-verified; executable
provenance was recorded. This run predates the later test-only additions below.
No source was edited during that run. Earlier focused source/logs remain retained.

After that run, source-only additions cover booleans and an emulated pre-change
private2 handler using the actual unchanged strict2 parser over real remote client
and socket framing. They check API1 rejection without a mutating fallback. This
is NOT execution of a historical daemon binary. After explicit END, all five `original_api` tests passed on the final test
source; fmt check passed and Clippy exited0 with890 library/954 test warnings
(865duplicates). The274 input files were sealed and rechecked unchanged. Only
origin_tests.rs differs from the completed full-suite source, verified by hashes.
The old-handler case is included in those five focused tests, not the earlier
1140-test full run.
The genuine old2 store fixture is separate evidence: strict reopen, exact raw
receipts, historical replay, export/import and production fork restore pass;
old-lineage retry after fork fails StoreLineageMismatch without sampling time.

## Remaining joins

Huygens owns public selector routing/API1 outward compatibility and shared failure
observation. Curie owns narrow forget1 writer admission. Exposure119 and manual107
origin propagation are independent worktrees. No new keyed-query or keyed sweep
noop semantics, schema migration, receipt encoding or default policy is added.
The final test-only tail is validated separately from the earlier full-suite
source. No1141-test full run is claimed. Evidence is sealed in
`work-notes/native-feedback-origin/REPORT.md` at `adb9e4c`.

Actual HOME copy: `/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-feedback-origin-adb9e4c`.
COPY.json SHA256: `c5a6e87a2305e4ec4e84cf72b6d4f29c391bb1f19aa10edb456c09a645de023a`.
All40 files/7,319,811 bytes were individually hash-verified; the existing research
notebook was appended. No executables or caches were copied.
