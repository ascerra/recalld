# Original application contract through lifecycle observation and maintenance

Status: caller inventory and interface proposal,2026-11-14. No runtime change in
this branch yet. PR126 remains frozen at179edf0 (runtimed00cebf). The shared
1|2 guard and compound request-context codec are Feynman's sole ownership;
implementation consumes his validated actual consumer, not a parallel type.
Curie owns the narrow native_records context/child-binding consumer, on that
common parent only. Huygens owns public version selection/outward compatibility;
Hume owns the A2 selector/ranking changes. Pascal owns the lifecycle request and
observer/manual propagation described here.

## Inspected actual leaves

- PR126: `179edf0d14d383466253a62b892c3741b5d4d9b1`, runtime
  `d00cebfdfc6ca67586c9fbb5d329243028b1c6e0`, actual121+88. Native feedback/forget
  private transport, current ready-success barrier, no public2 activation.
- PR119: `8ab79b45cf53f277f96c587808615273b0055882`, runtime
  `3d8b0090ab3db3cc2c305d06d1c4ffe2a3e1e6c5`, actual113+116. Same configured
  query policy/sample, final-UUID grouping and accepted native exposure.
- PR107: `785dfec4` (actual96+98), runtime38e8598. Existing startup/timer/manual
  runner consumes native maintenance on the same owned storage/registry.
- Feynman's approved plan40720153 is a plan, not a frozen runtime dependency.
  Curie's narrow native insert/forget child is approved but not frozen yet.
- Hume's legacy delta candidatee78c5e1/docs62a uses96+116+117. It is not imported
  for convenience; preserve its optional sealed-delta changes when actually joined.

No broad union of these leaves is required to begin independent implementation.
Each runnable slice takes only its actual consumer parents.

## Invariant and representation

Reuse the existing `MutationContext.contract_version: u32` and its request-hash
component. The closed supported values are1|2, already validated by
`MutationContext::validate`. Feynman confirms there is no new duplicate validator:
module owners remove their API2-only restriction while retaining scope checks
and the existing independent native guard/control proof, on his validated actual
consumer. His new request_context.rs codec is for compound Some1/absent-old2 only. Do not invent another public protocol selector, origin enum, control
record, proof field or receipt codec for standalone lifecycle operations.

The existing `serialization::contract::ContractVersion` parser is the sole public
MCP/HTTP selector: missing means1. Carry its numeric result explicitly through
private request/observer parameters. Never derive it from RetrievalSemantics,
namespace LifecycleVersion, record schema, native owner mode, current provider,
or which planner branch runs. Application1 can target explicit_feedback_v2;
application2 can target legacy_access_v1.

A native capability/schema check is independent of that number. Legacy wrappers
must not silently execute native writes or relabel contract1 as2. Same-key
cross-contract requests conflict because the original number participates in
identity; matching same-contract replay precedes fresh CAS/policy/clock. Existing
old2 receipts and their bytes stay unchanged. No new public keyed-query promise
follows from internal exposure receipt hashing over selected UUIDs.

## Caller inventory and intended ownership

