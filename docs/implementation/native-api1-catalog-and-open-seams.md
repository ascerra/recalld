# N1 concrete catalog and format-open seams

Implementation worktree: `impl/native-api1-admission`, exact composition
`5e6f163bd104112f5a1b3d2573a8567383d0ecac` (actual103/main `b374f07e` plus frozen
E1 `6ebfd250`). Halley released the named N1 runtime paths; Huygens confirmed
disjoint local catalog ownership. Client/server and outer MCP dispatcher hunks
remain with Huygens. The historical proposals and review provenance below are
retained; this paragraph records the subsequent implementation release.

The first source slice adds private format selection under both existing locks,
then invokes the unchanged full selected preflight. Ordinary `open` and the
application constructor remain legacy-gated until route/wire closure. The format
selector repairs only a disposable metadata copy and reads header/table shape,
not lifetime receipts; it adds one metadata-copy inspection to detected startup,
with constant table-enumeration state. It is not independently publishable as
ordinary native admission. Four focused tests are authored; execution is pending.

Source correction to the earlier recovery-order concern: on this exact base,
`recover_text_compaction` only rejects legacy artifacts and performs no writes.
Detection still precedes it, without a writable-open-and-retry fallback. Existing
restore-journal recovery is separately authorized by `acquire_recovering` and is
not claimed to be immutable inspection.

Source-only proposal, 2026-11-16. Feynman sole implementation owner; Huygens wire
review, Pascal E1 completion owner, Halley literal runtime/hunk release. No runtime
edits/builds/activation. Pin actual103 source
`086f827c7c721ca3304df3f22e9a641f4c12f726`, not a moving queue checkout.
This refines [N1](ordinary-native-admission-plan.md); no112/184 dependency.

## Catalog transaction and identity

Keep `NativeCatalogAdapter::create(request)` as the existing v2 wrapper. Extract
its existing accepted worker to receive explicit `MutationContext` and
`NativeNamespaceInput`, plus optional bounded publication/frame admission.
The API1 adapter constructs Catalog scope/original1 from its existing parsed
`CreateNamespaceInput`; no public key/CAS fields are introduced. Fixed numeric
defaults expand as today; omitted dimension/profile selectors remain omitted.
The producer's existing native owner guard and original hash remain authoritative.

Add bounded internal variants of `create_native_namespace_with_provider`,
`create_native_namespace` and `create_native_catalog`; old wrappers pass None.
First perform existing exact replay/CAS lookup before provider/profile/clock.
Inside metadata `create_native_catalog`, the exact gate is **after**
`record_reserved_receipt` constructs the full candidate and **before**
`prepare(&config)` and `commit_catalog`. At this point all candidate record/control/
receipt changes are private to the redb transaction, and the final namespace
directory has not been staged/renamed by `publish_namespace`.

The gate counts actual complete candidate encodings, not a static profile maximum.
Rejection aborts the transaction: no namespace/controlG/receipt/required-feature
change, no final directory or vector installation. A clock sample and operation
ID may already have been consumed; test/report that explicitly. There is no
retroactive profile byte limit or hash/history rewrite. Check known full receipt
and namespace outcome before formatting success; use its actual namespace fields,
not a racy name lookup. Any later projection failure retains the in-hand receipt.

Original replay remains ahead of fresh candidate limits. Existing v2/private
keyed calls retain their current None mode and return historical receipts. Public
API1 creation is unkeyed and cannot request an oversized historical keyed replay:
do not claim new public keyed support or that this closes future public2 replay
framing. A bounded entry must not be reused for keyed/public2 serving until that
separate retained-receipt transport contract is proven. Test genuine oversized
retained replay through the original internal entry unchanged; duplicate-name
API1 remains its existing precommit conflict, never an invented replay.

## Measured and emitted frames

Use a small native-only typed failure carrier internally, projected through the
existing `BridgeError::PartiallyApplied` JSON string and HTTP catalog error path.
Legacy error mappings and success NamespaceInfo/NamespaceResponse stay unchanged.
Finite native cases: known publication/dependency failure with full own receipt;
typed commit/worker unknown with actual operation ID or null and any already-known
own receipt; precommit size refusal without receipt. Fixed messages only. Generic
storage/dependency failure uses SERVICE_UNAVAILABLE; INDEX_NOT_READY requires
actual typed publication evidence. No readiness is inferred from receiptG.

The bounded completion below guarantees at most the same one current catalog
receipt (possibly repeated identically), not an arbitrary foreign receipt list.
Full-byte equality deduplicates identical wrappers; conflicting evidence fails
closed and must be tested, never relabeled as a successful creation. Walk typed
causes for unknown precedence; do not parse error displays or invent current IDs.

