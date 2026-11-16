# Native insertion recipe and original API identity

This slice consumes actual insertion wire141, native plain record API1 consumer131,
and common retained-context consumer134. Private native writer formats remain2
(plain insert/forget) and3 (same-insert links/advisory). They are not public API
versions. Public routing and outward API1 compatibility are a separate consumer.

## Actual interfaces

- Linked single callers pass their original MutationContext1 or2 to
  `store_native_single_by_profile_id` (or the existing prepared-descriptor seam).
  The actual native owner/namespace/profile guards select capability. Same-key
  cross-contract calls conflict; replay uses retained expansion and precedes
  current profile, policy, provider, clock and payload staging.
- Internal batch consumers set `BatchInput.request_context` with the existing
  `RetainedRequestContext::from_contract(original_contract)` and select the actual
  native writer codec separately. The current public API2 parser continues to
  emit None. No new public requestContext field or API3 is advertised.
- The runner calls the version-explicit reservation transaction. The retained
  parent, caller/resolution hash, final rejection receipt and child canonical
  hash use the original API. The private BatchChild copies the parent's validated
  context, and final `load_child` compares it again before commit. Operation IDs
  remain the already reserved IDs. Parent CAS remains reservation-only, using the
  existing reservation_precondition proof, never the current/fork generation.
- Historical native2/3 context absence means API2. New native/API1 carries the
  shared closed proof. Legacy writer1 absence means API1 and rejects an attached
  native proof. This does not implement legacy-disk API2 insertion or batches.

## Storage and offline compatibility

No new table, journal, payload receipt or public version is introduced. Existing
strict BatchProgress decoding preserves the additive context. Existing startup,
logical export/import and backup/restore validation all call the same manifest
validator. It recomputes original-context caller/resolution/reservation and
bookkeeping hashes; the canonical parent request hash anchors the optional
context even before the first valid child. Explicit null, unknown proof/API
values, redundant Some2 and context on legacy format1 reject. Older readers with
the existing deny_unknown_fields reject the new field by construction; this
slice's execution proof covers genuine old bytes in the new reader, not a claim
of rerunning the old executable on new state.

Matching keyed or operation-ID resume uses the original full payload and frozen
namespace/default/profile/settings resolution. Changed payload or origin conflicts;
missing/foreign parent cannot create a fresh reservation. Unfinished batches still
block export/fork as before. Complete API1 histories survive export/import and
retained restore, with original receipt bytes; fork lineage rejects old-store
replay. Legacy format1 and existing native2 bytes/hashes remain unchanged.

## Generated documents and numeric failures

One `NativeInsert::document_v1` helper composes summary, optional full text and
ordered merged tags. All four fresh native generated paths use it after replay.
Generated outputs do not enter request identity. Original supplied float bits do;
normalization still works on a separate vector. Caller numeric errors carry the
specific CallerVectorInvalid type; the actual single wire maps it to422
OUT_OF_RANGE /embedding. Invalid provider output stays unavailable503, while
canonical corruption stays a storage failure. Generic InvalidMutation is not
blanket-mapped to caller422.

Older experimental native generated vectors are not certified for the declared
recipe or retrieval quality. No re-embedding, history rewrite or silent provenance
blessing is performed. The validated recipe companion is required before native
activation. See [the report](../evidence/native-document-recipe/REPORT.md).

## Scope boundary

The implementation changes real producers and the canonical transaction, not
public activation. Huygens owns actual API1 routing/projection consumption; A6
native ingestion remains a later validated consumer. Ordinary native startup,
all-writer participation, legacy-disk API2 insertion/batches, full-core barriers
and whole-store CAS are not claimed. Accepted work, graph-to-storage lock order,
same-insert durable links, publication completion and full error receipts remain
owned by the existing coordinators; no second registry or postcommit rediscovery
was introduced.

The remaining legacy-disk API2 compound work is explicitly separate. Existing
legacy format1 can carry a caller CAS without the native format3 retained
reservation_precondition. Before widening it, the batch and shared-codec owners
must audit the actual legacy request/child/allocation hash inputs, define an
immutable parent anchor for original API/CAS before any child, and execute genuine
old1 None/Some compatibility plus fork/stripping negatives. Native context
absence and the native purge compatibility algorithm are not evidence for this
legacy batch boundary. No new shared-codec change is included here.
