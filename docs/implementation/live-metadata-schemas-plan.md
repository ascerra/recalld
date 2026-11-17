# A4 discovery and remaining operation consumers

Source/ownership proposal, 2026-11-17. Inspection pin:
`6f064789473eea310be28e004de38fd52d2aa3d5`.
Huygens owns wire review; Halley releases exact implementation hunks.
The plan was reviewed locally at720ea344/55f9f4ea. Halley released the D1 hunks
for sole Huygens implementation in a new worktree based exactly on this main pin.
Only this plan was copied; no research-descendant history was imported. Validation
and activation claims require their separate evidence.

## Observed boundaries

- `mcp/protocol.rs::ToolInfo` has inputSchema and annotations but no outputSchema.
  `ToolCallResult::json` preserves JSON text and adds structuredContent only for
  objects. In particular, v1 list_namespaces remains an array, not a wrapped object.
- `tools::tool_definitions` advertises only application version1 on all14 tools.
  `dispatch_tool` and `server::handle_tools_call` independently reject version2
  before dispatch. MCP transport era is a different selector.
- `server::handle_tools_list` currently lacks request context and returns a public
  cacheable static catalog. `handle_tools_call` strips structuredContent in the
  repository's legacy era. Adding schemas must preserve that negotiated boundary,
  not change old clients or advertise prepared2 as available.
- Existing query/page schemas validate real prepared consumers, but are explicitly
  not live capability advertising. They cannot simply be installed as the schemas
  for v1 tools with different success shapes.
- `serialization/session.rs::document` is shared by recalld://session and REST
  /capabilities. It preserves [1], frozen default, actual optional backend identity,
  limits and sampled-scan incompleteness. MCP resolution uses source `bridge`;
  HTTP truthfully reports its current legacy literal default. Neither is proof of
  readiness, a coherent read generation, profile attestation, or native startup.
- SERVER_INSTRUCTIONS already states v1-only, non-exhaustive legacy scans,
  namespaces-not-access-controls and no dimension-based profile inference. The
  remaining guide must not promise public keyed2 retry/cursor behavior early.

## Proposed next executable D1 slice

Use the exact queue-released main containing this pin. No Curie O1 or Feynman N1
parent is needed for these live v1 metadata schema consumers.

1. Add optional output_schema to ToolInfo, omitted when absent. Start with actual
   modern `namespace_stats` and `list_tags` object successes, whose serializers and
   v1 handlers already exist. Attach schemas to those ToolInfo entries and validate
   real outputs. Do not replace array/prose successes or invent object wrappers.
   Ordinary text-only errors remain text-only; schema describes structured output.
2. Pass existing RequestContext to tools/list and omit outputSchema on the legacy
   era, matching existing structuredContent stripping. Keep catalog caching static;
   never put namespace/profile/store context into this publicly cacheable catalog.
3. Extend the existing session document only with factual v1 guidance shared by
   MCP/REST: access default, by-ID scope rule, legacy scan semantics and native
   supplied-vector profile restriction/remediation. Explicitly distinguish API
   version from disk/lifecycle version. Do not add capability booleans inferred
   from local provider width, private aliases, or another adapter's support.
4. Keep the short stable instructions under150 words. Use session context for
   resolved values. Preserve current defaults and supportedContractVersions [1].

Proposed files: protocol.rs ToolInfo and instruction constant; tools.rs registry
schema attachment only; server.rs tools/list context/era projection only;
new narrowly used mcp/tool_schema.rs; serialization/session.rs and its actual
resource/REST tests. Necessary ToolInfo test-literal initialization is mechanical.
No changes to prepared_get, daemon negotiation/decoder, catalog admission,
namespace writers, query kernels, or public version gate.

