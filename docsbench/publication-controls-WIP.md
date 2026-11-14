# Owned publication controls: unvalidated consumer work

This is working implementation documentation, not a publication or completion report.
The registry will ship with its first actual coherent-query/barrier consumer and
required writer joins, after isolated validation. No registry-only PR is proposed.

## Current source and evidence

Last compiled source: `671bede` on `impl/publication-controls`, based on actual
73 + 70 + 61 + 78 + 79 dependencies. Focused isolated validation at
`/tmp/recalld-publication-controls-test-jgc8kgiu/output` passed 27 publication,
11 completion, and 13 observation tests. Its entity filter passed 38 and failed
2 fixture tests, then stopped before batch/native filters. The failures were an
undeclared 2-dimensional fixture provider and text embedding against unknown
legacy provenance. Source now uses the same explicit synthetic query vectors
and fixture dimension declaration as the actual native reader parent `d1d916e`;
those corrections have not been rerun.

Earlier failed runs remain intact:

- `recalld-publication-controls-test-qg_aik89`: initial compile integration errors.
- `recalld-publication-controls-test-93kw_s3y`: two canonical entity fixture errors;
  its old observation filter matched zero tests and is not observation proof.
- `recalld-publication-controls-test-n5y70_d3`: native/batch merge compile errors.

Source after `671bede` is unvalidated. A5 sampling hold began
2026-11-14 15:27:38 UTC, hard end 15:47:38 UTC, with explicit END required before
new builds/tests/formatting/copies. All local heavy jobs were drained beforehand.

## Actual consumer being implemented

`Recalld::search_coherent_read_only` is an internal Rust entry point through the
actual QueryEngine. It rejects requested true mode; it never rewrites the caller's
request. It registers an owned root before waiting, registers each blocking
child before spawning, and pins the same live graph/vector/FTS/entity handles
used by production adapters. The adapters read these leases directly, avoiding
recursive locks behind queued publishers.

The canonical view retains one actual redb read transaction. Namespace policy,
member discovery, and canonical record hydration all read that transaction.
Capture requires every derived component stamp to equal its actual GenerationRef;
a missed notification cannot bless stale views. Behavioral record caches are
bypassed and the query's clock is sampled once. The response includes that actual
read generation. A newer canonical commit can coexist with the pinned older
query; this is not an as-of reconstruction or copied-file snapshot.

Readiness barrier source drains the existing owned WorkTracker before acquiring
publication/derived/storage locks. It rejects foreign/future targets and verifies
actual pending effects, purge progress, batch items, and derived stamps. It does
not repair a failed durable mutation or invent missing batch input. Completion
remains the explicit actual rebuild/ack path.

The current-state metadata check uses a healthy already-validated owned handle.
It checks actual pending tables, including stale-dirty-flag cases, with cooperative
checks through enumeration. Full startup/offline proof and the original strict
metadata freeze remain unchanged. This does not promise a hard timeout over
blocking OS I/O. Barrier readiness is an observation; admission reopens afterward.
It grants no snapshot/reset permission.

## Shipping prerequisites and remaining scope

- Curie owns queued catalog/policy, compaction install, maintenance/background,
  and standalone graph roots/tails. Consume his frozen tested child, not file
  overrides. Keep known completion errors repairable while durable pending work
  blocks capture; unknown/cancelled/unavailable work remains unresolved.
- Pascal owns native reader/profile and same-owner native acknowledgement join.
  Preserve strict schema/proof/lineage guards and full committed receipts.
- True/prepared query roots through returned-only exposure, HTTP provider-before-
  insertion root, and standalone feedback still need actual root plumbing.
  Current source is not proof of global producer coverage.
- Native/public v2 activation, physical full-store snapshot/reset, owned canonical
  clock/ID/RNG capture, frozen config and cache state remain unimplemented here.
  Memory UUIDv7 and operation/store UUID generation are still real nondeterminism.
- The independent manifest consumer keeps coherentReadView/barrier/reset/native
  capabilities false until its actual runtime prerequisites and validation exist.
  No datasets, models, training, or human-quality judgments are introduced.

## Prepared validation after explicit END

Use the pinned `recalld-ci:1.94` image, Rust 1.94.1, offline network-none Docker,
2 CPUs/4 GiB, no home/credentials/socket mounts, separate private source targets.
Clean only this crate in the privately copied dependency target and preserve
per-source executable hashes. Do not reuse a test executable after source switching.

1. Offline formatting, freeze the intended source commit, then focused coherent
   query/barrier and metadata-current-state regressions.
2. Existing publication/completion/observation/entity/batch/native checks.
3. Full library and both actual CLI targets; clippy and formatting on exact source.
4. Required producer/native joins and actual transport/fault regressions before
   any global readiness or shipping handoff. Keep all failures/raw evidence and
   seal final artifacts to authorized work-notes only after validation.

Prepared new regressions cover physical false-mode canonical bytes, requested-mode
rejection, pinned records across a real observation commit, a queued graph writer
without recursive-lock deadlock, barrier waiting on an actual query root,
foreign/future targets, close-timeout-reclose, stale views after catalog commit,
current checks without lifetime receipt scans, cooperative deadline rejection,
and durable pending work despite a stale clean flag. These are planned assertions,
not passing results.

## Reset inventory handoff (Feynman, source review only)

Canonical capture must include namespace allocation high-water state, text
generation descriptors/selection, batch reservation and child identities plus
expanded defaults/policy hashes/outcomes, purge identities/cursor/sequence/stage,
schema publication/profile/migration proofs, and lineage ancestry with retained
per-origin receipts/ledger. Preserve canonical effects rows, including applied
but not yet acknowledged effects; they are not disposable derived checkpoints.
Pending effects must be discharged by the actual coordinator or capture fails.
Active batch/purge/import/restore journals cannot be dropped to fabricate readiness.

Freeze actual namespace bytes and policy/profile revisions, resolved lifecycle
configuration, owned clock, default namespace/provider mode, and zero pending work.
Capture RNG stream state/counter after draws, not only a seed, and restore before
workers can be admitted. Preserve supplied IDs exactly. Isolated fixture reset may
retain lineage only in its inaccessible retry domain; production restore forks it.
Use the existing logical-export canonical table inventory as a coverage reference.
No credentials or unknown profile provenance are inferred.

Pascal confirmed native insertion uses the shared metadata memory allocator after
replay/validation and native receipts use `control::record_receipt`. Additional
legacy ledger operation UUID sites and offline schema-migration/store IDs must
also be covered or explicitly rejected by reset controls. Reserved batch/purge
operation identities must pass through unchanged, never be redrawn by a new
owned source. No competing native allocator is being introduced.
