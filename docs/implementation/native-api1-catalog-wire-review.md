# N1 catalog wire review boundary

Proposal for Huygens's wire review, not an implemented or validated transport.
Source: actual103/main `b374f07e` + E1 `6ebfd250`; bounded producer
`04406c8b5fbc4fcaa78b527a65815b40e8919f3f`. No R1 parent, public2 activation,
daemon/client edit, or outer MCP dispatcher edit follows from this document.

## Producer and accepted work

The implemented, not yet tested producer signature is
`create_native_namespace_with_provider_bounded(ctx, input, provider, policy,
clock, parent: Option<&WorkTicket>, admission: Option<&ReceiptAdmission>,
check_candidate: impl FnOnce(&WriteReceipt) -> Result<(), StorageError>)`.
The original context reaches hashing and persistence unchanged. API1 will use
unkeyed Catalog scope/original1; the existing v2 entry remains original2.

Exact replay precedes provider lookup and fresh admission. The metadata writer
holds the existing native mutation guard and write transaction while checking
the committed pending-work snapshot. Candidate sizing follows private receipt
construction and precedes namespace file preparation and canonical commit.
The callback is validation only: it never records the candidate as committed.
Clock and operation ID can already be consumed at size refusal.

The actual adapter will keep the existing worker-held publication lease, record
only a returned receipt or one in a known-commit wrapper, release storage/lease,
then call E1 `complete_with_admission` with the same accepted work and collector.
No scheduler, foreign-outbox drain, replay normalization or second collector.
An invalid evidence invariant is not projected as an ordinary small failure.

## Proposed native-only failure carrier

Keep the original `CatalogError`/`StorageError` as the internal cause. A private
serializable projection carries `error`, `commitState`, a fixed `message`, and
the following conditional fields. It does not accept arbitrary source messages.

| Actual evidence | Projection | Full receipt and identity | Readiness |
| --- | --- | --- | --- |
| Fresh candidate refused before preparation/commit | `SERVICE_UNAVAILABLE`, `notCommitted`, fixed operation-too-large message | No receipt or operation ID | Omitted |
| Fresh dependency or ordinary precommit storage refusal | `SERVICE_UNAVAILABLE`, `notCommitted` | No receipt or operation ID | Omitted |
| Known own commit, generic storage/dependency failure afterward | `SERVICE_UNAVAILABLE`, `committed` | Exact in-hand own receipt; operation ID only from that receipt | Omitted |
| Known own commit, actual typed publication failure | `INDEX_NOT_READY`, `committed` | Exact in-hand own receipt | `ready: false` |
| Commit acknowledgement unknown | `COMMIT_OUTCOME_UNKNOWN`, `unknown` | Actual allocated ID; retain any separately known own receipt | Omitted |
| Worker outcome unknown after send/start | `COMMIT_OUTCOME_UNKNOWN`, `unknown` | Null unknown ID unless allocated identity is actually known; retain any separately known own receipt | Omitted |
| Conflicting or out-of-scope internal evidence | Preserve original typed failure; no ordinary success/no-commit fallback | Never relabel foreign evidence as this creation | No readiness inference |

Unknown classification takes precedence through nested typed causes. A receipt
proves its own canonical transition, not the state of subsequent publication.
Whether a source is a publication failure must come from the accepted worker's
typed boundary, not the mere presence of `CommittedMutation` or its error text.
Existing plain legacy mappings remain unchanged. The exact malformed-Remote
projection and bounded preservation of unverified received evidence remain for
joint review; this table does not authorize dropping an acquired full receipt.

Successful API1 responses retain `NamespaceInfo` (MCP/daemon) or
`NamespaceResponse` inside `ApiResponse` (HTTP). Derive namespace fields from
the validated `NativeNamespaceCreated` transition; do not look the name up after
commit. Success has no new receipt, readiness or generation field. API1 is still
unkeyed; oversized keyed replay through the old internal v2 entry is not solved
by this fresh-candidate admission and must not be silently routed through it.

