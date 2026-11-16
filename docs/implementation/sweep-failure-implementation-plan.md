# E1 correction: guarded completion and bounded typed HTTP evidence

Historical proposal and review contract. The subsequent implementation and
source-specific validation are recorded in
[the correction report](../../work-notes/sweep-failure-fix/REPORT.md); the
proposal-stage release language below is retained for attribution.

Proposed implementation child of sealed RED `530f3bfc3c2ef777aacf7cf55d2bcfbf68633280`,
whose sole runtime base is actual main `33865ca0d19599a13ba999d31a1ec736c89db233`.
Worktree `/tmp/recalld-implementation/sweep-failure-fix`, branch
`fix/sweep-failure-evidence`. Pascal owns implementation after literal coordinator
release. Halley owns queue integration; Feynman reviews storage/completion proof,
Huygens HTTP encoding, Curie shared effects, Hume worker/lock participation.
No PR184 parent, frozen branch edits, new scheduler, schema or durable format.

This amendment supersedes provisional completion and serialization choices in
the original plan. It is source-only, **not an implementation/build release**.
The original RED source, executable and logs remain immutable. Its two expected
HTTP failures, two positive controls and earlier compile-only failure retain
their exact attribution in `work-notes/sweep-failure-contract/REPORT.md`.

## 1. Entry and compatibility

Keep the existing HTTP route and `FsrsEngine` trait signature. The concrete
`FsrsEngineAdapter` trait method for manual sweeps selects bounded execution,
with original1 and the existing configured clock. Its existing internal
origin-explicit helper and autonomous/background runners retain their old
unbounded/counter-only mode unless a caller explicitly selects the new bounded
helper. Do not infer the mode from original API number or disk format.

One shared runner implementation accepts an optional private pass context.
Existing wrappers supply None; the new bounded adapter supplies Some. Preserve
the existing public `SweepResult` and success JSON shape; return a separate typed
`SweepFailure` through the existing boxed-error path. The handler downcasts that
specific type. No global remapping of unrelated storage/recovery errors and no
string parsing, mutation retry, new public keyed/no-op or resume contract.

The pass context holds only bounded ephemeral evidence, checked encoded costs,
stop cause and exact receipt authority. It is not a second work tracker, storage
registry or durable journal. Reuse the existing accepted root/children and worker
leases. The one sampled time and existing planner/policy/graph rechecks stay intact.
Public asOf/as_of rejection and omitted-clock behavior remain unchanged.

## 2. Exact response representation and budget

Use a typed `Serialize` failure body, serialized directly by `axum::Json`.
Receipt values never pass through `json!` or `serde_json::Value` on the production
response path. The budget/identity helper uses the same compact `serde_json`
serializer on the same complete immutable `WriteReceipt` type. Cache its encoded
bytes once for cost and exact identity; this preserves negative-zero spellings,
replay flags and all other fields. Do not compare only IDs or normalize receipts.

Compatibility boundary: the old known-failure `json!(receipt)` path promotes f32
numbers into Value spellings (the sealed control has `0.020643016323447227`); direct typed
serialization can emit the shorter same-f32 spelling `0.020643016`. This is an
intentional failure-response encoding change, not a wrong old fixture or a stored
receipt modification. Preserve the sealed old raw response. The updated known
control must assert exact decoded `WriteReceipt` equality plus the new body's
receipt representation against `from_slice(to_vec(retained_receipt))`, rather
than silently retaining the old Value-conversion expectation. Never round/cast
the stored receipt, change its request hash or rewrite historical bytes. Success
responses remain unchanged.

Failure body fields are fixed: error code, commitState, ready=false,
complete=false, retryable=false, ordered full receipts, operationId UUID-or-null,
and a fixed code-selected message. Only the existing known-failed case has the
optional singular full `receipt` alias. Unknown precedence suppresses that alias
but preserves all legitimately in-hand prior/outer known receipts. No counters,
timing, raw error display strings or arbitrary diagnostics enter this body.
The RED known control demonstrated why raw `source.to_string()` is unsuitable:
it already embeds another entire escaped receipt in message text.

