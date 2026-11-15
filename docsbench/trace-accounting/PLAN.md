# A6 actual trace and accounting consumer — source-only plan

No runtime edits, new tests, experiment, dataset/model call or activation in this
plan. Next bounded A6 implementation gap after grouped PR161. PR161 remains
READY/FROZEN with Halley; its historical source, fixture and evidence stay intact.

## Minimal parent and concrete omission

Direct published #90 `003d857dd99acca9eb33801fa6c5d7b137d124a2` already supplies the
real serialized MCP client, current-question-only actor, strict offline manifest,
source ledger/scorer and real fail-closed provider miss. Grouping #161 is not
necessary to account for one real episode; native/control/reset and Hume's A5
producer instrumentation are also unnecessary parents. Do not stack them for
convenience. Integration with group maps later belongs in the actual combined
consumer; this slice retains the named single-group synthetic fixture.

Frozen source evidence:

- `scripts/public-mcp-actor.py:27–44` records attempts/outcomes around actual
  pipe writes/reads, but does not record bytes or observed duration. Parse,
  timeout and mismatched-response failures can leave only the raw attempt.
- `scripts/public-mcp-evaluation.py:245–258` sends actor stdout directly to the
  retained raw wire and acknowledges a delivery after the child finishes.
- `scripts/public-mcp-evaluation.py:289–293` creates normalized `trace.jsonl`
  afterward from outcomes only. It is not a live append-only attempt/error trace.
- `scripts/public-mcp-evaluation.py:155–170` serves the actual synthetic replay
  provider and records requests/cache keys/status. Its zero usage values are
  fixture protocol data, not measured tokenizer consumption or production cost.
- No pinned tokenizer/answer context assembler is present in these consumers.
  A5 server-side counters and complete external benchmark denominators are
  separate producer responsibilities, not inferable from this client's logs.

Prioritize the trace/observed-accounting gap over bootstrap in this slice. It is
needed to retain partial action outcomes and truthful reliability/cost inputs
before later context-budget and paired-uncertainty consumers can use them.

## Bounded actual integration

Add a separately named `serialized-mcp-accounting-synthetic-diagnostic-v1` track
in the existing runner, with explicit accounting policy/version and unchanged
manifestVersion1/sourceMappingVersion1/traceSchemaVersion1. Preserve old-track
behavior and pins. Reject unknown accounting policy/unsupported guarantees in
preflight before output, provider or store side effects. No second harness,
metric registry, dispatcher, provider or engine.

For this track only, the same separate actor records facts at its existing RPC
boundary: intended request bytes from the exact serialized buffer, completed
write count when exposed by the real write result, exact response bytes read,
notification/no-response distinction, observed elapsed monotonic nanoseconds and
a bounded completion/error category. Serialize once. Do not label an attempted
write as completed after a partial/failing write, guess missing byte progress,
add retries to unkeyed calls, or infer backend commit/publication from transport
failure. Distinguish MCP protocol/tool error, EOF, timeout, parse/ID mismatch,
write/read failure and unknown process interruption. Preserve raw partial data
or hashes without inventing parsed outcomes. Wall-clock observations are not a
controlled clock/reset feature or a performance claim.

The supervisor reads the actor's event stream as it arrives, appends the original
wire event and a versioned trace event, then reconciles completion. Each attempt
must be durable in the trace file before its matching outcome is consumed; an
interrupted child leaves the attempt and an explicit incomplete classification.
Use append-only writes/flushes, not post-hoc rewriting into a success-only trace.
The supervisor supplies run/episode/source-group/provenance context; no evaluator
labels, future questions or private control information enter actor projections,
code, environment or inherited descriptors. Maintain the same production MCP
session across current-step actor invocations and the existing ACK schedule.

Trace fields identify event kind/step, request/outcome/error, public source/rank
information actually observed, model/cache references and decimal-string counts.
Real public receipts/context fields may be copied when present; unavailable
GenerationRefs, ReadView, private readiness, simulated time, snapshot and reward
remain null/explicitly unavailable. Do not synthesize them from a post-step barrier
or read the private store to fill gaps. Source mappings still derive from actual
acknowledged public UUIDs, without evaluator labels in canonical data.

The existing replay HTTP handler records actual request/response **body** bytes,
observed requests, cache hit/miss and returned vector count at the real handler
boundary. HTTP framing/socket/kernel bytes and production provider internals are
not inferred. Distinguish this synthetic replay cache from a production runtime
embedding cache. Join these observations into the real episode's evaluator
accounting result and trace references, retaining underlying event records.

The episode aggregate reconciles attempts/completions/errors/incomplete actions,
notifications, measured payload bytes and actual observed provider requests with
the retained wire/provider events, using explicit denominator units. It is a
complete count of this small external episode, not an estimate from sampled A5
spans. Never double-sum parent/child elapsed time or label client-observed duration
as server/storage/provider duration. A5's existing sampled hierarchy and complete
benchmark counters remain untouched; consume validated producer facts later when
an actual frozen interface is available.

## Token, context and cost boundary

Actual tokens, tokenizer revision, answer-context usage and production dollar cost
remain explicitly unavailable. Do not treat synthetic provider `usage=0` as a
measurement, estimate tokens by characters/bytes, invent a tokenizer/model
revision or license, or claim a4096-token cap without a real tokenizer and context
assembler. Reject manifests requiring those unsupported guarantees. Keep fixed
prompt/question overhead separate once the actual answer consumer exists.

The declared synthetic offline provider performs no paid external model calls;
that invocation fact does not imply production inference would cost zero. Record
synthetic-cache execution separately from unknown production cost. No price table,
paid call, dataset, answer/judge model, training or default activation is needed.
Token/context budget enforcement, transformation provenance, upstream adapters and
group-paired10,000-resample uncertainty remain implementation work, not permanent
unsupported completion or experiment-only deferral.

## Meaningful validation and ownership

- Actual existing CLI/MCP episode with successful stores/queries, protocol denial,
  notification and real replay miss: complete attempt/outcome trace, exact bytes
  reconciled to captured buffers, true per-step/source/cache attribution and
  explicit error denominators. Same-binary old-profile positive remains valid.
- Narrow child/pipe fixtures exercise partial write/read, EOF, malformed/oversized
  response, mismatched ID, timeout and interruption. Label fixture/unit injection
  separately from production-runtime observations; preserve failed attempts and
  partial outputs. A process termination must not create a fabricated rollback.
- Real gold/future canaries retain current-step-only isolation. Changing evaluator
  labels can change provenance/cache keys, but not actor requests or provider
  vectors. Measured times and independent UUIDs are not compared for equality.
- Unsupported policy/version/tokenizer/context/cost guarantees reject before
  output/provider/store. Counter reconciliation must reject missing/duplicate
  steps and mismatched observations rather than silently reduce denominators.
- Targeted Python checks and actual serialized episodes. No production Rust source
  change planned, so no broad library repeat. Recompile the existing provenance
  scorer only if embedded source pins change, and retain exact binary/source
  relations. Reuse the single private cache, offline2CPU/4GiB; no cache clone.

Owned worktree `/tmp/recalld-implementation/a6-trace-accounting`, branch
`impl/a6-trace-accounting`. A6 owns scripts/evaluator schema and output accounting.
Hume owns A5 sampled runtime producers; Huygens actual public adapters; Halley core
and integration. No production transport/performance/shared-initializer hunks are
planned. Implementation starts after this concrete parent/API plan is reviewed.
Completed raw runs are centrally released only after PR/results, preserving all
worktrees/source/reports and actual future inputs. No new raw root/cache yet.