## Actual emitter table and budget handoff

The candidate callback must exercise the following actual representations for
success and every reachable typed failure, using a checked capped writer.
Count the full typed receipt before allocating any corresponding JSON Value.
Only after that bounded first encoding may the existing Value/pretty pipeline
be constructed and counted. `json!(receipt)` is not an interchangeable estimate.

| Route | Actual emitter on the released base | Budget source |
| --- | --- | --- |
| Daemon success | `DaemonResponse::success(id, Value(NamespaceInfo))` | Current connection's negotiated frame limit; maximum numeric ID width is safe |
| Daemon known/unknown error | `DaemonResponse::error(id, -32006, compact_failure_json)` | Same actual peer limit; count the string's additional JSON escaping |
| Local MCP modern | `ToolCallResult::json(info or parsed_failure_value)` with pretty text and structured content; `isError` for failure; then `ResultEnvelope::complete`, then `JsonRpcResponse::success(actual_id, ...)` | Explicit native response policy plus actual request ID and era, supplied before work |
| Local MCP legacy era | Same tool result, but dispatcher removes `structured_content` before the same complete-result/JSON-RPC wrappers | Same request-specific accounting, with the actual legacy shape |
| HTTP API1 success | `ApiResponse<NamespaceResponse>`, `took_us: Some(u64)` | Explicit native response policy; count maximum timing width |
| HTTP API1 native failure | Dedicated typed body retaining the full own receipt and cause classification above | Same explicit native response policy; do not route through a generic error conversion that drops evidence |

Source corroboration: `mcp/tools.rs::handle_create_namespace` parses the compact
PartiallyApplied message into Value and then pretty-prints it through
`ToolCallResult::json`. `mcp/server.rs::handle_tools_call` removes structured
content for legacy era; `McpDispatcher::dispatch` adds the actual
`ResultEnvelope` fields (`resultType`, `_meta`) before the outer JSON-RPC envelope.
The tool-call outcome has no cache hints. Counting only the inner tool result
or only an echoed ID therefore does not prove the emitted modern/legacy frame.

Proposed transport handoff: a private, non-deserializable response-admission
object owns an exact candidate-check closure and the separate E1 receipt
authority. Endpoint constructors supply actual peer capacity or the explicit
local response policy and actual MCP ID/era. The native adapter receives this
object; old legacy calls remain unchanged. An absent native admission object
must refuse before canonical work rather than assume a budget. No request JSON
field may supply or enlarge the authority.

Local MCP currently has no negotiated reply capacity. A native-only local reply
ceiling must therefore be explicitly chosen and documented as an application
resource policy, **not** inferred from daemon8MiB, HTTP request-body limits, or
R1's inner-response bound. The policy value and exact construction signature
remain review questions; no hardcoded local8MiB claim is implemented. A request
whose echoed ID alone exhausts that policy must not start a canonical write.
This does not promise that an arbitrary oversized ID can itself receive a
bounded error response, and does not alter legacy request acceptance.

Only count admitted complete representations; no speculative padding, truncated
receipt, receipt reference, changed public number encoding or automatic new-key
retry. Actual tests must compare the estimator with the emitted complete frames,
including heavily escaped string IDs/profile fields, modern and legacy eras,
known/unknown outcomes, and the exact just-under/over limits. Remote send and
correlation state requires its separate reviewed same-connection handoff.

## Wire review constraints (Huygens, source review)

Review of `cdc2348e` and producer `04406c8b` confirmed the transaction placement
and emitter sequence, not the final carrier, capacity proof or runtime behavior.
No transport-file ownership transfer follows from that review.

