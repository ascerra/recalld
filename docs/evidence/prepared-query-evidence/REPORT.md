# Prepared query evidence consumer

Frozen runtime: `8e0a5521315a35a2fd4931686b0a55e2833e6675`.
Validation is complete. This is a frozen consumer handoff, not public activation.

## Actual dependency and ownership scope

This is an actual consumer of PR151 `2b256051`, cosine evidence PR155
`46b43ae1`, canonical eligibility PR160 `e9b823a3` (including PR152's source
units), and own/origin RIF evidence PR150 `d2e04cce`. Normal merges compose
these parents at `7a86b674`. Halley released the isolated branch and serializer,
prepared caller, schema and test hunks. Hume reviewed the producer composition
and exact frozen consumer source. Those reviews are source evidence, not
independent test execution. No catalog, namespace-width or list parent is added.

The coordinator approved this bounded implementation and validation under the
user's standing authorization. Frozen parent and queue branches were not edited
or pushed. Source lives in `/tmp/recalld-implementation/prepared-query-evidence`.

## Behavior

Prepared similarity accepts strict optional `explain`; omission/false still
omits the field. True projects the actual selected row's `cosineMax` evidence:
own cosine, four distinct own-admission states, threshold, admitted own score,
inherited floor, eligible origins, omitted count, final similarity and version.
The returned explanation displays at most64 eligible origins; internal
O(scored) capture/work is not bounded by that display cap. This consumer neither
recomputes a dot product nor labels cosine as the ranked `baseline-v1` formula. Missing evidence
fails before observation. Selection, ordering and exposure logic are unchanged.

Ranked coverage now exposes each channel's nullable `examinedUnit` and separate
nullable `canonicalEligibility` with examined/passed rows and sparse first
rejection counts. These are the producer's measured units and loop rows, not
physical reads, unique fused coverage, total corpus coverage or additive channel
counts. Not-requested and unmeasured values remain null. Existing final
supplemental graph/chain budget and omission diagnostics remain intact.

Ranked own/inherited-origin evidence includes each actual `rifMultiplier`.
Prepared ranked queries explicitly reject configured Shadow/Experimental RIF
before accepted work, namespace lookup, provider or observation. Direct coherent
operator queries retain their experimental behavior; a real producer test proves
an inherited origin's multiplier differs from the returned terminal's1. That
operator result is not presented as a public prepared Off-schema fixture.

Strict schemas describe the distinct cosine shape, known units/rejection keys,
nullable measurements and prepared Off multipliers. Generation identities retain
strict decimal strings. Full projection precedes the existing whole-response
budget and observation boundary. No v1 result/default, scorer, codec, registry,
public capability or activation change is included.

## Executed regressions and fixture corrections

- `001`, source `42d30b30`: compilation stopped because the test called a private
  projection module and omitted a required recorder-double method. No executed
  behavioral RED is claimed. Both fixture issues were corrected through the
  existing projection entry point and full trait implementation.
- `002`, source `4c9c8ca1`: three intended failures showed absent canonical
  eligibility (`null` instead of `canonicalRecords`), absent origin RIF
  multiplier (`null` instead of the actual producer's approximately0.870694),
  and similarity `explain:true` rejected as unsupported. Its fourth failure was
  invalid metadata test input returning400, not proof of the RIF defect.
- `003`, source `d7879564`: with valid metadata criteria, configured Shadow and
  `recordAccess:false` actually returned MCP success instead of rejecting. This
  is the executed RIF admission RED. Production was unchanged across these
  test-only commits. All failed sources/logs and produced executable hashes are
  retained separately.

The four new consumer tests pass on frozen `8e0a5521`. Actual prepared MCP and
REST adapters agree, including MCP compact text and structured content. Dense
similarity uses a genuine owned fixture with70 eligible origins and verifies64
displayed plus6 omitted; removing explanation exactly recovers the plain result.
The metadata fixture verifies actual first-rejection and channel units with
canonical bytes unchanged. Both configured non-Off modes and false/true access
reject through actual adapters; panic-on-use provider/observer doubles and empty
pending/unresolved work controls remain satisfied.

The inherited selector-negative fixture now uses invalid `explain:null`, because
true is deliberately supported; no assertion about invalid-before-exposure was
weakened. Existing current/history, allCompatible scope, unknown profile and
full multi-namespace receipt checks remain. The actual4MiB rejection and3MiB
successful observation cases now include explanations; the latter verifies the
whole framed MCP response fits. Existing final-barrier failure preserves the
full receipt and known identity. These controls do not prove a general transport
bound by measuring one fixture; the existing counting admission remains active.

## Validation and provenance

Focused: **25 passed** (4 new,14 strict query,6 cosine producer,1 coherent RIF).
Formal JSON Schema2020-12: **61 actual output/input cases passed**, plus mutation
negatives for nested unknowns/types, cosine algorithm/states/origin cap, coverage
units/counts/reasons, prepared Off multiplier and strict generation endings.
Full all-feature suite: **1,137 library +2 CLI passed**,0 failed;4 pre-existing
ignored doctests. Library duration369.31 seconds. Fmt and clippy exited0. Clippy
reported749 library warnings,809 library-test warnings (714 duplicates), and
one binary warning (also duplicated for its test target); this is not a
warning-free run. No runtime or test change followed the focused/full freeze.

Every run exports259 tracked build/test files from its exact committed blobs and
mounts the source read-only. Source hashes are checked before/after; each source
switch uses `cargo clean -p recalld` in the one owned target only after preserving
the previous test executable hash. Focus and full use the same004 source and
target without a source switch. Rust1.94.1, `recalld-ci:1.94`, network none,
2CPUs/4GiB/jobs1; no home, credentials, personal store or daemon/socket mounts.
The focused GREEN peak was3,576,360,960 bytes, memory.max events0 and OOM0.
The full/lint container peak was2,698,870,784 bytes, memory.max events0 and OOM0;
it reused the already compiled focused target. All4 snapshots'259 files match
their exact committed blobs, and9 final executed/CLI binaries are hashed in
[RESULTS.json](RESULTS.json). The library hash stayed
`d07dfe450c7a40a0032db23200a3ad8cffebbed9ae33581e3baf1b9151ce4aa7`
across focused and full. Source manifests, exact scripts and bounded failure
excerpts are adjacent; complete raw logs remain at the protected paths.

Canonical byte comparisons cover the fixture's metadata, edge, vector and
selected text files. They do not claim every file in the directory, SQLite SHM
coordination, or external state is byte-immutable. Native similarity fixtures
use the inherited owned migration/attestation setup; this is not proof of later
ordinary native startup or complete query certification readiness.

## Remaining gates and cleanup

Public contract2 remains closed. Eventual main integration must preserve PR177's
same-view supplied-vector certification guard and validate the actual joined
native producer/startup boundary. Remote query RPC, live discovery/output schemas,
remaining writer/origin closure and A5 overhead/page measurements remain separate
gates. This child is not full A4 acceptance or a performance promotion.

Raw roots `prepared-evidence-001-red` through `prepared-evidence-004-green` under
`/tmp/recalld-huygens-active-validation` remain protected with the worktree and
one existing cargo/target. Central cleanup has exact ownership notifications;
no raw release is implied by a passing local run. No raw HOME archive or cache
clone was made. All owned containers have been retired; the one cargo/target is
retained for future work. Cleanup performed by this task:0 raw bytes deleted.
