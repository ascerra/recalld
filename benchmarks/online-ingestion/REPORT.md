# Legacy online insertion diagnostic — bounded observations

The actual single-store path rebuilds every live record's derived views before acknowledging each insertion. At initial N=1000, the three measured daemon acknowledgments were **240.269, 256.335 and 231.645 ms**; the combined record-view rebuild occupied **233.865, 250.285 and 225.691 ms** respectively under full tracing plus strace. At initial N=100, daemon acknowledgments were **22.893, 23.958 and 23.949 ms**. These are individual diagnostic observations on a busy shared host, not a p95 estimate, causal scaling estimate, speedup, SLO or promotion result. No optimization was made.

## Exact source and measured scope

Runtime/probe/protocol/QA commit `67274c5c4c01dc8ce36ca695f2b9d6873c942748`, direct parent PR #75 `8ecdd5a4b20c1f1555b98c84bd0771625e51c67d`. No native, batch, producer, RIF or sampling parent. [PROTOCOL.md](PROTOCOL.md) was frozen before the run. The source manifest is `c2d71b2a5b6b66f25838dbe79be80d989b8278db3a0b848c86fd5de02eae3f28`; release binary `96d3ffe7ce8099c29c9983ee415afd5e6b19783c4e418fa993fdbf8aa3f76d86`. The final source snapshot contains 196 allowlisted files and was mounted read-only. Source, binary, runner, protocol and QA hashes matched after the run.

- `adapter100`: typed McpStorageAdapter with explicit vector, **not MCP wire**.
- `mcp100`: actual tool dispatcher/parser/handler and result serialization, deterministic local provider. No embedding wire field was added; no stdio JSON-RPC/external client or network provider was measured.
- Daemon cells: actual DaemonClient/DaemonServer Unix-socket framing/RPC and explicit vector, client/server tasks in the same isolated process. No daemon startup time in write windows.
- `daemon1000`: 1000 initial records. `history1000`: 100 initial records plus **1000 real legacy exposure receipts**, not native creation history. `edges99`: 100 initial records plus 99 canonical parent edges. Other axes match daemon100.

All cells use d384, a single legacy namespace, a fixed injected lifecycle clock, one warmup and three measured serial writes. Duplicate advisory, autolink and autonomous sweep are disabled. The provider returns a deterministic unit vector; its actual call counter was one per MCP write and zero on both explicit-vector paths. Full text and entity publication were exercised.

## Individual observations

Acknowledgment includes the actual call and probe argument/result conversion. The separate `with_sink_flush_ns` value in [RESULTS.json](RESULTS.json) includes trace flush/bookkeeping. RSS is process high-water **including setup**, not steady serving RSS.

| Cell | Three measured acknowledgments (ms) | Observed range (ms) | High-water RSS (MiB) |
| --- | --- | --- | ---: |
| adapter100 | 21.145, 24.493, 22.532 | 21.145–24.493 | 18.69 |
| mcp100 | 17.244, 25.262, 20.140 | 17.244–25.262 | 18.70 |
| daemon100 | 22.893, 23.958, 23.949 | 22.893–23.958 | 18.48 |
| daemon1000 | 240.269, 256.335, 231.645 | 231.645–256.335 | 26.65 |
| history1000 | 24.709, 29.411, 23.587 | 23.587–29.411 | 19.15 |
| edges99 | 26.475, 26.473, 24.942 | 24.942–26.475 | 19.37 |

Selected nested stages below are **not exhaustive or additive**. Canonical commit contains payload writes/syncs; record-view rebuild combines graph-node/vector/text/FTS/entity work and FTS clear. It does not isolate each component. Effects acknowledgment is the actual metadata acknowledgment after publication. Full stage observations, worker waits, lock waits/holds and top-level exclusive accounting remain in RESULTS/raw events.

| Cell | Three combined record-view rebuilds (ms) | Three canonical commits (ms) | Three effects acknowledgments (ms) |
| --- | --- | --- | --- |
| adapter100 | 13.096, 16.472, 16.152 | 1.944, 3.360, 2.517 | 2.431, 1.058, 1.112 |
| mcp100 | 12.134, 19.595, 17.100 | 1.720, 1.368, 1.010 | 0.843, 0.269, 0.282 |
| daemon100 | 17.527, 20.395, 16.612 | 2.702, 1.205, 3.112 | 0.304, 0.463, 0.436 |
| daemon1000 | 233.865, 250.285, 225.691 | 2.359, 1.775, 3.041 | 0.788, 1.151, 0.535 |
| history1000 | 18.854, 22.423, 18.501 | 1.632, 2.459, 1.829 | 0.618, 0.624, 0.447 |
| edges99 | 19.894, 20.228, 20.178 | 1.949, 2.900, 1.819 | 1.009, 0.594, 0.516 |

Every write produced exactly one rebuilt completion and one later no-op completion. Measured writes rebuilt 102/103/104 live records in N=100 cells and 1002/1003/1004 in N=1000. The edge cell loaded all 99 edges both before canonical insertion and during completion; other cells loaded zero. The 99-edge insertion scan took 0.006–0.007 ms in the three measured writes. This confirms executed work, but the small edge/history cells cannot establish larger-scale cost.

