# Standalone native graph producer closure

Approved inventory recorded during the2026-11-14 performance quiet window.
The implementation now lives in its own direct121 worktree; the actual source,
focused results, remaining validation and scope are recorded in the
[evidence report](../evidence/native-graph-producers/REPORT.md). This plan records
design/ownership decisions, not completed ordinary-startup or activation proof.

## Actual sources and smallest dependency

Inspected baseline: published PR121 `3fa91d4e542ec8ed5519fe4c3b87e9e665043a40`.
It already contains native graph changed/no-change proofs, final peer/phase/profile
checks, native recovery/history validation and PR96 all-view completion. It does
not contain `DerivedCompletion::native_graph`.

The existing coordinator is in published PR106
`fe99020926eb81a6994c7e9214ad4623180eeed1`,
`src/derived_completion/native_graph.rs`. Its use in the separate PR123+121 join
was covered by the1,152-test validation at7dc368b, but that is not proof that actual
standalone producers dispatch through it. Do not add the whole batch parent just
to obtain this helper. Hume agreed to adapt the small existing seam into a real
producer slice directly on121 and retains review of proof/final-token invariants.

On the inspected121 baseline, `metadata/native_graph.rs` rejects API1 with a
local `contract_version !=2` condition. The intended change retains existing
`MutationContext::validate()` (closed1|2), exact namespace assertion, native owner,
physical profile and final capture checks. No new common validator, graph proof,
receipt shape, hash domain or generic approval flag is needed. This is separate
from the native-record context change; neither requires the new compound codec.

## Production caller inventory

Line numbers below refer to121 and are navigation hints, not a claim about later
moving wire work. Caller searches included the entire src tree, then distinguished
production paths from test fixtures and reconstruction code.

| Actual entry/caller | Shared path and current writer | Native closure |
| --- | --- | --- |
| `mcp/bridge_adapters.rs` legacy store, around1324/1349/1391 | semantic/entity/temporal `_with_work` after legacy insertion; native owner reaches legacy edge guard | Huygens owns format-aware store dispatch. Native store must use same-insert links and skip this entire legacy callback sequence. |
| `api/handlers.rs:create_memory`, around312; `RelationshipGraphAdapter::post_creation_links_with_work`, around1084 | same three link producers after insertion; warnings/ignored per-helper failures in legacy callback | Huygens branches on owned native format before legacy conversion/provider/insertion/callbacks. Do not repair native insertion durability by redirecting these postcommit calls. |
| `bench/locomo.rs`, around1285/1318/1335/1351 | legacy insertion then three helper calls; caller ignores helper errors; later `complete_pending_effects` | Curie owns real native benchmark ingestion later, using validated linked-store API1 consumer and frozen same-insert selection. This is a real additional dependency, not a reason to add wire/batch ancestry to the standalone graph slice. |
| public `graph::perform_autolink`, `_with_work` | vector candidate search, cached tags, graph prefilter, `persist_edges_with_work` | Existing-record standalone request needs accepted owned native selection, canonical scope/profile/phase checks before cap, stable ties, one frozen edge plan, original API and complete publication. |
| public `perform_entity_link`, `_with_work` | entity candidates, cached counts/Jaccard, cap, same persistence helper | Same owned native path; no missing-cache fallback may invent canonical native labels/counts. |
| public `perform_temporal_link`, `_with_work` | supplied recent-ID/timestamps, cap, same helper; namespace parameter currently named `_namespace_id` | Canonical namespace/phase/time validation before cap for native candidates. Do not treat a caller list as authoritative or let foreign candidates consume the native allowance. |
| `graph/explicit_links.rs:apply_parent_edge/apply_supersedes_edge` | private `apply_explicit_edge` clones graph, prechecks, then `publish_edge_plan_with_work` | Existing-record native explicit graph operation needs typed canonical write plus final Supersedes/peer checks. Creation's explicit links remain in its insertion proof. |
| `api/state.rs` explicit-link methods and `RelationshipGraphAdapter` implementations | expose the preceding helpers; no production handler invocation of these methods found on121 after canonical insertion was introduced | Preserve legacy compatibility deliberately. Do not invent a graph HTTP endpoint solely to make a helper appear consumed. Existing direct/core helper use still needs explicit ownership and truthful errors. |
| `StorageEngine::{add_edge,batch_add_edges}` → engine `put_edges_with_effects` | legacy canonical effects, native mutation guard rejects | Remain explicit legacy entry points unless an actual native owned consumer can supply clock/context and publication. Do not weaken their guard or silently bless them by storage format alone. |
| `NativeLifecycleSession::{capture_graph,put_graph_prepared,put_graph}` | real native engine/proof, typed receipt, offline primitive; currently API2-only | Core API1 identity can be supported without changing proof formats. Offline success is not online all-view readiness. Actual online producers must also consume the owner coordinator. |

