# Legacy-store API2 consumer inventory

Source audit, not a runtime defect reproduction or activation proof. Baseline:
published PR121 `3fa91d4e542ec8ed5519fe4c3b87e9e665043a40`; comparison:
insertion-wire focused source `2cb4946c4fcf02f2100ddfa071f8237376fc341d`.
CONTRACTS separates API, lifecycle, and canonical-format versions. Native API1
support alone cannot complete public API2: legacy stores also need actual API2
consumers without an implicit migration.

| Actual source boundary at PR121 | Finding | Required consumer work |
| --- | --- | --- |
| `metadata/insertion.rs:199`, `insert_reserved_with_effects` | Rejects context other than 1 before replay/staging. Separate later checks fence namespace/control schema. | A legacy-format explicit2 single writer must carry original context through hash, replay, CAS, allocation and completion. Preserve physical fences and legacy1 hash bytes. |
| `metadata/tombstone.rs:31`, `tombstone_reserved_with_effects` | Rejects context other than 1; replay precedes current existence/CAS once admitted. Record/version/schema fences are separate. | Actual explicit2 forget consumer, retaining namespace assertions, replay order and original full receipts. |
| `metadata/batch.rs:348/729` | Reservation and child completion require context1. `storage/batch.rs` constructs context1 for input and child; manifest validation reconstructs historical domains. | Compound original API identity must be retained and checked across reservation, child, bookkeeping, resume and historical validation. Removing only these guards is insufficient. |
| `metadata/catalog.rs`, `engine/catalog.rs` | Production receipt methods take a supplied context; `control::replay_or_check` validates 1 or 2 and request hashing includes that version. The cited `metadata/catalog.rs:178/258` context1 constructors are tests. Production `create_namespace_with_work` uses `MutationContext::unkeyed`, which defaults to1. | Audit and wire the explicit2 legacy catalog/policy route to the receipt-bearing methods; prove actual runtime acceptance/publication rather than infer it from signatures. Do not change the API1 convenience default. |
| `metadata/observation.rs:328`, `legacy_scope` | Context1-only guard is shared by legacy observation operations. | Explicit2 feedback and observing reads on legacy lifecycle need original-context propagation and their real receipt/publication path, not native-only exposure dispatch. |
| `metadata/lifecycle_updates.rs:239`, `decay_with_effects` | Legacy maintenance requires context1 and matching namespace scope. | Separate manually requested origin from autonomous maintenance policy; do not globally relabel existing timer work as API2. |

The current insertion-wire composition already contains later private batch
formats: `contract_version(version)` maps private1 to API1 and private2/3 to
API2. Child contexts and hash reconstruction use that mapping, and native
defaults/outcome checks distinguish their physical semantics. This replaces the
older literal guards but does **not** demonstrate legacy-store API2 support.
Do not reinterpret historical private1/2/3 manifests, silently change their
defaults, or reuse a native-format selector to bypass legacy physical fences.
Any additive retained-origin proof needs same-transaction hash validation and
genuine historical absent-proof compatibility. Coordinate its exact scope with
Curie/Feynman before implementation.

## Required bounded proof before activation

- Use explicit owned legacy fixtures and independently owned native fixtures;
  exercise original API1 and API2 through actual MCP/REST/RPC consumers. API and
  disk selection must be independent, with API1 shapes/defaults unchanged.
- Freeze genuine pre-change API1 receipt/manifest bytes; reopen, replay and
  resume after the change without rewriting their hashes. Verify changed API
  origin under the same key conflicts rather than adopting the old receipt.
- For fresh legacy API2 writes, check scope and stale CAS before allocation,
  provider work or filesystem staging; matching replay precedes fresh CAS and
  current settings. Retain complete known-commit receipts on publication failure.
- For compound changes, test partial outcomes, original reservation CAS,
  pending resume, proof removal/tampering/unknown versions, export/import/fork,
  and startup rejection with immutable source bytes. Preserve every old
  format's existing absent-proof meaning.
- Keep record/schema/native-writer negative guards. Selecting API2 must not
  widen legacy structured-label encoding or claim unknown profile provenance.
  Public API2 query activation additionally requires actual A2 selection and
  coherent observing reads; write acceptance alone is not completion.

No runtime guards were changed by this audit. These are explicit remaining
activation prerequisites, not a decision to make API2 native-only. The proposed
follow-up is a minimal actual legacy2 writer/adapter consumer after its original
context interfaces are frozen, not an unused helper or blanket guard removal.

Coordinated follow-up ownership: Curie owns insertion/batch original proofs,
Feynman catalog/compound-history review, Pascal observation and maintenance
origin, and Huygens actual MCP/REST/RPC selection and activation. This follows
the current native consumer work; no extra legacy guard edits are included in
that consumer. Read-only API2 on either store format remains gated by its actual
coherence, semantics and observation prerequisites.

## Integration ownership

Before an integration agent changes or merges an owned branch, provide READY
with its exact validated SHA, dependencies and evidence. Stop pushing that
branch until ownership is returned or it is merged. No force-push, branch
deletion or stack rewrite. This insertion-wire branch has not been handed off
READY; reported merges elsewhere must be rechecked by the integrator.
