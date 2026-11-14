# Native single and profile-ID batch wire consumer (in progress)

Actual dependencies: sealed native batch wire PR122 `abd0371`, profile consumer
PR123 `d7c4d5f` (including PR118/96), and readiness observation `cc865c2`
(runtime `f8765b2`, direct PR96). Local joins are development compositions,
not inherited validation. Public application contract 2 remains closed.

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
