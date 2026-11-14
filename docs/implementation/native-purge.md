# Native purge implementation boundary

Approved plan, not an implementation/validation claim. Draft child begins at
PR94/8deb56c. Consume Pascal's actual validated maintenance/hard-delete parent
before wiring the witness. Corrected82 already contains legacy67 and offline74;
80/77/91/83 and a broad checkpoint are not automatic parents. Add only actual
consumed APIs. Feynman owns native purge and its strict history/offline integration.

Reuse namespace_purge_schema version1 and the existing PurgeProgress table/shape,
reserved child operation IDs, cursor/counts, and Deleting -> Cleanup -> Complete
stages. New progress rows have version2 and new typed receipt variants, so old
readers reject them. Version1 serialization and historical receipt/hash bytes
remain unchanged. There is no second journal or caller-selected child identity.

NativeNamespacePurgeStarted retains a typed namespace proof: immutable ID/name,
dimension/creation time, existing NamespaceSchemaV2 (binding, lifecycle version,
policy revision and provenance), and NamespacePolicyState. It contains no memory
text, labels or embeddings. This state is already reachable through the start
receipt in progress; a duplicate optional progress field is unnecessary.

NativeNamespacePurgeAdvanced retains the existing parent ID/sequence/stage/counts/
cursor plus predecessor generation and an optional typed deletion chunk. Chunks
carry at most the existing1000-record limit, Pascal's NativeDeletionWitness for
each removed row, NativePeerChange and exact NativeEdgeProof removals. Native
purge may remove any retained phase; it must not reuse the automatic Ghost-only
hard-delete planner as its eligibility rule. Per-chunk removal ledger HardDelete/
ALL, record and peer mutations, receipt, progress and outbox share one metadata
transaction. Per-record vector release is unnecessary inside a namespace whose
leased directory is removed after all chunks; surviving peers are not freed.

The actual owning engine performs begin(ctx, force, parent), bounded reserved-ID
steps and final cleanup. Replay stable IDs before mutable names/policy/CAS. No
provider or clock is needed. No insertion while Purging; final emptiness and
configuration removal are checked in the same transaction. Directory cleanup is
recoverable separate filesystem work; the name lease survives interruption until
completion. Preserve the current child's full receipt/unknown acknowledgment on
failure, never mask it with older progress. Await actual DerivedCompletion after
locks; whole-workflow readiness requires Complete plus publication agreement.
No atomic whole-purge or automatic readiness claim.

Strict history builds validated historical namespace configurations from the
start proof after the live configuration is removed. Numeric revisions and
retained no-change receipts still validate against their original policy state;
all-origin graph/maintenance profiles still match the immutable binding. Active
reverse validation restores namespace proof state at Cleanup, then records and
peer tokens at each chunk. It reconstructs only validation state, never payloads.
Historical origin counters are compared only within their lineage. New required
proof/progress versions fail closed.

Native startup may recover only validated version2 active progress and exact
pending outbox deletions. A missing endpoint is allowed only if that exact UUID
is proven deleted by the chunk witness, every surviving endpoint exists, edge
proofs match bit-for-bit and reverse-index consistency holds. Unknown/extra
missing edges fail before writable source open. Ordinary offline validation still
rejects active/Cleanup work. Completed progress, receipts, ledger and retained
namespace proofs survive export/import/schema copy and production fork restore,
including target/source history merge; original IDs and origins never change.

Native batch integration uses the actual frozen manifest validator at its join.
Pending children remain pending or become explicit final scope errors; they
cannot rebind a recreated name, silently complete, or lose their original
namespace/default/profile identity. Completed outcomes remain historical after
purge. Unknown canonical tables stay rejected until their actual consumer joins.

Required evidence: mixed Full/Summary/Ghost/Tombstone chunks and foreign incident
edges; insert/policy/feedback attempts during Purging; original-key and reserved
child retry; exact cumulative counts, ledger masks and peer tokens; interrupted
start/chunk/edge/config/directory/ack boundaries; no source mutation on malformed
proof/outbox; namespace recreation under a new ID; completed export/import/backup/
fork and unfinished immutable rejections. Feynman owns the later combined
policy -> graph/maintenance -> purge -> export/import/fork proof using actual
frozen parents. Hume owns graph core, Pascal maintenance/witness, Curie producer/
batch roots, Huygens actual A4 consumer, Halley sole completion/readiness registry.
