# Remaining live MCP schema consumers

Source-only plan, 2026-11-17. Pin: published PR190
`295fdac03b6bdf93d59c2dd68876c18393b5086f` (runtime16b, validated13ff).
PR190 is frozen under Halley's ownership. Only this reviewed plan was copied from the local planning worktree; no
research-descendant history is included. Halley explicitly released sole
implementation to Huygens on impl/live-read-schemas at this exact parent,
limited to the three schemas, disjoint conformance tests and guidance below.
Hume source-approved the sameNamespace wording and identified the existing
actual compatible-profile regression. This is not a full-A4 completion claim.

## Actual live output matrix

Every tool still advertises application contract1. Prepared2 methods and schemas
do not authorize advertising their shapes as live v1 output. Modern object
results receive structuredContent through ToolCallResult; legacy projection
strips it. Text-only validation/error results have no structured object to
validate. A tool which also emits a structured failure needs that real union,
not a success-only schema falsely presented as its complete output contract.

| Live tool | Actual v1 producer/structured shape | Required schema/consumer work |
| --- | --- | --- |
| namespace_stats | bridge::NamespaceStats, required numeric counts and phaseCounts | PR190 attached and validated modern schema; keep legacy omission. |
| list_tags | bridge::ListTagsResponse, four bounded buckets, nullable global namespace/count | PR190 attached and validated known truncation/total shapes. |
| list_memories | bridge::ListMemoriesResponse {memories:[ListMemoryEntry],total,offset,limit} | Next read group: actual bounded page with optional entities/topics/labels; no fullText or fabricated G/cursor. Ordinary failures remain text-only. |
| recall_memories | tools handler compact=true by default: {memories:[id,summary,optional fullText/entities/topics/labels],count}; compact=false emits SearchHit plus optional graphContext {neighbors:[NeighborMemory],neighborCount} | Next read group: explicit compact/full alternatives, actual optional omissions, nested related edges/neighbors and canonical-label provenance. Existing failures are text-only; do not invent v2 coverage/explanations. |
| find_similar_memories | single {sourceId,memories:[SearchHit],count}; scan {namespace,threshold,clusters:[DuplicateCluster],clusterCount,coverage} | Next read group: union of actual modes. DuplicateEntry contains id/summary, not a per-entry score. Coverage has numeric known counts or explicit null unknown counts, complete:false in both. No generation-pinned/exhaustive claim. |
| get_memory | bridge::MemoryRecord; fullText explicitly nullable, labels optional, u16 edgeCount, formatted timestamps; missing/errors text-only at this pin | Curie owns ordinary API1 native point consumer. Freeze its actual success/error projection before attaching a complete schema; do not pull O1 merely to describe the current legacy shape. New typed failure must not emit prepared contractVersion2 on API1. |
| store_memory | serialization/store_api1::Reply: StoredMemory success, optional SupersedesOutcome/NearDuplicateReport; Legacy errors text-only; Owned/Uncertain use typed API1 Failure | Actual success/failure union with original commitment, full receipt, unverified evidence, readiness/warnings. A generic arbitrary outcome object is not a strict known-receipt schema. Requires real legacy/native/capable/old-peer specimens. |
| store_memories | legacy object {results,total,stored,errors}; durable/native API1 also emits batch state and compatibility results through batch_api1; partial results can be isError:true | Enumerate actual ordinary/durable/native variants, known reservation vs child outcomes, all partial statuses and full receipts. Selector-triggered durable routes exist even when an adapter lacks the ordinary durable capability. Preserve original hashes/proofs. |
| reinforce_memory | lifecycle_api1::Reply::mcp: ReinforceResult success, or structured native lifecycle Reply failure (also used for typed legacy failures) | Actual lifecycle success/failure union, including original1 receipt and genuine unknown ID. Pascal owns lifecycle semantics; no public2 envelope coercion. |
| forget_memory | lifecycle_api1::Reply::mcp: {id,deleted}; structured lifecycle failures; unscoped missing may project deleted:false | Preserve asserted-scope missing distinction and ID spelling. Full native/legacy receipt/unknown/error variants need actual emitted fixtures. |
| forget_memories | simple {results,total,deleted,errors}; durable_batch_result adds batch and per-item status/ready/receipt/errorDetail, including unresolved states | Actual v1 union first; prepared2 consumer/CAS is separate. Existing native Forget version2 cannot accept expectedGeneration; Insert-only3 is not a substitute. Coordinate Curie/Feynman rather than silently dropping CAS. |
| create_namespace | NamespaceInfo success; PartiallyApplied JSON object is emitted structured/isError; other errors text-only | Legacy known-create receipt/unknown worker variants at this pin. Feynman's N1/Remote catalog consumer may add real variants; consume it only when used, with exact serializer/frame proof. |
| delete_namespace | DeleteNamespaceResult success; validated native_purge_api1::structured_failure object for known/unknown progress; old errors text-only | Full start/latest/final evidence distinctions, metrics-unavailable race, operation-too-large refusal and real receipt shapes. No invented physical counts or readiness, no new REST route. Feynman owns runtime. |
| list_namespaces | Vec<NamespaceInfo> serialized as bare JSON array text; ToolCallResult::json deliberately omits structuredContent for arrays | Preserve v1 array. Do not wrap it or advertise an object outputSchema. Final versioned catalog consumer can expose its own actual object schema only upon activation. |

Relevant sources: src/mcp/tools.rs handlers and durable_batch_result;
src/mcp/bridge.rs DTOs; src/serialization/{store_api1,batch_api1,lifecycle_api1}.rs;
src/mcp/native_purge_api1.rs and existing catalog error projection. Full typed
receipt decoding and cross-field identity validation remain runtime obligations,
not promises that JSON Schema can compare unrelated fields or establish lineage.

