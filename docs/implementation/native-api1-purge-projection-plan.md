# Native API1 purge: exact failure projection plan

2026-11-15. Source-only proposal, no runtime implementation, build, Rust typed
fixture proof, or public activation. Local plan branch is based on actual main
`b481942dd8f998291cfef2abc175197fbeaa29fa`; it is not a shipping-parent proposal.
Source audit uses reviewed PR176 composition
`e1320179143b8434d7fcaa2a032e1096dc57de6e` and Feynman's plan
`0de1d9314ca94238cf97a417d55131a325746bd3`. PR179 remains frozen and is not a
required code parent for this adapter merely because its owner is coordinating.

## Ownership and smallest reuse

Feynman acknowledged sole runtime ownership of the existing purge worker,
cleanup/completion metrics and agreed adapter projection. Huygens supplies this
wire decision and reviews conformance; no parallel edits of the same adapter.
Pin the actual minimal integrated base and explicitly hand off shared files before
code. Do not edit frozen179 or the integration worktree. Main reviews this plan
before runtime changes.

Keep `BridgeError::PartiallyApplied(String)` and its existing daemon numeric code
`-32006`. The existing RPC codec transports that message unchanged, and the remote
adapter reconstructs the same variant. PR176 already uses this carrier for a JSON
object retaining a full receipt. No new public Rust/RPC enum, protocol version,
receipt format, database table, registry, operation-ID allocator, or public route.
Do not wrap an unknown outcome inside `StorageError::CommittedMutation`.

The existing `INDEX_NOT_READY` create projector is not reused for generic purge
failures: neither missing invocation metrics nor a progress/storage error proves
indexes are unready. Add `PURGE_METRICS_UNAVAILABLE` for the narrow completed-purge
case. For known-committed generic progress/storage/recovery errors, reuse the
existing API1 code spelling `SERVICE_UNAVAILABLE` (api/errors.rs), **not** its
AppError projection that loses the receipt, and not CommittedMutation
INDEX_NOT_READY/ready:false. Keep the existing framed carrier and full receipt.
This distinction was explicitly approved after f5d0574 review. Omit INDEX_NOT_READY
from this new projector unless actual typed publication evidence is supplied;
no such new barrier branch is required by this plan.

A small **private, in-process** typed failure is justified because the current
purge worker's join failure is flattened into a RecoveryRequired string, and an
unknown completion early-return otherwise drops the already known start receipt:

```rust
enum PurgeApi1Failure {
    Storage {
        source: StorageError,
        known_receipt: Option<Box<WriteReceipt>>,
    },
    WorkerUnknown {
        known_receipt: Option<Box<WriteReceipt>>,
    },
    MetricsUnavailable {
        receipt: Box<WriteReceipt>,
    },
}
```

This is consumed by the actual API1 adapter, never persisted or advertised. Carry
it before the existing lossy flattening point, not by parsing error strings. Reuse
the same underlying owner/completion implementation; do not duplicate purge loops.
Feynman source-ACKed these distinctions. The exact module/signature can follow his
single implementation, with the same discriminants and evidence requirements.

## Evidence and payload selection

The ordinary metrics failure occurs only after the existing mandatory progress
lookup succeeds and validates `PurgeStage::Complete`. Its exact `last_receipt` is
already in hand. Use it directly; no additional read of a poisoned owner to obtain
counts, no synthesis of a final receipt, and no canonical history rewriting.

| Typed state | API1 payload | Receipt scope |
| --- | --- | --- |
| Complete progress, both actual physical observations available | Existing `DeleteNamespaceResult` success | No new receipt/readiness fields added to the v1 success shape |
| Complete progress, at least one physical observation unavailable | `PURGE_METRICS_UNAVAILABLE`, `commitState: committed` | `purgeComplete`, full exact final receipt |
| Known committed child/progress/storage/recovery failure | `SERVICE_UNAVAILABLE`, `commitState: committed` | Scope derived from that exact receipt; known child receipt takes priority over prior start |
| Mandatory completion/progress lookup fails after acknowledged start | `SERVICE_UNAVAILABLE`, full known start receipt | `purgeStart`, never whole-purge completion |
| Typed `CommitOutcomeUnknown` after a known start | `COMMIT_OUTCOME_UNKNOWN`, `commitState: unknown`, actual unknown operationId | Preserve prior start receipt with `purgeStart`; unknown child outcome is not upgraded |
| Begin worker join returns no receipt/allocated ID | `COMMIT_OUTCOME_UNKNOWN`, `commitState: unknown`, `operationId: null` | Omit receipt and evidence scope; never invent either |
| Known prework validation/not-found/refusal | Existing API1 error behavior | No receipt or unknown-commit claim merely from a generic RecoveryRequired |

