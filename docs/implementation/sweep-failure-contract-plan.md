# E1: bounded HTTP sweep failure evidence

The concrete completion/encoding amendment is now in
[sweep-failure-implementation-plan.md](sweep-failure-implementation-plan.md).
It supersedes the provisional choices below; runtime implementation still needs
the coordinator's literal release after exact owner review.

Plan-only child of actual main `33865ca0d19599a13ba999d31a1ec736c89db233`.
Proposed sole implementation owner: Pascal. Halley owns queue integration.
Huygens reviews HTTP schema/size; Feynman reviews receipt/producer proof;
Curie must release the named shared effects hunks. No PR184 dependency or WIP join.
No runtime edits, tests or new builds are authorized by this document alone: the
coordinator releases implementation after those exact reviews and hunk agreement.

## Source findings, not executed RED

At this base, `FsrsEngineAdapter::trigger_sweep_with_origin` preserves
`failed_commit` as `StorageError::CommittedMutation`; HTTP downcast and
`AppError::CommittedMutation` return 503 with its full receipt. Keep this positive
path. `uncertain_operation` instead becomes a `RecoveryRequired` display string.
Returning `CommitOutcomeUnknown` directly is insufficient: its generic
`AppError::from` arm also loses structured identity. Generic `publication_error`
becomes a boxed string and HTTP 500.

`SweepResult` has one failed receipt and a compaction receipt, not an ordered
collection of successful receipts. Native `native_commit` retains counters and
events only. Legacy strength updates match `Committed(_)`, and phase/hard-delete
adapters likewise discard their successful receipt. A mapping-only fix cannot
preserve earlier commits. Those exact producer boundaries are needed.

## Public scope and compatibility

Only POST `/decay/sweep` failure evidence and the existing sweep producer plumbing
are in scope. Success keeps the exact existing API1 `ApiResponse<TriggerSweepResponse>`
shape. Supplied `asOf`/`as_of` remains rejected before work. Omitted time samples
the configured clock once; original API1 and autonomous original API2 choices
remain explicit, independent of disk/lifecycle versions.

No new endpoint, public API2 activation, key/cursor/resume contract, scheduler,
canonical schema, durable receipt/hash format, storage plane or readiness claim.
No change to stale/exempt/no-change behavior or promotion of an unkeyed skip into
a durable keyed success. No automatic retry of a partial sweep.

## Proposed typed failure and exact HTTP shape

Introduce one internal `SweepFailure` carrier in the sweep module, returned boxed
by the existing adapter trait and downcast explicitly by this one HTTP handler.
It carries a typed cause, bounded in-hand evidence and the actual unknown ID when
available. Keep storage causes typed internally; log diagnostics separately.
Do not manufacture `redb::CommitError`, parse display strings, or globally remap
all `RecoveryRequired` errors in unrelated APIs.

All failed sweeps return HTTP 503 with this object (not the success envelope):

```
{
  "error": "INDEX_NOT_READY | COMMIT_OUTCOME_UNKNOWN | SWEEP_UNAVAILABLE | SWEEP_RESPONSE_LIMIT | SWEEP_EVIDENCE_CONFLICT",
  "commitState": "notCommitted | committed | unknown",
  "ready": false,
  "complete": false,
  "retryable": false,
  "receipts": [ /* complete, actually returned WriteReceipt values */ ],
  "operationId": "actual uncertain operation UUID, or null",
  "message": "fixed concise message selected by the error code",
  "receipt": /* optional existing known-failed receipt compatibility alias */
}
```

The alternatives in this illustration are enum values, not literal wire strings.
For the current known-failed-commit case retain `error=INDEX_NOT_READY`,
`commitState=committed`, `ready=false`, and the same full singular `receipt`.
The new ordered array also contains it once. The singular alias is only emitted
for that known failed commit when no unknown outcome supersedes it. An unknown
outcome uses `COMMIT_OUTCOME_UNKNOWN`, keeps prior known receipts in the array,
and never fabricates the unknown mutation's receipt. A lost worker has a null
operationId. A known publication failure after prior successful commits remains
committed even if no single failed receipt exists. Pre-work refusal has no receipt
and is notCommitted; it is not described as a postcommit publication failure.

Unknown takes precedence over known failure evidence. Fixed public messages do
not embed raw storage errors, receipt JSON, payloads, arbitrary record-error lists,
counts, duration, or presumed readiness details. This keeps the failure overhead
bounded. Preserve full internal diagnostics for logging. No aggregate receipt or
invented sweep generation. Existing ordinary skip/record-diagnostic semantics are
not silently redefined; these typed exits cover existing commit/publication
failure paths plus the new bounded-stop path.

## Evidence collector, order, conflict handling

