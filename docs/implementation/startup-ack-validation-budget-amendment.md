# Startup ACK validation budget amendment

The final immutable source is `72b9a51ff0f0e35c83fb77bb1e3a3dc20715368e`.
The original final driver completed compilation and 12 focused tests, then its
900-second full all-targets invocation expired. Final retained stdout contains
1,190 unique completed test lines and no observed failure line; it has no suite
conclusion. Driver exit was 1 (`TimeoutExpired`), and the original handler did
not record the cargo exit. Post-run source verification matched all 7,003 files.
This is incomplete validation, not a pass or evidence of a runtime regression.

The plan's 15-minute cap was intended for a focused invocation. The coordinator
reported other full native suites taking about 20–25 minutes and explicitly
authorized one timeout-only amendment: full-only validation with a 30-minute
wall cap, then fmt/clippy with a separate combined 10-minute cap. Any actual
failure stops this path for diagnosis. There is no further retry authorization.

The follow-up uses the same physical immutable source export, same existing
compiled executables and same private cargo/target. It does not clean, switch
source roots, repeat focused tests or run the optional 1,024-row case. Complete
source and executable hashes are verified inside the container before and after.
Docker remains offline, 2 CPUs, 4 GiB, jobs=2; no personal mounts. The original
partial logs and late resource sample remain separate from follow-up evidence.
That late sample includes compilation/focused/partial-full work, is not a final
counter snapshot, and cannot attribute CPU or memory cost to ACK batching.

Original run: `.validation/final-001`; follow-up: `.validation/full-002`.
The follow-up records timeout/process-group termination explicitly and captures
cgroup counters before, after full validation and at final verification. No
runtime/source change accompanies this budget amendment.

Outcome: the unchanged full-only follow-up exited 0 in 1,035.154 seconds;
1,378 library tests passed (1 existing fixture generator ignored), 3 CLI
integration tests and 15 example tests passed. Fmt and clippy all-targets exited
0 within their separate combined budget. Both before/after source and all
14 executable hashes matched. The original partial remains incomplete.
