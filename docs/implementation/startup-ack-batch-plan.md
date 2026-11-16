# Startup effects acknowledgement: bounded implementation plan

Status: source plan for review, 2026-11-15. No runtime edits or tests on this
leaf yet. Owner: Hume. Branch `impl/startup-ack-batch`, worktree
`/tmp/recalld-implementation/startup-ack-batch`.

## Parent and ownership

Use actual main `fe1dacbb53b0dca6dfdb1b765297c834d4addb3e`. Halley confirmed
this exact base and released the metadata effects ACK/caller/test hunks; there
are no active queue edits to these methods. PR176 is not required: its catalog
completion and test fault hook do not change the ACK algorithm. Do not edit
frozen queue branches. Feynman confirmed no overlap and will review A1 semantics.

Read-only authorities: CONTRACTS (receipts/readiness/barriers), A1 sections 2–3
(durable effects and canonical recovery), A5 optimization gates, and
INTEGRATION-REVIEW. This is neither a schema migration nor a new registry.

## Evidence and limits

A6's original candidate is `096195d9`, with its recorded observer/adapter overlay.
Its original binary is
`526ba1ee48d112d134be9532892b8795569c12ad71b2a1db6ac1264134514a47`;
original source manifest is
`f8a7e82392d2a78731a202ae6e29eee06aebe98935c474bf9ef4ca4fb1d80642`.
The aborted pilot produced no candidate query, provider, or FTS-query result.

A6 subsequently ran one separately instrumented startup-only diagnostic. It
observed preflight open about 0.301s, canonical-before about 0.00669s, entry into
`with_clock`, then ACK plan size 5,183. Sparse progress recorded at least 2,816
successful ACKs at ACK-local 55.916391s before the 60-second wall timeout.
No ACK end or constructor completion was observed; final ACK count is unknown.
The three marker clock origins must not be subtracted across scopes. A6 sealed
the evidence at `e5db5f2db41eb86408dceea2f1416d627061696f`. The copied
[handoff manifest](evidence/startup-ack-batch/a6-handoff/HANDOFF.json) records
exact upstream paths and hashes; the [report](evidence/startup-ack-batch/a6-handoff/REPORT.md)
hash is `e8ddb4e9430b0b5951b1c77149ed331b82086ed15e99ef6b073a95da9c0dce97`.
The diagnostic executable hash is
`26984986969f1f11ac4d5ee9782eecab4fb77ff029b9bb8140675c0ad6441836`.
Its preserved worker result has 19 stage observations, exit -15 and timeout=true.
Only 86,673 bytes of compact report/result/source provenance were copied; no
dataset, database, executable, cache or failed clone was copied.

This localizes executed work, not scan-versus-commit dominance or measured
asymptotics. Synchronous diagnostic output has overhead. The original seeds,
failed clone, source, executable and study results remain unchanged. This leaf
is a new candidate; a later A6 use requires an explicit amendment, not replacement
of the failed comparison or a predrained fixture.

## Source finding

At both the frozen study source and this parent,
`acknowledge_rebuilt_effects_with_work` validates/snapshots the pending IDs, then
calls `acknowledge_derived_effects_with_work` per ID. Each call validates all
remaining effects before deletion, commits one deletion, then validates all
remaining effects again to derive the dirty flag. `validate_existing` decodes
each effect, validates its retained receipt and typed outcome, and sorts results.
For N successful deletions without concurrent insertion the two per-ID passes
alone visit N squared effect rows, plus the initial batch pass and other work.
Each deletion also has a separate durable redb commit. Neither cost is an
application requirement to delete already-applied, fully rebuilt outbox entries.

## Proposed change

Optimize only the full-rebuild acknowledgement path. Keep the existing single-ID
ACK path and its callers unchanged. No per-record mutation, receipt deletion,
history rewriting, generation advance, format change, durability relaxation,
provider call, indexing algorithm, or ranking change.

The production batch method will require the actual rebuilt `GenerationRef`
alongside its existing parent work ticket. DerivedCompletion passes the canonical
generation already captured for its real rebuild and published to each required
view. Never accept a caller-supplied boolean claiming completion.

1. Admit owned work and acquire the existing actual-owner mutation guard: native
   guard for a native owner, legacy guard otherwise. Keep the metadata publication
   write guard from validation through commit and dirty-state update. Keep the
   caller's existing derived publication, index, storage and worker ownership.
2. Under that guard, read actual canonical lineage/G from a real transaction and
   require equality with the caller's rebuilt generation. Check the existing four
   view stamps against that actual G through the existing registry; do not call a
   helper that recursively acquires the same publication lock. No new authority.
3. Invoke the existing full effects validator once on that protected read snapshot.
   Preserve every schema, identity, scope, receipt, foreign-origin, typed-effect,
   edge and release check. Check **all** returned effects have `canonical_applied`
   before removing any. Do not reuse a stale vector supplied by the caller.
4. With the same mutation guard still held, open one redb write transaction. Check
   actual lineage/G again in that transaction; remove exactly the validated IDs,
   requiring they still exist. Verify the resulting effects table is empty using
   its table cardinality, not another decoding scan. A failed check aborts the
   transaction. Legitimate canonical writers all use this same publication guard,
   so no writer can insert/alter an effect between the read and write transactions.
