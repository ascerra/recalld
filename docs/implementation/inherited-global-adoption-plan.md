# Proposed bounded inherited-policy adoption and preview

Status: approved implementation plan; implementation in progress, not validated. Feynman owns A1 schema,
proof and startup admission; Pascal reviews A3 planning/clock semantics. Coordinate
shared system.rs hunks with the controls helper before edits. Actual BEFORE on
published143 is in ../evidence/inherited-global-startup/README.md. Native sweep
consumer147 is published independently and contains no audit code.

## Scope and minimal representation

Close the reproduced native owned-initializer path first. The actual file/env
configuration currently feeds the inherited decay multiplier; the initializer
supplies built-in DecayConfig for the other shared-planner inputs. Do not claim
arbitrary custom Rust NativePolicyInputs or physical-v1 restart policy adoption
are certified by this bounded correction. No hot-reload/watcher subsystem exists
or is proposed.

Prefer an additive typed optional binding in NamespaceSchemaV2, not a new global
store/engine or duplicated table inventory. Proposed `inherited_decay` is absent
and omitted byte-for-byte on existing histories. Absence means unknown/unadopted,
never 'the current configuration was historically active'. A present version1
binding contains the validated finite effective f32 multiplier and the original
adoption GenerationRef. Explicit namespace Some(value) continues to override;
changing an unused global multiplier must not change its effective policy or
produce a revision. None remains None and remains inheritance, not a rewritten
explicit override. A future externally configurable connection cap/partial-access
weight requires a reviewed versioned extension, not an unversioned hash change.

The optional field is only read compatibility. Once an adoption receipt exists,
strict current/historical validation must require the exact binding it anchors;
removing it cannot fall back to old unknown history. Add a required typed native
adoption outcome to the existing receipt enum with before/after binding,
namespace/lifecycle identity, previous/next revision, source/previous canonicalG,
all-row count and token digest. Reuse the existing policy revision-chain logic
and final transaction structure; do not fake a numeric before!=after patch when
ordinary namespace values remain unchanged. Old outcomes and original hashes
are immutable. Unknown new versions fail closed in old/new readers as appropriate.

## Actual preview/apply interface

Names below are proposed interfaces, not implemented exports:

- `preview_inherited_decay(scope, desired_multiplier, expected_source, clock)`
  returns an owned preview: source GenerationRef, namespace policy revision,
  normalized candidate inputs, one sampled clock, current canonical graph/record
  digest, record count, phase counts and a bounded content-free preview token.
  Use the existing shared planner and real graph-before-storage lock order.
  Unknown prior input produces an explicit unknown prior comparison; candidate
  phase counts are still computed from retained anchors. Never manufacture a
  historical 'before' multiplier from the current config.
- `adopt_inherited_decay(context, preview)` does lifetime replay before fresh
  clock/config work or CAS, then validates the exact preview source/revision,
  intended config token and final canonical record/graph evidence. Under one
  existing owner/write transaction, update the namespace binding/revision and
  all canonical row policy revisions/tokens using the existing policy writer's
  rules, with one canonicalG and receipt. Retain anchors, phase, materialized
  strength, payload/provenance and graph content. No phase loss occurs here.
  A stale source/config/graph token rejects before mutation; user regenerates
  preview. The accepted preview's original clock is not resampled on apply.
- A supplied key on an exact no-op must still bind a typed no-change outcome and
  advanceG once, without policy/record revision churn. An unkeyed exact no-op may
  returnNone. Changed input with the same key conflicts; same key replays its
  original result before newer config/CAS. Preserve the in-hand receipt through
  actual publication errors using the existing DerivedCompletion path.

Expose an actual explicit operator preview/apply path with bounded JSON inputs,
reusing the lifecycle/admin CLI infrastructure. No helper-only publication. The
preview must be shown before adoption; startup does not silently perform it.
Operator acceptance of unknown initial inputs is a new present-day adoption, not
an attestation of what affected old observations.

## Consumers and activation boundary

Coordinate one helper-owned initializer seam: validate candidate config against
canonical adopted inputs before enabling dependent lifecycle projections or
writers. A mismatch/unknown inherited binding gives a typed actionable admission
error for that namespace; it must not block offline metadata/export or compatible
namespaces' metadata operations. Keep existing profile checks separate. No
constructor write or automatic fallback to a new current value. Ordinary native
startup stays gated until actual readers/writers use the seam.

Operation resolution must use the binding from the same canonical namespace view
and actualG as the record/policy capture. Reuse this in native lifecycle/feedback,
maintenance, and explicit-feedback query projection; do not replace it with an
independent startup-only check that later operation builders can bypass. Replay
must still happen before fresh admission where an original receipt is known.
Preview/adoption is the explicit administrative exception that can establish a
missing binding; it does not generally weaken ordinary mutation guards.

Numeric policy Some→None needs an explicit adoption before inherited semantics
become active; None→Some uses its explicit value and must not invent or rewrite
past bindings. Newly created empty native namespaces need a declared current
input snapshot anchored in their actual creation proof before ordinary defaults
can work; this is current creation state, not retrospective blessing. Existing
unknown namespaces require explicit adoption. This creation consumer is a named
follow-up in the same dependency closure, not an excuse to activate a core helper
alone. Preserve legacy_access default and original API request identity.