Before implementation, snapshot actual v1 success fields from the pinned handlers
and settle the exact additive session field names with the coordinator. Formal
schema checks must exercise the actual published tools/list schema, not a separate
test-only copy. Actual modern/legacy MCP and REST metadata positives, two frozen
default contexts, private/no-store session caching, and explicit2 prework rejection
are required. Unknown field/type/count mutations must fail the structured schema
where it makes a claim. Schema string lengths are characters, not a false UTF8-byte
validator. Existing parsers remain authority for byte/cross-field restrictions.

This deliberately closes only two live schema consumers. The other tools and the
eventual v1/v2 schema unions remain explicit D1 work; do not label this full D1.
No standalone unused-schema PR, schema-only global framework, or automatic2 union.

## O1 non-get operation matrix at the pin

| Operation | Actual prepared/public boundary | Next actual consumer or owner |
| --- | --- | --- |
| recall, single similarity, exact pairs | Prepared MCP/REST plus actual Remote R1; public2 closed | Preserve selection/coverage/bits and final177 certification; no duplicate RPC implementation. |
| metadata list, namespace/tag catalogs | Prepared MCP/REST plus actual Remote R2 | Existing page schemas are proof inputs; wire into final activated discovery only with the real gate. |
| get | No prepared get arm at this pin; ordinary MCP storage.get_memory differs from HTTP coherent route | Curie owns O1 core/RPC and typed carrier. Ordinary API1 caller projection remains a separate explicit closure. No overlap. |
| single store and insert batch | Prepared wrappers and Remote observed aliases exist; native_batch::parse constructs Insert only | Existing producer/CAS/known-receipt matrices must be included in activation; do not infer all legacy2/native1 support from a method name. |
| feedback / single forget | Prepared lifecycle wrappers and private methods exist; ordinary API1 adapters present | Pascal lifecycle ownership; reuse exact typed failures and original1/2 proof, no new fallback. |
| forget_memories | v1 durable route exists; prepared dispatch_v2 has no arm and native_batch::parse is Insert-only | Proposed later Huygens/Curie consumer: real BatchKind::Forget parsing, original2 reservation/CAS/replay forwarding, existing partial outcome/full receipt owner. First source/ownership plan and actual unsupported-route RED; no new codec by default. |
| create_namespace / update_namespace_policy | Local prepared arms exist; update policy is not in the live14-tool registry. RemoteNamespaceAdapter has no native create/policy overrides | Feynman N1 owner; actual local189 admission and reviewed Remote catalog dependency first. Policy private2 forwarding is distinct from API1 create negotiation. No convenience get parent. |
| delete_namespace | v1 MCP purge route and typed evidence exist; prepared dispatch_v2 falls to unsupported. No existing REST namespace-delete route | Feynman sole purge owner plus Huygens wire. Consume actual original2/keyed purge semantics and full evidence; don't fabricate metrics or add an unsolicited REST route. |
| namespace_stats | Existing v1 MCP/REST/Remote values; no prepared2 dispatch arm | Later actual same-generation stats selection/envelope with storage owner. Never put a freshly sampled G around independent legacy counts. Live v1 schema above adds no coherence claim. |

For each missing operation, add an actual prepared consumer and tests before
capability advertising. Keep unsupported explicit, not a permanent completion
claim. Schemas must cover actual typed errors, committed/unknown evidence and
partial batches; foreign receivedEvidence remains separately unverified.

## Activation and dependency constraints

V0 strict selectors/32 historical probes, D1 complete schemas/discovery, O1 real
operation consumers, N1 ordinary native startup/API1 compatibility, E1 typed
maintenance failures, V1 full transport conformance, and P1 measured overhead
remain separate acceptance groups. R1/R2 completion does not close these groups.
No A5 latency claim follows from correctness/frame diagnostics.

Public2 activation must consume actual validated startup/writers/A2/coherence and
per-operation transport support, including legacy-disk API2 where required. The
ordinary startup must not depend on public2 activation. Prepared owned fixtures
can prove callers first. Keep MCP era gating, version schemas, HTTP selectors,
capabilities and default scopes consistent in the final actual composition.