For each candidate, enumerate every finite failure variant and the actual success
projections. Encode typed failure JSON through a capped writer first. If it exceeds
the transport cap, stop before allocating a Value/body copy. For admitted bodies,
use the same compact JSON string that will be emitted; parse only that bounded
string where the existing MCP handler requires Value. Count the actual
`DaemonResponse::error(u64::MAX, -32006, message)` and actual
`ToolCallResult` text/structured projection, including its current pretty formatting,
with checked capped writers. HTTP counts its actual typed error body. Count
success daemon/MCP/HTTP envelopes too, with maximum response-ID/timing spellings.
This is finite actual serializer evaluation, not an ad hoc padding multiplier,
assumed8MiB bound, `json!(receipt)` estimator or unbounded proof Vec.

The daemon's existing dispatch receives the current connection's peer frame limit.
Add `NamespaceRegistry::create_namespace_bounded(input, limit)` with a compatible
default; the actual owned native adapter uses it and the server passes the real
limit before work. Legacy creation delegates unchanged. Local MCP has no negotiated
peer capability: use an explicitly documented internal response ceiling, not a
fabricated client limit. HTTP has its explicit body ceiling. A native write only
starts after the endpoint can enforce its actual response representation/cap.
Old1MiB peers may create small namespaces; oversized valid profiles refuse before
canonical commit. Negotiated8MiB only changes the actual cap, not proof quality.

## Remote transmission classification

Keep the existing `create_namespace` RPC and NamespaceInfo success. The new narrow
Remote call owns the existing connection mutex through negotiation and exactly
one exchange; it never replays the mutation or falls back to another method.
Reuse framing/state, not a new client. `ExchangeError.lost` alone is insufficient:
current `exchange` marks ID0/empty responses as settled despite sending a mutation.

Proposed shared additive exchange classification, coordinated with Huygens R1:
`BeforeSend`, `SentUnattributed`, `MatchedReply`. Assign BeforeSend only before any
socket write; failed/partial write, read loss, mismatched/ID0/missing result and
malformed responses are SentUnattributed. A correlated typed server response is
MatchedReply, whose existing known/unknown payload still determines mutation
evidence. Preserve current behavior for unrelated `call_locked` users; the catalog
entry consumes this classification without string parsing. No automatic retry.
Postsend unproven outcome returns structured unknown/null ID, not notCommitted.
Malformed typed receipt evidence follows the existing unverified-evidence boundary
and must not be promoted into a current catalog receipt. Pin its exact bounded
shape with Huygens before implementing that decoder; never truncate it to fit.

## Actual E1 dependency

The 103 worker releases its shared publication lease before ordinary completion,
which may resume purge or return an arbitrarily large foreign pending receipt.
Candidate sizing alone cannot bound that response. N1 therefore genuinely needs
the reviewed E1 **restricted completion/admission/ACK seam**, not its HTTP sweep
response or scheduler. Reusing this seam avoids a duplicate completion protocol.
Consume its exact validated runtime only after Pascal freezes it; no WIP merge.

Use an empty receipt authority at the final guarded producer entry to reject
preexisting pending effects/purge before fresh work. After a known catalog commit,
admit the exact already-counted receipt and call restricted completion. Under
existing publication-write/view/engine-write guards it rejects foreign/unapplied
pending effects and never resumes purge. Existing exactG/all-view/row-byte ACK
checks remain, including the empty-pending finalTX recheck. A peer race returns
this creation's own receipt plus fixed dependency failure, no foreign payload and
no invented unknown. Actual worker/ACK unknown retains typed precedence.

If E1 exposes a sweep-specific context rather than the reviewed private storage
admission view, request the smallest shared interface from Pascal before code;
do not import sweep policy/HTTP types into catalog or invent a second registry.

## Same-owner ordinary format selection

Add private `OpenMode::{Legacy, Native, Detect}`; retain old public/open_native
wrappers and bool compatibility entry, which map to explicit modes. Add
`open_detected_with_ids(path, ids)` for `Recalld::with_runtime_sources` only.
It acquires `StoreOwnership::acquire_recovering` once and transfers that same value
into one `open_owned_with_mode(ownership, mode, ids)` implementation.

Inside that implementation, acquire the existing directory lock once before
inspection, retain both locks through preflight/live-open/recovery. An absent or
truly empty fresh root selects legacy creation. Existing nonempty roots require
metadata; missing metadata is corruption, not fresh initialization. Inspect a
disposable metadata copy with the existing startup copy-recovery helper, validate
control/table structure, and choose Native only for supported control schema2,
Legacy for supported1 or genuinely absent pre-control metadata. Unknown/incomplete
control/schema evidence errors before writable metadata open. Never select mode
by API, an error message, or writable legacy-open-and-retry.

