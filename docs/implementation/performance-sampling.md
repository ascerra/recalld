# A5 sampling readiness handoff

Candidate runtime/protocol/QA: `8a36b339ffe876d3f88580e9361a333a6602a2f7`,
direct parent PR71 `7b1a17f`. Probe-only uninstrumented anchor:
`3e87acff8631bc3d60a1fd23ef7f41f7ff3a1c5e`, on the prior probe anchor with
runtime parent `3b590fb`. No RIF/native/batch/ReadView ancestry is added.

The candidate is ready for coordinated measurement. **No paired measurement has
run, no quiet hold is active, and no sampling overhead gate has passed.** The
historical full-trace <=5% gate remains failed. Full emission is retained for
diagnostic compatibility, not promoted by the sampling correctness proof.

## Implementation and validation

Whole-request selection uses a fixed hash of an atomic root ordinal with a
positive sampling denominator. The target remains off by default; unset/rate 1
preserves the original full event-v1 span format. A task-local decision crosses
nested engine requests and existing explicit blocking-worker handoffs. Sampled
event-v2 spans identify rate, root ordinal and selected-root count. Those counts
describe selection, not completed population metrics. Standalone instrumentation
outside a request retains the old behavior; arbitrary spawned tasks are outside
the inheritance guarantee.

All performance counter events use a common gate before evaluating fields.
Disabled/unsampled timers skip instrumentation clocks; workers skip dispatcher
capture; no performance span is created. A test-only allocator verifies zero
allocations in these instrumentation paths after subscriber/config setup. This
does not claim an allocation-free QueryEngine. No async sink or queue is added.

The benchmark independently records every warmup/measured/throughput attempt,
completion, engine/sink error, latency-observation count and summed caller latency.
All 1,000 measured caller latency samples remain unsampled JSONL. Timing surrounds
the engine call with its prepared query and synchronous sink flush. The same
updated probe is compiled into both arms. Sampled stage/work counts are explicitly
partial and are never presented as exact full-population counters.

Frozen candidate source manifest:
`2d28c6cbbd2844369940fa346e204cdb809b5e22f409fb2e238ab23acb7d60c5`.
All 232 allowlisted files match before/after testing and release compilation;
the working source was checked against that snapshot before commit. Files were
copied, made read-only and mounted `/work:ro`. Targets/cargo caches are private
reflink copies, never writable hardlinks. Each source switch cleaned only this
task's `recalld` package, retaining dependency builds. No global/shared-target clean.

- Final suite: **750 library + 1 CLI passed**, four existing doctests ignored;
  normal parallel library runtime 88.23 seconds. Formatting passed; clippy
  completed with warnings. Candidate and anchor release probes built offline.
- Seven focused performance tests passed before the actual transport test was
  added. The new actual REST route/MCP dispatcher test first failed because its
  tag-only REST input was invalid on this parent: **749 passed, 1 failed**. That
  frozen failed run is preserved. Correcting only the fixture to use a valid text
  query and deterministic local embedding provider produced the final pass.
- Nested transport/engine/worker selection, errors/cancellation, full-format
  compatibility, skipped argument evaluation, zero instrumentation allocations,
  real-engine result parity, redaction and false-mode byte immutability are covered.
  No retrieval formula, filter, lifecycle, canonical write or admission change.
- Functional A/D/S/full smokes each completed 200 warmups, 1,000 measured queries
  and 1,000 throughput queries. These are **not overhead measurements**. A/D emitted
  zero performance events; S emitted 836 events for exactly 22 selected roots;
  full emitted 83,600 events for all 2,200 roots. All request/error/latency counts
  reconciled; result and logical canonical hashes matched; query-interval physical
  bytes were unchanged. Ten of the 1,000 measured requests were selected at N=100.
- Streaming QA checked the exact selected ordinals, nonzero complete event sets,
  hierarchy/worker continuity, timing partition/bounds, fixture counters and strict
  field allowlists. Copies with a missing selected root, truncated event, wrong
  rate or unexpected payload were rejected. Original raw traces remain untouched.

[READINESS.json](../../benchmarks/pipeline-sampling/READINESS.json) records exact
source, probe, executable, fixture-manifest and protocol/runner/QA hashes.
Candidate executable SHA-256:
`2148f0ee50cdfe1c78a683ce61e3d045ec9c85b54694df99d89c759fecb57156`.
Anchor executable SHA-256:
`93572193374fc79dd2c56e2af36806a5d5e55c369896e1629ea1e4a6a77c8759`.

## Proposed coordinated window

[Frozen prospective protocol](../../benchmarks/pipeline-sampling/PROTOCOL.md):
**20 minutes maximum**, 15 sequential A/D/S arms across seeds 7/19/41/73/101,
counterbalanced ADS/DSA/SAD/ASD/DAS. N=100 is fixed. Each arm has 200 warmups,
1,000 measured queries and at least 60 seconds/1,000 throughput completions.
At most 90 seconds per arm or remaining time. Stop scheduling 15 seconds before
the explicit UTC deadline and clean up only this study's containers before it.
Expected duration is 16–18 minutes; all failures/incomplete pairs are retained.

Runtime: offline Docker, two CPUs, 4 GiB, Rust 1.94.1. Both builds and all five
private closed fixtures are ready. They reuse hash-verified copies of the original
synthetic seed stores; no personal corpus/provider/daemon is involved. Host/image
identity was verified: Mac17,6, 18 logical CPUs, 38,654,705,664 bytes RAM, macOS
26.7.3; ARM64 image ID is in READINESS. No current host quietness is asserted.

The coordinator must arrange the interval; other agents may continue building
until then. Preflight needs three >=90%-idle CPU intervals with <=5-point spread
and successful container capture. Preserve host/container snapshots throughout,
without external project names, and leave unrelated workloads running. Build
exclusivity does not establish whole-host exclusivity. Shared-host uncertainty,
missing evidence, failures or insufficient paired intervals block promotion.

D/A and S/A must clear the preregistered adverse 95% interval bounds for both
p95 <=1.05 and throughput >=1/1.05. Absolute budgets remain p95 <=25 ms,
throughput >=40/s, RSS <=1 GiB, zero errors/changes. Complete five-seed evidence
and post-run trace/host/budget review are required. No in-window tuning/rebuilds.

## Evidence and remaining scope

Raw evidence remains outside Git under this worktree's `.validation`, including
both failed and successful source freezes, binary identities, build/test logs,
smoke traces, counters, fixture hashes and negative QA outcomes. Repository
manifests inventory the retained evidence; no cache, binary or raw archive is
staged. The actual work-notes evidence location and verification receipt are in
`benchmarks/pipeline-sampling/COPY-VERIFICATION.json`.

Full A5 still lacks actual transport serialization/enqueue, true-mode writer,
flush/fsync/publication, dense-graph and contention measurements. After sampling,
measure actual MCP/daemon ingestion across growing corpus N, edge count,
creation-receipt history and batch size. Source inspection shows whole-EdgeStore
reads in native/legacy insertion, receipt-history scans for native supplied-ID
reuse after fork, and `DerivedCompletion::complete` rebuilding graph/vector/FTS/
entities plus cache invalidation from canonical state, potentially per real MCP
write. Harness75 canonical seeding followed by one startup does not measure those
online per-insert rebuilds. These are scaling hypotheses and possible correctness
costs, not measured bottlenecks. Include full publication, actual fsync counts and
durable acknowledgment before optimizing; preserve proofs, validation and
durability. No universal ingestion or integration performance improvement is claimed.