D1 ownership/hunks were released by Halley; other operation plans remain pending. Curie's active get child and Feynman's
catalog plan remain untouched. No tests, builds, cache copies or cleanup were run
for this source audit; no personal recalld access. Existing failed fixtures and
shared caches retain their prior ownership/protection. No recovered-byte claim.

## Exact first-slice schema and session proposal

Pinned v1 namespace_stats emits exactly: name:string; memoryCount,
tombstoneCount, permastoreCount, edgeCount, vectorBytes:integer u64; avgStrength:
number; phaseCounts:{full,summary,ghost:integer u64}. Every field is emitted,
including tombstoneCount when an old daemon omitted it and the current typed
adapter defaults it to zero. JSON Schema uses integer minimum0/maximum u64::MAX;
no decimal-string conversion, invented average range, or new generation field.
Root and phaseCounts reject extra fields in the output schema. Cross-field
memoryCount equality is tested against actual outputs, not falsely expressed by
a schema which cannot sum fields.

Pinned v1 list_tags emits exactly: namespace:string|null,
memoriesCounted:integer u64|null, and tags/entities/topics/emotions buckets.
Each bucket has total:integer u64, truncated:boolean, items:[{name:string,
count:integer u64}]. All keys are emitted. Bound item arrays by MAX_TAG_LIMIT500;
do not invent a limit on total vocabulary, counts, or display-label string length.
Root/buckets/items reject extra fields. Validate global/null and scoped values
using the actual producer, including capped buckets with untruncated totals;
do not turn descriptions into unsupported cross-field schema assertions.

Proposed exact additive session fields (same shared document for MCP/REST):

```json
{
  "defaultNamespace": {"byIdResolution": "record"},
  "accessDefaults": {"recall": true, "get": true, "singleSimilarity": true},
  "versionPolicy": {
    "omittedApplicationContract": 1,
    "independentOfDiskAndLifecycle": true
  },
  "embeddingInputs": {
    "operation": "api1MemoryStore",
    "nativeRestSupplied": "explicitMatchingBoundProfileId",
    "legacyRestWithoutProfileId": "unverified",
    "legacyRestWithProfileId": "rejected",
    "mcpV1SingleEmbedding": "ignored",
    "nativeRemediation": "Use generated embeddings or the namespace's exact Bound embeddingProfileId.",
    "legacyRemediation": "Omit embeddingProfileId on legacy storage; supplied vectors remain unverified."
  }
}
```

The defaultNamespace snippet extends its current object; it does not replace
requestedName/source/exists/resolved. These are conditional compatibility rules,
not a claim that ordinary native startup is enabled. Existing [1], all limits,
duplicateScan, backendContext/unavailable and configured/frozen scope values stay
unchanged. Backend identity remains configuration identity, never attestation.
No new support/readiness booleans or guessed per-adapter negotiation registry.
Receipt-retention/version-name/action-matrix completion remains a later explicit
D1 obligation; this narrow slice does not announce unimplemented public writes.

Instructions need only add a short direction to read recalld://session first if
the under150-word check permits; do not rewrite already-correct guidance. Actual
live output schemas are attached only to these two tools. All other outputSchema
fields remain omitted pending their real variant/structured-result coverage.

Literal implementation ownership requested: protocol ToolInfo/instructions;
tools registry/schema attachment (no dispatcher/handler changes); server
tools/list RequestContext/legacy omission; new mcp/tool_schema; shared session
document and actual metadata/discovery/REST tests; mechanical ToolInfo literals
only elsewhere. No Curie private client/server/get/decoder hunks or Feynman
catalog-admission hunks. Proposed new branch/worktree must start at exact6f064789,
copy only the reviewed new plan delta, and never import this local branch history.

The embeddingInputs guidance applies to API1 memory store (single/batch), not
all REST operations. Native and legacy remediation are intentionally separate;
legacy storage cannot verify an explicit profile selector. No serving capability
is implied by these conditional rules.