NamespaceSchemaV2 is already nested in the typed retired NativeNamespaceProof,
so retain the binding with that content-free witness after purge. Strict policy
history must reverse adoption/revision transitions before older numeric/graph/
maintenance proofs, including restored foreign origins and reused names. Audit
all logical import/export, backup/restore/fork and canonical/raw validation paths
for the typed field and required receipt; no bare allowlist. No new memory payload
retention, receipts rewritten to a new storeID or ancestry inference.

## Validation and staging

First implement the codec/proof and actual preview/apply consumer on the smallest
validated native-policy parent. Add the shared143 initializer/query consumer only
where the actual restart test needs it; do not import broad integration history.
Keep helper system edits coordinated and consumer147 frozen. Set exact parents
with main after this plan's field/CLI semantics review.

Required checks: genuine old absent bytes and history reopen; strict missing/
unknown/tampered bindings and foreign/fork cases; initial unknown explicit adoption;
source/config/graph changes between preview/apply; replay-before-CAS and keyed
no-op; all-row revisions including terminal rows; no anchor/content changes; known
receipt on publication failure and readiness; exact Some(1) unaffected control.
The actual restart BEFORE must become either same admitted projection/revision or
an explicit pre-serving mismatch, then change projection only after the real
preview/adoption receipt/revision. Include fixed-clock false get plus maintenance
and feedback consumers, not just a pure resolver test. Later actual creation and
purge/export/fork composition closes the remaining named consumers before ordinary
startup activation. Do not mark all A3 global-policy participation complete on
this native multiplier proof alone.

## Frozen parent choice and reviewed constraints

Implementation parent choice: published143
`0f34cd30e48b59dd655b3285d00f8b99e92c32d1` plus published135
`24da9206ba141493cdb9ca988188be6d5690a814`. The former supplies actual138
native numeric/catalog/retired-history proof and the validated shared query
initializer; the latter supplies the actual native manual/autonomous phase
producer. Both are real consumers required for the restart/feedback/sweep proof.
Do not add149/147 diagnostic/physical-compaction history merely for convenience;
147 remains the separate frozen physical consumer and must compose in later
ordinary-activation validation. Primary review base143, additional135 explicit.
No second native engine or broad checkpoint parent.

Before implementation, Pascal must finish the requested semantic plan review;
helper owns coordination for shared system hunks and has stated no overlapping
initializer/config edits. Main approved direction and requested these refinements:

- Binding identity must retain its original adoption GenerationRef and exact
  operation receipt pointer through import and every fork. Resolve that pointer
  with the retained original store/scope/key and verified ancestry bounds, never
  substitute the current storeID, reset its G, or recompute the request hash.
  The new typed outcome anchors the present binding; stripping the field while
  its receipt remains fails. Retired namespace witnesses retain the same typed
  binding and immutable identity, including after name reuse.
- Public preview output is bounded and content-free: scalar counts, fixed-size
  digests, one namespace identity, source/revision/config tokens and sampled time.
  No array of memory IDs/text or per-record witnesses is returned. Stream the
  record commitment in canonical order; bound examined rows, edge work and
  temporary allocation before copying, including rows examined but excluded.
  Exceeding the explicit budget returns a typed error before any write. Apply
  rechecks the bounded preview identity under final graph→storage guards.
- Multiplier-only version1 must explicitly reject unsupported changed custom
  shared-planner inputs. Compare passed DecayConfig fields with the supported
  built-in profile, reject nonfinite or changed custom values before fresh
  projection/capture, and keep replay-before-fresh-policy validation. Do not let
  a multiplier binding certify custom connection caps/partial-access weights.
  A reviewed richer version is required before those become configurable under
  this proof; ignored raw customization is not silent acceptance.
- Newly created native namespaces and numeric Some→None are required consumers
  before ordinary activation, owned by Feynman on the existing catalog/policy
  paths. Creation records declared present-day resolved inputs in its actual
  creation proof (no fabricated past); existing unknown namespaces need explicit
  adoption. Some→None cannot expose an unbound current-global fallback. Include
  actual creator/update/feedback/sweep/query/restart tests, not a helper-only PR.
- First explicit adoption of unknown input is not a claim that the candidate was
  historically active. Existing pinned operations remain valid under their
  original source/revision; adoption creates a new revision. The explicitSome1
  comparator is unaffected by an unused global multiplier change and receives
  no spurious policy revision. Preserve anchors, original receipts and phase.

The operator workflow is explicit preview then apply; no automatic startup
mutation. Choose its existing CLI plumbing with A4 before editing public option
shape. An offline admin invocation may require the existing exclusive owner lock
and report canonical receipt/reopen-needed readiness truthfully; a live consumer
must use actual DerivedCompletion rather than claim publication from a receipt.
Do not introduce a config watcher or invent mutable Arc configuration plumbing.

Pascal's semantic plan review is aligned (source-only). Normalize, preview and
hash the same effective finite f32 used by the resolver; distinct finite f64
configuration spellings that round to that same value are no effective change.
Reject nonfinite/overflow/underflow before allocating IDs. Retain unknown prior
inputs as unknown in preview, and preserve raw timestamps/phase markers even
when their origins are unknown. No foreign adoptionG is compared numerically
with the current fork's counter. Main additionally requires any scan/edge/time
budget exhaustion to return explicit error/incomplete with **no applicable
preview token**, never partial phase counts or a partial digest usable by apply.
Streaming the all-row commitment avoids an arbitrary memory cap where possible.
Shared helper hunk handshake was requested before any runtime edits.
