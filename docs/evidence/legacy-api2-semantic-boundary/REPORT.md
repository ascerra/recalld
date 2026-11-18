# Legacy API2 semantic admission: bounded correction

Validation sealed for publication review. Final runtime is
`82ad4d949a7b52ca414d2c54517574db28e8a92b`; later seal commits change only
documentation/evidence. No push, PR creation, main merge or deployment is implied.

## Scope and actual parent

Worktree `/tmp/recalld-implementation/legacy-store-selection`, branch
`impl/legacy-store-selection`, directly on published PR157
`5e6115d051fbe102cc22059337e24725aca362be`. Its actual PR148/144 dependencies
are inherited. This correction does not add158,159 or a moving batch/checkpoint
parent. Huygens released the narrow single-wire preflight/projection hunk; native
API1 batch preparation remains his separate work.

A4 §9 and CONTRACTS distinguish an API version from physical format. API2 vector
insertion is semantic even with advisory and autolinking disabled. Schema-less
legacy vectors lack canonical Bound profile evidence. Fresh such writes must
fail; metadata/lifecycle operations can still use actual legacy API2 consumers.
No second profile authority, certificate table, format5 or migration is added.

## Actual BEFORE evidence

Production remained exactly157 during both exporters. Test-only source
`0c6a93c8dcf81fe3572169d7c963eeaf73a59444` produced **three actual failures**:

- Direct owned storage accepted an API2 vector write and advanced G.
- Actual daemon format4 supplied-vector mutation committed parent and child.
- Actual daemon format4 generated-vector mutation called the provider and
  committed parent and child.

Advisory and autolinks were explicitly disabled. These are insertion admission
failures, not evidence about native activation or retrieval quality.

The same old-runtime executable exported one actual completed single, completed
format4 batches with absent/present CAS and valid/all-invalid items, an unkeyed
pending batch and a partially completed batch. The exporter passed1/1.
Test-only source `23f7bfded81cdf2a080f458f9467300662cae4ad` exported actual
post-child publication failure and commit-unknown outcomes, then seven actual
process-exit boundaries (exit91). That exporter passed1/1. Each retained fault
records the actual current-child ID/receipt rather than substituting the parent.

[BEFORE-fixtures.json](BEFORE-fixtures.json) maps every immutable synthetic
fixture to source and SHA256. `before/` and `fault-before/` retain logs, executable
hashes, exit statuses and cgroup evidence. Exporter tests remain intentionally
ignored and are only valid on their recorded pre-admission source; running them
on corrected production cannot manufacture equivalent historical evidence.

## Admission and replay behavior

- Fresh format4 insertion: the existing reservation callback rejects after
  original request/key lookup but before parent/child reservation, provider,
  sampled clock or payload work. All-invalid indexed bookkeeping is retained.
- Existing parent: finalized children replay exactly; unresolved children retain
  reserved IDs and parent/bookkeeping receipts. Admission does not finalize,
  cancel, reallocate or pretend to drain them. The actual RPC returns a structured
  unresolved result, including the already committed subset.
- Direct or reserved low-level insertion: the canonical redb transaction repeats
  admission under the final mutation/publication guard, after original replay,
  lineage/CAS, child capability/policy and physical writer checks, before UUID,
  slot or text staging. Passing an adapter is not authorization to bypass this.
- Matching legacy histories keep their original writer kind and hash. Completed
  replay precedes current profile/config/provider/clock/CAS validation. Changed
  payload/default context/CAS/origin and foreign lineage do not alias a receipt.
- API1 compatibility and API2 nonsemantic forget/feedback/exposure/read controls
  retain their independent participating writers. API1 explicit endpoint tests
  remain honestly labeled API1; they no longer create forbidden fresh API2
  vectors to exercise structural constraints.

The prepared native single endpoint checks the immutable owned entry mode before
profile resolution/provider/clock/reservation. A known legacy-owner rejection
returns typed `UnsupportedMutation`. The direct-variant projection preserves its
profile diagnostic as existing `STORAGE_UNAVAILABLE`/unavailable/`notCommitted`;
HTTP returns503, MCP text equals structured output, and the daemon preserves the
typed result. No receipt, operation ID or readiness certificate is fabricated.
It never guesses a legacy receipt kind or falls back to the API1 producer.
Generic `RecoveryRequired` remains unknown; `CommitOutcomeUnknown` retains the
original operation ID, and `CommittedMutation` retains the entire receipt.

## Validation attribution and findings

