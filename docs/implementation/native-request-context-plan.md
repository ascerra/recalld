# Native writers with original API request identity

Proposed coordinated correction, 2026-11-14. **No core edits yet.** Coordinating
agent review is required before changing these shared primitives. This plan is
part of ordinary startup closure, not a new API1 storage engine. Source baseline
is PR121 `3fa91d4` (runtime75bc809), with explicit inspected leaves103 `1b4876d`,
122 `abd03717`,123 `d7c4d5f`,107 `785dfec`,119 `8ab79b4`. Exact full refs and
ancestry are in `native-startup-dependency-closure.json`.

## Source finding and invariant

`MutationContext.contract_version` already represents the original public
contract and is encoded as bytes in `control::request_hash`. The keyed index is
(storeId, stable namespace/catalog scope, key) across kinds/transports, with no
contract-version partition. Matching replay precedes fresh CAS; stale lineage
precedes replay. Keep all of this unchanged.

Several private native writers currently require contract2 and purge equates
contract2 with namespace schemaSome and progress version2. Batch private writer
versions2/3 derive contract2, including historical parent/rejection/child hash
reconstruction. Those checks are safe while private2-only, but cannot become the
ordinary API1/native route. Setting ctx2 on an API1 call would erase identity;
letting same-key API1/API2 replay each other would violate CONTRACTS.

Separate three things explicitly:

- Public originating contract: original1 or2, with omitted wire value1.
- Canonical/native writer capability: established by owned metadata mode, checked
  schema/table inventory and native mutation guard, never inferred from API.
- Operation/progress/proof format: existing native domains and private versions,
  independent of public contract and namespace lifecycle.

Use the existing `MutationContext` for the first item, existing owner/guards for
the second, existing proof types for the third. Preserve operation-domain strings
and requestHashVersion1. In particular, a domain ending `_v2` is an operation
codec domain; API1 native can use it with the original contract1 hash component.
No request field, default, provider result or missing provenance is guessed.

## Proposed shared API and minimal retained representation

Keep `MutationContext` fields and existing request hashes unchanged. Native
entry validation accepts the already-supported closed contract set1|2, checks
resolved scope, and independently requires native owner/transaction schema2.
A common context-validation function can be reused by writer modules, but must
be consumed by real routes in the same slice; it does not acquire another lock,
rewrite ctx or select storage based on API version.

Single-operation retries already supply their original API and full payload.
Retained receipts contain hash identity; creation/store proofs retain expanded
numeric defaults/profile/settings, and121 supplies immutable retired scope.
These are sufficient to recompute the exact historical hash using the incoming
context, then compare it. A hash does not let an offline tool infer the original
API or recover removed input: we do not claim that it does. Single-operation
history validation checks the retained canonical transition, not a reconstructed
full original request after erasure. Therefore **no per-record, per-outcome or
StoredReceipt extension is currently justified for standalone writes**.

Compound workflows do reconstruct requests after restart without the caller,
so their current retained context is insufficient. Reuse one shared additive
shape, provisional name `RetainedRequestContext`:

```text
RetainedRequestContext { proofVersion: 1, contractVersion: 1 }
```

It is present only for newly written native/API1 compound workflows. It has a
closed schema; unsupported proof/API values and explicit null reject. Absence
on known historical native formats retains original API2 semantics and encodes
exactly as before. Legacy progress1 still means its historical API1 codec. This
is not a default applied to unknown formats. Do not emit a redundant Some2;
reject it so canonical encoding has one representation. Shared decoding is used
by production, current readers, startup and offline validation.

Batch progress retains this optional shared context; it is anchored by hashes
that the existing validator already reconstructs. Continue using the existing
reservation_precondition for original parent CAS, with its source reservation
G+1 relationship, never current-fork G or child CAS. Do not create a second CAS
field in the shared context.

Purge requires a stronger placement: add an optional **immutable start proof**,
provisional `NativePurgeStarted.request: Option<NativePurgeRequest>`:

```text
NativePurgeRequest {
    context: RetainedRequestContext,
    force: bool,
    expectedGeneration: optional decimal-u63
}
```

