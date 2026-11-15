# Legacy API2 insertion and compound origin proof

This experiment exercises actual canonical insertion, reserved batches, the
daemon bridge, startup recovery and offline history consumers. It does not
activate public API2 or complete A1. The implementation and remaining boundaries
are described in [the implementation note](../../implementation/legacy-compound-origin.md).

## Sources and reproduction

- Branch/worktree: `impl/legacy-compound-origin`,
  `/tmp/recalld-implementation/legacy-compound-origin`.
- Actual parents: PR148 `508d98c65b0f824bbcc0290812c63e0dea426edd` and
  PR144 `31a0463b2d7e0e6920c2daf6025759fb2e1d73ca`.
- Test-only baseline join: `277402be42ae5684c3aa0df735244f33f684408d`.
  Manual readiness construction did not compile against the combined interface;
  `beeced5678c71efdd8278d1615c9fc145ca9e282` reuses `report.into()`.
- Actual BEFORE probes: `beeced5678c71efdd8278d1615c9fc145ca9e282`.
- Genuine old1 fixture exporter: `73f1254beb346a4f7017229a4b7f79f9030a6331`.
- Last production change: `a30c39df2a5307a3b22a6d3452c5b569e5571d8f`.
- Full-run source: `af76d557cc74fc7e041c77e77526721184e8dde3`.
- Final test source: `b6fcb6b75a71e8afc2fedd1eb16edf655771b533`. Its only source
  delta from af76d55 is the approved unknown-version test correction and explicit
  format4/native-owner rejection assertions. Production code is unchanged.

Docker image `recalld-ci:1.94`, installed toolchain `1.94.1`; all cargo runs
use `--offline`. Runtime: `--network none --cpus 2 --memory 4g`,
`CARGO_BUILD_JOBS=2`, test threads2, dev/test debuginfo0. Only assigned source,
one owned Cargo cache/target, and small evidence output are mounted. Source is
read-only for tests. No HOME, credentials, Docker socket, personal service or
personal corpus is mounted or accessed.

The source root stayed fixed after the initial package-only clean in the owned
target. No BEFORE/AFTER root switching with an unchanged package fingerprint or
target cloning occurred. Each run records the actual Git source, source-input
hash manifest, logs, and executable hashes. A compile-only failure may list a
previous executable still in the target; it is explicitly **not an executed
binary for that failed source**. See [RUNS.json](RUNS.json),
[FIXTURE.json](FIXTURE.json), and [the final runner](final.sh).

## Actual BEFORE and retained failed runs

The two actual consumer probes failed at the old guards: single API2 insertion
returned `InvalidMutationContext("explicit-v2 insertion is not enabled")`, and
internal format4 batch admission rejected the unsupported mutation version.
Result: **0 passed / 2 failed** at beeced5. Executed test binary SHA256:
`32a4984329f30762a3cda696bf1c9a555b19226ecb4e98eb639e90f9b33773ac`.

The genuine old1 exporter first used the public wrapper, which already rejected
caller CAS. That failed experiment is retained. The fixture then used the actual
private old runner; its first exact-generation assertion exposed optional legacy
autolink mutations. Explicitly disabling autolinks in the fixture isolated the
parent/child transactions. At source73f1254 the exporter passed1/1 and produced
the bundled actual old1 backup plus original receipt/progress rows. It covers
absent/supplied CAS, successful/all-invalid batches. This is internal-runner
compatibility evidence, not a public CAS claim. The exporter is intentionally
ignored in ordinary test runs; it must not regenerate an alleged old fixture
using the new writer.

Runtime a30c39d passed9 focused tests, with the exporter ignored. Expanded tests
then had compile-only corrections at7707b5 (private backup helper, inferred
atomic type, nonexistent receipt scope) and92aea26 (moving a receipt out of a
vector). Those logs remain; no runtime proof is attributed to either compile.

Source b31339d passed15, failed1, ignored1: the offline test incorrectly invoked
legacy-text migration on the already-selected-text fixture. Source50ad020 also
passed15, failed1, ignored1: the corrected backup test then attempted retained
restore into a fresh target. Both are actual validator rejections caused by test
setup. Final af76d55 uses the existing target and its retained ledger; no runtime
validator was weakened. The source backup's metadata bytes are checked unchanged
before the intentional restore/fork.

## Final proofs

At af76d55, **16 focused passed / 0 failed / 1 intentionally ignored**, 15.60s:

The count includes the subprocess entry test, which is inert without its harness
environment; the parent test actually invokes it at all seven exit boundaries.
The ignored historical exporter is not counted as a passing export experiment.

- Actual single API2 insertion through derived completion; exact receipt replay
  before invalid fresh clock/vector, stale CAS and cross-contract conflict.
