# Public API1 lifecycle projection plan

Parent: published PR144 `31a0463b2d7e0e6920c2daf6025759fb2e1d73ca` only.
New branch `impl/public-api1-lifecycle`, worktree
`/tmp/recalld-implementation/public-api1-lifecycle`. PR144 stays frozen for Halley.
No runtime edits or test results are claimed by this plan.

## Ownership and scope

Huygens explicitly handed off the MCP `reinforce_memory` / `forget_memory`
handlers and corresponding bridge methods; REST reinforcement/delete handlers
and corresponding adapters; RemoteStorageAdapter lifecycle methods and dedicated
RPC handlers. Files can overlap, but these method hunks are disjoint from his
query work. Pascal owns the lifecycle projection helper. Huygens retains recall,
similar, query traits, store/profile, shared contract admission/output schemas,
session context and global activation. Batch methods are unchanged unless a real
shared caller requires a mechanical adjustment. No native insertion/profile work.

A follow-up handshake covers the existing daemon client's ping/capability state
and a lifecycle-specific call method; no competing generic protocol mechanism.
Public2 and ordinary native startup admission remain gated. Tests use the actual
public API1 handlers/router with explicitly owned synthetic native engines; that
is consumer proof, not enabling ordinary native startup.

## Request and result contract

API1 keeps its existing supported inputs, defaults and unknown-field tolerance.
These public operations remain unkeyed; this leaf adds no public key/CAS fields or
promise. Quality and namespace assertions retain their existing strict checks.
An omitted public selector becomes explicit original application version1 inside
the owner/private request; physical format and lifecycle policy never select it.

Success stays byte/schema compatible apart from existing nondeterministic timing:
MCP reinforcement has id/strength/stability/phase/isPermastore; forget has id/deleted.
REST retains its existing ApiResponse data and timing wrapper. Preserve existing
ID rendering at each handler, including the MCP forget input spelling. False/absent and
already-terminal delete outcomes must be derived from canonical outcomes, not
inferred from an error string. Wrong asserted scope must not become deleted:false.

Use a lifecycle-owned API1 projection/reply, not a generic BridgeError/AppError
redesign. Native numerical success comes from the actual typed receipt outcome.
Legacy feedback receipts contain quality/time but no numerical state: capture
an ephemeral fresh FeedbackPlan from the SAME existing metadata write transaction
before commit, and return it only after commit and owner publication succeed.
No postcommit reread, second plan, durable receipt change, old hash/history rewrite,
or reconstructed historical strength. The ephemeral path is explicitly fresh and
unkeyed; existing PR144 keyed/private reply bytes remain unchanged.

Known failures retain the full original receipt before projecting to existing
HTTP committed/observation errors or a lifecycle-owned MCP structured error plus
text. Unknown outcomes preserve a legitimately allocated operation ID, or none
when unavailable. No fake rollback, receipt, operation ID or readiness stamp.
Existing API1 success shapes do not imply a public keyed-query/write contract.

## Actual execution boundary

Reuse PR144 scope resolution, typed owners, namespace policy and configured clock.
The actual owned engine mode selects native versus legacy canonical primitives.
Expose only the needed ephemeral legacy projection from the existing transaction;
there is no alternate storage plane or duplicated FSRS computation.

Exactly one accepted root covers scope, transaction and awaited owner publication.
Native clocks still sample once at the canonical boundary; legacy feedback uses
its existing injected clock inside the transaction. API1 handlers delegate that
root instead of nesting a barrier inside an unfinished outer root. Retain unrelated
admission checks. Remove only the HTTP delete handler's duplicate cache/index/
entity/graph mutations after the canonical owner has already published deletion.
Scope/terminal revalidation, graph-before-storage ordering and worker-held leases
remain in the existing owner. No self-draining failure barrier.

## Old peers and private transport

Select the RPC route BEFORE any mutation using actual negotiated capabilities.
Add one lifecycle capability to the existing ping exchange. Keep capability and
frame limit together under the existing connection mutex and reset/re-negotiate
both on reconnect; route selection and sending must use that same connection.

- Capable peer: dedicated `feedback_api1_v1` / `forget_api1_v1` with explicit
  original1, strict typed success/error/receipt handling and no automatic retry.
- Older peer without the capability: choose the EXACT existing API1 unscoped or
  scoped RPC before sending. Do not drop a namespace assertion. This preserves
  existing old-peer behavior; it does not claim the new typed-receipt guarantee
  for a peer that never implemented it.
- An advertised capability followed by unknown-method, malformed response or
  uncertain transport failure is an error, never a second mutating request.
- Establish the one-record receipt/frame bound. When the typed result cannot be
  supported by the negotiated limit, reject BEFORE work. Never replace a known
  committed receipt with a postwrite oversized-frame fallback.

Old native-only and PR144 lifecycle aliases retain their existing semantics.
No capability is inferred from storage schema, API number, protocol version alone,
or successful execution of a different mutating method. Older Rust test/adapter
compatibility must be handled before dispatch, never by retrying a failed write.

## Required validation

1. Before correction: actual public MCP dispatch and REST handlers/router on
   native owned fixtures reproduce the legacy-writer guard failures; preserve
   existing successful legacy API1 JSON/text shapes as the comparator.
2. After correction: both formats and both native lifecycle policies; original1
   request identity, configured clock/policy, one sample, scope and terminal
   behavior, existing success projection, strict reopen and no resurrection.
3. Legacy ephemeral numerical response equals the same committed transaction's
   result; a concurrent later write cannot leak into the prior response.
4. Actual public HTTP/MCP publication failures retain full receipts; real uncertain
   commit/worker loss retains only legitimately known identity and no invented
   readiness. No duplicate graph/index mutation or accepted-root deadlock.
5. Actual Remote client/framing: advertised new capability, genuine old handler
   shape, missing/scoped support, reconnect capability change and frame bounds.
   Assert exactly one selected mutation method; no new-then-old fallback or retry.
6. Malformed typed reply identities, quality, namespace and committed-state rules
   keep PR144's corrections; matching known receipts survive decode failures.
7. Public2/global startup remain closed. Unrelated query/insertion/batch paths stay
   under their owners; do not use private-only fixtures as public acceptance proof.

Use a fresh bounded validation root, one private Cargo/target and the authorized
read-only shared registry, offline Docker Rust1.94.1,2CPU/4GiB/jobs2. The released
`/tmp/recalld-legacy-api2-validation` must not be recreated. Preserve failing
source/results, then focused tests and appropriate full validation for this actual
consumer change. Seal recorded results and release raw scratch after publication.

The later PR135+145 native-compaction producer join is Feynman's task under the
explicit ownership transfer. Pascal reviews lifecycle/manual-origin semantics
source-only; there are no active consumer edits or hold here. It is not a
prerequisite for this public API1 child.
