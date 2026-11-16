# Whole-request sampling: gate not passed

The N=100 opt-in sampler preserves complete benchmark request/error/latency
counts and passes correctness/trace QA, but **does not pass the <=5% overhead
gate**. Its paired p95 upper 95% bound is 1.0676× the uninstrumented anchor,
above 1.05×. There was no retuning, repeated pair, or default enablement. The
historical full-emission gate in [the prior report](../pipeline-performance/REPORT.md)
remains failed; this study does not replace or reinterpret that result.

Frozen candidate: `8a36b339ffe876d3f88580e9361a333a6602a2f7`, direct PR71 parent
`7b1a17f`. Uninstrumented probe-only anchor:
`3e87acff8631bc3d60a1fd23ef7f41f7ff3a1c5e`, runtime parent `3b590fb`.
The probe is identical in both binaries. No RIF, native-writer, batch or ReadView
integration is covered. [READINESS](READINESS.json) records source, binary,
fixture, protocol and QA hashes; all were verified again before and after the run.

## Paired results

Five seeds 7/19/41/73/101 ran the frozen ADS/DSA/SAD/ASD/DAS order. A is the
uninstrumented anchor; D is compiled-disabled; S emits whole-request sampled
traces at fixed N=100. Each arm completed 200 warmups, 1,000 measured requests,
then >=60 seconds and >=1,000 throughput completions. Every timed request includes
the synchronous sink flush. No async exporter or sink-loss allowance was used.

Ratios are paired per-process p95/throughput ratios. Intervals resample the five
process seeds, not individual queries: 10,000 bootstrap resamples, seed 20261114.
All five pairs are included; the small number of pairs limits precision.

| Comparison | Geometric mean | Paired 95% interval | Preregistered bound |
| --- | ---: | ---: | --- |
| D/A p95 | 0.996486 | [0.976244, 1.026253] | Upper <=1.05: clears |
| D/A throughput | 1.004049 | [0.995990, 1.012172] | Lower >=0.952381: clears |
| S/A p95 | 1.031033 | [0.995697, 1.067622] | Upper <=1.05: **does not clear** |
| S/A throughput | 0.999348 | [0.992656, 1.004726] | Lower >=0.952381: clears |

| Seed | D/A p95 | S/A p95 | D/A throughput | S/A throughput |
| --- | ---: | ---: | ---: | ---: |
| 7 | 0.997036 | 1.072609 | 0.991432 | 0.986690 |
| 19 | 0.969911 | 0.984369 | 0.994876 | 0.997519 |
| 41 | 1.053263 | 1.079309 | 1.013238 | 1.001674 |
| 73 | 0.977129 | 0.987877 | 1.007422 | 1.007394 |
| 101 | 0.987242 | 1.034936 | 1.013489 | 1.003590 |

This is insufficient evidence that sampled p95 overhead is <=5%; it is not a
proof that the true effect exceeds 5%. No speedup or overhead promotion is claimed.

## Absolute budgets, correctness and trace QA

All 15 arms cleared the separately declared absolute budgets:

| Measure | Worst observed arm | Budget |
| --- | ---: | ---: |
| p95 caller latency | 1.324918 ms | <=25 ms |
| Completed throughput | 835.099 requests/s | >=40 requests/s |
| Peak process RSS | 72.816406 MiB | <=1,024 MiB |
| Engine/sink failures | 0 | 0 |

There were **778,274 total attempts/completions**, including warmup, measured
and throughput phases, and 15,000 unsampled measured latency samples. The per-arm
attempt/completion/error/latency-observation counts and measured latency sums
reconcile. All five pairs agree on result UUID/score and logical canonical-state
hashes. Physical canonical bytes remained unchanged within each arm's query
interval. Physical redb bytes are not compared across independent open/close
cycles; see the original protocol's recorded limitation.

Streaming QA verified all 15 arms. A/D emitted no performance events. S emitted
**109,250 events for exactly 2,875 selected root requests**, with 38 complete
events per root and the exact frozen hash selection over the full attempted
population. Required stages/counters, worker continuity, parent identities,
exclusive timing partition, nested/caller bounds and strict field redaction all
passed. Missing samples/events are not excused by the nominal sampling rate.
Only ten of the 1,000 measured requests per S arm were selected: sampled stage
means describe those 50 requests, not full-population stage distributions.

