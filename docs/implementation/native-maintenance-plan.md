# Native maintenance implementation boundary

Assigned 2026-11-14 after Feynman confirmed no maintenance implementation had
started. Parent: corrected PR82 bb1a086, runtime fb804a0. Existing native
MutationGuard, RecordMutation and effects publication are the implementation
boundary. The second parent is Hume's minimal profile-shape helper
`1f4eb18c24a3d3f512030bbee0f9c4adcecfe656`, based directly on fb804a0.
No 70/80, catalog, numeric-policy, graph-writer or batch dependency is imported.

Pascal owns phase materialization, phase loss and automatic Ghost hard deletion.
Feynman retains numeric namespace policy, native purge and shared history review.
Hume owns native graph puts/upserts; Curie owns producer/batch registration.
Additive shared match arms must compose without replacing their variants.

The explicit native owning-session entry delegates to the existing engine and
metadata transaction. Matching original-ID intent replay precedes capture,
clock, fresh policy and tokens. Captured preconditions are not retry identity.
One sampled clock is retained across final checks and projection. A final native
publication guard protects canonical edge loading and the metadata write
transaction. No graph await occurs under a storage/publication lock: this private
entry computes from canonical edges, not a caller-supplied bonus or cached graph.
`DerivedCompletion::apply_native_decay_accepted(ctx, intent, policy, parent)`
consumes the producer's existing WorkTicket. It holds the publication read lease
and acquires graph before storage, then releases those locks before awaiting
derived publication. Known publication failure retains the full committed
receipt; matching replay completes publication. It does not register a new root
or enable a background producer.

Projection uses the persisted decay anchor and effective time clamped to the
record's lifecycle_updated_at. Only canonical same-namespace Full/Summary donors
with retained reviewable content contribute; build a same-namespace graph so
foreign edges cannot alter fan counts. Ghosts receive no bonus. Recompute even
when the captured target previously needed only a strength update. Policy/token
changes skip stale work; unchanged/exempt/missing-summary cases commit nothing.
These are internal diagnostics and do not reserve a key or promise a public
keyed-noop finalization contract.
A pass captures each ID once and advances at most one destructive phase.

Phase loss preserves anchors, provenance, access/history, feedback timestamps,
stability and permastore. Full -> Summary drops full text only, requiring a
nonempty summary. Summary -> Ghost strips summary/tags/structured labels while
retaining any canonical vector. Phase time is max(effective_now, old marker);
strength-only changes leave the phase marker alone. Unknown extensions fail
closed before destructive changes.

Typed NativePhaseMaterialized and NativeMemoryHardDeleted receipts keep original
native tombstone receipt semantics untouched. A bounded deletion witness stores
UUID/namespace, NativeRecordState and numeric access history, never removed text,
labels or embedding bytes. Validate history/hash, predecessor state, origin,
ledger HardDelete/ALL, and exact peer token/count decrements. Reverse proof only
reconstructs ProofRecord, never canonical payloads. The existing final transaction
commits record/peer changes, removal ledger, receipt and outbox together. Actual
vector cleanup retains the existing canonical owner guard; a stale release may
not touch a reused slot. Purge will consume the validated witness later.

The shared witness is `NativeDeletionWitness { memory_id, namespace_id, before:
NativeRecordState, history: Vec<AccessEvent> }`. `validate()` checks non-nil
identities, lifecycle and profile shape, strict bounded history and its hash,
and rejects unknown extensions before destructive use. `proof_record()` returns
only a reverse-validation state. Automatic `NativeHardDeleteWrite` additionally
requires a Ghost eligible for deletion, exact sorted incident edges, and exact
surviving-peer token/count changes. All receipt origins bind profile commitments
to their immutable namespace configuration. Purge must supply its validated
beforeNamespace when that configuration has been removed.

Native startup may repair disposable copies, fully validate them under ownership,
then open the live databases. A missing physical edge endpoint is allowed only
for the deleted primary in an exact typed, validated, unapplied pending hard
deletion. Every other endpoint must exist. Matching uses every edge attribute,
including float bits; +0 and -0 are distinct. Offline inspection/export continue
to reject unapplied effects. Malformed proof or inventory rejection leaves the
source files unchanged.

New receipt kinds are explicit and unknown kinds fail closed. Older readers that
do not implement these kinds reject them; this is not full old-reader format
compatibility. Existing migration receipts and publication generations are never
rewritten. Existing tombstone phaseTimeRule semantics remain unchanged.

At the actual numeric-policy join, rewind namespace configuration before checking
maintenance evidence against the policy at that revision. At the graph-writer
join, reverse `NativeMemoryHardDeleted.removed_edges` before validating earlier
graph/insertion proofs; do not silently skip the deletion variant. Phase writes
change no edges. Batch integration must preserve the existing optional child
argument in the shared final transaction. Those owners retain their changes.

The retained history decoder enforces ACCESS_HISTORY_MAX before allocation grows
beyond the bound. Incident-edge proof size is proportional to the existing graph;
this change invents no arbitrary edge cap. Projection currently scans canonical
namespace rows and edges per evaluated record. No scaling/performance claim is
made. The graph freshness regression changes actual donor feedback and canonical
fan-out while the target token stays unchanged; native edge-put joins require
their own producer composition test.

Validated evidence: actual migration/native writes, phase boundaries and retained
payloads, stale feedback/policy/save-edge/no-transition plans, original clock and
regression, exemptions/missing-summary/noops, keyed replay, harddelete peers and
ledger, slot reuse safety, real precommit/process-exit recovery, strict reopen,
export/import/fork and malformed-proof immutable rejection. All execution uses
isolated offline Docker with private caches and the existing CPU/memory limits.
Ordinary schema-2 serving remains gated; no completed A3/A6 or performance claim.

Validation: 13 focused maintenance test entries (including the process helper),
one accepted-publication test, 940 library tests and two CLI tests passed; four
doctests remain ignored. Formatting passed. Clippy exited 0 with 623 library and
661 library-test warnings (621 duplicates), plus one binary warning. The preserved
recovery and signed-zero failures, source/binary identities and final raw logs
are in `work-notes/native-maintenance/`.