Use this detection solely to select the **existing full** native or legacy
preflight and recovery path; decoding a header is not source validation. Retain
strict native required-feature/certification/effects/ledger/lineage validation and
the original raw recovery scans before canonical effects replay/orphan handling.
Perform detection before legacy text recovery can mutate a malformed native source;
preserve supported legacy compaction recovery ordering after legacy selection.
Drop the disposable copy on every exit; source lock is never released/reacquired.
An already-started validated restore journal may be recovered by existing
acquire_recovering before detection, as today; do not mislabel that as an immutable
inspection. Ordinary offline inspection still does not recover/publish journals.

## Literal releases and focused proof

Runtime parents: validated103 exact handoff plus actual E1 restricted-completion
runtime where consumed. Add65 only for its real legacy creation_dimension call;
no112/184/private Remote-read parent. Huygens reviews wire/shape/transmission;
Pascal releases the shared admission signature without parallel edits; Halley
releases constructor/daemon/client hunks after queue runs. This document proposes
those interfaces; it is not itself a runtime release.

Actual BEFORE/AFTER: API1 native create rejection; context1/2 hash distinction and
unchanged originalv2 bytes; omitted/default and null behavior; provider0/IDs0 exact
replay; valid oversized descriptor fresh rejection before stage/commit; actual
1/8MiB frame boundaries; first known publication failure, unknown ACK/worker,
sameconnection lost/ID0/malformed response and next ping without replay; real peer
pending/purge race retaining own receipt only; legacy positive/error snapshots;
heterogeneous native startup and malformed-copy immutability under both locks.
Frame/decode tests must preserve actual prior artifacts, not authored JSON alone.
No ordinary startup/public2 activation until the rest of N1's route matrix and
startup warmup/per-namespace admission acceptance pass on the final composition.

## Actual BEFORE and shared-hunk freeze, 2026-11-16

Halley released an independent test-only child of actual main103
`b374f07efb2da994d3a82e85d4814e642d2864b2`. Executed source
`7e5d322d87a8933e043bad967dd866d46e036304` produced one legacy positive and two
intended native acceptance failures, both after retained receipt/G/canonical-file
checks. Ordinary construction failed at `open_storage`; the configured native
catalog adapter's original-API1 call failed through the legacy proof guard.
Existing owned construction succeeded on the genuine migrated/re-embedded
homogeneous fixture. This is not an unknown-provenance or provider-width failure.
The catalog diagnostic is adapter-only, not full transport proof. Sealed evidence
is `1ee904fdb1fd5ac138b27cc2e979c22b777c1196` on
`test/ordinary-native-admission-before`, under
`docs/evidence/ordinary-native-before/REPORT.md`; the tail changes only assertion
layout and documents. No production implementation release follows from the REDs.

Huygens is now sole owner of the R1 client/server exchange hunks, on actual
`5d18075eea193849056ae88cb07f27843d1c7019`. Feynman retains N1 catalog/constructor
and will consume the exact validated shared exchange seam, without concurrent
client/server edits or an automatic whole-R1 parent. The classification contract
above is unchanged: same connection, one send, no replay, typed send/correlation
state, complete evidence, unchanged existing callers. Malformed oversized peer
evidence remains a reviewed wire-policy prerequisite, not an assumed small error.

Pascal's E1 candidate `b62fcbd8fc1c22fad76bdfc68005a072920225c1` received a bounded
A1 source ACK for restricted completion/ACK, unknown precedence and internally
retained admission-invariant evidence. Its runtime proof was still running at
this update. Do not merge the WIP or treat the source ACK as validated readiness.
The next runtime release requires that exact validated interface plus wire-policy
closure; constructor format selection must retain the same sibling/directory
ownership through disposable inspection and live open. Current main103 still
runs legacy text recovery before preflight; detection must precede that on native
selection without changing supported legacy recovery or ordinary offline policy.

R1's subsequent cap proposal uses a private response bound
`floor((8MiB - fixed_overhead) / 3)` before accepting owned receipt evidence. This
is still a proposal, not an established bound reusable by N1. On pinned b374,
`ToolCallResult::json` pretty-prints its text copy, so compact input size alone
does not establish a two-times escaped-text bound. MCP `JsonRpcId::String` also
means outer overhead is not fixed without a per-request charge or an explicitly
inner-body-only ceiling; the daemon's numeric IDs are a different boundary.
`read_framed_response` currently decodes an owned `DaemonResponse`/Value, so a new
streaming admission check must precede that decoder. The existing raw frame
reader drains oversized frames only within its drain bound; other failures may
poison the connection. These exact source constraints were sent to Huygens and
Halley. The good server must enforce the same proven representation/cap before
observation, while a malformed peer rejected before evidence acquisition remains
post-send unknown. No currently accepted full typed receipt may be discarded.

