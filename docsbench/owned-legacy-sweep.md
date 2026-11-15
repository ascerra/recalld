# Explicit owned legacy sweep control

The minimal parent is PR125 `0b50168b64ad353e4f7a53c6459c270fe8500070`.
Measured runtime is `42d39a4` (the evidence report records the full hash).
The [frozen plan](owned-legacy-sweep-plan.md) preceded implementation.
This private control uses the actual legacy A3 runner and same owned core;
there is no second planner, registry, public actor tool or native activation.

## Protocol

Keep `controlVersion: 1`, `publicContract: "legacy-v1"`. An owned evaluator
advertises `sweepControlVersion: 1` and `guarantees.explicitLegacySweep: true`;
`nativeSweep` stays false. Send newline-terminated JSON on the existing private
control pipes, separately from actor MCP stdio:

```json
{"id":13,"method":"sweep","params":{"deadlineMs":30000}}
{"id":14,"method":"sweepStatus","params":{}}
```

`deadlineMs` must be an integer in 1..30000; extra fields are rejected.
The existing response envelope contains `id` and either `result` or `error`.
A successful sweep result contains `controlRequestId`, `sampledAtMillis`,
`phase: "completed"`, `executionCompleted: true`, actual CONTRACTS `readiness`,
and `result` with the existing runner's counts and transitions. Readiness has
actual `storeId`, decimal-string `committedGeneration`, zero `pendingWork` and
four graph/vectors/fts/entities component stamps. No guessed generation is used.

The nested runner result includes `errors`, `publicationError`,
`knownCommittedReceipt`, `knownCommitError` and `uncertainOperation`. The last
three preserve the available failed-commit receipt/error and uncertain operation
from SweepResult; they are not an aggregate receipt or a list of all successful
commit receipts. Successful receipts remain in the canonical store.

`sweepStatus` returns `{episodeInvalidated, lastSweep}`. Before any sweep,
`lastSweep` is null. While running, its phase is `running`, readiness/result are
null, and `executionCompleted` is false. It retains the latest final report,
including an error when the coordinator failed. This slot is in-memory diagnostic
state, not durable operation replay or canonical receipt storage. Request IDs
correlate responses; they do not deduplicate sweep execution. A supervisor must
not resend a sweep to recover an uncertain response; inspect status instead.

## Ownership, failure and cancellation

An accepted root precedes the single owned-clock sample. The actual runner
receives that parent ticket and the existing completion owner. After root finish,
the coordinator awaits the real join-retaining frozen-core readiness proof at the
actual current generation. Its busy permit survives the request and worker join,
so advance, capture and another sweep cannot race a surviving worker.

Sweep failures and deadline expiry invalidate this episode and block subsequent
sweep, advance and capture, including a later checkpoint consumer. Error results
are not safely retryable; `sweepStatus` remains readable. Timeout can return
before the worker finishes, and its late result remains inspectable. It never
makes the timed-out episode valid. The deadline is cooperative and does not
promise to bound blocking OS I/O or kill the worker.

Caller task cancellation alone is different: the coordinator retains the work
and permit. If execution and readiness succeed before the deadline, status can
be `completed` without invalidating the episode. The regression aborts the caller
while the graph lease is held, then releases it and observes completed readiness.
The separate deadline regression observes timeout, then a failed late status and
continued capture/advance rejection after join. These are not a claim that every
cancellation invalidates an episode.

## Measured chronological fixture

`scripts/test_owned_sweep.py` runs three fresh real public MCP processes with a
synthetic loopback embedding server, multiplier 1, RIF/autolink off and autonomous
sweeps disabled. Each publicly stores summary and full text, advances the owned
clock by `36500 * 86400000` milliseconds, then requests three explicit sweeps at
the same sampled time `4920825600000`. Actual results are Full → Summary → Ghost
→ deleted, one phase per call, with generations 5, 6, 8 and all four views ready.
The first and third calls to the planner trigger its actual compaction descendant.
False public fetches prove full-text loss, then summary loss, then absence, without
changing durable bytes. Seven malformed control requests per process also leave
bytes unchanged. Complete reports match across the three fresh processes; raw
transcripts, stores and provider calls are retained without normalization.

This is a named legacy chronological control diagnostic. It does not establish
whole-state reset or score parity. The supervisor's later PR127 plus this control
join owns actual prefix checkpoint, original/three restored suffixes and strict
independent canonical-hash comparison; its PR117 FTS prerequisite is absent here.
Native sweep/readiness and public-v2 activation, richer chronological datasets,
all clock/RNG/cache and tie behavior, and human judge calibration remain separate
A6 requirements. No external model, training or quality claim is made.
