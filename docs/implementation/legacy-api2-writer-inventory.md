# Legacy disk with originating API2: writer inventory

Source-only audit of actual PR121 `3fa91d4e542ec8ed5519fe4c3b87e9e665043a40`,
requested by the coordinating agent during the quiet window. This is not a
runtime reproduction, permission to widen guards, or public activation evidence.
API version, disk schema and lifecycle policy are independent in both directions:
API1-on-native work does not establish API2-on-legacy support. Public API2 must
advertise actual per-operation support across disk formats, not pretend API2
means native disk exclusively.

| Surface | Actual source finding | Required consumer work before support is claimed |
| --- | --- | --- |
| Single legacy insertion | `metadata/insertion.rs:199` rejects contract !=1 after existing context/guard checks. Receipt hashing already consumes ctx; child hash uses captured batch defaults. | A bounded actual contract2 consumer must retain original context and legacy input/physical-schema checks, replay before provider/allocation/CAS, and complete derived publication. Do not remove the guard without proving the entry path. |
| Single legacy tombstone | `metadata/tombstone.rs:31` rejects contract !=1. Fresh path separately rejects schema-bearing namespace/record and control schema !=1. | Preserve those independent physical fences, atomic effects/removal ledger, terminal/reused-vector rules and exact known/unknown receipt handling. Prove both API identities and unchanged legacy1 bytes. |
| Legacy batch reservation | `metadata/batch.rs:348` requires contract1. `batch_reserve_v1` hashes the incoming context and caller manifest; operation resume compares its retained reservation receipt. | Original API must be retained and anchored for restart/resume and strict history validation. Reservation admission alone is insufficient. |
| Legacy batch resolution/child capability | `metadata/batch.rs::resolve_batch_item` builds `MutationContext::unkeyed` (API1); `BatchChild::context` in `storage/batch.rs:122` constructs1; `record_child` at metadata/batch.rs:729 requires1. | Derive child identity from the validated immutable parent, not caller override or disk-format inference. Preserve stable operation IDs, captured defaults and final same-transaction child binding. |
| Legacy batch rejection/input | `finalize_batch_error_with_work` uses catalog_context (API1); `BatchInput::context` at storage/batch.rs:250 constructs1. The result_version explicitly negotiates the response envelope, not API. | Propagate originating API through the actual runner, rejection/bookkeeping hashes, all-invalid/zero-success histories and source/fork validation. Never use result_version as API identity. |
| Legacy catalog creation/policy core | `metadata/catalog.rs:178/258` are **test** contexts, not production restrictions. Production create_catalog_guarded/update_namespace_policy_with_work accept a supplied ctx, hash it, and call replay_or_check (which validates1|2). Mutation guard/config validation separately fence native schema. | Audit/prove actual contract2 producer propagation, replay, no-change behavior and publication. Do not claim a production ctx1 rejection from those test lines or blindly change the already-context-bearing core. |
| Legacy catalog creation wrapper | `engine/catalog.rs::create_namespace_with_work` builds an unkeyed API1 context. Actual121 MCP bridge:2170 and API adapter:1509 call it. Receipt-bearing engine entry accepts supplied ctx. | A real versioned producer must use the context-bearing seam and preserve original API/key/CAS/default identity through publication; old unkeyed API1 callers retain their established contract. |
| Legacy purge | Actual121 selects native purge domain/progress from contract2. The current native-context child instead selects native mode from owned state and deliberately still rejects API2 on a legacy owner. | A separate bounded legacy2 compound consumer needs original start/child identity and strict restart/current/offline validation. Neither121 nor the native1 correction establishes legacy2 purge support. |

## Compound compatibility boundary

The current RetainedRequestContext codec is deliberately scoped to known native
compound workflows: absent means historical native API2; present is closed API1.
It must **not** be blindly reused for historical legacy batch/purge where absence
means API1. Any later legacy2 extension needs an explicit, reviewed codec/version
strategy with genuine old1 artifacts, immutable parent-hash anchoring and strict
older-reader rejection. Preserve every old1 receipt/progress byte and hash.
No new field shape or runtime widening is authorized by this inventory.

Inspect all parent/resolution/child/rejection hashes and validators before edits,
including logical export/import, backup/restore forks and current readers. The
all-invalid/pre-first-child case must reject stripped origin proof across every
canonical/embedded copy; child validation cannot anchor an empty history. Do not
infer public API from lifecycle, disk, batch result version or a guessed hash.

## Ownership and minimum evidence

Huygens owns A4 versioned dispatch/capability truth and real transport coverage.
Curie owns insertion/batch producers and compound child binding; coordinate a
minimal actual legacy2 writer consumer only where required, not a broad startup
parent. Feynman reviews shared history/offline proofs and purge participation.
Pascal separately owns the audited observation/lifecycle legacy context seams.
Each adapter may be prepared and tested on its actual owned disk fixture before
public activation; ordinary native startup does not depend on this later public2
switch. Do not merge unvalidated work merely to assemble a capability matrix.

Required evidence for any later supporting slice: genuine BEFORE old1 artifacts;
API2 commit once and original replay before stale CAS/provider/current defaults;
same-key cross-contract conflict in both directions; preserved physical-schema
rejection; provider/clock/ID counters and source-byte negatives; exact receipts,
ledger and current-generation publication; restart and original-lineage/fork
replay rules; pending/malformed compound rejection without source mutation.
Ordered batch results remain per-child transactions, never an atomic-batch claim.
