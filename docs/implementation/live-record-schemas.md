# Live API1 record output schemas

Modern MCP tools/list now describes the existing object outputs of get_memory,
store_memory, store_memories and forget_memories. This brings object schemas to
11 of 14 tools. Legacy protocol catalogs omit outputSchema as before. Application
version defaults, plain-text compatibility errors, producers, receipts, HTTP/RPC
serialization and public contract2 activation are unchanged.

## Minimal composition and ownership

The actual source parents are PR195 c4a62065aac6192f8b3b30ca6c3bd63fd48c7416
(point-get producer/projection) and PR196 10d58c70bc18d936a0e2369e02c8440c4d2dc288
(lifecycle schema definitions). No catalog/N1 or unrelated consumer parent is
included. Huygens owns this schema/test/docs child; the parent branches remain
frozen under Halley. The coordinator approved this scope under the user's
standing authorization. Feynman reviewed the full retained receipt union against
the actual Rust definitions; this is source review, not executed proof of all
canonical writer variants.

## Existing emitter contracts

| Tool | Object output and failure boundary |
| --- | --- |
| get_memory | MemoryRecord success, nullable fullText, optional exact labels/provenance. Failure retains observationReceipts, optional known receipt, scope/readGeneration/readiness/operationId, and warnings. Unverified receivedEvidence.receiptsJson remains original opaque strings. |
| store_memory | StoredMemory success, optional supersedes and nearDuplicates. Owned failures retain complete typed receipt and actual commitment/readiness fields. Malformed wrapper evidence remains explicitly unverified receivedEvidence.receipt. Legacy errors can remain text. |
| store_memories | Original compatibility results/counts and durable batch evidence, including reservation/bookkeeping/item receipts. Partial/final-barrier failures retain evidence. Reservation-only commitment uses commitEvidenceScope=reservation, not whole-batch success. Uncertain aggregate output omits trusted counts. |
| forget_memories | Original indexed compatibility results/counts plus durable batch evidence when supported. Missing IDs, invalid elements and short legacy result arrays retain their original semantics. Plain-text failures are not replaced with invented structured results. |

Sources are serialization/{get_api1,store_api1,batch_api1,native_single,native_batch}.rs,
mcp/tools.rs and storage/batch.rs. The schemas describe existing emitters; they do
not replace them with prepared API2 envelopes.

## Receipt shape is not commitment proof

The generic single/batch error carrier can retain any supported typed WriteReceipt.
store_api1::Reply::decode calls memory_response only when Owned.error is absent;
a failed nominal-success identity check also retains the received typed receipt.
BatchFailure preserves typed committed errors and relocates BatchReserved into
reservationReceipt. A remote unbound batch failure can retain those fields with
unknown commitment and an unverified warning. These existing locations are not
silently rewritten to match another tool's failure layout.

Accordingly, retainedReceipt describes all 33 actual MutationOutcome variants,
with strict fields using their real serde spellings. Its six headers are storeId,
operationId, committedGeneration, receiptVersion, outcome and replayed. The generic
codec accepts u32 receipt versions and UUID syntax including nil: schema shape
must not assert the stricter canonical validator ran. Unknown kinds are rejected.
Typed payloads, hashes, bit patterns, receipt bytes and original identities remain
complete. This does not claim any single operation canonically commits all kinds.

Explicitly unverified receivedEvidence uses a separate six-header schema with a
known outcome kind and opaque variant payload. It does not validate proof or
current-write attribution. Get's raw receiptsJson strings remain opaque. Known
get exposure kinds and existing lifecycle matcher restrictions remain separate.

Declared extensibility is preserved: NamespaceSchemaV2.extensions is a map of
Value; BatchItemOutcome.result is response-only Value; legacy compatibility merges
its result object fields. These are not strict canonical proof schemas. Finite f32
bounds use the actual typed emitter's endpoint spelling inherited from196, not
widened numeric expectations. Generation strings retain strict terminal matching.

## Validation and limits

The focused tests reuse actual native/legacy storage, public MCP, capable Remote,
known publication failures, unknown replies and malformed evidence regressions.
One additional decoder fixture wraps a genuine separately committed BatchReserved
receipt in an authored single error: it proves retained shape without claiming a
real current single-write commit. A real durable legacy mixed-forget fixture covers
indexed results and full evidence. Stub short-array and shared REST projection
captures are explicitly labeled; they are not native end-to-end writer evidence.

The formal checker validates separately parsed emitted text and structured values,
then meaningful required/unknown/type/receipt-kind and decimal-newline negatives.
Existing exact receipt, history, no-fallback, provider and byte assertions remain.
No exhaustive cross-product matrix or new generic transport harness is introduced.
Affected tests, formal validation and fmt/Clippy are sufficient for this schema-only
child; no new full-suite claim is made. Exact-head CI and final integration remain
Halley's gates. See the accompanying evidence report for frozen sources/results.

Remaining A4 work includes catalog create/delete schemas against their actual final
producer contracts, array-only list_namespaces compatibility, activation prerequisites
and measurement gates. This patch does not claim full A4 completion.