Keep the concrete StorageError/source chain internally. The wire message is a
truthful bounded explanation of the actual failed operation (at most512UTF8bytes),
not a string classifier. Preserve/truncate only diagnostic prose if needed, visibly;
never truncate the receipt or change its evidence scope. No new cause enum or HTTP
route is added. Actual typed serialization tests must enforce the message bound.

For known errors, `commitEvidenceScope` is one of `purgeStart`, `purgeStep`, or
`purgeComplete`, derived from the validated receipt's actual native outcome/stage.
It does not mean an atomic batch receipt, total deleted counts, current content,
current generation, or derived readiness. `purgeComplete` means the final canonical
purge transition only. It must match the retained namespace ID and parent operation.

Preserve the entire typed receipt, including historical generation, outcome, UUIDs,
receiptVersion and the owner-returned `replayed` value. Do not round/re-encode its
canonical stored bytes or hashes. Normal JSON serialization for transport does not
modify the retained receipt. Do not emit duplicate receipt aliases.

**No readiness fields are added to these API1 failure payloads.** Specifically,
omit `ready`, `readiness`, `applied`, and `pendingWork`. `complete_with_work` or a
Complete receipt is not a successful current readiness barrier; the root may still
be accepted. Do not run a new barrier just to decorate an error or self-drain the
failed root. A future readiness field would require actual owner barrier evidence
at the current generation after the caller's root is settled; that is not proposed.
A successful uncontended purge still must complete the actual owner and preserve
its existing success/publication guarantees without adding wire readiness claims.

## Exact API1 JSON examples

[Examples](../evidence/native-api1-purge-projection/examples/) contain complete
JSON values, including full receipts, with no ellipses or symbolic receipt slots.
UUIDs, generations, namespace7 and counts are **illustrative authored values**, not
executed fixture evidence or authentic retained history. The start example retains
an UnknownLegacy profile explicitly; it does not bless semantic compatibility.
All text/structured/daemon-message payload equalities were checked using Python's
JSON parser. Rust typed/validation and actual transport assertions remain required.

The existing request is `delete_namespace` with `{"name":"scratch","force":true}`.
No API1 idempotency key, expectedGeneration, namespaceId selector or new HTTP delete
route is added. There is no advertised namespace-delete HTTP route in this source;
therefore this plan does not invent an HTTP status/endpoint example.

[Known complete, metrics unavailable](../evidence/native-api1-purge-projection/examples/metrics-unavailable.payload.json):

```json
{
  "error": "PURGE_METRICS_UNAVAILABLE",
  "commitState": "committed",
  "commitEvidenceScope": "purgeComplete",
  "receipt": {
    "storeId": "11111111-1111-4111-8111-111111111111",
    "operationId": "33333333-3333-4333-8333-333333333333",
    "committedGeneration": "14",
    "receiptVersion": 1,
    "outcome": {
      "kind": "nativeNamespacePurgeAdvanced",
      "transition": {
        "parentOperationId": "22222222-2222-4222-8222-222222222222",
        "namespaceId": 7,
        "previousGeneration": "13",
        "sequence": 3,
        "stage": "complete",
        "chunk": null
      }
    },
    "replayed": false
  },
  "message": "Purge completed canonically; this invocation did not observe physical cleanup metrics. Do not retry deletion by name to recover metrics."
}
```

No `data`, `deleted`, removal counts, or vector-file boolean is present in that
error. There is no partial success object pretending to contain complete metrics.

- [Modern MCP result](../evidence/native-api1-purge-projection/examples/metrics-unavailable.mcp.json):
  `isError:true`; exact same JSON object in text and structuredContent.
- [Legacy-era MCP result](../evidence/native-api1-purge-projection/examples/metrics-unavailable.legacy-mcp.json):
  same text and isError, structuredContent omitted under the existing era gate.
- [Daemon response](../evidence/native-api1-purge-projection/examples/metrics-unavailable.rpc.json):
  JSON-RPC error `code:-32006`, message is the complete compact JSON payload string;
  result is absent. Existing RemoteNamespaceAdapter preserves the carrier unchanged.
- [Known start only](../evidence/native-api1-purge-projection/examples/start-only-failure.payload.json)
  and its `.mcp.json` / `.rpc.json` files retain the entire start proof and original1
  request context; they make no final-purge claim.
- [Unknown later child with known start](../evidence/native-api1-purge-projection/examples/child-unknown.payload.json)
  retains both actual unknown child ID and full prior receipt with its limited scope.
- [Worker unknown without receipt](../evidence/native-api1-purge-projection/examples/worker-unknown.payload.json)
  uses null operationId and no fabricated receipt.
- [Unchanged success shape](../evidence/native-api1-purge-projection/examples/success.payload.json)
  keeps all existing field names/types. `ftsRowsRemoved` is actual removed FTS rows,
  including otherwise orphaned rows; it is not a canonical-memory count.
  Existing `vectorFileRemoved` actually reflects namespace-directory removal.

