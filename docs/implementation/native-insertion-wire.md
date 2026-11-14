# Native single and profile-ID batch wire consumer (in progress)

Actual dependencies: sealed native batch wire PR122 `abd0371`, profile consumer
PR123 `d7c4d5f` (including PR118/96), and readiness observation `cc865c2`
(runtime `f8765b2`, direct PR96). Local joins are development compositions,
not inherited validation. Public application contract 2 remains closed.

The prepared single-write consumer includes published PR121
`3fa91d4e542ec8ed5519fe4c3b87e9e665043a40` for
`MetadataStore::capture_namespace_binding`: canonical namespace identity remains
available from retained purge evidence after namespace removal. This is identity
evidence, not fresh policy admission or derived readiness. Replay must precede
fresh namespace/configuration/provider/CAS checks, and a binding-capture failure
must preserve an already known full receipt.

An ancestry check of PR122 `abd0371`, PR123 `d7c4d5f`, observer `cc865c2`, and
PR121 `3fa91d4` found no candidate head contained in another. Their shared PR96
and other ancestors need no additional merge. PR121 is joined directly into
this development composition. Frozen source `d12a9a3` is under validation; its
results must be read separately from earlier focused passes. Curie's `e2b4f94` validation branch (join
`866482da`, reported 1,152 library and two CLI tests) is resolution/reference
evidence only; it is not a shipping parent or validation of this consumer.
The quiet window ended before this join. Its conflict resolutions reuse the
reviewed `866482da` source unions while preserving the additional canonical
vector-snapshot module and exact-scan fixture changes from this branch. The
existing CAS proof, profile-ID batch preparation and observation changes remain;
the actual PR121 native-purge pending-reader checks are retained. Neither the
reference branch's test count nor the prior focused runs validate this new join.

## Activation dependency order

Prepared format-aware adapters and producer dispatch can be validated and shipped
against explicitly owned native fixtures before ordinary native startup is
enabled. Such fixture validation proves the exercised adapter path, not ordinary
startup admission or public API2 readiness. Original API1 request identity and
payload defaults remain independent of disk format and lifecycle version.

Feynman's ordinary native admission then consumes the validated writers and
adapters, including existing public API1 compatibility. It must not depend on
public API2 activation. Huygens' public API2 activation follows that startup work
and the actual A2 retrieval selection consumer, together with the remaining
writer/coherence/schema gates. An API2 envelope around legacy retrieval is not
that selection consumer.

A6 can construct a native instance using API1 once those native API1 paths are
supported, then consume independently versioned capabilities. Neither A6 nor
startup needs an early public API2 advertisement. Only frozen, validated actual
dependencies may be joined; moving implementation branches are not prerequisites
merely because they make a combined checkout convenient.

Huygens owns format-aware routing and legacy response projection for MCP
`store_memory` and HTTP `create_memory`. The planned native branch must select
the owned native format before legacy label conversion, embedding, insertion,
and post-creation callbacks. Its single `NativeMemoryInsertedWithLinks` proof
already includes graph changes; it must bypass the whole legacy post-creation
link sequence rather than invoke legacy edge writers after native insertion.
This API1 route remains unimplemented and requires validated original-context-1
producer support; using context 2 internally is not a compatibility substitute.
Curie owns standalone graph helpers and eventual LoCoMo routing separately.

The narrow record-API1 child on PR121 enables only plain
`NativeMemoryInserted` and forget; it does not enable API1
`NativeMemoryInsertedWithLinks`, native-store replay, or the profile consumer.
The concrete sequence is validated explicit2 wire, then Curie's linked-store
and batch original-API child using the narrow record child (and Feynman's
retained context only where compound operations consume it), then the API1
native projection consumer. Ordinary startup follows that validated API1
closure. Prepared explicit2 wire remains testable on owned fixtures throughout;
none of these steps requires public API2 activation.

Final A4 query coverage must consume Hume's actual V2 result after supplemental
retrieval, including remaining graph/chain budgets and omission diagnostics.
Earlier pipeline-channel coverage alone cannot describe the final result.
Transport exposure accounting must use each actual returned set: HTTP hits and
MCP neighbors are distinct consumer paths, with returned UUID deduplication and
namespace grouping verified at their respective boundaries. This is an explicit
activation prerequisite, not coverage implemented by this insertion consumer.
It does not change the compact full-receipt design or any sealed write evidence.

## Prepared single-write transport

The strict request accepts an immutable `namespaceId`, optional namespace-name
assertion, shared store/key/CAS selectors, and an `embeddingProfileId` paired with
a supplied vector. Original vector bits remain part of retry identity; a copy is
normalized only for validation. HTTP retains its existing `createdAt` integer
millisecond and positive `initialStability` controls. MCP does not silently accept
those HTTP-only fields, internal observation times, or public profile descriptors.

The actual owned adapter resolves matching retained history before current
namespace policy, generation CAS, graph settings, provider identity or clock.
Published PR121 supplies the live or retired namespace binding; recreating the
same name cannot redirect an ID-scoped replay. Fresh admission conservatively
bounds the full typed receipt from the frozen graph settings and actual native
edge cap before provider work or allocation. It does not lower link settings or
change request hashes to fit a response. Retained replay ignores changed fresh
settings and returns the original full receipt.