Candidate admission must count both the known-committed wrapper and an unknown
outcome carrying the same separately known receipt, including the maximum real
allocated-ID, boolean and fixed-message fields. A typed publication boundary
must establish `INDEX_NOT_READY`; `CommittedMutation` alone does not establish it.
Count the actual failure Value and complete emitter representation, not a
multiplier over typed receipt bytes. The E1 collector limit is independent of
the MCP pretty-text plus structured-content limit. Completion may accept only
the recorded own receipt, never silently collect foreign evidence.

For Remote calls, the upstream MCP request's actual ID and era constraints must
reach downstream precommit admission. Admission must satisfy both the actual
daemon frame capacity and the upstream emitted-response capacity. A downstream
8MiB peer alone cannot establish this bound. The local native reply policy value,
private dispatcher capability signature and malformed-Remote evidence projection
remain unresolved; no transport implementation or serving activation is approved
by this source review. Refusing an oversized echoed ID before canonical work
does not establish that its own error response fits the response policy.

## Concrete local-policy proposal after R2 freeze

Proposal for review, not an implemented limit or a transport ownership release:
use an explicit native-only **8,388,608 encoded-byte** response policy for local
MCP and HTTP catalog creation. This is an application resource policy, not a
negotiated capacity and not inferred from the daemon limit. Legacy creation keeps
its current behavior. Native admission must count every actual success/failure
representation described above against this policy before preparation/commit.

Exact counting boundary: MCP allows at most 8,388,607 bytes of final serialized
`JsonRpcResponse` plus the one LF byte written by the existing stdio transport,
for a total of at most 8,388,608 bytes. This is not a claim about HTTP/SSE or
network framing. The local HTTP catalog policy covers the complete serialized
JSON response body, at most 8,388,608 bytes, excluding HTTP headers and transfer
framing. Neither boundary changes a daemon's negotiated frame-length semantics.

Proposed endpoint capability: `CatalogReplyAdmission`, with private fields and
crate-controlled constructors, carries the response surface and the existing E1
`ReceiptAdmission`. It is not deserializable from public tool arguments. Local
MCP construction requires the actual `JsonRpcId` and `Era`; HTTP construction
selects its actual success/error serializer. The capability exposes only a pure,
capped candidate check and the existing receipt authority to the producer.
No new work registry, generic serialization framework or receipt collector is
introduced. Its candidate check must include unknown outcomes with separately
known evidence, not only ordinary known-commit failures.

The proposed forwarding signatures are an additional handler entry accepting
`Option<CatalogReplyAdmission>` and a bounded namespace-creation entry accepting
the capability. Existing legacy handler/registry methods remain the fallback for
legacy storage only. A native invocation without endpoint admission refuses
before work. Exact public Rust trait visibility and the default method behavior
need review against the released transport composition; these signatures are
not implemented by this document.

An absent capability is never evidence that storage is legacy. Only the actual
owned storage format can select legacy fallback. Default handler/registry paths
must not silently discard required admission and then invoke a native writer.
The actual emitter order is tool result construction, legacy structured-content
removal, conversion to Value, complete-result wrapper and its Value conversion,
then the JSON-RPC response and transport serialization. Source review against
published composition `200a1` confirmed the dispatcher blob is unchanged from
this branch; this source comparison is not a transport-runtime proof or hunk
ownership release.

For MCP, `dispatch` owns the actual ID and must pass it, together with era, to
the catalog-specific admission constructor before `call_tool`. The bounded
serializer applies legacy structured-content removal, `ResultEnvelope::complete`
and `JsonRpcResponse::success` exactly as the final emitter does. The ID alone
can exhaust the policy: refuse canonical work, without promising that an error
echoing an arbitrarily large ID itself fits. Do not reduce or rewrite the ID.

Remote forwarding is a separate, still-unresolved private-protocol handoff. It
must preserve the upstream complete-response constraint and the actual daemon
peer constraint through precommit admission on the same connection, including
malformed-reply evidence expansion. A numerical minimum of two frame capacities
does not alone account for different encodings. Public arguments cannot raise
either bound; an unsupported peer refuses before send, never retries or falls
back after send. No capability spelling, decoder allowance or new private alias
is approved here before Huygens reviews the exact representation.