- Actual typed batch insertion and forget over the daemon bridge; original
  namespace/default-context, supplied scaled-vector/signed-zero bits and CAS
  remain request identity. Omitted/fixed true duplicate defaults normalize alike.
- Required-nullable origin codec and 16 startup/offline corruption cases across
  pre-child and all-invalid parents. Final write rejects an altered manifest and
  equally altered capability because the immutable parent hash disagrees.
- Actual endpoint deletion after reservation, foreign/missing endpoints,
  Supersedes branch/identity conflict and changed namespace policy reject before
  child payload/receipt/generation mutation. Successful explicit links share the
  insertion receipt/effects transaction.
- Fresh automatic/advisory selection blocks before reservation/provider/clock or
  file changes. Pending parents retain exact receipts/reserved child IDs.
  Completed matching replay precedes changed current selection settings.
- Actual provider latch creates a post-reservation child publication failure and
  an unknown child commit. The daemon result retains current child identity;
  reopen reconciles/replays exactly with no second provider call or generation.
- Seven actual subprocess exit boundaries around reservation, payload sync,
  metadata/edge commit and acknowledgement, followed by repeated reopen/replay.
- Genuine old1 raw history/full receipts remain identical. Actual backup/retained
  restore, schema migration, logical export/import and historical restore retain
  old1 plus new4 histories, and old-store requests fail before fresh settings.

The full all-features library run at af76d55 finished **1,318 passed / 1 failed /
3 ignored**, 727.47s, exit101. The sole failure is
`linked_batch_hash_domains_fixed_defaults_original_bits_and_unknown_versions`:
its inherited unknown-version list included format4. This is obsolete after
introducing the approved legacy format4; its exact failure log is retained.
Formatting passed; all-target/all-feature clippy passed with warnings, 42.04s.
Cargo stopped at the library failure, so no CLI/doc pass is inferred from that
command.

The user approved replacing only the unknown4 case with5, adding distinct
format4 hash-domain/API2 mapping and actual native-owner rejection assertions,
and rerunning the affected module plus focused/CLI/doc/fmt/clippy checks. The
separate forged native3→legacy4 startup rejection remains unchanged and passed.
Final b6fcb6b7 results:

| Check | Result |
| --- | --- |
| Native batch module, including corrected unknown-version/native-owner test | 29 passed, 1 intentionally ignored; 103.86s |
| Legacy compound proofs | 16 passed, 1 intentionally ignored; 15.92s |
| Actual lifecycle/text-migration CLI integration | 2 passed |
| Three binary unit-test targets | Exit0; zero tests defined |
| Doc tests | Exit0; four examples intentionally ignored, zero executed |
| Formatting | Exit0 |
| All-feature/all-target clippy | Exit0, 9.21s, with warnings |

The new test closure emits the existing `result_large_err` warning; no suppression
or production warning cleanup was introduced. All frozen source inputs were
verified unchanged after execution. Final library test executable SHA256:
`b1fb780c5eac617b002be232cc8d4725b801959d91147fa41d9ec96c8864c116`.
[VALIDATION.json](VALIDATION.json) and
[executed-binaries.json](runs/final-b6fcb6b7/executed-binaries.json) retain exact
statuses, source attribution and the actual CLI/test executable hashes.

Per user direction there was no redundant local full run for this test-only
delta. **Exact-head full CI remains required before merge; there is no final
local full-pass claim.**

## Limits and resource evidence

All stores and vectors are synthetic. Process exits are not power-loss or
filesystem-fault evidence. Test timings are validation runtime, not a performance
comparison. No backend comparison/default decision, public whole-store CAS,
ordinary native startup or whole-A1 completion is claimed.

Captured cgroup memory events/peak are retained with each measured run. b31339d
peaked at4218847232 bytes with `max=0`, `oom=0`, `oom_kill=0`. The full af76d55 run
peaked at3959500800 bytes with the same zero counters. No resource increase or
blind retry was needed. The correction run reached4294967296 bytes (4GiB),
`max=12`, `oom=0`, `oom_kill=0`, and passed without a cap change. If a future
larger join requires6GiB, it must record the actual OOM and resource change,
coordinate heavy linking, and reuse the owned target.

Fresh legacy API2 automatic/advisory selection remains an explicit unsupported
boundary pending the next durable same-insert consumer; API1 remains unchanged.
The separately coordinated handling of older Bound-but-uncertified native
vectors is also an activation prerequisite. New correct producer inputs do not
certify historical neighbor vectors, and historical receipts are not rewritten.

Per user retention policy there is no raw HOME archive. After a resulting PR and
recorded results, central cleanup may remove this worktree's completed
`.validation` output. Source, genuine fixtures, compact reports, worktree/.git and
one explicitly retained future-task cache remain protected. Publication is not
authorized by this report; exact branch review/READY handoff remains separate.