## First coherent implementation group

Proposed sole parent is PR190's exact295 head, or a subsequently handed-off main
which contains it. This is a real dependency on ToolInfo.output_schema, era
projection and the live registry hook. No O1/get, N1/catalog, batch or writer
convenience parent. The implementation should attach and validate **three**
additional live schemas together: list_memories, recall_memories and both modes
of find_similar_memories. It is a real read-output consumer group, not an unused
schema library or a plan-only PR.

Expected hunks: src/mcp/tool_schema.rs; actual schema/transport tests and formal
runner; the one inaccurate similar-tool description below; compact report/docs.
Use existing strict labels shape from prepared query/page schemas where sensible;
any extracted helper must have real current users and preserve those schemas.
No new schema dependency/framework, handler execution change or global serde
configuration. Do not edit the frozen190 branch.

Schema details to pin before implementation:

- Required fields follow actual emitters, not Option<T> guesses. ListMemoryEntry
  emits id/summary/createdAt; entities/topics omit empty arrays; labels omits None.
  A present StructuredLabels object always has entities/topics/emotions arrays
  and provenance exactly native_v2 or legacy_retained_only. Some(empty) remains
  present and authoritative; schema work must not change label selection.
- Preserve historical numeric counters (no decimal-string conversion). Bound
  list page arrays by actual MAX_LIST_LIMIT; do not invent limits on total counts,
  label lengths, graphContext or duplicate cluster size from unrelated v2 caps.
  Strings count Unicode characters in JSON Schema, not UTF8 bytes. Float score
  ranges require a producer invariant, not merely a comment saying 0..1.
- Compact recall and full recall can overlap on an empty result. Use a union
  that allows their real overlap; avoid oneOf which rejects a value satisfying
  both valid alternatives. Nested objects remain strict within each shape.
- Scan known coverage comes from DuplicateScanCoverage. Old/custom bridge None
  produces null sampledCount/eligibleCount/skippedVectorCount/truncated and
  countsAvailable:false. Both report sampledNearestNeighbors, complete:false,
  the actual200 sample cap and11 candidate cap. Schema/count coherence has to be
  tested against both actual producer and explicitly labeled old-peer fixture.
  This is not the exact-pair cursor's contract or its 201+/dense oracle proof.
- Tool defaults and version selectors remain unchanged: recall compact defaults
  true, similar mode defaults single and access defaults true. Scan is read-only;
  the mixed tool's annotation cannot become read-only merely because one mode is.

Concrete description discrepancy at295: sameNamespace says single mode always
uses the source namespace, even when false. McpSearchAdapter::find_similar at
bridge_adapters.rs717 instead selects RetrievalSemantics::V1 and
SimilarityScope::AllCompatible when false. Correct that description only after
Hume source ACK and an actual compatible-profile control. Do not infer profile
compatibility from equal width or Unknown provenance, change v1 ranking, or
rename this v1 behavior as v2. Scan still ignores this legacy selector.

## Acceptance and ownership

1. Obtain Halley's literal new-child/hunk release and Hume's scope-description
   ACK. Existing Curie API1 get and Feynman catalog plans remain disjoint.
2. Exercise actual tools/list plus tools/call on real storage/search adapters,
   not a schema-only golden. Cover modern and legacy eras, empty/nonempty pages,
   offsets/caps and terminal exclusion; compact/full recall with and without
   graph context; single/scan similarity, known/unknown scan coverage and ties.
   Use false access for canonical immutability controls; any true control must
   assert real exactly-once observation. No provider invocation for list-only.
3. Canonical Unicode/provenance fields need either a genuine native writer-owned
   fixture with its real admission or explicitly labeled serializer specimens.
   Never inject schema rows into an open legacy store and call that native serving
   proof. Include absent labels, authoritative empty and both provenances without
   upgrading legacy-retained labels. Preserve the v1 array-only namespace listing.
4. Formal tests consume the actual advertised schemas and emitted values. Reject
   missing/unknown nested fields, wrong/null types, negative/fractional/bool
   counts, overflow, unknown provenance and impossible coverage variants. Keep
   actual compact/full empty overlap positive. Do not normalize receipt bytes,
   substitute tolerance, or declare an opaque known receipt fully validated.
5. Source-specific offline focus then one appropriate frozen full/lint pass;
   exact source/executable provenance and compact report. Existing190 results
   are its own proof. No duplicate target clone or new heavy job for this plan.

The other eight structured-output tools need actual write/error unions and owner
coordination; list_namespaces remains deliberately array text on v1. Strict
selectors/32 historical validation probes, all public action matrices, v2
discovery/activation, complete transport budgets (including outer RPC IDs),
ordinary native admission and A5 measurement remain separate acceptance gates.
This read group would bring live object schemas to5tools, not finish D1/A4.

## V0 attribution carried forward

Halley's source map at2925 (`docs/implementation/merge-queue/V0-VALIDATION-CASE-MAP.md`)
accounts for all32 original research cases:17quality and15dimension, covered by
six actual MCP handler/storage tests. Two real HTTP counterparts also cover JSON
3.0/4.0. The original defects are not unimplemented. All eight named tests had
emitted ok during the still-running2925 full run when the map was written; that
is not its final full-suite result or a new standalone execution by Huygens.

Those helpers compare the affected record/history or absence of the named
namespace, not every canonical table/index/work queue. Public9 adds durable-file
checks for selected malformed inputs, not all32cases. Remaining V0 work is exact
transport coverage plus no-queued-work/canonical-state proof (or reviewed
equivalent), not reimplementing the quality/dimension fixes. This source map
adds no runtime parent to the proposed read-schema child.