It records only bounded original choices required to validate the start hash;
force and original CAS are not recoverable from current progress/effects. It
contains no memory payload. Derive child API from the canonical start receipt,
not a mutable unanchored progress flag. Validate original CAS+1 equals the start
receipt G when present. Existing NativePurgeStarted is deny_unknown_fields, so
old readers reject the new explicit extension; absent old2 bytes stay unchanged.
`PurgeProgress.version` remains native codec2, not public API1. Its embedded start
receipt must equal the canonical origin-scoped receipt, as it already must.

Why not only add a field to StoredReceipt: the inspected struct does not reject
unknown fields, so that alone would not fence older validators. Why not a new
native proof field everywhere: no demonstrated need for standalone reconstruction.
Why not only an API flag in mutable purge progress: current strict purge validation
checks transition/receipt chains but does not reconstruct stored parent hashes;
removal or alteration of that flag could silently change child identity.

## Per-writer dispatch, identity, replay and strict history

| Writer / actual source | Dispatch and hash change | Replay and startup/export validation |
| --- | --- | --- |
| NativeInsert / linked NativeStore (`metadata/native_records.rs`, `native_store.rs`) | Preserve original ctx1/2 through118/profile producer; use native owner independent of API. Hash same original payload/expanded retained defaults, settings, supplied vector/profile bits; no external output in identity. | Replay supplied original ctx+payload with stored primary numeric/profile/settings before provider/live namespace. Cross-contract same key conflicts. Existing typed creation/link proof and all-origin namespace/profile/dimension validation unchanged; no new outcome field. |
| Catalog create (`metadata/native_catalog.rs`,103 adapter) | Closed1|2 catalog ctx; native namespace schema determined by owner, not ctx. Fixed normalized defaults and optional dynamic selectors retain current hashing rules. | Original ctx+normalized input hashes before provider/current catalog. Existing NativeNamespaceCreated proof remains content-free numeric/identity evidence. New namespace default lifecycle remains legacy_access_v1. |
| Numeric policy (`native_policy.rs`, `metadata/native_policy.rs`) | Closed1|2 stable namespace ctx; native guard remains. Same normalized replacement hash and typed updated/no-change result. | Keyed equal-policy writes reserve once and replay before current policy/CAS; cross-contract conflicts without revision changes. Existing retained retired-namespace policy rewind validates history; no receipt rewrite/extension. |
| Standalone graph incl keyed no-change (`metadata/native_graph.rs`) | Curie actual producers pass original originating contract; native guard and same-namespace/final peer tokens unchanged. Same ordered edge bits/domain. | Request resubmission+ctx reconstruct hash; full/no-change proofs and reverse removed-edge history unchanged. No API field is needed in edge proof; no new general graph transport invented merely for helper usage. |
| Feedback (`metadata/native_lifecycle.rs`) | Pascal/Huygens propagate original API; quality and immutable scope same. LifecycleVersion never selects API. |121 same-guard binding and stored outcome reconstruct original name-bearing request hash under incoming ctx1/2. Missing proof retains known receipt, no current provider/policy/clock on replay. Numeric lifecycle history unchanged. |
| Forget/tombstone (`metadata/native_records.rs`) | Original ctx1/2, ID-only native domain, final structural graph and removal ledger unchanged. | Receipt lookup before assertion/fresh clock;121 stable retired name only for assertion. Existing phaseTimeRule None/Some semantics stay immutable. No proof field solely for API selection. |
| Query/get/fetch/similar exposure (119 recorder) | Carry explicit originating API through accepted query/observer context. Do not infer from retrieval semantics, lifecycleVersion or current global configuration. Reuse sampled clock and final UUIDs. | No new public idempotency promise for reads. Internal exposure operation identity includes originating API; replay records exposure only and reads revalidate current content. Per-namespace receipts/partial/unknown semantics unchanged. Existing transition proof has enough canonical history. |
| Manual/background maintenance (107) | Explicit request-owned manual operations retain original API; autonomous internal sweep has a documented internal contract2 context, not a relabeled public API1 request. | One sampled clock, final record/policy/graph recheck, content-free phase/hard-delete proof, same-tx ledger. No new proof merely for API where no reverse request hash is reconstructed. Distinguish internal no-opNone from keyed receipted no-change. |
| Namespace purge (`metadata/purge.rs`) | Select native hash domain/outcome/progress2 from checked owner/schema. Start ctx retains API1 or2. Native1 start embeds optional immutable request proof; native2 remains absent. | Replay original suppliedctx+force before live namespace/CAS. Resume derives child ctx from anchored start request, keeps stable reserved childIDs. Strict validation checks native1 start request hash and every child/cleanup hash under original origin/scope; altered/stripped proof must fail. Old2 history is validated under its unchanged legacy proof rules, not retroactively required to supply unavailable force/CAS. |
| Batch reservation/items/rejections (`batch.rs`, `metadata/batch.rs`, `batch/native.rs`) | Private writer/progress2/3 independent of originating API. New native1 BatchProgress carries shared optional context. Parent caller/resolution/reservation hashes, each child canonical hash and rejection hash all use it. Native schema guard chosen from writer format/owner, not API. | On retry compare incoming API to retained context as part of whole parent hash before current defaults/provider. Child capability derives from immutable validated manifest and is revalidated in final tx. Existing parent CAS proof retains sourceG+1; child contexts have no inherited CAS/key. Startup/export reconstruct every parent/rejection hash and child linkage with retained context. Stripped/changed context must conflict even for all-invalid/zero-success manifests. |
| System default creation / CLI importer / LoCoMo | Ordinary bootstrap is explicit internal catalog operation; real public CLI/importer passes API1 through actual adapter. Native benchmark producer uses canonical typed insertion, never fixture bypass. | Generated time/IDs come from owner and are excluded from caller hash. Do not bless old profile from current provider. Source regression identifies whether caller is internal versus public before choosing context. |
| Logical import / schema migration / restore / compaction | Offline import preserves receipts/proof/context bytes; it does not synthesize a new request contract. Compaction/internal recovery preserve their existing internal domains. | Validate new compound proof via common strict readers, retain unknown-version failure, old API2 bytes unchanged. Production restore forks storeId while preserving original receipts/context; foreign lineage always rejects before replay. Never compare original reservation CAS with fork generation. |

