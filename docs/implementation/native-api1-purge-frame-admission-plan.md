# Native API1 purge: frame admission alternatives

Status: source plan for review, no runtime change. Source is candidate006
4853c6e5 on actual PR176 parent 1f5d083d8142dd631f16f95617d23fddc85751ad;
compact failed-run evidence is committed in 5df5a4cc. The implementation remains
NOT READY. Independent integration queue work is unaffected.

## Actual boundary

Candidate006: 11 focused PASS, one FAIL, 33.62 seconds. The actual 1000-record
CHUNK_LIMIT fixture reaches canonical Complete. Its matched deletion child receipt
is 1,464,530 bytes; escaped DaemonResponse is 1,572,891 bytes. At the old peer limit
of 1,048,576, server.rs replaces -32006 with generic -32005 after dispatch. The
response loses all known commit evidence. No truncation, child-to-start replacement,
receipt reference, protocol change or weakened positive assertion is permitted.
The existing smaller start/child/final frames passing does not bound all inputs.

## Source facts that constrain a fix

- daemon/server.rs handle_connection knows negotiated peer_limit before dispatch;
  NamespaceRegistry::delete_namespace currently receives only user input. The
  server cannot safely infer physical schema from namespace names or API version.
- daemon/protocol.rs serializes the complete DaemonResponse, including escaping
  its string error.message, before checking the limit. Request/response id is u64.
  Legacy limit is 1 MiB; existing negotiated maximum is 8 MiB. Neither proves a
  native receipt fits. No existing larger negotiation is available above 8 MiB.
- NativePurgeStarted embeds beforeNamespace. EmbeddingProfile::validate checks
  model identity/preprocessing values but has no model_id/model_revision length
  limit. Even a start receipt is not universally bounded by a small constant.
- NativePurgeChunk has at most 1000 deletions, each with at most 32 numeric access
  events. Each record commitment/profile ID is fixed-shape (profile ID is 68 bytes).
  Removed edges and external incoming peer witnesses have no comparable aggregate
  cap. One deleted row may have enough incoming peer proofs to exceed a frame.
  The native graph per-operation edge limit is not a lifetime indegree limit.
- metadata/purge.rs begin_purge_with_work performs original replay lookup, validates
  fresh config/force, then constructs the start outcome before record_receipt and
  the next child ID draw. This is the authoritative fresh admission seam.
- advance_purge captures the candidate proof before finish_record_effects. The
  latter sets peer after.edge_count and after.record_generation before recording
  the child. Sizing only the captured pre-update proof is insufficient.
- engine/purge.rs startup and completion both resume through the existing shared
  CHUNK_LIMIT loop; another completion may win. A caller-local smaller chunk limit
  or process-local per-parent size registry would not constrain all future receipts.
- The final TX active-namespace guard calls require_no_active_write globally.
  Ordinary record/graph/catalog writers cannot add new target content while purge
  is active. Competing continuation can change peer counts/record generations, so
  a prestart bound must cover every permitted subset/continuation, not just current
  decimal widths or one predicted partition. Restore needs exclusive ownership;
  it cannot interleave into a live owned operation.

## Alternatives

| Choice | Feasibility and user-visible result | Legacy availability and unknown safety |
| --- | --- | --- |
| 1. Prestart full-evidence admission | Add an internal actual peer-budget route; validate a conservative bound under the original guarded start TX. If any required start/child/final envelope cannot be proven to fit, return existing TooLarge before canonical start or owned ID draw. Admit small native requests on old peers too. | Preserve the existing legacy-storage delete path and protocol framing. Some large native namespaces are conservatively refused even if a smaller chunk algorithm could serve them. Rejection is a fresh request refusal, never erasure/reclassification of an existing commit. |
| 2. Shared byte-bounded chunks | Select a prefix whose fully updated witness fits before canonical commit in the existing shared mutation path. Can reduce overly large multi-row chunks without rewriting past receipts or adding a new progress codec. | More native operations become available, but one row plus all incident peers/history may still not fit. Requires prestart worst-single-row/refusal plus start/final checks. All startup/competing resume writers must use the same bound; old oversized immutable receipts remain oversized. More core surface than choice 1. |
| 3. Negotiated larger transport | Existing ping can select up to 8 MiB; thread the actual value to the same admission check. This would admit candidate006 if its complete envelope passes. | Old peers still require refusal/fit at 1 MiB. Negotiation alone cannot authorize mutation: start strings or one-row peer witnesses can exceed 8 MiB. Raising the maximum or adding a route changes the protocol and still does not produce a universal bound. No such change is recommended. |

