# Native owned controls: bounded integration plan and source inventory

2026-11-14. This is a source audit and implementation plan, not native activation
or a combined-runtime result. Planning branch `impl/owned-native-controls` starts
at exact PR128 `5deb840a21ad5f1f80006e0d05a6372919292acf`; it changes no runtime.
PR128 and its measured source/evidence remain frozen. CONTRACTS and
INTEGRATION-REVIEW are authoritative; A6's snapshot requires zero accepted and
durable pending work, coherent actual generations, and controlled runtime state.

## Frozen inputs and unjoined prerequisites

| Input | Exact inspected reference | What it proves / does not prove |
| --- | --- | --- |
| Owned native allocation, PR124 | `a5865da02969631a5031f65c6e43269701d7a254`, measured `b1ebb6892b428f65b81ab52499d817b4c6d41d19` | Actual104+118; generated native single/batch identities, cursor/replay/conflict and postcommit recovery. Native fixture prefix is supplied; no deterministic native genesis/reset or ordinary constructor. |
| Namespace clock, PR125 | `0b50168b64ad353e4f7a53c6459c270fe8500070`, measured `535605d3c1ecc38f915dbe8daccbae9d5a0f0d0b` | Legacy namespace response/row/receipt use the existing core Clock. Must survive native adapter composition; it does not activate native catalog. |
| Explicit legacy sweep, PR128 | `5deb840a21ad5f1f80006e0d05a6372919292acf`, measured `42d39a4e80996706fdd1c74a24b2cb8df7b17382` | One existing runner/core/clock/parent/completion, late status and invalidation. Native sweep explicitly rejected. |
| Isolated legacy checkpoint, PR115 | `0750f13b69c3e7c6e1f6650dd26e6bb94896e15f`, measured `2920dec86c3d9f7a0788ef01433a3ab4f9093bab` | Real closed capture/fresh owner/A1 ancestor locks. Hash/track and constructor deliberately legacy-only. |
| Actual reset supervisor, PR127 | `31955c3dde7f0219f2ebcebb36f57a35476038af`, measured `95540eedd7764555c42e03dd006da8dfb35290c7` | Actual120+125 including115/117; strict named independent hashes and three resets. Helper owns its128 chronological child. |
| Native lifecycle wire, PR126 | `179edf0d14d383466253a62b892c3741b5d4d9b1`, measured `d00cebfdfc6ca67586c9fbb5d329243028b1c6e0` | Actual121+88; gated native wire, not ordinary native startup/API1 admission. Contains native catalog/policy/purge core needed for this audit. |
| Native sweep, PR107 | `785dfec4a543b585e3924eb9df298b0d0708d84d` | Actual runner native dispatch with sampled time and accepted completion; not inherited merely by124/128. |
| Ordinary startup/context plan | `40720153a1abd42796b58d3da1f0c1e910588f57` | Feynman confirmed PLAN ONLY; no frozen ordinary native constructor activation. Current context/purge/catalog child is under validation/correction, not a shipping input. |

Huygens's inspected insertion-wire `c9f4654` and later readiness WIP `872d6e3`
are source observations, not frozen parents. He reports actual122+123+observer,
with no owned-ID/128 convenience ancestry. Await his final source and recompute
ancestry after Feynman's common original-request-context consumer freezes.
Do not ship a broad validation checkpoint or merge every row for convenience.

## Independent gates and actual constructor

| Gate | Required behavior before a capability can be enabled |
| --- | --- |
| Isolated fixture process | Keep explicit absent destination, trusted inherited private pipes, actor/evaluator separation, loopback synthetic provider and source/config pinning. A fixture option grants no schema/API authority. No existing personal root. |
| Disk/schema admission | Existing ownership locks and strict disposable-copy preflight choose native format from actual canonical state. Unknown/malformed/historical read-only state rejects. No boolean inferred from API2, no unchecked migration and no missing-table allowance. |
| Core construction | Reuse `Recalld::with_runtime_sources` with the same engine, ManualClock, MetadataStore IdSource and registry. Feynman owns real mode selection/default bootstrap/recovery. Current124 system.rs calls `open_with_ids(..., false, ids)`; private `NativeLifecycleSession::open` calls gated `open_native` with SystemIds. Neither is an owned ordinary native constructor. Do not open a second session beside Recalld. |
| Namespace bootstrap | Native default creation must use the real native catalog owner, configured embedding profile and shared clock/IDs. New/migrated namespaces retain `legacy_access_v1`; lifecycle migration is explicit. Existing legacy system default has schemaNone and cannot be reused unchanged on native disk. |
| Original public API | Preserve omitted/explicit API1 and explicit API2 independently of disk/lifecycle/retrieval. The inspected pre-context native guards require contract2 (inventory baseline only; Feynman/Pascal standalone API1 guard removals are underway and their final source is not yet frozen); rewriting an API1 caller to2 would corrupt request/replay identity. Feynman's common context plus actual Huygens/Pascal consumers must precede ordinary native control claims. |
| Retrieval and observation | Actual coherent query/payload owner and returned-only exposure retain pre-read G and committed receipts. Selection semantics/current-history/RIF are independent gates; native disk does not certify public-v2 retrieval. A6 actor reads explicitly pass false. |
| Runtime readiness | Actual canonical GenerationRef must match all four required component stamps; catalog/policy/receipt-only G counts. Drain accepted descendants and durable pending effects/purges/batches. No manual stamp blessing, dirty-flag zero, extra registry or generic retry after committed failure. |