HTTP/manual execution opts into a pass-local bounded collector. Background and
other compatibility callers keep their existing counter-only mode; there is no
new unbounded receipt list on every timer pass. The mode is an internal consumer
capability, not inferred from API version or storage schema.

For the bounded consumer, append each actual returned receipt immediately on its
serial commit path, including legacy strength-only writes, legacy/native phase
loss, legacy/native hard delete, known postcommit failures and physical text
selection. Physical compaction is a distinct ordinary receipt in chronological
order, never counted as another record transition. Keep existing compaction
fields for internal compatibility. No receipt reconstruction from counts/events,
no poisoned-owner readback and no lookup used to guess a commit result.

Full-equal duplicate evidence is retained once. Same `(storeId, operationId)` with
different complete bytes is a typed evidence conflict, never last-wins or a
silent dedup. Preserve both bounded in-hand conflicting values, stop further work,
return unknown with no invented operation ID, and require reconciliation. Such
conflicting evidence cannot be declared ready. Tests must distinguish exact
repetition from conflict. Public sweeps are unkeyed, so historical replay flags
are not normalized or silently rewritten to make evidence agree.

## Budget: actual receipt bytes, before the next canonical commit

Use the existing 8 MiB `daemon::protocol::MAX_MESSAGE_SIZE` as a conservative
HTTP-body ceiling, not a new protocol-version or negotiated-RPC claim. Private
smaller test limits exercise exhaustion without enormous stores. Do not expose a
new user-supplied limit. Check minimum fixed overhead before accepting work.

Reserve 2048 encoded bytes for the bounded static failure fields above (all codes,
fixed messages, UUID/null slot, booleans and singular-receipt key). Test the maximum
serialized overhead for every shape; raw exception strings never enter it.
For N full receipts of exact JSON lengths Li, conservatively charge
`2048 + 2 * sum(Li + 1)`. Two copies cover the array plus the compatibility alias
without guessing which receipt will later fail. `serde_json::to_writer` to a
checked counting writer measures each actual receipt, including all JSON escaping,
UTF-8, IDs, decimal-string generations, timestamps, proof arrays and counters.
Cache each accepted receipt's checked encoded cost and maintain a checked cumulative
total. Do not reserialize the growing prefix per record; dedup uses a bounded
identity index plus full-value equality. This is deliberately conservative, not
a per-row byte estimate. No MCP/double-text
transport promise is added; a future transport must supply its own full framing
budget before using this consumer.

Before another producer is dispatched, derive its remaining single-receipt
allowance from that same pass budget. If no allowance remains, stop immediately.
The producer must enforce that allowance against its **actual complete candidate
WriteReceipt under the final publication/metadata guard**, after constructing the
canonical outcome and before commit, dirty/epoch publication or effects application.
This is pre-canonical-commit admission, not a claim of zero work: the transaction
may have opened, IDs may have been drawn and compaction may have staged a private
unselected file. Do not promise zero ID consumption or unchanged arbitrary scratch
files on budget rejection. No durable receipt, control generation, record/ledger,
selected text descriptor or effect may change for a rejected candidate.

Proposed minimal reusable storage primitive: a crate-private numeric
`ReceiptSizeLimit` and `check(&WriteReceipt)` that counts serialization and returns
one typed `ReceiptBudgetExceeded`. It has no API types, shared registry, clock or
allocation policy. Existing writer methods delegate to internal bounded variants
with `None`; only this sweep passes `Some(limit)`. Avoid signature changes across
unrelated insertion, graph, purge, batch or public mutation callers.

Required final gates:

- The actual common record-effects tail, for the sweep's native and legacy
  materializations/deletions, after complete receipt/outcome construction but
  before `derived_dirty`, `canonical_pending`, epoch advancement or `tx.commit`.
  Add a bounded internal variant while the existing `finish_record_effects`
  wrapper supplies None, rather than rewriting all unrelated call sites.
- The existing text-generation selection transaction, before its commit. Carry
  the same remaining allowance through physical compaction; include its complete
  receipt, including the native digest/proof. Preserve existing exact selected vs
  unselected recovery semantics. Known budget abandonment may clean only its own
  definitely unselected prepared file. Unknown commit never authorizes cleanup.

The admitted candidate's byte reservation is accounted for even when its commit
returns unknown; do not expose the candidate receipt merely because it was sized.
The actual operation UUID fits the pre-reserved fixed slot. Errors without a
commit consume no receipt slot; execution still stops on a budget rejection.
Review the concrete nested completion-error paths before release: any newly
returned full receipt must be the admitted candidate or already-budgeted in-hand
evidence. If completion can introduce a different preexisting pending receipt,
its actual bytes must be included in preflight admission before further canonical
work; never assume it is small or silently discard it. An oversized preexisting
receipt cannot be solved by truncation or a fabricated unknown outcome. Identify
that case to the reviewers rather than claiming the new-write budget proves
arbitrary historical receipt framing.