The first correction build caught a remaining diagnostic reference to the removed
selection-only helper. It was corrected without weakening admission. A diagnostic
helper then needed byte formatting compatible with the pinned sha2 version.

The first runnable focus had17 passes/3 failures/3 ignored: three replay tests
incorrectly compared disposable FTS WAL/shared-memory bytes after first-owner
completion. Compact per-file diagnostics independently confirmed canonical
metadata, edges, selected/original text and vector bytes unchanged. Replay tests
now assert every canonical file/hash plus exact raw receipt/manifest history;
fresh rejection still compares the complete file set. The raw failed output is
retained until completed-PR cleanup, with a bounded excerpt/hash in the seal.

Source `58be203fb479e240f732e829a05742a2edd11984` passed22 affected
admission/history tests (3 genuine exporters ignored) and17 actual legacy API2
nonsemantic tests (1 genuine exporter ignored). Its full suite passed1325 library
tests (5 ignored), both actual CLI tests, zero-test binary targets and doc checks
(4 ignored examples), in692.01s for the library. Format and all-feature/all-target
clippy exited0 (warnings;19.49s). Peak memory4294967296 bytes, max1194, oom0,
oom_kill0; no cap increase. This is source-specific core evidence, not a claim
that this intermediate source corrected the prepared-wire diagnostic.

The focused actual prepared-wire BEFORE source
`be4415889f457a88d9c19aa621dc65d5c28bbb93` executed HTTP, MCP and daemon before
asserting classification. All three reported `COMMIT_OUTCOME_UNKNOWN`/unknown
for a known pre-work refusal, with zero provider/clock/G/file changes. The single
aggregate probe failed as expected; its output contains all three actual errors.

Final runtime source `82ad4d949a7b52ca414d2c54517574db28e8a92b` passed22
admission/history tests (3 exporters ignored) and24 native single tests (1 external
dense-fixture exporter ignored). The actual new daemon postcommit fault retains
its unknown operation ID/unresolved work, then reopens and replays the exact
committed receipt without provider/profile access. The existing actual native
publication-failure test keeps its full receipt and repairs the same key. The
explicit generic RecoveryRequired projection still returns unknown.
The final frozen source passed **1327 library tests, 0 failures, 5 ignored**
in772.39s, both actual CLI integration tests, three binary targets with no tests,
and doc checks with four intentionally ignored examples. Formatting exited0;
all-feature/all-target clippy exited0 with warnings in10.69s. No warning
suppression or unrelated runtime cleanup was introduced. Full-suite counts
include existing fixture/process helper entries that do no work when their
control environment is absent; only the explicitly invoked BEFORE exporters and
processes receive experimental attribution here.

The final test executable SHA256 is
`e61a70a07f615ba87581da178cc8728fe5f276abccdca9156186370b557f3345`.
Every frozen tracked source/test/fixture input was hash-checked after execution.
[VALIDATION.json](VALIDATION.json), [SOURCE-MANIFEST.json](SOURCE-MANIFEST.json),
[executed-binaries.json](executed-binaries.json) and per-run manifests retain the
exact commands, statuses, image, source and binary hashes. The prior core run's
CLI outcomes are attributed by its frozen source/log; the final seal additionally
hashes every actual test/CLI executable, not only the library binary.

Final peak memory was4098428928 bytes, with max0/oom0/oom_kill0. Docker used
Rust1.94.1, networknone,2CPUs/4GiB/jobs2 and one owned Cargo/target; no resource
increase, source-root switch, extra target clone or blind retry. The validation
container exited and was automatically removed.

## Limits and release dependencies

Only synthetic tempstores were used, in offline Docker with no HOME, credentials
or socket mounts. Process exits are not power-loss evidence. No benchmark,
backend/default selection, public whole-store CAS, ordinary native serving,
complete semantic query admission or full-A1 completion is claimed.

Older Bound-but-uncertified native vectors remain a separate activation gate.
Correct fresh producer composition does not certify old neighbors. Feynman's
required-feature/inventory/re-embedding proposal is not implemented here.

Existing unsupported pending legacy semantic work remains explicit durable
pending state. No operator drain/cancel or automatic migration is invented.
Actual certification/re-embedding and public routing/discovery are separate
validated consumers; a current provider descriptor or supplied ID cannot bless
unknown legacy vectors.

Per user retention override, no raw HOME archive is required. After a resulting
PR and recorded results, central cleanup owns removal of this worktree's completed
`.validation` roots. Keep tracked code, genuine fixtures, compact reports,
worktree/.git and the one explicitly protected active cache.