## Compound reader changes that must ship with the writers

For batch, audit all hardcoded2/version-to-API sites together: `BatchInput.context`,
`BatchChild.context`, caller/resolution hashes, reserve guard selection, resolved
item hash preparation, `check_child_context`, rejection writes, resumed items,
`metadata/batch::validate_existing`, native policy-history bookkeeping recognition,
current pending/readiness inspection and typed logical control copying. Format3
insert versus format2 historical/forget support stays independently checked.
Legacy API1 batch outward arrays/defaults stay compatible; native manifest fields
are not automatically new public fields. Missing original payload remains pending,
not synthesized from retained hashes.

For purge, audit begin hash/native admission, start proof, progress version,
per-chunk and cleanup contexts, strict chain/history, original-lineage request
hash checks, native2 current completed reader and retained-binding access. The
current reader remains bounded point checks, not a lifetime rescan. Immutable
start request is checked against its canonical receipt anchor; startup proves
full chain hash/counter/inventory consistency. Native/API1 active recovery is
allowed only after these exact typed checks. Offline export/backup still rejects
unfinished purge or unresolved batches, while ordinary native recovery can admit
validated recoverable plans. No generalized missing endpoint/table allowance.

The standalone proof argument above is deliberately limited: originalAPI is
cryptographically bound in requestHash1 and supplied again on retry. We do not
invent a readable API provenance field from that digest. If a real future consumer
needs to reconstruct a standalone request without caller input, it must identify
the exact missing information before proposing any further schema extension.

## Ownership and minimal parents

Feynman owns the shared retained-context codec and purge/catalog/policy core
separation, strict history/offline/current-reader review. Curie owns batch codec,
runner and insertion/graph producer propagation; Hume reviews graph invariants.
Pascal owns lifecycle/exposure/manual-maintenance propagation and original clock
semantics. Huygens owns parser/default/transport selection and legacy outward
compatibility. Halley reviews owned acceptance/readiness.