Huygens clarified the actual R1 producer: `QueryReply::mcp` emits compact text;
the pretty-format caveat above concerns N1's existing API1 catalog path through
`ToolCallResult::json`, not that R1 producer. MCP accepts arbitrary string IDs.
HTTP's configured total request-body limit is not a field-length bound, and the
stdio line reader has no explicit line cap. A bounded inner ToolCallResult or
daemon response therefore does not prove a bounded outer MCP response.

N1 must carry the actual request-specific echoed-ID and metadata/envelope charge
from the MCP dispatcher to catalog admission before any fresh canonical commit.
Its prospective remaining budget must be checked against the actual emitted
catalog success/failure representation; a daemon numeric-ID maximum cannot
substitute for an MCP string ID. This is required for API1 N1, not deferred to
future public2. Pin the actual dispatcher/handler signature and shared-file owner
before edits. No new MCP input limit, external negotiated MCP capability or
blanket8MiB outer-frame guarantee follows from this source finding. Huygens keeps
sole R1 client/server ownership; N1 does not edit those hunks in parallel.

## Validated E1 handoff and remaining N1 prerequisites

The frozen E1 dependency is now
`6ebfd25080bda0d21a755d88211fba471da1e652`, with production runtime
`b62fcbd8fc1c22fad76bdfc68005a072920225c1`. Its actual-main composition
`ce61cf9921d7d2ce9a073817d03326edfd8b9de3` was reported by the queue owner as
passing 1717 library tests, 10 CLI tests and 15 example tests, with three existing
ignored tests. The queue's lint/public tail and seal were still pending when
recorded. This is owner-reported execution, not a second A1 test run. The earlier
`bc4e20c8` full result (1647 passed, four failed, three ignored) remains distinct;
its four diagnosed test corrections do not retroactively make that run green.

The actual reusable interfaces are storage-owned:

- `ReceiptAdmission::new(cap)`, `limit()`, `record(&WriteReceipt)`, and
  `check_receipt(&WriteReceipt)`. The collector charges its own fixed overhead
  and exact encoded receipt copies. Its cap is not a catalog wire-size proof.
- `MetadataStore::check_receipt_admission(Some(&admission))` rejects pending
  purge and foreign outbox IDs before decoding their payloads. It requires the
  caller's engine/publication guards. Fresh catalog admission must also run
  under its authoritative mutation/write-transaction fence; a preliminary
  adapter check alone is insufficient.
- `DerivedCompletion::complete_with_admission(Some(&work), Some(&admission))`
  reuses existing publication ownership and exact acknowledgement. It retains
  worker-unknown classification and cannot adopt unrelated pending work.

N1 will use an empty collector for fresh prework admission, count the candidate
against both the collector's limit and actual catalog response encodings before
namespace staging/commit, and record only an actual returned receipt (including
one carried by a typed known-commit error). Never record the private candidate
as committed evidence. Completion uses that same request-local collector; it is
neither a new generation authority nor a durable permission registry. Unknown
precedence and invalid-evidence retention remain unchanged.

Pascal's source review of `6de6d57d` confirms this use with these constraints:
`check_receipt_admission` opens its own read snapshot, so the authoritative
mutation fence must remain held through that check and the catalog commit.
Receipt equality includes the serialized `replayed` flag; a decorated replay
receipt cannot be normalized into pending-outbox authority. `record()` alone
does not classify a failure: retain the full typed wrapper and unknown precedence
through `fail`/`snapshot` or the catalog's equivalent typed carrier, including
`InvalidReceiptEvidence`. Release held publication/storage guards before
restricted completion reacquires them in its established order.

The minimal implementation composition is actual103/main
`b374f07efb2da994d3a82e85d4814e642d2864b2` plus frozen E1 `6ebfd250`, rather than
the broad `ce61cf99` validation composition. Literal queue ownership release and
the catalog-specific malformed-response/frame policy still govern implementation
of shared transport hunks. Huygens retains R1 client/server ownership. Its private
query bounds and newly corrected exact-pair scalar codec are not automatic N1
dependencies or substitutes for catalog response admission. Ordinary native
startup and public API2 activation remain separate acceptance gates.
