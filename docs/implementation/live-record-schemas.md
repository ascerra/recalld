# Grouped live API1 record output schemas

Source plan, 2026-11-17, Huygens. No runtime changes, new validation run or
publication from this local research-descendant branch. Do not push this branch's
history. A reviewed exact document delta may accompany the actual consumer.

## Actual composition and ownership

- PR195 frozen `c4a62065aac6192f8b3b30ca6c3bd63fd48c7416`: actual API1 point-get
  producer/decoder/public projection. Curie handed off the stable public shape;
  Halley owns the frozen branch. Its original full1831/6 failures/9 ignored and
  corrected affected checks remain separately attributed, not called fullgreen.
- PR196 frozen `10d58c70bc18d936a0e2369e02c8440c4d2dc288`: actual modern schema
  registry and lifecycle codec definitions. Runtime f383d400 passed1770+10,
  eight focused Rust and71 formal cases. Halley owns publication/integration.
- Proposed isolated child uses those exact two heads, pruning either if already
  contained in the actual handed-off main. No N1/catalog, new storage, prepared
  O1-only wire or unrelated validation composition parent. Do not edit either
  frozen owner branch. No new codec, handshake, registry or public2 activation.

One consumed schema package covers get_memory, store_memory, store_memories and
forget_memories. Existing helpers may be factored only where these consumers
actually share definitions. No standalone helper PR. Catalog create/delete stay
together behind their actual final producer shapes; list_namespaces remains an
array in text. This group would bring object schemas from7 to11 of14 tools,
without claiming all A4 acceptance.

Halley source-plan direction is explicit: map producer/codec reachability first,
then meaningful focused/formal checks; no default broad matrix or full repeat.
Huygens owns schema/registry/tests/docs only. Any newly demonstrated producer
correctness problem needs a separate explicit owned runtime decision, not a
silent serializer change inside schema work.

## Actual public emitters

| Tool | Existing success | Structured failure and compatibility lanes |
| --- | --- | --- |
| get_memory | `get_api1::Reply::mcp` returns MemoryRecord, not an envelope; nullable fullText, optional exact labels/provenance, display phase/time strings and u16 edgeCount. | `get_api1::project` emits Failure with error/category/message/retryable/commitState/ready, required observationReceipts and warnings; optional scope/receipt/readGeneration/readiness/operationId/field/receivedEvidence. Missing compatibility responses can remain plain text. Unknown raw evidence is `receivedEvidence.receiptsJson: string[]`, not parsed receipts. |
| store_memory | `store_api1::Reply::mcp` returns StoredMemory, with optional supersedes and nearDuplicates advisories. | Legacy failures remain text. Owned failures have error/message/commitState/ready and optional field/receipt/readiness/operationId/warnings. Explicit unverified evidence is `receivedEvidence.receipt`, a typed receipt with six strict headers and an opaque outcome payload; this permission does not weaken the normal known-receipt schema. |
| store_memories | Legacy loop uses results/total/stored/errors. Durable compatibility uses durable_batch_result; actual Insert+mutationVersion1 uses batch_api1::Reply, including its compatibility results and canonical batch. | Native partial/final-barrier failure retains all item/bookkeeping/reservation evidence. Whole failure removes contractVersion/scope/status from native_batch's envelope; reservation-only evidence adds commitEvidenceScope=reservation. Malformed aggregate keeps unknown status and only unverified received identities/receipts, never trusted aggregate counts. Explicit other mutation versions retain their existing route. |
| forget_memories | Legacy loop uses results/total/deleted/errors. Durable route uses durable_batch_result, complete batch and per-item fields. | Missing IDs can be deleted:false; malformed elements and short result arrays have per-item errors. Legacy PartiallyApplied remains text, not a fabricated zero-deleted structured result. Durable unknown/notAttempted and known child/bookkeeping receipts must retain their original shapes. |

Sources: src/serialization/{get_api1,store_api1,batch_api1,native_single,native_batch}.rs;
src/mcp/tools.rs handle_store_memories, handle_forget_memories and
durable_batch_result; src/storage/batch.rs BatchOutcome/BatchItemOutcome;
src/daemon/api1_get.rs. No schema should be copied from the prepared API2 envelope
and relabeled API1. No schema alters status codes, isError, caller spelling,
default recordAccess, receipt identity, hashes or publication readiness.

