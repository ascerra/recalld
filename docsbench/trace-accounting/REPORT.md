# Actual append-only trace and buffer-accounting consumer

**PASS — bounded synthetic public MCP diagnostic.** Validated source:
`915ef2a94a1fbe44b120e1d1b32af724408c5484`. The sole necessary parent is published
#90 `003d857dd99acca9eb33801fa6c5d7b137d124a2`. Grouped PR161, native/control/reset
and A5 producer instrumentation are not prerequisite parents. This evidence seal
adds documentation and a read-only output auditor; measured source is unchanged.

Actual production CLI/MCP execution passed three episodes /45 acknowledged actions:
baseline accounting, changed evaluator labels, and the old diagnostic profile.
The two accounted episodes produced60 append-only trace events. Fourteen unique
Python checks passed, followed by the source-pinned CLI/scorer build and actual
consumer checker. Eight unsupported-manifest cases rejected before run output.
No unexpected candidate failure occurred. Injected unit failures and expected
public protocol/tool errors remain classified and retained, not recast as success
or production runtime defects. Production Rust/Cargo/example source is identical
to #90; no broad Rust suite, clippy or formatter repeat was needed.

## Actual consumer and precise ordering

New track: `serialized-mcp-accounting-synthetic-diagnostic-v1`, exact independent
accountingVersion1 policy, existing manifestVersion1/sourceMappingVersion1/
traceSchemaVersion1. The existing actor and runner execute the existing production
MCP transport; no parallel engine, dispatcher, metric registry or provider exists.

`public-mcp-actor.py::accounted_exchange` records one attempt, the exact serialized
request buffer, actual write result, bytes actually read, observed elapsed time,
and a bounded outcome/error category. It completes partial response reads under
a deadline; a partial request write is not resent. A failed write whose progress
is unknown stays unknown. Parsed public replies are retained; they are not used
to invent canonical receipts, rollback or readiness. Interrupted operations keep
unknown mutation outcome, including when a remote commit might already exist.

`public-mcp-evaluation.py::{TraceRecorder,run_accounted_actor}` consumes actor
stdout as it arrives, appends raw wire evidence and the versioned trace, and
flushes each attempt **before consuming its outcome**. It does not claim the flush
precedes the remote mutation or even the actor's own response read. There is no
fsync/power-loss durability claim. An unfinished attempt receives an explicit
incomplete observation; actor process/stream failures remain separate diagnostics.
Attempt/error records are not overwritten by a reconstructed success-only trace.

The actor still receives only ingestion input or its current question and public
outcomes. The accounting CLI flag enables this declared public measurement mode;
it conveys no evaluator context. The supervisor supplies environment/run/group
metadata. Label/future-query canaries remain absent from actor code, projections,
environment, inherited descriptors, wire/provider records and permitted traces
until the appropriate current query delivery.

## Measured counts and byte reconciliation

`reconcile_accounting` is consumed by the actual episode finalizer, writing
`evaluator/accounting.json`. It checks sequence/attempt-terminal correspondence,
exact buffers/counts, response-to-buffer agreement, notification semantics and
provider body counts. `check_trace_accounting.py` independently exercises the
actual outputs; `audit_outputs.py` further checks source mappings/public ranks.

| Each accounted episode | Observed value |
|---|---:|
| RPC attempts / terminal observations | 15 /15 |
| Successful ordinary RPCs | 12 |
| Notifications / protocol errors / tool errors | 1 /1 /1 |
| Failed exchange / incomplete / process-error observations | 0 /0 /0 |
| Intended and completed client request bytes | 2,365 /2,365 |
| Client response bytes actually read | 30,009 |
| Observed provider requests | 9 |
| Provider HTTP status200 /400 | 8 /1 |
| Provider request-body bytes | 966 |
| Provider completed response-body bytes | 930 |

The protocol error is the real denied private-control method; the tool error is
the real fail-closed embedding replay miss. They are terminal public observations,
not successful tool outcomes. There is no generation/fallback or action retry.
These counts describe the complete observed external synthetic episodes, not A5
sample extrapolation or an invented whole-process/backend denominator.

Client bytes include the exact serialized stdio buffers (including framing newline
where actually present). Provider bytes refer strictly to HTTP bodies. They are
not HTTP-header/TLS/socket/kernel bytes, physical storage IO or backend durability.
Trace provider references identify a **delivery window**, which can include startup
and shared work. They do not establish exclusive per-RPC causality. Provider
records are counted once at episode scope, never multiplied across those references.

Observed client elapsed nanoseconds are retained per step. The interval begins
before request serialization/attempt emission and ends after response parsing
(or the observed failure), so it includes that client work. It is not a pure
network RTT, backend/provider duration or server service time. Durations are not
summed across nesting, normalized, compared for equality or presented as a
performance result.