5. Commit once with existing durability. Only a successful known commit sets
   `derived_dirty=false`; no additional full validation pass is needed while the
   guard still excludes writers. Commit errors mark the owner unavailable as
   today; they must not assert rollback, clear work, or produce ready success.
   Empty validated input performs no write commit, but still checks matching G
   and views. The batch ACK itself never publishes a view stamp.
6. Release the metadata guard before finishing its owned work, retaining the
   existing caller's worker/lease lifetime and finish ordering. The synchronous
   worker is not cancelled when its async caller is dropped. Panic/unknown commit
   retains unresolved work/unavailable state under existing rules.

The full-rebuild call has only one production caller in this parent. Unit fixtures
that currently call bulk ACK merely to clear synthetic pending work must not fake
four ready view stamps to satisfy the new production check. Use the retained
single-ID ACK for such explicitly low-level fixture setup, or the real publisher
where the test claims readiness. Production has no unchecked bulk bypass or
optional-generation mode. Work-admission tests must continue testing the actual
batch method, including a closed root and an accepted child after admission closes.

## Crash, concurrency and recovery

Retiring multiple outbox rows atomically strengthens the ACK boundary; it does not
make the underlying user mutations an atomic batch. Their original receipts/G
already exist independently and remain byte-identical. Before ACK commit, a crash
leaves all selected effects for ordinary recovery/rebuild. After commit, recovery
sees the retired effects and reconstructs process-local views normally. A commit
error remains uncertain, with readiness closed until reopen/reconciliation.

A writer committed before guard acquisition must make the expected G or four
stamps fail; no newer work is consumed. A writer queued behind the guard runs only
after ACK and must restore dirty/pending state for its own effects. Test this with
barriers, not sleeps. Keep original publication lock order and no new awaits under
metadata locks. Existing historical-replay completion uses current canonical G,
never the historical receipt's G.

## Validation after plan review

First freeze a test-only BEFORE on this exact parent. Use actual canonical
insertions to create a backlog. Add test-only work counters at existing validation
and ACK commit sites, enabled only for the measured test scope: validator calls,
effect rows visited, attempted/successful ACK commits. No wall-time assertion or
counter inferred from a returned heap. The actual bulk method must fail a linear
work bound on the old implementation; preserve that RED, source and executable.
Instrumenting the source is not itself a performance result.

Then implement and validate:

- Empty, one, 32 and 256-entry real backlogs: exact receipts, records, logical
  canonical G/history unchanged, expected outbox deletion, readiness and reopen;
  ACK validation visits N rows once and nonempty ACK commits once. Initial setup
  and other publisher scans are counted separately. A 1,024-entry case may run
  once if focused resource/time results fit the declared budget; no seed cloning.
- Real DerivedCompletion startup/completion consumer, not just an ACK helper:
  all four actual views ready, scores/payloads retained, historical replay does not
  advance G, and a second completion is idempotent.
- Corrupt/missing/mismatched receipts, malformed effect/version/scope, invalid
  typed action/release, and a non-applied effect: no partial retirement. Include
  actual native typed effects and valid historical/foreign-origin provenance
  using existing fixtures where available; never fabricate valid certificates.
- Stale/foreign expected G, one stale or invalidated view, and concurrent writer
  before/after guard: no false readiness or deletion of unrebuilt work.
- Process exits before and after actual ACK commit, transaction abort and ambiguous
  commit injection: original receipts/G persist, repair/reopen resolves state;
  no claim that process exits prove power-loss safety.
- Accepted-child drain and actual async-caller cancellation while the completion
  worker owns the ACK guard; no premature barrier success or lost durable work.

Focused checks first, then one frozen full all-targets suite, fmt and clippy with
the usual warning disclosure. Offline Docker `recalld-ci:1.94`, Rust 1.94.1,
`--network none --cpus 2 --memory 4g`, `CARGO_BUILD_JOBS=2`, no home/credentials/socket
mounts. Use the single retained private cargo/target, package-clean only this
agent's recalld package between source roots. Commit/export source atomically,
verify inside-container manifests before compilation and after execution, record
test executable hashes, and never edit a mounted source during execution.

Suggested focused execution cap: 15 minutes per invocation, explicitly terminated
and retained on expiry, no unrecorded retry. No exclusive measurement window is
requested. Work-count improvement is not a latency, throughput, fsync or promotion
claim. Commit counts are not physical fsync counts: redb remains opaque here.

## Evidence and cleanup

Keep the plan, source pins, RED/GREEN commands/results and limitations in this
repository. This new worktree and active input snapshots are protected; no active
container exists yet. Retain only the already-owned cargo at
`/tmp/recalld-implementation/coherent-v2-similar/.validation/cargo` and target at
`/tmp/recalld-implementation/a2-conformance-inventory/.validation/target-entity`.
Do not recreate centrally released run roots. After completed PR/results, release
exact disposable run paths to central cleanup and retain compact worknotes, not
new raw HOME archives. Current cleanup recovered bytes: zero; no cleanup performed.