R2 is frozen at `8b5fae5cf9a6f49cd6c6339aa0f741283c4dabf0` with Halley for
publication. No R2 WIP dependency, client/server/dispatcher edit or additional
build follows from this proposal. Request literal shared-hunk ownership only
against the actual queue-published composition, and coordinate O1 independently.

## Remote source inventory on published 200a1

Exact inspected composition: `200a1f10b9db52d8e5a758cab50c092ca2b07359`.
`RemoteNamespaceAdapter::create_namespace` still uses ordinary
`client.call("create_namespace", ...)` and `from_value`; that path is not a
prepared native evidence contract. `call_api1_mutation` holds the connection
mutex across capability negotiation and the selected operation, but returns a
plain `Result<(bool, Value), BridgeError>` through `call_locked`. Its public
return does not retain the new `TransmissionPhase` and admitted raw result.
Do not classify a post-send parse failure as a fresh no-commit refusal.

The existing `call_prepared` does retain `BeforeSend`, `SentUnattributed` and
`MatchedReply`, and uses the shared request allocator, bounded stale-response
handling and no post-send retry. It currently embeds query capability selection,
query raw-shape admission and the query-specific `response_limit()`. These
cannot simply be reused as catalog capacity or receipt admission. The next
literal proposal must separate those method-specific inputs while preserving
the existing single locked exchange and existing query behavior. This inventory
does not authorize a generic RPC framework or changes to frozen query callers.

Required native catalog classification remains:

- Before-send refusal: no current commit or invented operation ID.
- Sent but uncorrelated, drained oversize, invalid raw shape or lost response:
  current outcome unknown; no retry or speculative receipt acquisition.
- Correlated fully validated native reply: retain the exact declared evidence,
  validate catalog outcome/request identity, and preserve unknown precedence
  when a separately known receipt accompanies a later uncertain operation.
- Admitted but invalid reply containing complete valid receipt fragments:
  preserve them only as received, unverified evidence. They cannot establish
  this operation's commit, readiness, namespace or scope. Never recursively
  search arbitrary nested fields or turn unrelated evidence into a local receipt.

Before implementation, the raw decoder allowance must prove that the entire
received-evidence failure representation fits the actual upstream emitter,
including pretty text, structured content, outer ID/era, stdio LF and any daemon
error-string escaping. Reject unadmitted raw frames before acquiring typed
receipts. Once typed evidence has been acquired, no later size fallback may
discard or replace it. Query's fixed allowance and three-times-input argument
are not asserted for this different catalog emitter. Exact catalog framing and
raw-preflight proof remain open review items, including malformed peers and
historical oversized evidence; there is no implementation or READY claim here.

Huygens's additional source check on the same `200a1` composition: current
`PreparedExchange` retains only `response.result`, not the complete admitted
reply. Its correlated-success predicate requires the error member to be absent.
A matching-ID daemon `-32006` error containing a full receipt in its message
would therefore yield `SentUnattributed` with no retained result if N1 blindly
reused this path. This is a source finding about the proposed reuse, not an
executed catalog RED or a newly established bug in R1's own typed-result protocol.

The catalog protocol must either retain both bounded admitted error/result arms
with exact correlation, or use a reviewed private typed-result failure carrier.
Do not claim the present `PreparedExchange` preserves every receipt-bearing
daemon frame. Preserve before-send serialization failure separately from partial
write, lost response, oversize and malformed reply. No readback, receipt
reconstruction or automatic fallback may upgrade unknown evidence to a known
commit or a known refusal.

## Local consumer hunk freeze for queue review

Source base: `bad91bc09dc3513e0ecf3f4409be5a9d0d1a9a89`, containing
actual parents `b374f07efb2da994d3a82e85d4814e642d2864b2` and
`6ebfd25080bda0d21a755d88211fba471da1e652`. This is a proposed local
implementation boundary, not a runtime or transport validation claim.

