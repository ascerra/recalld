# A5 actual online producer counters

This is opt-in instrumentation for the existing legacy single-store path. It changes no canonical mutation, durability rule, retrieval policy or public schema, and establishes no performance/promotion result. The implementation uses the existing request tracing API on direct published PR81 `2fdd50637dd8e85abf6e2e87c8eb3bcdb5d6f1ee`; no native checkpoint, RIF or broad integration ancestry is required. The approved pre-code plan is `e6bb860bbd926e6ad9f7be8de3a7bc46e7481db9`, retained in `online-producer-counters-plan.md`. A6 and Halley acknowledged the schema/ownership seam before implementation.

## Observed boundaries and units

- Production MCP `run_stdio` uses the same `serve_io` tested with actual JSON-RPC lines and controlled output flushing. The root begins after a complete input line and ends after serialization/write/flush. Standalone `process_line` is an inner/shared-call control, not a substitute for wire delivery. The actual daemon connection handler runs over a private Unix socket pair with existing framing and explicit-vector input; it does not prove listener/process lifecycle or native serving.
- Transport intent is unknown before decode (`event_version=3`, `intent=unknown`, no fabricated `record_access`). Existing known-boolean full/sampled request events remain v1/v2. Nested workers reuse one decision and root ancestry. Counts describe selected requests; root ordinals are not completed-request counts or population extrapolation.
- Input/output counts use actual line/frame buffers. MCP input excludes the stripped delimiter; MCP successful output includes newline. Daemon payload and four-byte framing are separate. Failed write/flush reports attempted length and failed delivery, never all bytes completed. A parse/tool error can be delivered successfully. Canonical commit/readiness and transport delivery are separate outcomes.
- Embedding counts distinguish request/source bytes, cache hit/miss, checked-provider inner invocation/returned vectors and validation. Supplied-vector input proves zero provider calls on that branch. Provider-internal queue/retries/network attempts, cache eviction/coalescing and unwrapped provider internals remain unknown.
- Insertion measures actual storage wait/hold, canonical edge loads, vector/text append input bytes, commit-call attempts/returns and actual receipt replay. Vector bytes are logical `dimensions * 4`; text bytes are UTF-8 input, not physical I/O volume.
- Explicit text/vector/file/directory `sync_data`/`sync_all` wrappers are counted exactly once at the call, with success/error/duration. These are Rust wrapper calls, not complete system-call totals. Metadata/edge redb commits and effects acknowledgment are opaque backend operations; no inferred backend fsync zero or durable-rollback count. A typed committed-publication failure retains its receipt and pending effect, and keyed replay performs no second append.
- Completion measures publication/graph/vector/FTS/entity/storage waits and holds, blocking queue entry, actual rebuild/no-op, records/live records/edges visited, individual successful view publication and effect acknowledgment. Generation facts come from actual current canonical capture. Link counters are actual semantic/entity/temporal producer return results; `returned_edges` is omitted on error, never an adjacency scan count. Existing nonfatal linking errors keep their behavior.
- Queue timings cover the explicitly instrumented workers; the publication-gate lookup worker and provider/backend-internal queues are not separately timed. These are selected measured stages, not an exhaustive inventory of all internal work.
- Parent wall time and child/lock durations are not additive. Timeline marks plus residual partition each own pipeline; subscriber sink time remains within the surrounding request. Root cancellation is ambiguous and does not mean a child drained, rolled back or never committed. Arbitrary subscriber panic is not caught; the existing sink-I/O-failure guarantee does not cover panicking subscribers.

## Preserved failures and bounded causal evidence

| Source | Outcome |
| --- | --- |
| `2a8ea480` | Actual MCP store/provider/canonical publication succeeded, then missing-counter assertion failed. Instrumentation RED, not failed storage. |
| `e353888c` | Fixture compiler failure: missing required daemon JSON-RPC field. No test result. |
| `4b8fd747` | Focused binary aborted with stack overflow; same-binary isolated actual MCP test also overflowed. No full-suite result. |
| `604161ea` | Single future-containing branch correction hit compiler layout-query depth limit. Kept separately; no runtime result. |
| `05f9ff35` | Removed redundant async forwarding too: default-stack focused run 9 passed/2 failed. Actual formerly-overflowing MCP path passed; daemon byte oracle and abort event failed. |
| `9c7e1e86` | 11 producer tests and 17 existing performance/context tests passed. Raw frame oracle and safe pinned cancellation context corrected. |
| `13a69c0d` | Final worker/link cases: 11 passed/2 fixture failures. Tagged provider input differed legitimately; blocking pool barrier prevented an earlier gate lookup. No full run. |

The wrapper fix uses one future-containing path and direct non-async forwarding, with no larger stack, recursion limit, heap boxing, dependency or global-cache change. This is bounded causal evidence for exercised default-stack paths, not a universal stack guarantee. `tracing` 0.1.44's `WithDispatch` installs its dispatcher for poll but not Drop. `RequestDrop` uses safe `Pin::set(None)` to drop the stack-pinned future under its captured decision/dispatcher/span; disabled requests clone no active trace context. The actual task-abort regression covers this missing-drop context rather than replacing abort with a timeout inside an active subscriber.

## Final frozen validation

Final runtime **`8a0f2fb3781f12ebb18fa9c8b80f048dd936e80b`** preserves the production implementation from `13a69c0d`; its tail corrects only the tagged-provider and worker-barrier fixtures. On this exact source:

- Focused producer gate: **13 passed**, 1.86 seconds. Existing performance/context gate: **19 passed**, 1.80 seconds. These sets overlap; they are not 32 unique tests.
- Full offline `cargo test --all-targets`: **1,039 library tests**, **2 CLI integration tests**, **4 example/harness tests**, all passed. Library test duration 491.37 seconds; CLI durations 2.91/1.46 seconds; example durations 0.62/0.53 seconds. These are validation durations, not service latency or throughput measurements.
- `cargo fmt --all -- --check` passed. `cargo clippy --offline --all-targets` exited zero with warnings: 699 library warnings; 768 library-test warnings (680 duplicates); 1 binary warning (also duplicated in its test target). Compilation reported 27 library and 10 library-test warnings (6 duplicates). No warning-free claim or unrelated cleanup.
- Entire final container duration **599.215234 seconds**, including package compilation, tests, clippy and verification. Two CPUs, 4 GiB, Rust1.94.1, jobs2, runtime network-none; no exclusive-host or overhead result.
- All **231 exported source files** matched inside the container before compilation and after execution, and then matched the immutable host snapshot, current working source and committed Git blobs. Package-only cleaning preceded the final rebuild. No mounted source changes, stack-size override, recursion-limit increase, disabled-path boxing or shared-target reuse with another agent.

The actual tests cover complete stdio flush and failure after canonical publication, private daemon frame bytes, provider/cache/rejection/invalid-vector outcomes, explicit sync-call error, typed committed-publication failure and keyed replay, selected worker ancestry, no-state-change read tests, no-allocation/no-clock disabled paths, and original sampling decisions. The queued-worker case first permits the gate lookup, blocks actual completion submission, aborts the root, proves its child and graph guard still exist, then releases the worker and verifies publication under the original trace ancestry. Its root remains unresolved; neither event claims rollback or whole-work drain. Link success counts are checked against actual producer results. Their error branch is observed from returned results in source, not an injected persistent graph fault in this new fixture.

The full run includes the existing HTTP/MCP/daemon conformance tests. Exact published-head GitHub CI is still a separate gate after publication authorization; this local report does not claim that CI has run.

[Validation manifest](evidence/online-producer-counters/VALIDATION.json) SHA-256: `ed1165647866b19774f24c4575d5c9dc41057c011022403febf25f555a1a2613`.
[Source manifest](evidence/online-producer-counters/source.json) SHA-256: `e732c467b9435c40b02c939554f46c331a53e63821635f63d99b4435349158a6`.
Final full log SHA-256: `0b2a0105962bd286db048c69d94861348442ae9c671dd619bb6cde5ee7e87789`.
Final library executable SHA-256: `012846026f0fe19e87eb45b1ca050f9605a03ce41add94c49973379aee510b39`.
All eight executed test-target hashes, failed attempts, the same-binary stack isolation, environment and warning summaries are retained in the manifest. [Commands](evidence/online-producer-counters/COMMANDS.md), [inside-container checks](evidence/online-producer-counters/validate.sh) and the complete per-file digest list make the source attribution explicit. Runtime/evidence are frozen; the report tail changes no Rust source.

## Changed source and retention

- `src/daemon/producer_tests.rs`
- `src/daemon/protocol.rs`
- `src/daemon/server.rs`
- `src/derived_completion.rs`
- `src/embedding/cache.rs`
- `src/embedding/runtime.rs`
- `src/mcp/bridge_adapters.rs`
- `src/mcp/transport.rs`
- `src/performance.rs`
- `src/performance_tests.rs`
- `src/performance_write_tests.rs`
- `src/storage/edges.rs`
- `src/storage/engine/insertion.rs`
- `src/storage/fsync.rs`
- `src/storage/metadata/effects.rs`
- `src/storage/text.rs`
- `src/storage/vectors.rs`

The plan, this report and compact text evidence are the documentation additions. No raw trace archive, executable, target or cargo cache is staged. All owned run containers have been removed after recording their exits. Recovered disk bytes were not measured. Nine active run roots remain protected under `/tmp/recalld-implementation/online-producer-counters/.validation/online-producer-*` until the resulting PR and recorded-results handoff; exact paths will be released to the central cleanup owner then, not deleted concurrently. The single retained cargo at `coherent-v2-similar/.validation/cargo` and target at `a2-conformance-inventory/.validation/target-entity` remain this agent's bounded cache for follow-up work. No replacement cache clone or HOME raw archive was created.

Compact actual work notes are appended to `/Users/caleb.evans/work-notes/work/Experiments/Recalld correctness and retrieval research.md`; source/evidence remain in this repository. Under the current user retention override, completed raw data may be discarded after PR/results recording without an archive-copy requirement.

## Remaining scope and gates

Default remains OFF. Full-trace <=5% overhead failed historically; N100 sampling did not clear its adverse confidence bound; the incremental candidate failed collateral latency and has an incomplete syscall audit. None of those gates are rerun or superseded here. Correctness test durations/counts are validation, not performance measurements. No quiet window, native activation, batch/background completion claim, client enqueue/receive latency, whole-host I/O attribution, power-loss claim, optimization or public diagnostic schema is introduced. These hooks need a separate exact-source prospective overhead protocol before broader enablement; native/batch/provider-specific instrumentation and empirical A5 gates remain owned follow-ups.
