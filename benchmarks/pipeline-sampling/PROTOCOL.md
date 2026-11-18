# A5 request sampling: prospective protocol

This is a new study, not a reinterpretation of the failed full-emission gate in
`../pipeline-performance/REPORT.md`. Parent is published PR71 `7b1a17f`, with
its original Observed `3b590fb` runtime ancestry. No RIF, native-writer, batch, or
ReadView dependency is added. Final candidate, probe-only anchor, lockfile,
immutable source manifests, executables and fixtures must be frozen before a
measured run. Results and promotion are **not yet established**.

## Candidate behavior and counters

The performance tracing target remains disabled by default. With the target
enabled, absent `RECALLD_PERFORMANCE_SAMPLE_EVERY` or value `1` retains full
event-v1 emission. A positive integer N selects approximately one in N root
requests using a fixed SplitMix64 permutation of a monotonically assigned root
ordinal, modulo N. This avoids periodic query-list aliasing. Concurrent scheduling
determines ordinal assignment; it is not stable request-identity sampling or
randomized traffic evidence. Invalid/non-Unicode/zero settings suppress request
tracing; the probe rejects invalid rates rather than silently benchmarking them.
The environment is read once at first enabled root; startup parsing is outside
the steady disabled/unsampled allocation claim.

An async task-local decision is inherited by nested transport/engine spans and
explicit blocking workers. Unsampled requests do not create performance spans,
sample instrumentation clocks, capture a tracing dispatcher, or evaluate event
fields. Existing query work and StageTimings remain unchanged. An allocation
counter tests the instrumentation path, not the entire query's allocation count.
Standalone instrumentation outside a measured request retains prior behavior;
this slice does not promise selection inheritance into arbitrary spawned tasks.
Existing explicit worker handoffs carry it.

Sampled event-v2 spans add `sample_every`, `root_ordinal`, and
`sampled_roots_seen`. Counts describe root decisions at selection, not all
completed requests, and nested spans repeat them rather than incrementing them.
Concurrent emission order need not equal ordinal order. Full emission preserves
the original event-v1 fields. Operation/stage labels remain bounded; no query,
content, memory IDs, tags or namespace names are added. Actual receipt fields
retain their prior isolated trace-only treatment. Ordinary sink I/O failures
remain distinct from arbitrary subscriber panics; no new panic guarantee.

The probe's warmup, measured and throughput attempted/completed/error counters,
latency-observation counts and total caller latency are never sampled. All 1,000
measured caller latencies remain in raw JSONL. These are separate from sampled
stage/work counters; do not multiply stage counters by N and claim exact totals.
The sink stays synchronous and flushes inside every timed request, including
unsampled requests. No async exporter framework or dropped-event queue is added.

## Frozen measurement cell and budgets

Reuse the original actual-engine cell: 10,000 canonical legacy records, d384,
10 namespaces, 10% target scope, seeded normalized vectors, hybrid vector+FTS,
k20, graphDepth0, fixed clock 1767225600000, explicit recordAccess=false, no
provider calls, cache bypass. Real QueryEngine and production adapters only.
Closed fixture snapshots are prepared outside the quiet interval and copied
fresh per arm. The anchor receives only the identical new probe on the frozen
uninstrumented `3b590fb` source. Candidate source otherwise derives from `7b1a17f`.
This does not validate latest native/batch/ReadView integration.

Arms: A uninstrumented anchor, D candidate with performance target disabled,
S candidate with trace target enabled and **N=100**. Set the same N environment
in every arm; only S emits selected traces. Do not retune N from observed ratios.
Full emission has correctness smoke coverage, but is not a new measured arm;
the historical full-emission <=5% gate remains failed.

Five process seeds 7/19/41/73/101; counterbalanced order A-D-S, D-S-A, S-A-D,
A-S-D, D-A-S. Each arm: 200 warmups, 1,000 complete measured operations, then
at least 60 seconds and 1,000 successful sequential throughput operations.
Capture attempted/completed/failed counts separately. Any failure, missing trace,
missing sample or incomplete pair invalidates promotion; retain every attempt.
No in-window build, protocol change, replacement arm or expanded workload.

Absolute budgets remain p95 <=25 ms, completed throughput >=40/s, peak RSS <=1 GiB,
zero request/sink failures and no canonical query-interval byte changes. All arms
must agree on result UUID/score and logical canonical hashes. Physical redb bytes
are compared within each open query interval, not across independent opens.
Setup/rebuild is excluded and reported separately. These are this cell's budgets,
not production SLOs.

Report D/A and S/A p95 and throughput ratios for all five seeds, geometric means,
and paired bootstrap 95% intervals (10,000 resamples, seed 20261114). Promotion
requires adverse interval bounds p95 <=1.05 and throughput >=1/1.05, as well as
absolute budgets and all QA. Five pairs may be inconclusive. Retain old full-trace
failure separately; no full A5/default-enable claim follows from this cell.

## Resources, coordination and trace QA

Proposed capped window: **20 minutes**, requested only after candidate suite,
release builds, five fixtures and preflight smoke/QA succeed. No hold is active.
The coordinator supplies the actual UTC start/deadline. Stop scheduling 15 seconds
before the deadline; clean up only this study's containers before the deadline.
Maximum 15 sequential arms, each timeout bounded by 90 seconds or remaining time;
expected measured wall time 16–18 minutes. No silent retries. Preserve failures.

Runtime image recalld-ci:1.94, Rust 1.94.1, 2 CPUs, 4 GiB, two build jobs; runtime
network none. Private source-specific targets and allowlisted read-only source
snapshots, with pre/post hash verification. No home/credentials/socket mounts.
Record actual image ID, toolchain, OS/architecture/SIMD, host CPU/RAM, filesystem,
resource limits, binaries, fixture hashes and source freeze in the run manifest.
Prior hardware was Mac17,6 / 18 logical CPUs / 38,654,705,664 bytes host RAM;
verify rather than assume it is unchanged.

Before measurement, three interval CPU samples must show >=90% idle with <=5
percentage-point spread, and container capture must succeed. Record host/container
snapshots before/after arms and periodically throughout. Leave unrelated workloads
running; publish aggregate image counts/resources without external project names.
Build-exclusive is not whole-host exclusive. Competing build activity, failed
preflight, incomplete captures or unresolved contention means no promotion.

Stream JSONL QA rather than loading traces wholesale. A/D events must be absent;
S must contain precisely the nonzero root set selected by the frozen hash rule
over every attempted request, with 38 expected events per selected engine request
in this cell. Validate required stage/counter events, root/worker ancestry, exact
fixture counters, partition/nested timing bounds, caller-latency bounds, strict
field redaction, explicit rate/ordinals and sampled counts. Validate complete
unsampled probe counters and raw measured latency count independently. The QA
script is `scripts/pipeline-sampling-qa.py`; the old full-emission QA is unchanged.
Sampled stage means must use their actual selected sample counts, not all queries.
No selected event loss can be excused by a nominal 1% rate.

## Remaining work

Actual transport serialization/enqueue, true-mode write, fsync/publication,
dense graph/RIF and mixed contention cells remain unmeasured. For later insertion
studies, native and legacy writers load the whole EdgeStore graph before commit;
explicit supplied-ID reuse in the native writer scans creation-receipt history
after fork. These are source-supported scaling hypotheses, not measured causes.
Include edge-count and receipt-history growth, actual flush/fsync/publication
counts and durable acknowledgment latency before optimizing. Preserve ownership,
receipt validation, terminal-ID protection, proofs and durability. No write-path
optimization is bundled here.