| Actual caller and current path | Current origin loss/default | Required explicit propagation |
| --- | --- | --- |
| MCP `tools::dispatch_tool` then prepared `dispatch_v2` | Contract parser selects1/2; public2 remains gated | Huygens retains selection and v1 outward shape. Only the selected number enters the native request. |
| REST `api/native_lifecycle::{delete,feedback,dispatch,prepared}` | Existing v1 handler or explicit2 prepared body | Huygens selects writer/outward shape; Pascal's typed request preserves that origin to the same native coordinator. Existing v1 unknown-field/default behavior is not replaced with the strict v2 parser. |
| `serialization/native_lifecycle::{Request,parse,wire}` | Request lacks an origin field; public prepared parser requires2; private wire always writes2 | Add an explicit private numeric original-contract field with no inferred/defaulted native value. Public prepared2 parsing stays strict2. Private RPC requires an explicit supported number, and serializes exactly the saved number. |
| `mcp/native_lifecycle_adapter::{dispatch,execute}` and API/MCP adapter ports | Executor builds MutationContext with ContractVersion::V2 | Use the request's original validated number. Keep scope lookup, accepted root, matching replay, clock, name proof, known receipt and barrier ordering unchanged. |
| `daemon/bridge_adapters::RemoteStorageAdapter` and `daemon/server::dispatch` native aliases | feedback_native_v1/forget_native_v1 currently require explicit application2 | Preserve private RPC codec1 alias, original application1|2 in its typed request, no old-method fallback. Older private2-only handler rejects new1 before work. |
| `derived_completion::{native_feedback,native_forget}` | Already receive caller MutationContext unchanged | Do not rewrite the context. Feedback consumes Feynman's guard change; forget waits only for Curie's narrow actual native_records consumer. |
| `metadata/native_lifecycle::validate_context` | Explicit contract2 check before real native guard | Reuse existing MutationContext::validate on Feynman's frozen consumer; no competing1|2 implementation. Capture/apply/hash/proof use existing inputs. |
| `metadata/native_records::context` and child binding | Shared insert/forget check currently requires2 | Curie owns the change and actual insert/forget proof. Pascal consumes its frozen minimal child. |
| QueryEngine `search/search_with_semantics/search_checked_with_work/search_admitted` | RetrievalSemantics is explicit; application origin is absent | Add explicit private application-origin threading independently of semantics, including deferred selection. Hume retains A2 ranking/eligibility fields. |
| QueryEngine get/similar, coherent payload methods and pinned selection | Request's access intent and sampled query time are preserved, but application origin is absent | Carry the selected origin to the eventual observer; pinned RejectExposure must still reject every write. Do not turn recordAccess=false into observing work. |
| MCP search/get/similar in `bridge_adapters.rs` | Final hydration invokes QueryEngine record_exposures[_at]_with_work; legacy fallback calls observation directly | Carry the original selected number with the existing accepted parent and sampled time through both call sites. No session default/name inference and no second clock. |
| REST search/get/similar in `handlers.rs`, `state.rs`, `adapters.rs` | Prepared coherent or legacy selection is followed by record_coherent_exposures/record_exposures/record_similar | Same explicit origin on selection and final observation. Keep existing pre-exposure readGeneration; never replace it with a later capture. Huygens owns wrapper dispatch/response changes. |
| Daemon search/get/similar and remote query adapters | Existing legacy RPC carries no application2 selector | Treat the existing legacy entry as application1 explicitly. A future native2 query RPC must be the actual Hume/Huygens selector contract, not schema/semantics inference or silent legacy fallback. |
| QueryEngine `record_accesses`, `record_query_exposures`, `record_exposures_sampled` | Policy and optional sampled time flow to recorder; original contract does not | Forward a required numeric origin along this one observer path. Preserve inferred-sample forwarding only for native-capable recorders; explicit_at still preserves caller time. |
| `AccessRecorder::record_access_batch_for_publication` and StorageAccessRecorder implementation | Native branch calls accepted coordinator; legacy branch retains old recorder dispatch | Add explicit origin to the existing publication call, not a second observer. Never silently downgrade an unsupported origin through an old-only recorder. Preserve Hume's sealed-delta output and nativeNone/full fallback. |
| `derived_completion/native_exposure::{record_native_exposures_with_work,native_exposures_accepted}` | Creates each namespace's unkeyed ctx then hardcodes contract_version=2 | Required origin reaches each existing per-namespace context. Final returned UUID dedup/direct-dominance/grouping, sampled ManualClock and partial receipt handling remain unchanged. |
| `decay/observation::record_locked_scoped_with_work` | Legacy helper creates MutationContext::unkeyed, currently1 | Existing legacy convenience calls retain1. Any new explicitly versioned route must preserve its number in the same helper or reject unsupported origin before writes, never silently drop it. Coordinate this narrow input change with Hume; no delta algorithm change. |
| `api/handlers::trigger_decay_sweep` → `FsrsEngineAdapter::trigger_sweep` | Public v1/manual entry samples existing clock then calls common runner without application context | Explicit selected application1 for existing request; future2 only from Huygens selector. Native branch must use the already configured NativePolicyInputs, not create another default policy source. |
| `DecaySweepRunner::start` startup/timer | Captures same injected clock and registers accepted pass; no public request | Explicit internal2 at these autonomous call sites. Do not infer it from schema or reuse a public omitted selector. |
| `DecaySweepRunner::trigger` notification and pending ticket | Coalesces a ticket only; carries no application request | Keep internal-only unless a real public trigger is wired. A request-owned asynchronous trigger would need its context retained with the ticket and explicit coalescing rules; do not invent those now. Existing HTTP manual route executes directly. |
| `execute_sweep_with_work` → `execute_native_sweep_accepted` in sweep/native.rs | Native loop creates unkeyed per-record context2 for both timer and manual | Thread required origin through the existing runner; autonomous caller2, request-owned caller1/2. No new scheduler or planner. |
| `DerivedCompletion::apply_native_decay_accepted` | Already accepts MutationContext | Keep it unchanged; metadata maintenance guard consumes Feynman's validated helper when ready. |

## Proposed private interfaces to agree before edits

Use a required `original_contract_version: u32` on the existing private lifecycle
Request and on the existing observer/runner call path. Do not add it to public
SearchQuery JSON or create a second public selector. Reuse the existing `MutationContext::validate`; Feynman confirmed no new shared
validator function is needed. His frozen actual consumer establishes the revised
admission/proof seam before its other module owners remove API2-only restrictions.

1. Lifecycle: preserve `parse` as strict prepared-public2. A shared private
   parser/invoker accepts an explicit supported original number for the native
   aliases; public1 normalization remains Huygens's existing legacy path. The
   private Request transports the number and builds MutationContext from it.
   Method aliases, original target/default expansion, retry hashing and receipts
   do not change. A private RPC missing its application number fails closed.