## Reachable receipt and numeric boundaries

Get's private decoder checks native versus legacy outcome kind after strict
receipt decoding: LifecycleExposureRecorded versus ExposuresRecorded. Mismatches
fall back to original admitted raw strings. The public schema must keep those
raw strings opaque and explicitly unverified. It must not add known receipt,
operation identity, readGeneration or readiness to an unknown error. Actual
point success still contains no contractVersion/data wrapper.

Insertion receipts normally include NativeMemoryInsertedWithLinks (complete
NativeStoreWrite records/edges/settings/advisory/vectorExecution); durable legacy
and historical paths also require their actual MemoryInserted/NativeMemoryInserted
variants. Forget receipts use MemoryTombstoned/NativeMemoryTombstoned. BatchReserved
and BatchItemRejected are distinct reservation/bookkeeping evidence, never an
atomic whole-batch receipt. BatchOutcome includes reservationState,
reservationReceipt?, bookkeepingReceipts, unknownBookkeepingOperations, items,
total/succeeded/failed/unresolved. Item result is response-only, not immutable
receipt content. Keep unknown and notAttempted distinct from failed.

**Source finding requiring producer review before narrowing:** store_api1::decode
only calls memory_response when Owned reply.error is absent. With error present,
the full typed receipt flows through native_single::response into Failure.
BatchFailure::from_storage likewise preserves typed CommittedMutation evidence
(BatchReserved has special relocation). Fresh success kinds alone therefore do
not prove a narrow failure-kind union. Curie/Feynman were asked to confirm any
earlier validator or actual producer restriction. This is not an executed RED,
not a claim of current-write commitment, and not permission to replace known
receipt outcomes with arbitrary JSON. Resolve actual reachability first; keep
opaque outcome payloads restricted to explicitly UNVERIFIED received evidence.

Schemas describe the complete serialized codec, not authentication or canonical
proof invariants. Reuse the emitter-derived f32 endpoint bounds from196 for both
typed text and Value-first structured forms. Numeric bits and receipt bytes are
unchanged; no tolerances, normalization or global serde changes. Generation
decimal strings keep strict terminal matching. Do not invent restrictions on
compatibility display strings/arrays merely because one native fixture is small.

## Bounded implementation and validation

Scope: actual output registry, a used record-schema module/shared codec definitions
as needed, existing tools/list expectations, focused emitted-output fixtures,
formal validator and docs. Reuse actual owned storage/Remote fixtures; do not
build another generic transport harness or inject schema rows to fake native
writer acceptance.

Select meaningful existing emitter paths rather than a cross-product matrix:

1. Point legacy/native success including Ghost/nullable text and exact labels;
   actual known exposure publication failure; unknown/unverified peer evidence.
   Preserve the existing true/false, no-retry and no-payload-on-failure contracts.
2. Single store legacy/native success with actual advisories and canonical UTF8;
   known full-receipt publication failure and one malformed peer evidence path.
3. Batch insertion/forget compatibility and native projections covering mixed
   results, reservation/bookkeeping evidence, final-barrier known failure and
   unknown/unrepresentable response. Reuse existing genuine histories where
   relevant; do not invent new historical writer evidence.
4. Pair actual tools/list schemas with emitted text and structured values using
   their real serializer boundaries. Formal checks cover required/unknown/nested
   types, null/omission, actual receipt kinds and the raw evidence distinction.
   Rust assertions retain exact receipt/identity/unknown-state evidence.

One source-specific focused Rust run, the relevant formal checks and fmt/Clippy
are the default. No broad full rerun or exhaustive artificial mutation matrix.
Preserve any actual failing source/executable/log and classify before fixing.
Docs-only seals need no test repeat. Exact-head CI and final composition remain
queue gates. Reuse one private cache/target with immutable RO source and proper
package clean/source-specific rebuild when switching source; no large cache clone.

Passing196 raw roots001/004 were released to central cleanup after publication;
failed002/003 remain protected. Do not reuse released paths or edit personal data.

## Confirmed source review before implementation