Shared sink today: `persist_edges_with_work` → `publish_edge_plan_with_work` →
`put_edges_with_effects_with_work` → `commit_graph_effects`, followed by graph/cache
publication and graph-effects acknowledgment. It creates legacy unkeyed API1
context and does not become native merely because a caller has a WorkTicket.

Other graph mutations require classification, not blanket conversion: startup
`system.rs` reconstruction and offline lifecycle/migration rebuilds derive graph
state from canonical evidence; they are not new edge intents. API graph adapter
`add_node` and `tombstone_node_with_work` mutate derived state and currently have
no production handler caller found by this inventory. They cannot serve as a
native canonical mutation or independently certify a current publication stamp.
Their native use must be owned reconstruction/publication or reject before edits.

## Concrete owned boundary to implement

1. Adapt the existing coordinator as an internal operation of the actual
   `DerivedCompletion` owner. Inputs are original `MutationContext`, a frozen
   `NativeGraphInput`, `NativePolicyInputs`, the owner's clock and optional parent
   WorkTicket. Output retains `Option<EffectResult>`/full typed receipt: keyed
   no-change is a real receipt; unkeyed no-change is not a fabricated commit.
2. Register before selection, provider work or enqueue. Native selection must
   use a pinned canonical generation and real records/profile/policy, not merely
   cache membership. Derive stable ordered candidates/edges and recheck capture
   tokens in the final transaction. The standalone request selects once; it is
   not replay of an insertion's volatile afterthought.
3. Replay keyed explicit/frozen `NativeGraphInput` before fresh endpoint/policy/
   clock capture. Its existing hash binds the ordered edges, original API1/2,
   exact weight bits, scope and CAS. It does not bind an automatic seed/config
   selection request. Preserve the existing unkeyed automatic helper contract;
   do not offer keyed autolink by resampling neighbors or ignoring changed
   selector settings. A keyed automatic request requires actual caller-supplied
   retained frozen intent; otherwise reject the unsupported key before selection,
   without inventing a transport or proof. Never infer API from lifecycle policy
   or private graph format. Native selection must agree with both canonical and
   required derived generations and apply eligibility before caps.
4. Use the existing lock order. Publication lease, owned graph guard, accepted
   child and storage owner must remain inside the blocking worker's lifetime so
   cancellation cannot release protection while commit continues. No storage
   guard is held while awaiting graph ownership.
5. Apply the native engine/proof and then complete all four views at actual current
   generation before ready success, including keyed no-change. Preserve exact
   `CommittedMutation` receipt, native unknown hints, and unresolved accepted
   work on uncertainty. Never convert known/unknown outcomes to a generic string
   or a success count.
6. Consume the coordinator in real semantic/entity/temporal and explicit-edge
   producer paths in the same slice. Do not ship an unused coordinator alone.
   The existing standalone helpers need an explicit owned publication seam;
   their current graph/cache-only signatures cannot manufacture the missing FTS,
   vector/entity ownership or clock. Reuse `DerivedCompletion`, not a second
   tracker or a global service locator.

## Approved owner interface and remaining source details

`RelationshipGraphAdapter` has graph/vector/entity/storage/cache/config, but no
FTS handle or injected clock. Its real construction sites include `main.rs` and
system/API test assembly. The owned standalone path therefore needs actual
completion/clock injection at construction. The coordinating agent approved
injecting the real `DerivedCompletion` and `Clock` at actual
`RelationshipGraphAdapter` construction, with a typed native path through the
real producer. Existing legacy wrappers are for legacy ownership only; missing
native publication ownership must reject before mutation rather than silently
falling back. No broad PR106 batch parent is authorized for coordinator reuse.