Use the existing 8 MiB constant as an HTTP-body ceiling, without claiming an RPC
or MCP framing guarantee. Reserve 2048 bytes for all fixed fields/punctuation and
the maximum UUID spelling. Charge `2048 + 2 * sum(Li + 1)` for N distinct complete
receipt values with exact typed encoded lengths Li. Two copies conservatively
cover the array and a singular alias. Every addition uses checked arithmetic.
Maintain a cumulative total, not repeated serialization of a growing prefix.

The serializer's capped writer stops before exceeding the supplied allowance;
candidate bytes are retained only when admitted. This bounds response evidence,
not complete proof construction, transient serde allocations, the whole store or
rebuild memory. No performance claim is made from this correction.

Before work, a test over **every finite error/state/alias/UUID-null shape** must
prove that fixed overhead is at most 2048. Additional actual-handler tests compare
the full emitted body length against the calculated charge and configured cap,
including native floats, escaped strings, maximum valid numeric/UUID spellings,
physical receipts, alias duplication and unknown forms. Do not validate only a
helper's typed length then emit a different Value representation.

## 3. Receipt authority and producer admission

An exact `(storeId, operationId) -> encoded receipt bytes` set contains only
actually returned, budget-admitted evidence from this pass. Cache receipt values
alongside those bytes in canonical return order. Repeated identical evidence is
deduplicated. A same-ID/different-byte in-hand conflict retains both admitted
values and stops with typed conflict/unknown, never last-wins or guessed ID.
Count both values in the budget; do not silently discard one. Candidate admission
and completion's closed set must prevent an unbudgeted foreign value becoming
returned evidence. Corrupted persisted mismatches are refused, not copied into
this response as purported in-hand commits.

The bounded context may be shared with its existing worker by a pass-local Arc
and short mutex, avoiding repeated copies of the growing receipt prefix. No
context lock is held while awaiting another guard, performing I/O, committing,
rebuilding or acknowledging. Receipt insertion and lookup are short synchronous
steps; current execution remains serial. This is not a parallel observation path.

Storage receives a crate-private `ReceiptAdmission` view and numeric
`ReceiptSizeLimit`, not HTTP types. Existing public/internal methods remain
wrappers with None; bounded variants are used only by this consumer. Before each
producer starts canonical work, under its existing engine and metadata mutation
guards, require a healthy/nonpurging owner and every pending effect to be
canonical-applied and exact-byte-identical to already-admitted pass evidence.
Recheck at the actual guarded producer entry, not only an earlier async preflight.
Thus `apply_pending` cannot encounter foreign pending work after this check while
the producer still owns those guards. An empty initial authority set refuses any
preexisting pending effect without reading it back as a sweep result.

At the final metadata transaction, measure the **complete candidate receipt**
against its allowance before certification preparation, dirty/canonical_pending
or epoch publication, and commit. The staged transaction may already contain
record/index/ledger/outbox/control writes; aborting it must discard them all.
IDs may already have been drawn. No zero-allocation/zero-ID claim. A candidate
measured in this private transaction remains private if its commit returns unknown.

On actual success or a known postcommit wrapper, append the returned receipt
before the next fallible completion. Walk typed nested causes, collecting exact
known wrappers, retaining a real `CommitOutcomeUnknown` ID or null for an actual
worker-outcome-unknown, and giving unknown precedence. Do not infer unknown from
a dependency refusal, validation error, known precommit budget abort or display
text. Do not expose a merely staged candidate to fill an unknown slot.

## 4. Completion under the existing final locks

Extend the existing completion implementation with optional receipt admission;
do not introduce another completion engine. None preserves existing behavior.
Bounded calls use the same accepted worker, publication **write** lease, graph /
vectors / FTS / entities guards and final engine **write** guard.

Inside that worker, immediately after acquiring the final engine guard and before
target FTS removal, `resume_pending_purges_observed`, any early no-op, rebuild or
ACK, do all of the following in order:

1. Ensure the owner is healthy and has no active purge. A known preexisting purge
   is a typed dependency refusal. Never resume one in bounded mode.
2. Load/validate the pending effects through the existing validator.
3. Require every effect's `canonical_applied` flag and full encoded receipt to
   match the pass's already-budgeted in-hand authority set.
4. Refuse foreign/mismatching/unapplied pending effects before publishing views
   or acknowledging any effect. Return a fixed typed dependency failure with no
   foreign receipt attached. This pass's already-known receipts remain in its
   collector; refusal alone does not make their outcome unknown.