Receipt collection happens before returning or awaiting further fallible work,
so later publication failure cannot lose earlier evidence.

Budget exhaustion is an **incomplete sweep error**, never a successful counts-only
response: `SWEEP_RESPONSE_LIMIT`, 503, ready/complete/retryable false. If earlier
commits exist, commitState is committed with every known full receipt; with none
it is notCommitted. Unknown supersedes budget classification if already present.
Do not continue with a new transaction, compaction or a background drain after the
stop. Existing completion may account for already-accepted work only; no readiness
is invented from success counts or receipt generations. Final HTTP serialization
must satisfy the same ceiling without truncation. A failure of that invariant is
a bug, not permission to omit evidence.

## Exact proposed hunk ownership

Pascal owns these E1 hunks after formal release; reviewers make no parallel edits:

- `src/decay/sweep.rs`, `src/decay/sweep/native.rs`: bounded consumer option,
  ordered evidence, typed failure state, stopping after budget/unknown, legacy
  returned-receipt handoffs including strength-only and hard-delete paths.
- `src/api/adapters.rs` sweep adapter only; `src/api/handlers.rs`
  trigger_decay_sweep only; `src/api/errors.rs` new explicit sweep projection only.
  Huygens reviews shape/status/body bound; unrelated API errors stay unchanged.
- `src/derived_completion/native_decay.rs` and `text_compaction.rs`: optional
  allowance forwarding on existing accepted work. Preserve lock order and worker
  leases. Worker JoinError is typed unknown, never a fake known commit.
- `src/storage/engine/{maintenance,native_maintenance,effects,text_compaction}.rs`
  and `src/storage/metadata/{lifecycle_updates,native_maintenance,text_generation}.rs`:
  only bounded sweep calls and the existing final transaction checks. Preserve
  old wrappers/default None. Feynman reviews text-selection/recovery seam.
- `src/storage/metadata/effects.rs`: Curie must explicitly release the bounded
  finish_record_effects variant/final size check and the test hook below.
  `src/storage/metadata.rs`: cfg(test) instance field/init only.
- `src/storage/control.rs` or a tiny adjacent private module: numeric receipt-size
  check; `src/storage/error.rs`: typed budget failure only. No durable codec edit.
- Focused HTTP sweep and native sweep tests; no shared fixture default changes,
  no new scheduler, no unrelated maintenance/purge/graph behavior.

If the named path needs an additional caller to compile, disclose the exact
forwarding hunk to the owner before editing; do not absorb another feature parent.

## Fault hook and actual RED/GREEN probes

Curie has source-reviewed, but must explicitly release, an instance-local
AtomicUsize Nth fault in the existing effects `committed.and_then` test closure.
Arm after setup/rebuild. Zero disables it; decrement only after an actual successful
record-effects commit. Nth1/2 gives a synthesized lost ACK after a durable commit,
not an actual I/O/power-loss test. Preserve existing thread-local and batch hooks.
The certification Nth hook proves known postcommit failure and must not be relabeled
unknown. A request-clock callback runs on a different thread and is not a valid
way to arm the effects worker's thread-local hook.

Smallest probes, immutable source/binary provenance before correction:

1. Real POST `{}` known-failure control on legacy/native storage with an actual
   wrong-width derived view or the existing known-postcommit certification hook.
   Full receipt and 503 survive today; retain that behavior.
2. Real POST `{}` native first and later record effects lost ACK using the approved
   instance Nth hook. Strict reopen confirms exact generation, canonical phase,
   retained payload/masks, outbox and scoped durable operation identity BEFORE
   response assertions. Caller remains unknown with actual ID, no guessed current
   receipt, and all earlier known evidence. No later compaction/mutation occurs.
3. Actual public adapter pre-work closed-admission control, separately labeled as
   no mutation. Do not use it as evidence of postcommit publication failure.
4. Known publication failure after an earlier successful record and compaction
   selection failure after record commits: ordered complete receipts, incomplete
   response, no inferred readiness. Distinguish selected durable text from an
   unselected budget-rejected prepared generation.
5. Private small byte limits with valid real phase and hard-delete/peer proofs:
   first-candidate rejection leaves canonical state unchanged; later rejection
   preserves prior commits and stops; compaction cannot exceed the remaining cap.
   Test escaped/Unicode data where actual receipts contain it, maximum valid
   numeric/ID spellings, wrapper duplication, unknown slot, and serialized full
   HTTP-body length. Do not invent invalid oversized proofs or assume fanout size.
