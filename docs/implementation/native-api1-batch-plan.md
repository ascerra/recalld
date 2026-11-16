# Native API1 batch consumer: source plan

Status: implementation approved by coordinator; Curie reviewed the shared
hash-first/ephemeral-surface seam. Validation in progress, no public activation. Parent is published PR158
`9c90f8201d476918077d3d08920fdb989269c205` only. It already includes actual148
(original1 native insertion/compound producer) and153 (same-connection capability
selection). No157 legacy-format4, query, catalog or startup convenience parent.
Worktree `/tmp/recalld-implementation/native-api1-batch`, branch
`impl/native-api1-batch`. Published158 stays frozen for Halley.

## Actual source gaps

- MCP `tools::handle_store_memories` selects `handle_durable_batch` on actual
  adapters, constructing mutationVersion1 and calling `mutate_batch`. The direct
  adapter does not select a native3 original1 runner for that request.
- REST API1 `batch_store_versioned` still extracts `BatchStoreRequest` and calls
  the legacy loop with legacy label validation, individual legacy insertion and
  post-insert links. Native public batch creation therefore is not closed by158.
- Existing `mutate_native_batch` already executes native3 through the bounded
  accepted runner and `native_batch_readiness`, including original current-G
  completion, failure observation and retained receipts. Use it, not a new loop
  of single-store calls or a second journal.
- `RetainedRequestContext::from_contract(1)` already encodes original1 for native3;
  absent context remains historical original2. No codec or public version bump.
- REST `CreateMemoryApiRequest` already accepts initialStability and createdAt.
  Shared `batches::parse_store` rejects createdAt, native_fields sets created_at
  to None, and native wire item_error excludes both fields. Copying the v2 parser
  would silently lose or reject existing API1 intent.
- Actual MCP durable batch parses supplied embedding/initialStability. Do not
  extend the single-store ignored-embedding compatibility exception to this path.

## Minimal consumer and ownership

Huygens owns the new API1 request/projection module, public MCP/REST routing,
bridge/Remote/daemon capability wiring and focused fixtures. Curie released
batches.rs/batch_adapters.rs for this bounded consumer and will review shared
origin/hash/preparation changes before publication. His future legacy-selection
work stays separate. Pascal lifecycle code is unchanged.

Add a narrow format-aware API1 insertion-batch bridge request/reply and private
`api1StoreBatchV1` / `store_batch_api1_v1` capability. Extend158's single locked
handshake/state-reset mechanism with an independent flag, not a second ping or
registry. The capable route selects physical format on the owned server. The
incapable route sends the exact prior `mutate_batch` request only when representable,
selected before sending; no fallback after mutation, malformed reply or lost reply.
The new alias requires full negotiated frames before work. Legacy raw mutate_batch
on native must reject before reservation/provider when it cannot retain this
consumer's result contract. Existing legacy old alias behavior remains intact.

Direct/native request construction uses insertion mutationVersion3, resultVersion1,
and Some(original1) from the existing constructor. Do not let public API1 callers
select internal codecs/context. Preserve existing MCP retry identity fields and
its rejection of public expectedGeneration. REST keeps its current typed outer
validation/default namespace/unknown-field behavior; it does not gain public CAS
or key semantics accidentally through the internal batch request. Native supplied
vectors require explicit canonical Bound profile IDs; legacy explicit IDs must
reject if unsupported rather than being ignored. Generated input uses148's actual
recipe. No same-width or provider-default provenance inference.

For REST original1 initialStability/createdAt, use the existing NativeInsert fields
and validated API1 values. Any narrowly origin-aware admission must leave all old
None/native2 and existing native3 request hashes and retained failures unchanged.
In particular createdAt previously follows the invalid-item raw hash: do not turn
that historical identity into the successful-parser hash on replay. Review the
exact preparation/hash reconstruction with Curie before implementation. His source
review confirms the minimal pattern: compute the OLD preparation/hash first, then
build the original1 execution plan retaining that hash, like the existing profile-ID
selector override. Valid parsing would otherwise add a different normalized tuple
and v2 wrapper. A genuine original1 rejected-createdAt manifest/error fixture must
replay unchanged; a fresh key/unkeyed request may execute the new admission, and a
changed timestamp conflicts on an existing key. Preserve null/default identity.
REST-only timestamp controls stay REST-only; do not add MCP benchmark controls.
No new persisted format is proposed to solve this seam. Preserve raw supplied f32
bits in request identity. No accepting private descriptor/observedAt controls via
public API1 or coercing contract1 into2.

## Projection and sizing

Keep legacy-disk routes and success shapes unchanged. Native MCP retains
results/total/stored/errors and existing indexed result fields. Native REST retains
created/total/optional failed and normal all-ready201 shape. Partial/unready native
outcomes require explicit additive batch evidence: every index/status, full item
and bookkeeping receipts, reservation evidence, unknown operation identities and
actual owner readiness. Never classify committed-but-unready children as failed
writes; unresolved items remain unresolved. Native partial failures use207 and
MCP isError; all-final-failure/unresolved statuses follow the shared typed outcome.
Do not infer aggregate readiness from earlier per-item success booleans.

Reuse BatchBudget::STANDARD plus actual compact counting serialization. Measure
whole REST/private daemon/MCP text+structured projection, including compatibility
fields and any duplicated item data, before treating its bound as sufficient.
Avoid duplicating large receipts across compatibility entries and batch evidence
on the new native projection. Preserve legacy projection unchanged. Historical
oversize uses the existing whole-error rule with a small explicitly named
reservation receipt, never truncated child receipts, false rolled-back status,
or a new-key retry hint. No new bulk receipt endpoint or receiptReference shape.

## Validation after plan approval

Reuse existing explicit migrated/attested native batch fixtures, real daemon/client
fixtures and source-specific BEFORE/AFTER protocol. No duplicate harness or ordinary
native activation. Before changes, reproduce actual public MCP/REST native failure
on158 with valid Unicode/generated/supplied cases; keep legacy positive controls.

Focused checks cover mixed valid/invalid ordered indices; authoritative empty and
Unicode labels after reopen; correct generated recipe and explicit links in the
same child commit with no legacy second pass; REST timestamp/stability preservation;
profile missing/malformed/wrong binding and legacy explicit-ID rejection; original1
manifest/children and mismatch with2; matching replay before changed provider,
namespace defaults/settings and clock; exact full receipts and canonical bytes;
all-invalid parent bookkeeping; partial publication failure and final-barrier
failure after successful children, repair/replay without new work; accepted work
tracking through cancellation; whole frame limits; and old/new capability cross
products, reconnect, unnegotiated rejection and malformed/lost no-fallback.

Use existing process-exit/resume fixtures only where this consumer introduces a
new boundary. Retained None/original2/Some1 fixtures test no historical hash change,
including previously rejected timestamp items. Full/focused/lint attribution will
be source-specific and run on immutable RO snapshots offline with the one retained
private cache/target. No new build has been started for this plan.

Remaining gates: current-vector certification/adoption, ordinary native startup,
other API1 writers, legacy API2 semantic/profile admission and complete public2
query/schema/discovery. This slice closes public native API1 batch insertion only.