The response uses the existing single-memory fields inside the explicit2
envelope. MCP emits compact typed JSON plus equal structured content; REST keeps
typed JSON, and the dedicated non-retrying daemon method is
`store_native_single_observed_v1`. A missing method or lost reply never falls
back to a legacy write. Prepared adapters are exercised directly on owned native
fixtures while the public contract2 gate remains closed. Default API1 routing,
payload shapes, and pretty formatting remain unchanged in this slice.

After the known root finishes, success requires the actual owner's current-G
barrier. Publication failure keeps the full receipt and observes readiness without
draining unresolved work. Missing private reply identity or data fails closed
without erasing an in-hand receipt, warnings, or readiness. Unknown outcomes do
not invent a receipt. Scope/profile conflicts are409, absent namespace404;
malformed vector shape is400 and semantic norm/length rejection422.

The genuine BEFORE123 replay fixture uses an HTTP-only creation timestamp. Its
HTTP and daemon paths execute the prepared consumers; its MCP check exercises
the real full-reply serializer, not a fabricated MCP acceptance of that field.
A separate ordinary-fields test executes actual MCP dispatch, HTTP and framed
RPC. Neither fixture proves ordinary native startup or that every valid
historical receipt fits a transport frame. No receipt-reference exception is
introduced.

## Failure readiness clarification

CONTRACTS' integer `pendingWork` is not a deduplicated aggregate promise. An
observational failure response always carries `pendingWorkScope: "acceptedTickets"`
and `observedWork` alongside `pendingWork = accepted_work`. `observedWork` includes
`unresolvedWork`, `admissionOpen`, `pendingEffects`, `pendingPurges`, and
`pendingBatchItems`. Unresolved tickets are already included in accepted tickets;
durable categories can overlap tickets and each other. None of these are summed.
Both observation fields are required together in this variant. The fields are
absent in an ordinary successful barrier response, whose existing shape and
proven zero pending count remain unchanged.

An observation always reports `ready: false`, even when every observed stamp
matches and every observed count is zero. It does not close admission or drain
its own accepted root. `derived[].appliedGeneration` is a decimal string when
known, and explicitly `null` when unpublished. Unknown stamps are never zero.
The current observed canonical generation does not replace the operation's
original receipt generation or an earlier query read generation.

If observation fails, readiness is absent and the warning code is
`READINESS_UNKNOWN`. The original complete receipt, operation ID, commit state,
and original publication error remain intact. A failed observation is not a
rollback or permission to retry with a new key. This clarification is local to
the actual failure consumer; authoritative research specs remain read-only.

## Receipt framing evidence and limits

An isolated test-only producer appended to unchanged PR123 `d7c4d5f` produced
and reopened a genuine native single insertion with 1,024 edges and 1,025 record
proofs, using zero provider calls. The compact typed receipt was 3,220,502 bytes.
The old pretty MCP result was 12,305,907 bytes; compact text plus structured
content and JSON-RPC framing was 6,648,160 bytes, below 8,388,608 bytes.
This fixture needs its full receipt, not a receipt reference. API1 pretty
serialization remains unchanged. These numbers characterize that fixture,
not all valid histories.

A separate synthetic typed serializer stress test measured 4,110,022 receipt
bytes and 8,483,341 MCP bytes. Repeated IDs and invented maximum hash commitments
make it invalid as canonical native state. It is not a reachability proof and
does not authorize a new historical oversize receipt-reference shape. Fresh
worst-case admission, actual bounded serialization, and final transport tests
remain part of this consumer's unfinished work.

## Profile-ID batch preparation

An optional profile ID belongs to an ephemeral plan, never a new manifest
format. Historical item hashes, including unknown-field/invalid fallback hashes,
are computed before interpreting the new selector. Matching historical outcomes
therefore replay before fresh namespace, settings, provider, or CAS checks.
Fresh resolution uses the canonical Bound descriptor captured inside the
reservation transaction. Pending children resolve only their retained defaults.
Caller vector bits are not normalized into request identity. Generated None and
existing internal descriptor-input paths retain their old hash meanings.

Profile mismatch uses the existing persisted `CONFLICT` diagnostic with field
`embeddingProfileId`; only that exact typed pair maps to the public
`EMBEDDING_PROFILE_MISMATCH` code. No persisted diagnostic vocabulary is widened.
Original API1 context propagation is a separately owned dependent core change;
this consumer continues using explicit API2 without coercing API1 contexts.

## Actual batch publication consumer

The native wire batch reply wraps the existing canonical outcome; manifests and
core batch output types do not acquire transport readiness fields. A completed
per-item effects result is only a candidate success. After the runner finishes
its known accepted root, the actual owner's barrier checks current canonical
lineage/generation and required publications. Barrier failure preserves every
committed item/receipt and changes readiness to false, producing HTTP207 and MCP
isError instead of a stale HTTP200. Unknown or failed work is observed without
attempting to drain that caller's unresolved root.

The remote adapter uses the dedicated non-retrying
`mutate_native_batch_observed_v1` alias. Older servers reject it rather than
silently omitting the required observer/profile-ID behavior. The server retains
the prior alias for compatibility; the new client never falls back after a
mutation. Public API1 response types remain unchanged.