6. Retain existing same-origin hashes, configured one-clock behavior, asOf rejection,
   unknown-ACK/cancellation, strict reopen and no-repeat recovery tests. Budget
   tests explicitly record any consumed IDs or private staging artifacts rather
   than claiming zero allocation. No public keyed sweep replay guarantee.

Only after actual RED classify the runtime defect. Before sealing, run affected
producer/HTTP/unknown/reopen/budget tests, fmt/clippy and the appropriate full gate
for the actual runtime composition (coordinate Halley to avoid duplicate fulls).
Report known-failure controls, source-only expectations, setup/compile failures,
zero-match filters and actual RED/GREEN separately.

## Coordination and resources

### Independent RED-only stage (source review before build release)

This stage is deliberately independent of the unapproved production solution.
Its sole code base is `33865ca0d19599a13ba999d31a1ec736c89db233`; the plan commit
is `970e7310104d2d05d52d141af42cfe00b025e275`. No PR184 parent is required.
Pascal owns these exact test-only hunks; Halley retains queue/full/merge ownership:

- `src/storage/metadata.rs`: cfg(test) instance `record_commit_failure_nth`
  AtomicUsize field and zero initialization only.
- `src/storage/metadata/effects.rs`: decrement that counter only inside the
  existing successful `committed.and_then` closure; Nth1 returns the same synthetic
  lost-ACK error class as the existing probes. Old thread-local/batch hooks remain.
- `src/api/validation_tests.rs`: register the new test module only.
- `src/api/sweep_failure_tests.rs`: local real migration/adoption fixture, real
  router POST `{}`, first/later record lost-ACK probes, existing known derived
  publication-failure positive control, and supplied-asOf rejection control.

The hook is armed after fixture insertion, migration, explicit policy adoption
and initial completion. A wrong-width vector view supplies the existing known
failure control; it must not be relabeled as an unknown commit. The request uses
the configured counting clock, not public asOf. Full response bytes are logged.
Before assertions on the missing HTTP fields, the unknown tests require the
unavailable owner and unresolved accepted work, actual failed readiness, exact
G increments, durable scoped receipt/hash/original1, unchanged historical receipt
bytes, pending outbox and full-text ledger mask, retained Summary content/anchor,
and strict native reopen without another canonical commit. The later test gets
the earlier full receipt from actual durable history for comparison only; the
production handler does not gain a readback/reconstruction path. No unreturned
current receipt is passed off as known to the caller.

Expected RED is only the final structured HTTP evidence assertion, after durable
proof succeeds. Compilation/setup failures are not defect evidence. All source,
binary and logs are retained separately from a later corrected run. No production
mapper, error schema, receipt collector, completion or transaction budget changes
belong to this stage. Curie released the hook boundary; exact diff source ACKs
from Curie/Feynman/Huygens and Halley's literal RED build release are still required.

### Unresolved production design review

The earlier production outline is **not approved for implementation**. Two
concrete requirements remain before its byte-bound/readiness claims are valid:

1. Measure exactly the representation emitted by HTTP. In particular, encoding
   a typed f32 receipt and later passing it through `serde_json::Value` may change
   number spellings and length. Either emit the measured typed representation or
   cache/count the actual Value representation. Exact serialized identity must
   retain replay flags and numeric spelling, including negative zero.
2. Enforce completion admission under its existing publication/graph/storage
   locks, before purge resume or rebuild. A read-only preflight before awaiting
   those locks does not exclude concurrent foreign pending work. Native-decay
   and compaction errors can nest a distinct preexisting receipt/unknown leaf;
   the unconditional final completion can also start additional canonical purge
   work. Merely bounding the candidate receipt does not bound those outcomes.

The proposed narrow direction is a bounded mode of the existing completion path
which only completes already-admitted pass effects, refuses foreign pending work
and active purges under the same final locks, and never runs generic completion
after a budget/unknown stop. It must not report a historical foreign operation as
this sweep's commit, omit a distinct in-hand nested receipt, invent an unknown ID,
or silently discard a known receipt to fit the cap. Exact API, race checks and
encoding choice remain subject to owner review; RED-only tests do not validate
this design or release those production hunks.

No edits to frozen PR172/181 or queue worktrees. Runtime child will use this exact
main base, plus this plan, after literal owner release; no PR184 ancestry. Reuse
one Pascal-owned bounded cache/target, no cloning or shared concurrent target.
Tests are synthetic offline Docker, Rust1.94.1, jobs2/2CPU/4GiB, network none,
no home/credentials/socket mounts. No builds now while plan reviews are pending.
Protect failed evidence under main's explicit rule; passing raw release only after
queue publication/results acceptance. No personal recalld access or deployment.