Do not edit the common control context in parallel. Freeze its small actual
consumer-used change first on121; Huygens/Curie consume it in their actual
insertion/batch join, and Pascal consumes it in lifecycle wire. Catalog/policy
core consumption can use121 because its core already exists there;103 is required
only for its real public adapter test. Purge uses121. Batch integration uses actual
122/123 plus the shared child, not an unrelated broad checkpoint. The exact
minimal shipping split will be confirmed with owners and coordinating agent
before source changes. A helper-only public PR is not the objective.

## Genuine BEFORE and RED/GREEN validation plan

Freeze pre-change executable/source manifests on actual old2 leaves before
building corrections. Use fresh synthetic native stores, private source-specific
targets, no personal data/provider/network/credential mounts. Record source/tree,
commands/toolchain/image, executable hashes and every canonical file hash.

The BEFORE fixture must contain real old2 keyed catalog/policy and keyed no-change,
linked store with settings/profile, graph/no-change, feedback/forget/exposure,
maintenance loss, completed purge, and separate completed/pending batch fixtures.
Where a minimal writer branch lacks a sibling, use an isolated validation join;
never import its broad ancestry merely to obtain a fixture. It must be authored
and opened by the actual old binary, not current code serializing None afterward.

Required probes:

1. Actual oldAPI1/native entry fails or reaches its legacy guard BEFORE; after
   correction, native canonical outcome and unchanged lifecycle/default payload.
   Cover each real MCP/REST/RPC route and CLI path, not only metadata helpers.
2. API1 keyed request commits once; retryAPI1 with original staleCAS replays full
   original receipt. Same key+same payload API2 conflicts, and reverse direction
   conflicts. Provider/clock/allocation/currentG/bytes remain unchanged on replay
   and conflict. UnkeyedAPI1 also routes correctly; no fabricated idempotency.
3. Restart and post-purge/name-reuse replay use original immutable namespace and
   retained expansion; foreign restore storeId fails before replay. Current-key
   retries do not resurrect removed content. Every retained record/receipt/f32
   bit/profile/provenance survives logical export/import/backup/fork.
4. Old2 fixture opens/replays after correction with exact canonical receipt,
   proof and progress bytes unchanged; None fields stay absent. Current writer
   emits identical old2 encoding/hash for the same request. New API1 optional
   compound proof is rejected by the genuine older binary, not silently ignored.
5. Reject null/future/unknown/double/surplus shared context, Some2, wrong native
   format, API1 context stripped or changed, wrongforce/CAS/startorigin, mismatched
   parent/child/resolution/rejection hash, and missing progress anchor. Malformed
   repaired-copy preflight leaves originals unchanged with both ownership locks.
6. Interrupt after compound reservation, child commit, rejection commit, purge
   chunk/cleanup, and before derived publication. Replay/resume retains IDs and
   originalAPI; no double mutation or empty success; exact full receipts and
   truthful ready=false/unknown state. All-invalid batch still binds the key.
7. Join actual readiness and full consumer closure: API1 changes catalog/policy,
   lifecycle, graph and manifests advance/invalidate actualG even without
   effects_epoch changes. Bounded current-reader validation does not downgrade
   old or new proof validation. Full fmt/Clippy/tests and actual bench/example
   compile on the frozen final tree only after focused failures pass.

This document is a reviewed-plan candidate. No old receipt rewriting, schema
allowlisting, current-profile blessing, second engine or API1-to-API2 coercion is
permitted as an implementation shortcut.

## Coordination evidence

Pascal explicitly confirmed standalone feedback/forget incomingctx+retainedscope
is sufficient and exposure must carry explicit transport/application origin,
not lifecycle/retrieval inference. Current lifecycle wire is private2 only and
will not falsely claim API1-native support. Huygens confirmed public force/name/
error projection ownership; current native core seam remains unchanged pending
this review. Curie's current profile+121 validation remains undisturbed. All
proposed names above are design candidates, not claims of implemented APIs.