Bounded mode **never calls `resume_pending_purges_observed`**, even when preflight
previously saw no purge. It also rejects a supplied purge-cleanup target; that is
not a bounded-sweep operation. The early no-op check follows these predicates.
For an admitted set, retain the actual existing four-view rebuild and exact
canonical-generation publication, followed by complete outbox acknowledgement.
No target-only ACK, guessed readiness, no-change generation advancement or new
canonical operation is introduced by completion.

Retain the worker-held guards through rebuild and ACK. All actual engine callers
must remain serialized by those guards; audit their metadata access lifetime.
In addition, pass the same authority to an internal bounded variant of
`acknowledge_rebuilt_effects_at`. Under its existing metadata mutation guard,
recheck the full pending authority before any removal, retaining all current
checks: exact rebuilt G/current views, canonical_applied, byte-identical validated
rows, and no remaining unvalidated rows in the ACK transaction. The old wrapper
supplies None. This closes same-G effects changes without weakening the existing
ACK proof or relying solely on a broad claim about every possible direct writer.

Include the empty-pending case: the current ACK helper can invoke its test hook
then clear dirty/return before opening the final transaction. Bounded ACK must
revalidate current G, views and empty authority in the final guarded transaction
before that early success/dirty clear, detecting a same-G inserted effect too.
Do not change the ordinary None path under this release. Empty validation need
not create a canonical operation or commit an otherwise empty metadata write.

Preserve the main182 healthy-handle validation boundary: these checks inspect
only current pending effects and their keyed canonical receipts. Do not call
whole-store `control::validate_existing`, strict schema-history validation or scan
the lifetime receipt table on each mutation/completion. Strict startup/offline
proof remains unchanged. The existing effects validator's normal initialized-table
path checks current rows by keyed lookup; its absent-schema historical fallback
must not accidentally become the new bounded hot path. Prefer rejecting a foreign
row before collecting an unbounded foreign outbox, but do not weaken validation to
do so. Retain existing instrumentation and add a small-vs-large retained-history
decode-count regression with the same current pending set at the actual guarded
helper boundary; no timing or whole-sweep complexity claim follows.

If a peer commits after this pass's producer but before completion takes its
exclusive guards, bounded completion refuses that peer's pending effect. Return
this pass's own known receipt(s), not-ready/incomplete and typed dependency failure.
Do not attach the peer receipt, ACK either effect, resume a purge or fabricate a
new unknown operation. No retries or self-draining barrier follow this refusal.

## 5. Exact call paths and stop rules

| Actual path | Required bounded forwarding / evidence |
| --- | --- |
| HTTP concrete manual adapter | Select bounded runner; typed failure downcast/typed JSON; original1/one configured clock |
| Runner initial completion | Empty pass authority; reject preexisting pending/purge before new work |
| Legacy strength-only update | Return its actual `Committed(receipt)` instead of discarding it; guard admission and final metadata limit |
| Legacy Full→Summary / Summary→Ghost | Existing atomic patch/index/ledger/effect transaction plus admission/limit; retain exact returned receipt and existing phase event |
| Legacy Ghost hard delete/bridges | Existing graph-guarded plan and delete core; admission/limit in common effect transaction; retain returned `EffectResult.receipt` |
| Native phase / hard delete | Existing policy/expected-state/graph guard and planner; admission/limit, append before bounded completion; preserve actual unknown worker cause |
| Compaction before selection | Same bounded completion; recheck guarded producer admission after that await, before private staging/canonical selection |
| Text-selection transaction | Full legacy/native physical receipt limit after proof validation, before cert preparation/commit; internal original1/hash unchanged |
| Compaction after selection | Append actual selection receipt before bounded completion; preserve known/unknown selection and cleanup semantics |
| Runner final completion | Bounded only, using current admitted set; never generic completion in this mode |

The common record-effect final size gate belongs in a bounded internal
`finish_record_effects` variant; existing unrelated callers keep None and incur
no receipt encoding work. Narrow forwarding reaches legacy lifecycle updates,
native maintenance, legacy hard delete and the engine wrappers above. The text
selection variant similarly keeps old wrappers and None behavior.

Budget exhaustion, any unknown outcome, dependency refusal, evidence conflict or
known publication failure stops further record mutations and compaction. **Skip
the unconditional final completion** on those exits. A successful record-only pass
still receives actual bounded completion; no success is inferred from counts.
Ordinary existing per-record diagnostics/skips retain their old meaning.

