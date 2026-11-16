# Actual readiness observation for committed errors

Measured runtime `f8765b28fdc899988c26ce5c64e82cbfed60f7d9`, branch `impl/readiness-observation`,
direct parent PR96 / `387c7118864040c68d7d28c44fb2600ade520a45`.
This is a minimal consumer seam for Huygens's failure envelope; no allocation,
checkpoint, later catalog/policy, or broad integration ancestry. It is handed
off with its actual envelope consumer rather than published as another unused
registry layer. [Interface](interface.md); [changed files](changed-files.txt).

`Recalld::readiness_observation(deadline)` uses the existing owner and registry.
It reads actual canonical generation and durable effects/purge/batch state in
one redb transaction, retains real component stamps including None, and observes
the caller's existing tracker without closing admission or draining its root.
It neither emits ready=true nor sums overlapping work categories. An unknown
canonical handle or expired deadline produces an error, not a guessed generation.
The caller must retain its committed receipt independently of this observation.

## Exact isolated validation

PASS **1002 library + 2 CLI**, **2 focused actual observation tests**, **10 inherited
metadata-freeze regressions**, fmt and clippy exit0 with warnings. Actual source
`f8765b28fdc899988c26ce5c64e82cbfed60f7d9` was tested; no result inherited from a different checkout.
Driver status passed/exit0 at `/tmp/recalld-readiness-observation-full-ifcj2irw/output`.

Rust1.94.1, image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`, Docker network none,
2CPU/4GiB, private cargo and source-specific target; `cargo clean -p recalld`
ran after privately cloning dependencies. No home/credential/socket/personal
store mounts. Cgroup oom/oom_kill0; max reclaim events4830. Source manifest pins
224allowlisted files; per-source executable hashes,
commands, full logs and resource evidence are retained. No CI or transport-v2
conformance claim is made by these Rust checks.

The actual fault test commits a canonical delete, makes the vector rebuild fail
with the wrong dimension, and retains the complete committed receipt. Observation
reports that receipt's actual current generation, FTS=None, old other stamps,
1durable effect and the caller's 1live root. It cannot deadlock by draining its
own caller. Real completion repairs all4stamps at the same generation and clears
the durable effect; the caller remains registered until explicitly finished.
Initial unpublished stamps and unavailable/deadline cases fail honestly.

## Remaining consumer and correctness gates

The successful barrier Readiness shape is unchanged. This observation is not a
snapshot, admission closure, readiness certificate, public-v2 activation or full
A6 implementation. Counts are actual observations, not unique operation totals.
Current-state row scans check the monotonic deadline cooperatively; blocking
OS I/O is not hard-bounded. Startup/offline validation and completed restored
purge ancestry remain unchanged.

Huygens owns the actual failure envelope and the explicit nullable/absent-stamp
wire decision. That consumer must keep receipts on every postcommit observation
failure. Matching applied generations cannot certify an incorrect publisher:
Hume's separately confirmed unchanged-96 SQLite FTS5 score/rank rebuild bug still
requires its actual fix and raw-score/rank replay. No readiness stamp exception
or score normalization was introduced here.

HOME artifact copy:
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-readiness-observation-f8765b2`.
Final copy verification is handed off separately.