Huygens confirmed the local subset does not require the unresolved Remote
protocol. Constructing an ID/era descriptor in the dispatcher MUST NOT reject
the request against the native cap. Only the actual owned-format Native branch
applies the endpoint allowance, before accepting work, provider calls, clock or
ID allocation. The legacy branch preserves its existing behavior, including
large IDs. The old unbounded create entry remains legacy-only, so an old Remote
alias cannot discard the descriptor and reach a native writer.

Proposed exact local hunks and signatures:

- `serialization/catalog_api1.rs` plus its module declaration: one
  `CatalogReplyAdmission` with private fields and crate-only MCP `(id, era)` and
  HTTP constructors, holding the existing E1 `ReceiptAdmission`. It is not
  deserializable request input. Its pure candidate check uses the actual full
  success and failure encodings; no new collector or capacity authority.
- `mcp/server.rs`: add a default handler method
  `call_catalog_tool(arguments: Value, admission: CatalogReplyAdmission) ->
  ToolCallResult` forwarding to the existing `call_tool("create_namespace", ...)`.
  Pass the actual ID into `handle_tools_call`; select this method only for the
  API1 create tool after existing argument/version checks. Default forwarding is
  safe only because the old owned-native entry remains rejected.
- `mcp/bridge.rs`: override that method on `McpBridge`; add
  `NamespaceRegistry::create_namespace_bounded(input, admission)` with the
  existing `Result<NamespaceInfo, BridgeError>` and legacy default forwarding.
  `mcp/tools.rs` changes only the create handler/forwarding, not get or search.
- `mcp/bridge_adapters.rs`: implement the bounded override with actual format
  selection, original legacy call or native adapter dispatch. Preserve the
  existing `PartiallyApplied` full-evidence carrier at the final wire boundary.
- `native_catalog_adapter.rs`: add
  `create_api1_bounded(input: NativeNamespaceInput, admission:
  CatalogReplyAdmission) -> Result<WriteReceipt, CatalogError>` using the same
  accepted worker, worker-held lease and original API1 context. Delegate to the
  already validated bounded producer; keep V2 and policy entry semantics.
  Record only the actual committed receipt. After releasing worker leases, use
  restricted E1 completion; never drain unrelated prior receipts into this reply.
- `api/state.rs` and `api/adapters.rs`: add bounded create forwarding with the
  existing create inputs/result, plus admission. Keep legacy errors unchanged;
  retain native `CatalogError` as a typed boxed error until the HTTP boundary.
  `api/handlers.rs` changes only `create_namespace_v1` response projection to
  preserve native full evidence; no generic `AppError` conversion that drops it.

The final HTTP/MCP success is projected from the committed namespace transition,
not a later catalog read. API1 numeric defaults remain unchanged, and the
original API1 context determines the hash. Endpoint-only refusal precedes work;
the exact full candidate check still runs in the authoritative transaction before
staging. Candidate construction may consume an owned ID on later rejection as
already proved by catalog-002; it must not be described as zero allocation.

Tests for this local slice must exercise real dispatcher and HTTP emitters:
legacy and modern MCP aliases/ID escaping/LF, HTTP body bounds, exact below/above
limits, Value float representation, full known receipt and unknown precedence,
publication failure, waiter cancellation, unrelated pending receipt refusal and
unchanged legacy behavior. Native direct/old Remote unbounded entries remain
rejected. No daemon client/server/capability, O1 get, R2 page, ordinary startup or
public2 activation edits belong to this slice. Halley granted literal local
hunk ownership on this exact base, with Huygens to review the implementation
before proof. Source/hunk and cache/test plans must be frozen before a build.
The E1 x86 stack investigation remains an integration gate. No local heavy
validation is scheduled by this document.

## Historical batch finding disposition