Extend only delete_namespace's PartiallyApplied rendering using the existing176
create pattern. Native typed failure JSON gets the modern structured error; old
plain-string legacy errors remain unchanged. Validate known code/receipt-scope
consistency before structured projection; an arbitrary malformed remote object
must remain unclassified error text, not manufactured committed evidence. Never
use the message text to determine whether a mutation happened or authorize retry.

Full-frame acceptance measures actual MCP text+structured and daemon escaping,
including the legacy negotiated frame. Complete-stage metrics errors have only
fixed-shape IDs/counters and a bounded static message; no proof here that every
possible retained start/child receipt fits. Validate those actual error cases too.
If a reachable full receipt exceeds a transport frame, preserve the evidence and
bring that exact case for review; do not silently drop/truncate it or invent a
receiptReference/transport exception in this task.

## Ownership of metrics and retry semantics

Use actual ephemeral observations from existing cleanup/completion, keyed to
(store origin, purge start operation ID, stable namespace ID). Distinguish an
observed zero/false from **unobserved**. FTS removal must run under the existing
FTS lock before clear/rebuild; physical directory removal must report the actual
remove action. No directory existence guess, canonical-count substitution, shared
registry, durable metrics table, new cleanup pass, or extra purge work for display.
The current lock order is publication→graph→vectors→FTS→entities→storage.

If another completion wins between begin's lease release and this caller's
completion, return the known-complete metrics error. That is a bounded runtime
error, not a permanent feature gate. Feynman owns the actual capture and race hook.
A larger exclusive begin+completion refactor is not part of this proposal.

API1 deletion is explicitly non-idempotent and name-based. The client already
forbids automatic retry of delete_namespace; preserve that behavior and add no
mutating fallback. After success or known-complete metrics error, do not retry by
name to recover display values. If the name is absent, a new API1 call returns its
existing NotFound behavior; if recreated, a new explicit deletion targets a new
namespace ID, not the old operation. Never automatically issue that new deletion.
Unknown/start-only errors require reconciliation of actual retained identity;
no assertion that retrying by name is safe, and no new public reconciliation API.

Existing private keyed same-operation resume/replay remains replay-before-fresh
admission/CAS. It must not allocate/mutate just to recover missing ephemeral counts.
If completed history has no invocation observations, return the same truthful
projection-unavailable state with exact retained evidence. Do not return invented
historical metrics, change receipt commitments or promise API1 name replay.

## Required forced-race and compatibility acceptance

1. Use a valid actual native store and real owner. Create a nondefault namespace,
   live and tombstoned rows, cross-namespace edges and actual FTS rows. Establish
   the normal readiness baseline; use no fabricated schema/certificate shortcuts.
2. Pause caller A after its begin receipt is delivered and its read lease is
   released, before its completion can capture physical metrics. Keep A's accepted
   root/worker ownership real. Caller B uses the same existing completion owner to
   drive the same durable purge to Complete; B must not start a new purge operation.
3. Record B's exact final receipt, G, canonical bytes/history, real directory/FTS
   state and provider/clock/ID counters. Resume A. Its mandatory validated progress
   lookup returns Complete; absent metrics must produce PURGE_METRICS_UNAVAILABLE.
4. Assert actual direct adapter, prepared MCP and framed RemoteNamespaceAdapter
   carry that same full receipt and limited scope, MCP isError true, no success
   counts/data/boolean, no readiness fields. Text/structured/error-message equality
   and whole-frame bounds are checked on actual outputs, not only a small helper.
5. A's projection does not change B's G/bytes/history or call provider/clock/ID
   allocation. Accepted known work settles; a separate test barrier after root
   completion proves all four current views, without inventing wire readiness.
6. Force known-start lookup/publication failure, known-child failure, unknown child
   and begin join/lost-ack cases separately. Retain every in-hand receipt selected
   by the typed priority rule; unknown identities stay unknown. Cancellation keeps
   real worker leases; ambiguous work is not finished merely to return an error.
7. Uncontended success retains exact legacy/API1 shape and real physical metrics;
   empty namespace yields genuinely observed0/false where appropriate. Preserve
   default-name/force validation, existing legacy positives, no-name automatic
   retry, recreation safety, and existing keyed same-operation replay semantics.

Main approved the concrete wire decision, then explicitly approved reuse of
SERVICE_UNAVAILABLE for generic known failures with bounded truthful messages and
full receipts. Feynman is the sole runtime implementation owner; before edits he
pins the minimal actual source composition and shares exact hunks with Huygens for
wire review. This local docs branch is not an extra code prerequisite. Actual
forced-race/typed serialization/full-frame tests remain required. No ordinary
native/publicAPI2 activation follows.