Recommendation for this bounded consumer: choice 1, using the current peer limit
rather than a blanket old-peer/native ban. It establishes an honest operation-size
limit with the existing protocol. Choice 2 is a separate availability improvement
only if the conservatism is unacceptable; it is not necessary to pretend that an
unbounded receipt fits. Choice 3 supplies capacity to choice 1, not a safety proof.
No runtime implementation is approved by this document itself.

## Proposed choice 1 seam and bound

Wire owner proposes a provided internal-trust Rust trait method such as
`delete_namespace_bounded(input, frame_limit)` whose default delegates to the
existing legacy implementation. McpNamespaceAdapter overrides it: native uses the
admitted path, actual legacy storage delegates unchanged. The daemon supplies its
own peer limit, never a field trusted from request JSON. Existing direct calls use
the shared implementation with their actual transport policy; no artificial daemon
8 MiB cap is assumed for an MCP transport that has no such response limit. Local
modern MCP still must prove full text plus structured serialization preserves the
same receipt; remote MCP additionally inherits the daemon admission. Final names
and Rust API compatibility are subject to wire review. The provided default is compatibility delegation for
existing implementors, not a proof that an arbitrary custom native implementation
obeys the budget. The actual Mcp override and daemon dispatch require end-to-end
admission tests.

Budget checking has two stages, one authority. A cheap check can reject before
accepting asynchronous work. The definitive positive decision must run in the
original fresh-start TX after original replay/namespace validation and before any
canonical write or owned ID draw. The guarded check can run after transient root/child work tickets were admitted;
fresh refusal promises no durable work, owned ID/provider/clock/FS side effects
and all known tickets settled, not zero tickets ever accepted. Cheap rejection
before ticket admission is useful where feasible. A cached positive is insufficient.
It consumes
current canonical namespace/records plus authoritative incident edges under the
existing publication -> graph -> storage/mutation lock order. No lock crosses an
await; no second generation, serialized summary, registry or durable permit.

A conservative bound may sum all target deletion witness components, all incident
edge components and all possible incident source peer before/after components.
Overcounting is deliberate: any legal <=1000-row continuation is a subset, even if
another worker selects a different chunk. A target record may first be a peer and
later a deletion, so both roles must be budgeted. Count commas, array/object syntax
and the maximum envelope separately. Do not allocate a full namespace-sized proof:
stream a checked count and stop as soon as the sufficient bound exceeds capacity.
A row/edge examination or temporary-memory cap also refuses before start; it must
not return a positive on a partial scan. Existing edge loading costs must be named,
not hidden by calling the extra counter streaming.

The bound must use current retained variable fields and validated fixed-width
hash/profile commitments, with explicit worst-case widths for all values that can
change before return: u63 generation/record-generation decimal strings, u64 sequence
and response ID, UUIDs, u16 edge counts and optional known-unknown operation ID.
Preserve actual f32 bits; do not change history/proofs to simplify sizing. Hash
bytes need their maximum JSON integer width if the digest is not read; this is only
a sizing bound, never an invented canonical commitment. Current full profile
strings are counted including JSON escaping; unknown extensions still fail the
existing validators. A bounded counting writer can avoid a second large Vec.
Implementation review must check the inequality against the real serializers.

All three evidence shapes require complete outer-frame budgets:

| Shape | Required variable content and envelope |
| --- | --- |
| Start | Exact beforeNamespace/request, full WriteReceipt; both committed generic and unknown-with-real-operation-ID cases, plus UTF8 diagnostic up to 512 bytes and outer escaping. |
| Deleting child | Full deletion histories, incident edges, complete before/after peers, original parent and sequence/G; preserve matched-child precedence and full receipt, even if canonical progress separately completed. |
| Cleanup/final | Full chunk=None receipt with maximum numeric widths; SERVICE_UNAVAILABLE, unknown or MetricsUnavailable envelope as allowed by actual evidence. Actual success includes bounded namespace name and numeric counts/observed bools. |

Do not select a magic receipt-only margin. Count the actual DaemonResponse shape,
code, diagnostic's worst JSON escaping, and u64 response ID. The four-byte framing
prefix is outside the protocol's payload limit but must be recorded in frame tests.
For MCP, text is pretty JSON and modern structuredContent repeats the object; report
those actual complete frames separately rather than equating receipt bytes with
transport bytes. Unknown begin has no fabricated receipt/ID. Unknown child retains
its actual allocated ID plus known start; sizing cannot upgrade it to committed.

Historical oversized receipts are not rewritten or silently substituted. The new
name-only API1 operation has no idempotency key and must not retry/reconcile by name.
The internal keyed continuation remains replay-first. If a future bounded transport
exposes historical replay, it needs a separate prework size refusal that explicitly
preserves known historical commitment; it cannot call that existing operation
notCommitted. This child must test that its new route cannot fall into such an
unhandled historical path. Old offline/startup recovery remains available.

A final child TX assertion can catch a broken upper-bound implementation before
commit, but it is defense, not the proof: after start, a rejection still carries the
actual known start/earlier receipt and cannot strand an admitted workflow by design.
Cancellation and another completion must preserve the prestart bound without a
worker-local permit. An exception cannot become generic postcommit TooLarge.

## Required execution before readiness

1. Keep candidate006 unchanged. New actual old-frame test: oversized native request
   refuses before start, with identical canonical bytes/G/ID cursor/progress and no
   purge cleanup or accepted worker left behind; old legacy storage behavior stays.
2. On an admitted native fixture, inject failure at start, deletion child, cleanup,
   complete publication and unknown commit. Check exact full frame, canonical
   receipt identity, scope, reserved ID and no extra mutation/readback.
3. Actual negotiated 8 MiB path admits the 1000-row example only after proof; a
   deliberately larger one-row incoming-peer or profile-string case still refuses
   before work. Old peer doesn't receive a new-sized frame or a reconnect retry.
4. Max 32-event history, escaped profile strings, maximum counter-width bounds,
   internal target peers, foreign incoming peers and empty namespace. Verify
   streaming cap/overflow refusal, not partial positive or OOM.
5. Forced competing completion, cancellation, crash/reopen and historical replay:
   default startup chunking cannot escape the proven admitted bound. Preserve old
   oversized retained receipt bytes and avoid readback or receipt substitution.
6. Modern/old MCP text and structured content, real remote adapter and actual
   daemon frames, unknown-vs-known errors, genuine observed zero/false cleanup,
   recreate-name protection, then affected regression/full/fmt/lint checks on a
   final frozen implementation. No publication based only on estimator unit tests.

## Ownership and current status

Feynman owns runtime and canonical feasibility; Huygens reviews wire/source plan.
Halley confirmed no queue edits to the protocol/trait/adapter hunks and delegated
source-plan coordination to these owners. Huygens reviewed this combined plan and
ACKed option 1 plus existing negotiation, both target record roles, u63 widths and
absence of an invented MCP 8 MiB cap; his two trait/ticket clarifications are included.
This is source feasibility review, not runtime approval or executed proof.
No additional parent is needed for this
plan; live protocol integration must preserve the already published prework guards.
The failed pilot's clone/evidence/seeds and personal recalld remain untouched.