## Actual sync evidence

`strace -f -ttt -T -y` captured fsync/fdatasync/msync across all threads/children. Each measured write had **six fdatasync calls**: one vector, one text, three metadata and one edge-file call. Additional FTS fsync counts varied:

| Cell | FTS fsync counts for each measured write | Sum of captured syscall elapsed times (ms), each write |
| --- | --- | --- |
| adapter100 | 0, 3, 3 | 4.769, 6.503, 5.038 |
| mcp100 | 0, 3, 3 | 2.186, 3.402, 2.140 |
| daemon100 | 0, 3, 3 | 2.346, 3.814, 5.148 |
| daemon1000 | 24, 27, 24 | 21.423, 18.468, 16.205 |
| history1000 | 0, 3, 3 | 1.985, 4.105, 3.748 |
| edges99 | 0, 3, 3 | 3.058, 4.881, 3.376 |

There were 213 sync calls inside the 18 measured acknowledgment windows, 10,702 across setup/warmup/verification/shutdown and all writes; all captured return codes were zero. No syscall crossed a write-window boundary. Raw and RESULTS retain every syscall duration, file class, result and OS TID. These OS TIDs are not equated to Rust ThreadId values or inferred worker roles; strace follows workers/children without silently discarding their calls. Thread syscall durations may overlap stages and must not be added to wall time. Realtime correlation is separate from monotonic latency.

Source inspection shows FTS clear followed by one FTS add transaction per live record (`src/search/fts.rs`), nested in completion's full rebuild. The syscall evidence measures syncs, **not all SQL/redb transaction counts**. Full tracing and ptrace perturb syscall-heavy work, including unreported syscall types; no untraced overhead control was run. This does not prove power-loss durability or justify weakening any sync/receipt/publication behavior.

## Validation, host and failures

851 library tests + 2 CLI tests passed in the normal offline Docker suite; four doc examples were ignored. fmt passed; clippy completed with warnings (including the probe's owned-path comparison). Full-suite source manifest `e3007e7e7b143bd1855a01c4a888542475ca63bf2e01d62f9cdc7e110c9df485` is byte-identical to measured production source. After that suite, only the example's `/proc/self/status` output was added; its new immutable source was fmt/clippy/release-built and exercised by all 24 actual online writes. Private package clean preceded source switching; no shared target or cached before/after test executable was reused. Per-source release hashes and logs are retained.

Streaming QA passed **6 cells / 24 writes / 1348 events**, including all post-ack canonical/vector/graph/FTS/entity checks, admitted read-only gets, one committed-generation increment and real receipt identity per write, exact rebuild/no-op counters, original provider calls, request hierarchy, top-level accounting and payload redaction. The syscall parser also passed unfinished/resumed-thread attribution and truncation rejection self-checks.

Measured run: `2026-11-14T16:26:21.254652+00:00` through `2026-11-14T16:27:08.041530+00:00`; all own containers were absent after cleanup. Docker runtime used `--network none --cpus 2 --memory 4g`, no home/credential/socket mounts and no privileged/ptrace capability. Rust 1.94.1, strace 6.1; runnable image `sha256:9f20e9ac3f1bb9b234e0e05f815780cfb9247e3af19e37dc0de60c5747172b1a` (config digest `sha256:95096149e07a8fb719d4b72908a83c40986433e230becf07f654caeab8f7fc42`). Host Mac17,6, 18 CPUs, 36 GiB RAM, macOS 26.7.3; Linux aarch64 guest.

Twelve before/after snapshots observed **two concurrent recalld-ci containers and 14 unrelated PostgreSQL containers**; host idle was 59.92–71.71%. These are point samples, not continuous interference coverage. No workloads were stopped and no exclusive window was requested. Aggregate image/resource snapshots contain no external project/container names.

Retained failures: initial fmt-check failure; a scratch formatter invocation lacked its sibling insertion_tests.rs; initial harness compilation had four API/type mismatches; and the first diagnostic launch exited 125 before executing the probe because a config digest was supplied as Docker's runnable image ID. The aborted launch directory is retained. A separately recorded run used the inspected manifest ID of the same image; no measured write, source/protocol change or tuned retry was substituted. All six cells in that run exited zero.

## Next scope and limits

This supports investigating the full per-write publication rebuild, especially its combined record-view/FTS work. First separate those costs and test exact/incremental or batched publication against the existing receipt, recovery and readiness contracts; this diagnostic does not authorize a durability shortcut. Whole-graph insertion scans and native supplied-ID creation-history scans remain scaling hypotheses beyond these small legacy cells. Batch/native paths require their actual parents and serving prerequisites; they were not imported or measured.

The earlier full-trace and sampled-query <=5% overhead gates remain failed. No default enable, public native-serving claim, full A5 completion, production latency/throughput claim or optimization promotion follows. Three measured writes per cell are deliberately insufficient for inferential percentiles or confidence intervals.

Raw artifacts remain outside Git at `/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-online-ingestion-67274c5`. See MANIFEST.json and COPY-VERIFICATION.json for the exact verified HOME location, archive/member hashes, source/binary hashes and failure evidence. No raw archive is staged or uploaded.