All four acknowledged stores in each accounted episode map their real returned
UUIDs to the correct source IDs. All four query trace projections exactly match
the public response order/IDs/exposed scores, and resolve to the expected source0.
These are public scores, not claimed raw BM25 or internal composite scores.
Source mappings are ordinary evaluator sidecars; no private dump supplies missing
ReadView/generation/receipt state.

## Isolation, compatibility and negative scope

Changing evaluator labels leaves actor code, current-step inputs, actual requests,
provider requests and returned vectors equal. Environment identities and replay
keys change as expected. Independent public UUIDs and observed durations are
retained without equality or deterministic-startup claims. Both actual runs
withhold the future-query canary until delivery4; the existing actor boundary
still excludes evaluator factories and future schedule inputs.

The old track executes15 actions with its original15 outcome-only normalized
trace and no accounting output. Accounting fields on that old profile reject.
Historical evidence retains its original source/binary pins, rather than silently
running this new example under an old study identity.

Eight real preflight negatives: accounting version2, boolean version, invented
tokenizer, a4096-token guarantee, production-price0, sampled accounting, new fields
under the old track, and unsupported coherent-ReadView capability. Every rejection
precedes creation of its run-output path. Real replay miss remains an observed
HTTP400/tool error in successful diagnostic episodes.

Focused unit fixtures exercise partial/failed writes, partial-response EOF,
malformed response, mismatched ID/shape, timeout and bounded oversize handling;
notification/protocol/tool outcome distinctions; attempt flush before outcome
consumption; missing/duplicate/bad-byte reconciliation; and actor exit/timeout
with retained incomplete attempts and joining the owned child. These injected
pipe/process faults are unit proof, not claimed failures of the production CLI.
The initial host run counted24 executions because inherited checks were duplicated;
that log is retained. The final unique suite contains14, including seven inherited
checks. Repeated checks are not counted as extra coverage.

## Provenance and resources

All234 inputs match the frozen Git blobs and host/container pre/post hashes.
Immutable read-only source, network-none Docker, two CPUs,4GiB memory/swap cap,
offline locked Cargo/-j1, no ambient credentials/personal store/daemon, and one
reused private Cargo/target with shared registry mounted read-only. Image:
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
The cgroup peak reached4,294,967,296bytes, `max=1554`, `oom=0`, `oom_kill=0`.
This is memory-pressure evidence, not free headroom or a performance comparison.
No retry/resource/flag change occurred; the build retained12 inherited warnings.

Actual executed SHA256, checked before and after episodes:

- Production CLI: `603bfa58f26c2e5eb66c3dc73372d5ed910ed014e087593a7f78e8aa1ccdfcf8`.
- Provenance scorer: `bcb519165536ddfd5d47971af24e1ac0d40adcd5fe241e51343e87c40c1adb92`.

The CLI hash equals the already published grouped consumer's CLI; the scorer is
rebuilt from #90's unchanged source with this track's embedded provenance pins.
Source equality was verified, not inferred from a stale cached binary. Commands,
source inventories, executable hashes, cgroup records, unit logs, all actual raw
wire/traces/provider/score outputs and eight negative receipts are in `evidence/`.
`validation.json` and `SHA256SUMS` index the evidence. The post-run auditor reads
recorded outputs only; it neither rewrites traces nor changes runtime proof.

## Explicit remaining gates and cleanup

Tokenizer, measured tokens, answer-context cap, production cost, provider-internal
retries, HTTP framing bytes and backend durability are explicitly null/unavailable.
Synthetic provider usage=0 is fixture protocol data, not a token measurement.
Paid-call0 is scoped to this offline synthetic replay invocation, not an estimate
of production inference cost. A real pinned tokenizer/context assembler/cost
consumer still needs implementation; bytes are not substituted for tokens.

This is legacy public API1, canonical schema null/unattested. No native/ordinary
startup, public2, reset/clock/RNG/ReadView, full-A6, quality, paid model/dataset,
training or activation claim. Remaining work includes full receipt/generation
producer joins, generalized trace/transform provenance, token/context/cost
consumers, upstream scorer adapters and grouped paired uncertainty. Public rank
projection and delivery-window provider linkage do not close those broader gates.

Branch `impl/a6-trace-accounting` is unpublished pending coordinator review.
PR161 is published at6265d05, exact-head CI43669922681 succeeded and the branch is
READY/FROZEN with Halley. Its completed raw run was centrally deleted after results.
Current `/tmp/recalld-owned-native-controls-validation/trace-915ef2a9` remains
protected until this slice has a PR/results; retain the single future Cargo/target.
Compact HOME report/results are hash-verified and notebook Log appended. No raw
archive or cache clone. Central cleanup now totals407 exact path actions; latest
batch34 removed two released Halley154 roots with observed filesystem delta
+464,011,264bytes, including concurrent/APFS effects, not exclusive allocation.