2. Observation: extend the existing publication recorder method and native
   accepted coordinator with the numeric origin, and thread it through the
   existing QueryEngine observation helpers. Default trait behavior may retain
   the old1-only dispatch; it must reject any origin it cannot preserve rather
   than silently discarding2. The real storage adapter can pass the explicit
   number to the same canonical legacy/native implementation as appropriate.
   Existing unversioned legacy helper signatures continue to mean1.
3. Query entry: transport-facing private methods require the parsed number.
   Rust compatibility query entry points without a negotiated transport are
   explicitly documented as application1 unless the caller uses the explicit
   origin entry. `search_with_semantics(V2)` alone does not mean application2.
   Internal native exposure fixtures/consumers deliberately selecting2 pass it
   explicitly. No mutable process-wide or QueryEngine-wide per-request origin.
4. Sweep: append required origin to the accepted runner/record producer path;
   startup/timer pass2, request-owned manual execution passes the parsed number.
   Existing internal trigger remains explicitly internal. Snapshot clock,
   captured IDs/tokens, donor/fan revalidation and partial-completion receipts
   remain the current implementation.

The public missing-selector default1 is agreed with Huygens. The precise Rust
compatibility-entry/default-trait adaptation above is a proposal to confirm with
Hume/Huygens before source edits, so old-only recorder fixtures are not accidentally
bypassed as in the preserved PR119 regression. Never infer a fallback from owner
schema or retrieval semantics merely to keep a test green.

## Minimal runnable sequence

- Lifecycle feedback origin: PR126 + Feynman's validated actual common consumer.
  Real private request/RPC→coordinator→metadata application1 can be tested without
  waiting for all public routing work. Preserve prepared-public2 and ordinary
  serving gates. Forget1 adds Curie's narrow native-record child when frozen,
  not his broader insertion/batch wire ancestry.
- Exposure origin: PR119 + the actual common consumer. Apply the same agreed
  explicit request parameter to actual selection/observer callers. Hume's A2
  selector and Huygens public wrappers join when their actual APIs are ready;
  independent internal/adapter proofs need not wait for unrelated wire leaves.
- Manual origin: PR107 + the actual common consumer (and the concrete adapter
  dependency only if that adapter is exercised). Request-owned versus autonomous
  origins run through the same runner. No need to import exposure or all public
  writers just to change this actual consumer.

Do not change published126 while waiting. Current planning worktree is
`/tmp/recalld-implementation/native-origin-propagation`; split code worktrees
around actual dependencies when each interface is frozen.

## Keyed maintenance constraint and non-goals

Internal NativeDecayResult::Skipped is unkeyed diagnostic, with no receipt/key
binding. Preserve unchanged/stale/exempt/missing-summary diagnostics. A public or
background wrapper must not promote a supplied key plus Skipped into durable
successful keyed no-op: graph/policy A→B→retry could otherwise execute later.
Either reject an unsupported keyed workflow before effects or consume an actual
validated bound-noop/manifest design; this slice does not invent one. Existing
manual v1 has no new keyed sweep promise.

No public keyed search is added. Selected UUIDs are exposure outcomes, not stable
caller query intent. Any future keyed query must look up its original caller
intent before fresh selection and then rehydrate current visibility. No new
provider routing, whole-store generation claim, deterministic UUID/RNG claim,
receipt rewrite or ordinary native serving activation is included.

## Validation required for each actual code slice

- Preserve a genuine old2 source/executable fixture before changes; reopen and
  replay its unchanged receipt/proof bytes after the shared guard consumer.
- Actual private application1 commit, matching1 stale-CAS replay before clock,
  same-key1↔2 conflicts both directions, unkeyed1, exact original namespace after
  purge/name reuse, wrong restore lineage. Provider/clock/allocation counts and
  canonical bytes must remain unchanged on replay/conflict.
- Exposure cross product application1/2 × lifecycle legacy_access_v1/
  explicit_feedback_v2 × retrieval semantics1/2, through actual configured
  query/observer. Check the receipt's original request-hash context separately
  from anchors, sampled timestamps and scores. Do not infer origin from results.
- recordAccess=false preserves bytes/history/generation and queues no observer
  work under either origin. Duplicated final UUIDs, mixed namespaces, partial
  receipts, unknown operation identity, readGeneration and completion retain
  existing tests. Keep native deltaNone/full fallback and old-only legacy dispatch.
- Manual1 versus autonomous2 has one clock sample per pass, identical eligible
  phase/strength behavior under the same policy, real current barriers, and no
  silent binding of skipped keyed attempts. Preserve graph stale-projection,
  terminal/no-resurrection and exact retained raw timestamps.
- Unsupported origin0/3, missing private origin and malformed encoding fail
  before writable work/clock. Native schema ownership is checked independently.
- Focused real migration/transaction/publication/reopen probes first, then full
  offline Docker/fmt/Clippy on exact frozen source. Public ready/activation claims
  require the actual Huygens/Hume routing join, not these private proofs alone.