Compaction may prepare/sync an unselected file and consume IDs before its complete
receipt can be measured. A typed budget abort leaves selection/control/records/
outbox unchanged; only its definitely unselected private file may be cleaned.
Selected or unknown files must never be reclaimed by this path. Reuse existing
recovery/selection logic and keep historical unselected-orphan behavior explicit.

Known bounded failures finish only work whose canonical outcome is known, using
the existing tracker rules. Unknown/cancelled workers retain unresolved accepted
work and their leases. Never invent a receipt/ID or automatically run a readiness
barrier to conceal the failure. Success shape and public activation gates do not
change; ordinary native activation is not part of this correction.

## 6. Actual validation gates before handoff

Preserve the exact 530f RED source/binary/logs. Freeze each corrected runtime before
execution; fixture/compile failures are separate from intended RED/GREEN.

1. Re-run all four actual HTTP probes: first and later unknown GREEN with actual
   ID/prior full receipt; known receipt and asOf controls remain GREEN. Check raw
   body shape/size and original1/one-clock/strict reopen invariants.
2. Actual legacy strength-only, phase-loss and hard-delete/bridging cases, and
   native phase/hard-delete cases: chronological full receipts, unchanged planner
   semantics and durable proof. Reuse existing valid fixtures; no synthetic invalid
   large proof. Successful legacy/native compaction is a distinct physical receipt.
3. Budget first-candidate and later-candidate rejection at the real metadata gate:
   compare canonical rows, G, receipt bytes, indexes and ledger; preserve earlier
   exact receipts, no later mutation or compaction. A text selection rejection must
   leave its actual selected descriptor and canonical proof unchanged. Record any
   consumed IDs/private staging separately.
4. Deterministic peer race: an instance-scoped test-only pause after this pass's
   canonical return but before completion's exclusive guard; a real peer commits
   while paused. On release, prove its pending effect remains, no ACK/publication
   or extra canonical G occurs, only this pass's known receipt is returned and the
   owner is not falsely ready. A real active purge at the same boundary is refused
   without resuming any purge child. No global fault state or parallel scheduler.
5. Preexisting pending/purge before work, compaction's await-to-entry race, and a
   final ACK same-G row substitution/addition: fail closed through the same guarded
   predicate and retained exact-G/no-leftover ACK checks. Keep ordinary None-mode
   completion/purge and existing cancellation/ACK fault tests unchanged.
6. Actual known later-record failure and compaction selection/publication failure
   retain current plus prior full receipts. Nested typed known/unknown causes and
   worker loss retain precedence/identity without duplicate message serialization.
   No completion after budget/unknown stop; assert no extra canonical generation.
7. Exact typed response budget tests enumerate fixed variants and validate actual
   emitted bodies, duplicate equality versus distinct conflict and alias cost.
   Preserve original1/autonomous2 behavior and historic strict reopen proofs.

Run focused producer/HTTP/guard/budget/race tests and fmt/clippy, then the appropriate
full runtime gate coordinated with Halley (not duplicate queue fulls by default).
Offline Rust1.94.1 Docker only, 2CPU/4GiB, one bounded private cache/target and jobs1
unless coordinator changes that limit. No study retrieval, personal data or daemon.

## 7. Hunk release checklist

The named scope includes `api/{adapters,handlers}` and a private typed response
module; `decay/sweep.rs` and its native/evidence helpers; existing completion core,
native-decay and text-compaction forwarding; storage maintenance/engine wrappers;
metadata legacy/native maintenance, common effects and text selection; one private
admission/size helper and typed error variants. Existing ACK receives only its
optional bounded predicate. Test-only pause hooks are instance-scoped at the
actual existing worker boundaries. No generic error-string parsing or unrelated
graph/purge/catalog/batch behavior is added.

Curie's earlier common-effect variant/hook release is retained; obtain ACK for the
exact added bounded-ACK predicate. Feynman reviews all final-lock/ACK/selection
checks; Huygens measured/emitted representation and HTTP shape; Hume worker leases
and actual writer serialization. Freeze this amendment for literal source review
before runtime edits. Once released, Pascal implements and proves the actual fix;
the plan is not the final deliverable.