A native owned constructor must inject sources before any bootstrap allocation,
recovery or namespace creation, then finish actual derived publication before
actor listeners. Reopening a fixed native prefix with seeded IDs is useful
consumer evidence but does not prove independently reproducible genesis.
The owned fixture constructor remains separate from production fork/restore
semantics. No generic `nativeControls: true` capability will be added as a proxy
for passing only one row.

## Concrete clock and identity inventory

References below are tied to the inspected frozen tree, not moving line numbers.
Source findings are not reproduced runtime failures unless prior evidence says so.

| Actual site | Coverage / concrete remaining consumer |
| --- | --- |
|124 `storage/ids.rs`, `metadata.rs::record_receipt`, `control.rs::record_receipt_with_ids` | MetadataStore owns one cursor. Memory UUIDv7 uses supplied operation time; operation/store/text-generation allocations use the versioned source. Preserve exhaustion/range/collision validation. The free `control::record_receipt` intentionally still falls back to SystemIds, so new callers can bypass owned allocation. |
|126 `metadata/native_catalog.rs:84`; `metadata/native_policy.rs:62,143` | **Concrete uncovered join:** these new files are absent from124 and directly call free `control::record_receipt`, including keyed numeric-policy no-change. A conflict-free merge cannot redirect them. Use the existing `self.record_receipt` wrapper in the actual consumer, with create/update/no-change/replay/cursor tests. Feynman acknowledged this and assigned this adaptation to Halley after common-context freeze. No duplicate allocator. |
|124 `metadata/native_records.rs:307,334,595` | Actual generated memory UUID and native record use one passed Clock sample. Replay/terminal/explicit-ID handling remains ordered. Native store links are in the typed producer; do not route back through legacy post-store helpers. Test supplied IDs, generated IDs and resumed children, not only hand-selected UUID fixtures. |
|124 `metadata/batch.rs:533`, `metadata/purge.rs:376,503` | Reserved child and purge continuation operation IDs already use the MetadataStore source on124. Later122/123/126 trees predate that adaptation; their direct Uuid allocations/free receipt calls must not replace it at joins. Preserve original API context, parent CAS, retained manifests and pending-child identity, including all-rejected batches. |
|126 `metadata/native_catalog.rs:50` | Actual native namespace creation samples its passed Clock after replay lookup; namespace integer IDs use checked canonical max_id. Wire nested catalog owner must receive the exact system Clock Arc.125 covers the independent legacy adapters, not this nested constructor automatically. |
|Huygens inspected c9 `mcp/bridge_adapters.rs:2222`, `api/adapters.rs:1492` | Legacy namespace creation still uses Utc on that independent branch before125 is joined. This is the already-fixed125 ancestry gap, not a new regression. Preserve both legacy `.with_clock(system.clock)` and nested native catalog injection. |
|107 `decay/sweep.rs`,126 `derived_completion/native_decay.rs` | Native pass already accepts sampled time, final policy/record/graph rechecks and parent work.128 always supplies Some(now), but rejects native. After real native constructor/107 join, retain same completion and deadline/receipt handling, then prove both lifecycle policies. No second planner or implicit sweep on advance. |
|126 native feedback/forget/exposure/maintenance | Use passed Clock or captured sampled_at and immutable policy. Preserve corrected monotonic phase marker and original exposure API context. Final consumer tests must prove no new clock/IDs/provider calls on keyed replay/conflict and retain complete receipts on publication failure. |
|124 `engine/text_compaction.rs:68`, `text/generation.rs::create_generation_with_ids`, `metadata/text_generation.rs` | Selected text generation and receipt IDs use the owned source. Temporary `.text-gate.<Uuid>` at generation.rs176 is filesystem staging identity, not a canonical generation; retain safety/cleanup and do not rewrite production identity merely to hide paths. Exercise actual compaction at reset boundaries. |
|124 `storage/schema_migration.rs:346` and free receipt call | **Uncontrolled native genesis operation:** migration_id is new_v4 and schema migration uses default free receipt allocation. An offline migrated fixture has a real frozen prefix; it cannot establish cross-initialization native equality. Either supply a genuinely frozen validated prefix with that scope, or implement a separately owned migration/genesis source consumer without changing ordinary migration defaults. Do not fake provenance from current provider settings. |
|124 `bench/locomo.rs:164,1274` | Dataset helper samples Utc and generates MemoryId::new before canonical insertion with Some(id). Canonical writing alone does not control identity/time. Any native dataset adapter must use the owned actor/producer allocation path or a separately proved mapping; current public synthetic supervisor does not use this helper. |
|124 `graph/explicit_links.rs:281`, `graph/autolink.rs:428,567,691`, `graph/structure.rs:407,691` | Concrete SystemTime calls remain in legacy explicit/autolink persistence and in-memory add/bridge logic. Initial owned track disables autolink. Audit reachability from actual native link producers and rebuild before any full-clock claim; transient timestamps are not automatically canonical, but explicit_links persists one. No global replacement and no claim these are all reachable native bugs. |
|124 `metadata/lifecycle_updates.rs:117` | The new_v4 hit is inside `#[cfg(test)] prepare_legacy_patch`; it is **not** an uncovered production writer. Tests-only IDs elsewhere are similarly excluded from the production gap list. |
|124 `backup/restore.rs:157,158`, `logical_export.rs:376`, backup timestamps | Production fork operation/store IDs, export ID and backup timestamps intentionally remain ordinary random/wall-clock operations. A6 same-lineage cloning must not call production fork as its reset. Native logical hashing should reuse the typed walker/hash without generating an export envelope or changing restore semantics. |

