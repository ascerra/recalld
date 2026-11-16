# Effects writer and validation audit

Parent: `fe1dacbb53b0dca6dfdb1b765297c834d4addb3e`. Source-only findings,
before optimization. Main approved plan ac87c345; Halley released these hunks and
confirmed no PR176 dependency. Feynman returned A1 source ACK conditional on the
listed generation/view, validation, writer exclusion and unknown-commit fences.

| Effects-table operation | Existing serialization / boundary |
| --- | --- |
| `effects::initialize` | Empty table/schema creation during metadata initialization, before the live owner escapes. |
| `MetadataStore::finish_record_effects` | Receives and checks the caller's `MutationGuard`; canonical record/effect/receipt transaction stays within that guard. |
| `acknowledge_canonical_effects` | Requires/checks the same guard before changing `canonical_applied`, even though this acknowledgement does not advance canonical G. Recovery `apply_pending` checks the guard and constructor recovery obtains `canonical_effects_guard`. |
| `acknowledge_derived_effects_with_work` | Actual native/legacy mutation guard held through validation, deletion commit and dirty update. Existing single-ID path remains unchanged. |
| `finish_purge_cleanup` | Requires/checks its caller's guard. Purge guards acquire the same metadata publication write lock, with narrower recovery availability rules. It creates a receipted effect within its guarded transaction. |
| `restoration::reconcile` | Offline fresh staging only. Actual restore caller uses an inaccessible TempDir under stable target ownership; no live MetadataStore sharing the database is present. This is not an in-process effects writer bypass. |
| Other `EFFECTS`/`mutation_effects` references | Validators/readers/export inventories or explicit corruption fixtures under cfg(test); no further live production table mutation found. |

The guard argument is essential: exact G alone cannot detect same-G canonical or
derived acknowledgements. The new batch must retain one guard continuously from
the validated read snapshot through the write transaction and dirty-state update.
The actual table-empty check after deleting the validated IDs rejects leftovers
atomically, not by clearing/truncating the whole table. Race tests must exercise
both canonical writers and the same-G canonical-application acknowledgement seam.

On this parent `native_guard_with_work` obtains `publication_guard`, checks
availability, and calls `native_control_at` on a real read transaction. That
method validates the native control singleton/schema and serving lineage; it does
not invoke the complete effects validator or lifetime-history validator.
`mutation_guard_with_work` likewise validates actual control and serving lineage.
This is a source conclusion about this parent, not a permanent assumption about
later schema joins.

The test-only BEFORE probe starts at the actual bulk ACK method entry, before work
admission and guard acquisition. Its scoped worker-thread counters include **all**
calls to `effects::validate_existing`, all effect rows visited by those calls,
and ACK commit attempts/successes while that batch executes. It excludes fixture
creation and the publisher's earlier pending scan; those boundaries are stated
rather than counted as zero work. Counter output is not physical redb I/O, receipt
lookup count, whole-startup work or fsync count. Later guard changes that invoke
the same validator are included automatically; additional validation families
must be audited and reported separately, not silently folded into a one-pass claim.

All counters and their store-local delivery hook are cfg(test), with no production
clock, allocation or parallel metrics authority. The actual DerivedCompletion
worker delivers the result to its test; the test does not substitute a fake
publisher or assert view readiness from manufactured stamps.
