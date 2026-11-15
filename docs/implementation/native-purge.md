# Native namespace purge

The implementation uses the existing owning engine, accepted task tracker,
namespace purge state machine, removal ledger and mutation outbox. The minimal
parents are native numeric policy PR94 and native maintenance PR98, including
its historical peer-policy and worker-lease corrections. No batch, graph/store
producer or broad integration checkpoint is a shipping parent.

## Canonical transaction and recovery

`NativeNamespacePurgeStarted` retains a typed namespace identity, dimension,
immutable profile/provenance binding and numeric policy. It contains no memory
text, labels or embeddings. Namespace extensions are rejected before receipt
creation, and unknown record extensions are rejected before accepting durable
Purging; an unknown payload must not be copied into a retained witness or leave
an accepted operation that cannot drain.

`NativeNamespacePurgeAdvanced` binds the reserved child identity, predecessor
G, sequence, stage and optional deletion chunk. Each chunk has at most 1000
`NativeDeletionWitness` rows, exact removed edge proofs and surviving peer
before/after evidence. Counts and cursor are derived from the witness and
checked against the actual rows in the transaction. All retained phases,
including Tombstone and vector-owning Ghost, can be purged. The automatic
Ghost-only hard-delete planner is not the purge eligibility rule.

Record deletion, peer count/token changes, HardDelete/ALL removal-ledger entries,
receipt, progress and outbox commit in one metadata transaction. Edge-file
publication and namespace-directory removal remain separate recoverable work.
The leased namespace directory is removed after the final canonical emptiness
check; no delayed per-record vector free can target a reused slot. Surviving
peer vectors remain owned and unchanged.

The existing progress table/container stays version1. New native progress rows
are version2 with new typed outcomes; version1 bytes, hashes, IDs and historical
receipts are not rewritten. Deleting -> Cleanup -> Complete, stable reserved
child IDs and sorted UUID cursor remain the existing state machine. Replay
precedes mutable namespace/policy/CAS checks. A replay of an old completed purge
cannot target a recreated namespace with the same name and a different ID.
Purge needs neither provider work nor a fresh clock sample.

Strict preflight permits native recovery only for validated version2 active
progress and its exact pending outbox. A missing edge endpoint must be the
specific UUID proven deleted by that chunk; surviving endpoints, edge bits and
reverse indices still validate. Other missing/corrupt edges fail before writable
source open. Ordinary offline export/backup/restore rejects Deleting/Cleanup
immutably. Completed native progress survives actual logical export/import,
backup and production restore fork with original origins and operation IDs.

## Retained history and publication

After live namespace removal, strict history reconstructs validation-only
configuration from the validated start witness. Numeric policy/no-change and
lifecycle revision histories still bind to their original namespace. Every
purge and hard-delete peer binds to its own policy revision/version, including
foreign-origin receipts after restore. Same-origin operations must precede
the affected namespace's purge start; a purge's own internal remaining-row
updates are checked against its retained policy. Reverse validation restores
namespace configuration at Cleanup and rewinds deletion/peer evidence per chunk.
This reconstructs commitments for validation, never deleted payloads.

`DerivedCompletion::purge_native_accepted(ctx, force, &parent)` accepts work
beneath the existing root. Its blocking worker owns publication-read lease,
graph-read guard and engine-write access in that order. Cancellation of the
async waiter does not release publication ownership while the worker remains
queued or blocked. Guards are released before awaited `complete_with_work`.
The response reports Complete only after completion succeeds. An uncertain
child acknowledgement remains `CommitOutcomeUnknown`; a known committed
failure preserves its full current child receipt, not an older parent receipt.

The actual historical peer-policy corruption is a reproduced acceptance bug.
The separate cancellation probe shows premature publication-lease release in
the previous helper while graph guard and unresolved tracker evidence remain.
It does not demonstrate false Ready or a data race. Moving the lease into the
worker is a preventive lifetime correction backed by that deterministic probe.

## Validation and remaining consumers

Final validation results and exact runtime/tree/binary/log hashes are recorded
in `../evidence/native-purge/README.md`. Failed source/binary manifests and logs
are retained, including admission, historical ordering, peer-policy and lease
probes. Tests use synthetic roots, offline Rust1.94.1, two CPUs, 4GiB and a
private target with no credentials, personal data, providers or socket mounts.

The actual public purge consumer belongs to Huygens. It must join the sole
Halley publication registry and preserve accepted roots, full receipts and
current-generation publication. This minimal parent does not claim the later
all-component whole-query ReadView or ordinary native serving activation.

Feynman owns the subsequent mixed policy -> graph/linked-store/maintenance ->
purge -> export/import/fork proof on actual validated dependencies. Retired
namespace dimension/profile bindings must participate in every all-origin
validator; edge history must rewind purge and maintenance removals before older
graph/insertion transitions. Native batch joins must consume the real manifest
validator and child binding, not an outcome allowlist. Unsupported required
canonical tables/variants continue to fail closed here.

Feynman also owns ordinary engine/system/daemon startup admission, after actual
writer and coherent-reader closure. Huygens owns public routing/capabilities,
Curie producer roots, Pascal lifecycle/maintenance and Halley the sole registry.
The next admission plan must name exact minimal frozen parents, the existing
constructor/preflight/recovery seam and startup evidence before implementation.
No second engine, independent tracker or premature gate removal is introduced.