Pre-measurement validation remains **750 library +1 CLI passed**, formatting
passed, clippy completed with warnings, four existing doctests ignored. Tests
include actual REST/MCP/engine/worker decision consistency, redaction, false-mode
canonical bytes, zero instrumentation allocations on disabled/skipped paths,
error/cancellation behavior and full event-v1 compatibility. The earlier invalid
REST fixture failure and its corrected passing source are preserved. Functional
smokes and four negative QA cases are separate evidence, not extra measured pairs.

## Host interval and limits

Authorized window: **2026-11-14 15:27:38–15:47:38 UTC**, including preflight/setup.
All 15 arms exited 0 and the runner exited 0. Cleanup was verified at
**15:46:14.109369 UTC**, with zero remaining study containers; unrelated containers
were untouched. All six other agents were released before post-run QA/packaging.

Preflight idle was 96.32%, 95.42%, 96.63%, satisfying the >=90%/<=5-point-spread
rule. Across 112 captures, no capture command failed or unexpected image appeared;
only the study probe and 14 pre-existing Postgres containers were observed. Two
per-container stat rows were missing for the study probe at 15:32:48.180440 and
15:44:27.979420 UTC, near arm transitions. This is consistent with a container
ending between `ps` and `stats`, but that cause is not established; the gaps are
retained and prevent any claim of continuous resource coverage.

Observed host idle ranged **83.66–97.60%**, median 91.865%. Background Postgres
aggregate CPU peaked at **34.62% of one Linux CPU**. Host-wide idle and container
CPU have different denominators and are not added. The drain acknowledgments and
observed images support build exclusivity; they do not establish whole-host
exclusivity or exclude brief unobserved activity. Results remain shared-host
characterization. The statistical p95 gate already fails independently of these
limitations.

Hardware/resource/workload details are frozen in [PROTOCOL](PROTOCOL.md) and
[READINESS](READINESS.json): Mac17,6, 18 logical CPUs, 38.65 GB RAM; ARM64 Docker,
Rust1.94.1, two CPUs/4 GiB, no runtime network, home, credentials or socket mounts.
The actual-engine cell uses 10k legacy records/d384, ten namespaces/10% target
scope, hybrid retrieval, k20, graphDepth0, fixed clock, recordAccess=false,
precomputed vectors and cache bypass. No provider calls, online ingestion or
public v2 serving-readiness claims are included.

## Retained evidence and next scope

[RESULTS.json](RESULTS.json) includes all paired ratios, per-arm absolute budgets,
host review and trace QA. The measurement manifest inventories every raw sample,
trace, exit/process log, host capture, frozen initial fixture and validation file;
raw payloads stay outside Git. Verified persistent evidence is under
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-performance-sampling-8a36b33/measurement`;
`MEASUREMENT-COPY-VERIFICATION.json` records the copy-manifest hash. The earlier
readiness evidence remains intact in the parent artifact directory.

Sampling stays opt-in and performance tracing stays default-off. No retry,
sampling-rate retune, full-trace enablement or new long exclusive interval is
authorized by this result. Actual transport/online write/flush/fsync/publication,
provider/queue and mixed contention costs remain unmeasured.

Next is diagnostic profiling of actual MCP/daemon ingestion across growing
canonical N, creation-receipt history, edge count and caller batch size. Include
whole acknowledgment, provider/queue time, flush/fsync and full publication.
Whole-graph loads in insertion, native supplied-ID receipt scans after fork, and
per-write DerivedCompletion graph/vector/FTS/entity rebuilds plus cache invalidation
are source-supported hypotheses, not measured bottlenecks. Canonical seeding then
one startup (including harness75) excludes that online rebuild cost. Establish the
measurement scope and actual dependency parent first; preserve validation,
ownership, publication and durability before considering optimization. There is
no universal performance-improvement claim.
