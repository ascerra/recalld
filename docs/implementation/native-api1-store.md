# Public API1 single-store consumer

This consumer selects the actual owned physical format for ordinary API1
`POST /memories` and MCP `store_memory`. Native owners use the accepted atomic
producer from PR148 with original contract version **1**; this is not a version2
envelope around a legacy writer. Native structured labels retain exact UTF-8 and
native explicit/automatic links commit with insertion. The legacy post-insert
link pass does not run a second time. REST retains `ApiResponse<MemoryResponse>`
and MCP retains `StoredMemory` successful shapes.

The actual parents are PR148 (`508d98c65b0f824bbcc0290812c63e0dea426edd`) and
PR153 (`a2a8866dd898dff7eae01dd49b23d8cd18532980`). PR153 provides the validated
same-connection negotiation, reconnect and API1 lifecycle consumer; this child
extends its one locked capability selection. Query PR151, catalog/list consumers
and ordinary-startup branches are not added as convenience parents.

## Vector identity and compatibility

For native REST supplied vectors, `embeddingProfileId` must explicitly match the
canonical namespace's immutable Bound profile. Missing ID returns 422; malformed
or null ID returns 400; a different valid binding returns 409. Profile-without-
vector is invalid. Width alone and the current provider default never prove the
caller's vector identity. Generated vectors use the actual provider descriptor
and the corrected `document_v1` composition, after the producer's replay checks.
This implements the approved prerelease native API1 compatibility amendment.

Legacy-disk REST without an ID retains the existing unverified supplied-vector
lane. An explicit ID is rejected with `UNSUPPORTED_CONSTRAINT` instead of being
ignored. MCP API1's historical single `embedding`/profile selectors remain
ignored: it generates embeddings as before. Unknown API1 fields do not select
API2, CAS or an idempotency key. API1 single stores remain unkeyed.

## Transport and failures

`api1StoreV1` is a private daemon capability independent of `api1LifecycleV1`.
The client captures both from the existing handshake, holds the connection lock
through method selection/send, and resets both on reconnect. A capable peer uses
`store_api1_v1`; an older peer receives the exact original `store_memory` input
including namespace, if representable there. No malformed, lost or rejected
new-method response triggers a mutating fallback. The new alias requires full
receipt-frame negotiation before admitting any work.

The historical raw `store_memory` alias on a native owner explicitly rejects
before provider work or owned reservation. Upgrade the client to negotiate
`api1StoreV1`; an old raw client is not silently routed into a writer whose full
receipt it cannot represent. The same old alias on a legacy owner retains its
successful shape and no-profile behavior.

Known publication/projection failures retain the complete immutable receipt and
available observational readiness. Malformed known-identity projections fail
closed without replacing that evidence. Unknown outcomes remain unknown and do
not acquire invented receipts. REST creation content comes from the fresh input
only after matching its summary/tag/label/text commitments against the exact
creation receipt; lifecycle values come from that receipt, avoiding a second
postcommit read that could race a later mutation. Failed unkeyed calls do not
advise blind retries or invented new keys.

Fresh full-receipt admission reuses the existing conservative typed native
single budget before provider work or payload allocation; an accepted work ticket
may already exist. Error projection uses compact JSON and
keeps the full receipt. Legacy successful formatting remains unchanged. Tests
assert no canonical byte/provider/profile/clock change only for the specific
fresh-invalid and predispatch cases exercised. That is not a blanket claim that
provider/worker failures avoid owned reservations.

## Deliberate remaining gates

The tested native owners are explicit, valid migrated/attested fixtures. This
consumer does not enable ordinary native startup or public API2. Existing Bound
vectors lacking executed-recipe certification remain an activation blocker;
see [the source audit](bound-vector-certification-audit.md). Feynman owns the
explicit current-vector remediation/admission proposal and Curie owns future
producer participation. Old immutable receipts and the personal legacy lane
are not reinterpreted.

Native API1 batch/public projection, complete legacy-disk API2 writer closure,
configured inherited-policy startup wiring and final API2 query/schema/discovery
activation remain separate consumers/gates. This single-store child is not
full A4 completion. Exact validation and retained failed-run attribution belong
in the accompanying evidence report.
