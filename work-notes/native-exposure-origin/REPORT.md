# Original application contract in exposure

Runtime `4f7d2e667829388b84f0f1feb1802618b20b25bc`, branch
`impl/native-exposure-origin`, direct [PR119](https://github.com/calebevans/recalld/pull/119)
`8ab79b45cf53f277f96c587808615273b0055882`. No feedback126, manual107,
compound/catalog/purge or broad activation dependency.

## Validation

Final frozen third source: **5 origin-focused +9 native-exposure-focused passed**;
**1021 library +2 CLI passed**,4 doctests ignored. Formatting/check passed;
Clippy exit0 **with701 library/773 test warnings (682duplicates)**.
All228 source/Cargo/test/toolchain inputs were sealed and verified unchanged.
The runtime commit's documentation initially transcribed the test warning count
incorrectly; the evidence/docs tail corrects it to the actual773 above. No code changed.

Two failed first-stage checks are retained with exact source archives, logs and
binary SHA256/size/mtime_ns: first4pass/1fail because the new metadata-only test
omitted its required filter (EmptyQuery); second4pass/1fail because the real read
handle requires multi-thread Tokio and the new test used the default runtime.
Only that test's filter and runtime annotation changed. The runtime implementation
was unchanged across all three stages. Third's full run uses the exact passing
focused source. No failing full suite was hidden or retried.

Each stage results.json records commands, start/end timestamps and exits. Full
suite561.74s is a validation duration, not a performance claim. Each source.tar.gz
and source.json is complete. Binary manifests describe the private target's
artifacts; executed paths are in logs. No executable or cache is committed/copied.

## Real path checks

- Original application1/2 remains independent of native lifecyclepolicy1/2 and
  retrievalsemantics1/2. Actual policy migration fixtures, configured clock,
  final receipt hashes, anchors, get/similar and strict reopen are exercised.
- Actual legacy StorageAccessRecorder compares retained oldNone dispatch with
  newAPI1None: each samples once. Explicit sample calls clock zero times. Both
  original1/2 produce the expected raw request hash; close, strict canonical
  inventory validation and legacy reopen preserve exact receipts/control/record.
- Old-only recorder preserves method dispatch for1 and rejects unsupported2
  before observer writes. False queries/get and empty observations do not invoke
  an unused observer or reject an unused origin. Actual nonempty bad3 rejects
  before recorder clock/mutation.
- Existing actual HTTP/MCP get/similar checks now assert original1 receipts.
  Existing partial-publication, per-namespace failure, unknown commit/reopen,
  deferred selection, cancellation/accepted-root and policy-negative proofs pass.

## Reproduction and ownership

Offline Docker `recalld-ci:1.94`, RUSTUP_TOOLCHAIN1.94.1, CARGO_BUILD_JOBS2,
network none,2CPU/4GiB, only own worktree/private cargo/private target mounted.
No HOME, credentials, daemon socket or personal data. Caches cloned from completed
owned feedback target; cargo clean-p recalld in this own target before firstbuild
prevents source-switch artifact reuse. See validation-command.txt, each run.py,
results.json and seal.py. Source remained frozen during each run.

## Scope limits

Existing QueryEngine compatibility wrappers select application1. New private
origin entries and existing recorder/coordinator carry explicit1|2 unchanged to
actual canonical transactions. No new query selector/type, origin inference,
implicit policy upgrade or public keyed-query promise. Native deltaNone retains
full publication fallback. Readiness/readGeneration semantics remain bounded.

Hume owns final hydrated ID/access-kind classification; Huygens owns public
routing and final transport sets. HTTP hits and MCP supplemental neighbors can
differ: final caller-visible deduped namespace-grouped IDs must reach the actual
observer at their integration boundary. This numeric-origin slice does not claim
that whole-query transport matrix, ordinary native serving or public2 activation.
See implementation document and changed-files for exact interfaces and joins.