`AutoLinkError::Storage(String)`, the void parent-edge method and string-only
Supersedes failure detail cannot preserve a native committed receipt. The native
producer path must return a typed canonical result/error all the way to its real
caller. Prefer an additive typed internal owner path consumed by existing helpers
and adapters, while retaining old wrappers only for legacy ownership. If a real
native caller still reaches a void/string wrapper, that is an unresolved boundary,
not permission to discard its receipt. Exact helper/adapter signatures require
owner review before edits; no public graph transport or unsupported success is
proposed to conceal this gap. The owner interface is approved; concrete typed
helper signatures must preserve the full result/error rather than erase it in
the legacy projection.

The first shippable standalone slice must exercise an actual existing producer
with native ownership, origin propagation and typed completion. If the full
three automatic producer families require a larger selector change, split by
actual producer consumption, leaving the others explicitly unavailable before
mutation and without claiming ordinary-startup closure until configured semantics
are covered. No silent disabling of configured linking is acceptable.

## Ownership / topology coordination

- Feynman confirmed standalone graph producers are stage1 ordinary-native startup
  prerequisites. Actual final peer checks and PR96 all-view completion are required;
  explicit owned fixtures precede ordinary startup. His compound codec is not a
  standalone graph dependency.
- Huygens owns MCP/HTTP insertion routing. Native format is selected before legacy
  conversion/provider/insertion; a single `NativeMemoryInsertedWithLinks` proof
  and same-owner completion replace the entire post-creation callback sequence.
  His latest inspected source handoff is e0472cd (focused2cb4946, production855bd6f);
  API1 native projection remains planned, not implemented/validated.
- Current narrow record API1 child supports plain `NativeMemoryInserted` and
  forget only. It does not enable linked-store/profile replay. The later linked
  store/batch-origin child needs validated explicit2 wire and the actual compound
  codec where consumed, then Huygens's API1 projection can follow. No circular
  dependency or API2 coercion is authorized.
- Curie owns actual standalone producer/coordinator plumbing and eventual LoCoMo
  native ingestion. Hume retains graph proof/final-token review; A2 selectors remain
  his separate work. Huygens's batch/metadata files are untouched.

## Required proof for this slice

Use real native fixtures and the actual producer, not only `put_graph` helpers:
semantic/entity/temporal/explicit writes, existing-edge/keyed empty no-change,
API1/API2 identity conflict and exact replay of explicit frozen edges, actual
API1 graph changed and typed no-change receipts through offline history/export/
import/snapshot validation, legacy old-hash compatibility, canonical all-origin
profile/policy/token binding, namespace/phase/profile/cycle/branch failures, stale capture race, close during
selection/commit, cancelled waiter with worker lease retained, all-four-view
completion, exact receipt on publication failure and reconciliation, and actual
process-exit reopen before/after metadata and edge application. No power-loss
claim from process exits. Native store tests also assert no legacy callback is
called or fresh neighbors selected on matching completed insertion replay.

Ordinary startup remains unavailable until all actual configured writer paths,
not merely this helper or same-insert links, satisfy their consumers and proofs.

## Additional source-review boundaries

Feynman reviewed the inventory and startup classification and confirmed the
owner-injection/typed-receipt boundaries. He required the explicit distinction
above between keyed frozen edges and unkeyed automatic selection. Hume confirmed
that121's only graph API-version restriction is the local metadata context check,
while requiring actual API1 replay and offline/snapshot validation for both changed
and typed no-change proofs. These are source-review findings, not runtime approval.

The coordinating agent explicitly approved the above direct121 actual-owner
consumer plan and original-key distinction. This is not approval of a keyed
automatic-linking contract. Configured native linking remains a startup-closure
requirement; neither an unsupported no-op nor silent disabling closes it. The record slice was subsequently validated and published as PR131. Final graph
validation completed after explicit SSD cleanup END on eb4959b:15 focused,1108
library+2CLI, fmt and clippy passed. No record parent was added. See the evidence
report for exact source/executable provenance and remaining startup boundaries.