Read-only live remote `main` verification on 2026-11-16 returned
`200a1f10b9db52d8e5a758cab50c092ca2b07359`. Fix
`6cbb2e53d05239c2bc68f34940666243178afbf1` is an ancestor. At this main,
`src/mcp/batch_adapters.rs:468-501` validates the matching `BatchItemRejected`
receipt's store, version, nonzero operation/generation, parent, child index and
error, then retains the full bookkeeping receipt with a terminal, unready item.
The following arm preserves a genuine unknown bookkeeping operation separately.
The diff from that fix changes async forwarding/pinning only;
`src/serialization/native_batch.rs` is unchanged. The historical 9ed finding is
therefore solved for this call chain, not a residual PR181 blocker. This is an
exact-source comparison, not a newly executed regression. No duplicate fix or
build was started; Halley and Huygens received the disposition.

## Local source draft, not validated

The initial local implementation follows the released hunks. Huygens's review
removed `CreateNamespaceRequest` from the API1 entry: accepting only
`NativeNamespaceInput` prevents key/CAS data from reaching this fresh-unkeyed
admission path. Existing V2 replay remains separate and unchanged.

A proposed new publication-error wrapper was removed before validation.
`complete_with_admission` can fail during admission/owner checks before any
publisher runs. N1 therefore preserves the actual `CommittedMutation` receipt
and reports `SERVICE_UNAVAILABLE` without readiness for indistinguishable
completion errors; recursive unknown outcome takes precedence. No message text
or completion call-site inference establishes `INDEX_NOT_READY`. The candidate
estimator also checks that larger shape conservatively, but the new projector
does not emit it without typed stage evidence. E1 core remains unchanged.

The source includes unexecuted tests for real MCP legacy/modern envelopes,
original API1 hash versus API2, old unbounded native alias refusal, actual
catalog fault/unknown reopen, oversized outer-ID refusal with legacy control,
HTTP bodies, exact serialized-byte boundaries and unknown plus known receipt.
A test tail adds a deterministic worker-clock cancellation pause and a real
supplied native insert left awaiting derived acknowledgment before catalog
admission. The latter retains the foreign receipt and uses a seeded owner to
check that refusal consumes no operation ID. A custom candidate test finds the
minimum actual outer-ID envelope allowance, then proves the later receipt
candidate is refused before namespace staging with exactly one clock/operation
draw and no retained receipt/G/counter change. Cancellation also checks the
actual unresolved root/drain rejection and clean volatile tracker after reopen.
All these regressions remain unexecuted. No compile, fmt, runtime GREEN,
full suite or activation is claimed for this source draft.

Proposed validation resource plan after literal patch review: one existing
private target `/tmp/recalld-owned-native-catalog-ids-full-xk182ybj/target`, the
retained shared registry `/tmp/recalld-unit-tests-l__ja1yo/cargo/registry`, no
cache clone or released-root reuse. Use a new owned run directory under
`/tmp/recalld-native-api1-admission-validation`, offline Docker with 2 CPUs,
4 GiB and one Cargo build job. Clean only this agent's recalld package before
switching source roots; capture source and executable identities. Run fmt and
focused `native_catalog_api1`/existing `native_catalog` controls first. Request
the queue's resource slot before creating any container; E1 investigation and
queue validation retain priority. No full run is scheduled by this draft.

Huygens identified one additional source gap in draft `5a8e96ac`: MCP failures
actually cross `Value.to_string -> PartiallyApplied -> from_str<Value>` before
`ToolCallResult::json`. The candidate now models that exact extra parse only for
MCP failure sizing. HTTP retains its different path. New assertions compare the
actual emitted full MCP/HTTP bytes for a 3.7145 receipt against the corresponding
candidate representation, and separately compare decoded canonical f32 receipt
bytes after reopen. This is a source correction and unexecuted regression, not
an observed N1 runtime failure. No global serde change or new transport format.