Curie checked the complete API1 single-store route at frozen df7 (the same
producer source in published195). DaemonClient::call_api1_mutation performs
capability/frame/exchange handling and returns Value; the storage bridge then
calls store_api1::Reply::decode. There is no earlier insert-kind/header/proof
validator for Owned(errorSome). A nominal success failing memory_response also
becomes an error retaining its original typed receipt. WriteReceipt::Deserialize
checks UUID/Generation/known outcome codec shapes, not receiptVersion==1, non-nil
identity or canonical transition validity. Storage validate_receipt is not on
this Remote error path. An insert-only or trusted-header error schema would
therefore assert a restriction the actual emitter does not provide.

Batch errors have a separate existing distinction: remote_failure converts an
unbound receipt-bearing failure to unknown and adds UNVERIFIED_RECEIVED_EVIDENCE,
but native_batch::response still emits the complete typed receipt/reservationReceipt
in their existing top-level fields. This is different from Uncertain(Some),
which emits receivedEvidence with no trusted counts. Neither path may be silently
replaced with the get consumer's raw receiptsJson representation.

This does not invalidate196's lifecycle header bound: native_lifecycle::receipt_matches
explicitly checks non-nil IDs, receiptVersion==1, operation kind/request identity
and supplied scope before retaining a Remote receipt. The store/get/lifecycle
decoders have different actual boundaries; do not extrapolate one to another.

Feynman's exact producer review is pending. Before implementation, resolve
whether the record schema package must describe the complete typed retained-error
codec union or an owned, separately evidenced consumer correction is required.
Opaque arbitrary outcomes remain restricted to explicitly unverified evidence,
not a shortcut for normal known receipts. No executed RED or runtime correction
is claimed by this source review. No extra build or artificial case matrix ran.

## Selected union after A1 review and queue release

Feynman confirmed the full typed carrier reaches Remote error projection without
an insert-only filter; the local wrong-outcome native_single_result also returns
a CommittedMutation preserving that receipt. His source review is recorded at
080662c6. Halley released this schema-only group with195+196 after review. The
chosen implementation is the complete supported typed receipt union in actual
retained error fields, not a protocol/producer overhaul to make the schema small.

The retainedReceipt header has exactly storeId, operationId, committedGeneration,
receiptVersion, outcome and replayed. UUID syntax/Generation encoding come from
the codec; receiptVersion is u32 and nil UUIDs are representable on the generic
typed error path. Do not copy lifecycle's stricter matcher restrictions. Unknown
outcome tags are rejected, and every supported outcome payload has strict typed
fields. Explicit unverified evidence may use its separately documented opaque
outcome schema; normal retained receipts do not.

The exact MutationOutcome union at both parents has these33 variants:
VectorsReembedded, InheritedDecayAdopted, NativeNamespacePurgeStarted,
NativeNamespacePurgeAdvanced, NativeNamespacePolicyUnchanged,
NativeNamespacePolicyUpdated, NativeNamespaceCreated, NativePhaseMaterialized,
NativeMemoryHardDeleted, NativeGraphUnchanged, NativeGraphChanged,
NativeMemoryInsertedWithLinks, NativeMemoryInserted, NativeMemoryTombstoned,
LifecycleExposureRecorded, LifecycleFeedbackApplied, BatchReserved,
BatchItemRejected, LifecycleMaterialized, NamespacePurgeStarted,
NamespacePurgeAdvanced, MemoryTombstoned, MemoryInserted, NamespaceCreated,
NamespacePolicyUpdated, ExposuresRecorded, LegacyFeedbackRecorded,
LifecyclePolicyMigrated, GraphChanged, SchemaMigrated,
NativeTextGenerationSelected, TextGenerationSelected and AccessRecorded.

Use the actual serde field names, including enum payload fields that remain
snake_case without explicit renaming. Reuse typed record/lifecycle/hash/generation
definitions where consumed; add native store/graph/catalog/purge/migration/text/
certification proof shapes from their declared types. This describes possible
carrier output, not a claim that a single store canonically commits every kind.
Existing get/lifecycle known receipt schemas keep their narrower validated kinds.
No runtime narrowing, new dependency or generic schema framework is planned.

Proposal: impl/live-record-schemas at /tmp/recalld-implementation/live-record-schemas,
base195 c4a62065aac6192f8b3b30ca6c3bd63fd48c7416, normal merge of196
10d58c70bc18d936a0e2369e02c8440c4d2dc288. Both old branches remain frozen.
One focused/lint pass after the actual emitter checks; no default full suite.
