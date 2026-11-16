# Native API1 purge: actual consumer composition

Source-only handoff after publication of the independent vector-owner PR181.
No runtime edits, diagnostic execution or activation is claimed by this document.

## Actual parent and ownership

The isolated child `impl/native-api1-purge-consumer` starts at actual PR176
`1f5d083d8142dd631f16f95617d23fddc85751ad`. Its published report attributes
1,365 all-feature library and three CLI passes to this exact source; that is parent
evidence, not proof of this new consumer. Git ancestry confirms it already contains
PR121 `3fa91d4e542ec8ed5519fe4c3b87e9e665043a40`, PR134
`b62d1e2f4b3f516b5889946605946e274f82286c` and PR138
`05e0d55aa3a92483e294b2a5e79cf490ce270609`. The complete native purge state machine,
original API1 compound context, owned allocation and existing full-receipt framed
carrier are therefore actual dependencies already present.

No extra parent is proposed for purge alone. Native catalog creation from PR103 is
a separately owned remaining consumer; it is not needed to route this existing
purge state machine. PR179 query, PR181 vector layout and PR175 certification are
not convenience parents. A later actual composition must preserve their respective
guards. The native fixture here can retain unknown provenance because purge is
structural and does not request embedding semantics.

Halley confirmed exact 1f5 is immutable, validated and published; later queue work
on 176 is separate and is not silently included in this proof. Huygens independently
checked this parent/ancestor composition and confirmed the purge/delete source seams.
These are source/ownership ACKs, not new execution evidence.

Huygens explicitly released namespace-delete MCP/tools, bridge adapter/trait and
DerivedCompletion purge/completion metrics hunks to Feynman. Huygens retains wire
review and will not edit those runtime hunks concurrently. Halley owns the frozen
parent branch; this child never edits or pushes it. The coordinator approved the
wire decision and SERVICE_UNAVAILABLE distinction. Huygens's exact reviewed plan
`921487dffe5de89e5abd94208857dc88c4a9806f` and its authored examples are copied here
with provenance, not merged as code ancestry and not represented as runtime proof.

## Exact source seams

- `storage/engine/purge.rs`: retain the same begin/resume/cleanup methods and
  durable transactions. Allow the existing cleanup step to return one optional,
  invocation-local observation for the requested `(origin store, start operation,
  namespace ID)`. Record the result of the actual directory removal/NotFound path,
  not `try_exists` or a canonical row count. Existing callers that request no
  observation retain their signatures/behavior via the same implementation.
- `derived_completion.rs`: the existing complete worker owns the optional target
  and O(1) observation. For that exact still-active target only, obtain actual FTS
  `remove_namespace` row count under the existing FTS lock before its ordinary
  clear/rebuild. Invalidate the real FTS publication before changing it. Do not
  remove rows solely to manufacture metrics after another worker already completed
  the operation. Capture directory observation from the existing resume loop;
  no second cleanup loop, map/registry or durable table. Preserve lock order,
  canonical/unknown error evidence, all four actual publications and acknowledgements.
- The existing completion join currently flattens JoinError into RecoveryRequired.
  Preserve that typed distinction before flattening for the new purge caller;
  legacy wrappers may preserve their existing interface. Keep the publication lease
  inside the actual blocking worker through cancellation, like the native begin
  worker already does. This is part of the new caller's lifetime proof, not an
  untested claim that cancellation already has the desired behavior everywhere.
- `derived_completion/native_purge.rs`: one shared begin/completion flow supports
  existing context-based internal callers and the API1 name-based producer. Resolve
  the name and begin with original contract1 under the same graph/storage guards;
  use the actual owned core, not API inference. No blind retry by name. A private
  typed result preserves StorageError, worker-unknown and metrics-unavailable
  distinctions, exact known start/child/final receipts, and actual unknown IDs.
  Preserve existing keyed internal replay and retain unresolved work on ambiguity.
- `mcp/bridge_adapters.rs` (or its private namespace-delete submodule): actual
  format-aware dispatch uses MetadataStore's owned entry mode. Keep plain legacy
  behavior unchanged. Consume the new native producer and project the approved
  private failure through PartiallyApplied/-32006; never reuse a projector that
  drops the receipt or invents INDEX_NOT_READY/ready:false.
- `mcp/tools.rs`: only delete_namespace's existing PartiallyApplied branch gains
  strict known-payload validation and modern text/structured equality. Preserve
  legacy-era plain text and existing malformed/plain legacy error handling. No
  new HTTP route or public setter. Daemon protocol remains unchanged; actual
  full-frame tests exercise its existing remote adapter and escaping.
- Tests use the real migrated native purge fixture and actual completion owner,
  including retained live/tombstone rows, foreign peers, directory and FTS data.
  Extend focused native purge/adapter/tool tests and existing framed daemon test
  helpers; no new public fixture loader or alternate engine.

Source files and hashes are pinned in
`../evidence/native-api1-purge-projection/consumer-source-pin.json`.
The existing legacy transport already calls `purge_namespace_checked`; whether
each native failure is currently accepted or misprojected must be demonstrated,
not inferred from the legacy method name. Preserve actual BEFORE observations.

## Error and race acceptance

Only a validated Complete progress row plus its exact final receipt can produce
PURGE_METRICS_UNAVAILABLE. Generic known committed progress/storage/recovery errors
use SERVICE_UNAVAILABLE with the full actual receipt, truthful UTF8-bounded message
and actual purgeStart/purgeStep/purgeComplete scope. No readiness fields or counts
appear in either error. Typed unknown errors retain the actual unknown operation ID
and any separately known prior receipt; a missing ID/receipt stays absent. Receipt
serialization is complete; no fabricated references or historical rewriting.

The deterministic race pauses caller A after its actual begin receipt/lease release.
Caller B drives that same operation through the existing completion worker. After B
finishes, A's existing validated lookup provides the exact final receipt but A has
no physical observations. A must return the approved complete-metrics error without
new G, ID, provider call, clock sample or automatic namespace retry. A separate
barrier after accepted work settles proves current four-view readiness; that fact
is not inserted into the error payload.

Additional executed cases must cover uncontended real metrics, genuine observed
zero/false, known start/child errors, unknown child/begin join, cancellation lifetime,
recreated names, keyed internal replay, malformed remote objects, legacy positives,
MCP modern/old-era text and actual framed daemon responses. Whole-frame bounds must
be measured for actual full start/chunk/final receipts. If a reachable retained proof
cannot fit, preserve it and report the exact boundary for review rather than silently
truncating receipt evidence. No standalone readiness or whole-native completion claim.

Implementation follows these agreed hunks after the exact parent/owner acknowledgement;
frozen parent/source reports remain untouched. One owned target is reused, with source
switch package cleaning and per-source executable provenance. Completed raw runs are
released centrally after a PR and recorded results, per the user's retention override.