## Closed native capture and fresh-owner restore

The existing115 pipeline is real and should be extended, not replaced: retained
controller permit → same owned freeze → actual G/all component stamps → sync
selected payloads → explicit canonical inventory → disposable-copy validation →
strict OfflineSnapshot → typed logical hash → publish manifest only after success.
`engine/checkpoint.rs::stage_frozen_canonical` currently calls
`isolated_legacy_hash`; that function explicitly rejects schema other than1.
`bench/checkpoint.rs` pins a legacy track, calls that hash before/after clone and
opens the legacy constructor. Simply widening a manifest string is insufficient.

Native capture must use the existing schema2 typed logical walker/hash domain,
including canonical labels/profile/lifecycle state, receipts/context proofs,
namespace/policy state, removal ledger, batches/purge history and actual vectors.
Factor a consumer-used non-writing hash entrypoint if needed; do not export a
random header or normalize arbitrary UUID/time/receipt differences. Preserve
cooperative checks in long loops and strict offline history checks; no hard
30-second guarantee across redb/OS I/O.

Manifest validation must bind independent disk/schema, lifecycle/policy and API
track versions plus native constructor eligibility, allocation source version,
clock/cursor, actual G, source/config/executable/lockfile, index versions/rebuild
inputs and declared cache policy. Unknown versions, unsupported clock/cursor,
unknown required proof/context and source/profile mismatch reject before publish
or writable destination. No opaque task queue or pending batch is cloned.

Restore retains115 safeguards: prior child/workers joined; real A1 source and
ancestor locks held through the new owner's lifetime; fresh absent sibling
root; explicit allowed canonical files, hashes and full strict validation before
atomic no-replace install; original storeId/G preserved only for this isolated
evaluation clone. Open the actual admitted native owner with restored sources
before any allocation; rebuild all four views with117 correction; verify exact
logical state/G/cursor and readiness before actor exposure. Retained source
locks, terminal IDs and historical proof must not be weakened for convenience.
Native repaired startup recovery and offline closed snapshot remain different
operations: validated recoverable pending work may be admitted by startup, but
ordinary evaluation snapshot requires all such work resolved first.

Timeout/unknown sweep invalidation from128 must continue blocking capture,
advance and sweep; inspectable status retains the known receipt/uncertain ID.
A canceled caller can recover completed status when retained work succeeds before
its deadline. No source publication/reset while a timed-out worker still owns
leases. The controller never registers a root that it subsequently drains itself.

## RNG, caches and ordering

The source is scoped, not a process-global random seed. Current system.rs413
hardcodes provider `cache_embeddings:false`; do not invent a rejected config
field. CacheManager is emptied under freeze, but every newly admitted constructor
must prove the same provider/cache behavior. Audit warm snapshot reload, graph
state, vector/FTS/entity rebuilds and query caches against actual suffix cost,
provider trace, eligibility, scores and ordering.124 cache/warming.rs188 records
wall time and priority ties without an ID tiebreak; control/disable actual loading
before promoting an empty-cache policy. It is not evidence of a current owned
constructor bug by itself.

OpenAI/Bedrock retry jitter (`embedding/openai.rs:168`, `bedrock.rs:131`) uses
randomness; HTTP request IDs (`api/middleware.rs:28`) and MCP session IDs
(`mcp/http_transport.rs:477`) are transport identities. The named offline fixture
must fail closed on replay cache misses/provider faults and declare whether
nonbehavioral transport metadata is excluded. Never hide changed provider work,
canonical receipts or identities with generic normalization. Real request IDs
can be retained in raw trace without demanding byte-identical network envelopes.

Equal-score/neighbour HashMap ordering and cap-before-filter legacy behavior are
separate determinism gaps. Full native promotion requires the actual A2/native
consumer's declared UUID tie rule and query read/payload proofs, not seeded IDs
alone. Hume owns selection work. Keep FTS117's actual rebuild correction; compare
raw scores/order, not only repeated ready stamps.

## Minimal implementation sequence and proof

1. **Next concrete owned-allocation consumer:** actual124 plus Feynman's final
   common-context/catalog/policy/purge child (which subsumes121), after he freezes
   it. The exact second SHA is deliberately pending, not guessed from WIP.
   Adapt the newly introduced catalog/policy receipt callers to the existing
   MetadataStore allocator. Preserve all new originalAPI/compound proofs and
   existing124 batch/purge/text allocations. No115/128/public transport ancestor
   is needed merely for this narrow storage consumer. Prove actual native create,
   numeric update, keyed no-change, same-key replay/conflict, same cursor across
   reopen and full known receipt after publication failure; count clock/provider
   calls and allocation consumption. Existing SystemIds behavior stays default.
2. **Actual native owned constructor/control join:** use Feynman's validated
   ordinary native startup as primary runtime, plus only missing124 allocation
   consumer,125 clock,128 controller and107 runner inputs. Consume actual final
   Huygens/Pascal public adapters and A2 reader/exposure closure where needed.
   Recompute git ancestry rather than importing integration history. Owner/source
   freeze must precede edits in shared system/engine/metadata files. An internal
   private native session is not the final public-action consumer.
3. **Native closed checkpoint/reset consumer:** add only the required115 capture/
   restore and117 rebuild fixes to that actual runtime; helper owns the supervisor
   and manifest/scorer/actor separation. Same core/constructor/registry, actual
   public STORE/QUERY/feedback/forget/catalog/batch/maintenance as independently
   enabled. Start with a named frozen native prefix if genesis remains uncontrolled;
   keep independent-genesis capability false until the real source gap is closed.
4. **Validation before promotion:** focused actual constructor/admission negatives;
   API1 versus2 request-identity/replay tests; both lifecycle policies and monotonic
   clock cases; pending effects/purge/batch rejection; graph/compaction/accepted-child
   latch→deadline→blocked advance/reset; malformed manifest/source/proof/cursor;
   retained old-worker locks/fresh destination; actual chronological one-phase
   sweep and full committed-error receipts. Then same-source full/fmt/Clippy and
   actual public HTTP/MCP/daemon controls, followed by original plus three fresh
   restored suffixes and strict independent typed hashes/raw scores/provider traces.
   Distinct source targets or own package-clean; private offline2CPU/4GiB Docker.
   No heavy jobs during an announced Hume window.

A future capability reports only its validated track. Native disk, one successful
internal sweep, a matching seed or a ready registry does not certify full native
controls. Remaining A6 dataset/scorer contracts, richer chronological fixtures,
public schema fault matrix, human calibration and all relevant runtime gates
remain explicit; no models/training or quality results are authorized here.

## Coordination at this freeze

Feynman confirmed40720153 is plan-only and owns ordinary admission/bootstrap;
he acknowledged the new native catalog/policy free-receipt allocation gap and
assigned its owned-source adaptation/proof to Halley after common source freeze.
Huygens confirmed his actual batch adapter receives the system Clock and reuses
the existing six completion handles; bodygate2 is still closed and his current
readiness wrapper is WIP. He will supply a frozen consumer, not a second registry.
Helper continues actual127+128 legacy chronological reset work and must not widen
its track to native based on this inventory. No peer source was edited here and
no new build, test, benchmark or large copy was started for this source-only plan.
