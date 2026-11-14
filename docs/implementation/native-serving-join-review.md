# Native serving and A2/A4 join review

Source-only review, 2026-11-14; no runtime defect reproduction or activation
certificate. No active builder or frozen PR133 files were edited by the review.

Reviewed frozen inputs: PR134 `b62d1e2f4b3f516b5889946605946e274f82286c`
startup plan and legacy API2 writer inventory; Huygens insertion-wire evidence
`fff6baf6ac85318c5904556363e34f84f777abeb`, its consumer inventory and plan;
PR132 `5864c09050e532454c97ba8e73e00e7ba1670fb4`; PR133
`27a6cd4c6fc89f19dcd8964c20ec3fbb8b24c36a`. Huygens's active serialization
edits are not treated as frozen implementation evidence. The older
native-startup-admission worktree at `4072015` is stale.

## Concrete remaining consumer gap

At PR133, `search/pipeline.rs::similar_with_access_admitted` searches `k + 1`
vectors, removes the source and applies the score threshold, takes `k`, then
loads canonical records and removes Ghost/Tombstone entries. Thus excluded
records can spend the candidate allowance. `search/read_view.rs::prepare_coherent_similar`
is explicitly V1, hydrates one source namespace and exposes no current/history
or all-compatible selector. `mcp/bridge_adapters.rs::find_similar` accepts an
unused `_same_namespace` argument. The older public-similarity-minimal source
also ignores this flag. No frozen equivalent consumer was identified by Huygens.

This is original A2 scope, not a PR133 regression. PR133 closes coherent search
payload selection and exposure classification; it does not close these similar
paths. Hume owns the explicit V2 backend, eligibility before caps, current/history,
profile-compatible scope and returned-only exposures. Huygens owns strict public
selectors, API1 compatibility mapping, envelopes and activation. Named V1 remains
unchanged; baseline runtime probes are still required before a fix is claimed.

## Exposure join constraint

PR132 supplies original API identity independently of semantics, sampled time,
namespace-ordered observation groups and partial receipts. However,
`record_exposures_at_with_origin_work` takes one `ExposureScope`; the native
coordinator reuses its namespace/source assertions for every group. Native
capture correctly requires source and target namespace agreement. An
all-compatible consumer cannot pass one source namespace for every target or
silently use `None` to discard that guard. It needs separately retained source
visibility/admission evidence and explicit per-target-namespace scopes, using
the existing observer, policy resolution and partial-publication behavior.

The PR132/133 mechanical join must carry the original numeric version through
`search_checked_with_origin_work -> search_checked_with_selection -> search_admitted`.
Pinned read-only internals defer recording; their compatibility number cannot
authorize a write. Actual outer exposures must carry the caller's original
number. A4 coverage must consume post-supplemental shared work and omission
diagnostics; early channel coverage is insufficient. HTTP ranked hits and MCP
returned neighbors remain different exposure surfaces.

## Already-owned startup blockers, not new assignments

The current PR134 plan correctly separates ordinary API1-native serving from
later API2 activation. `StorageEngine::open` still selects the legacy constructor;
`system::Recalld::with_clock` uses it and has a legacy default-catalog bootstrap.
`DaemonServer::new` consumes that system and installs the real adapters: it is
not a separate native admission route. Feynman already owns mode selection under
ownership, strict recovery, catalog bootstrap, publication, shutdown and listener
admission. No contradiction requiring a second engine or early API2 switch was
found in the updated plan.

Known writer closure remains: Huygens prepared single/batch wire, Curie original
linked-store/batch context and graph producers, Huygens API1 native projection,
Pascal feedback/exposure/manual maintenance, and Feynman catalog/allocation and
compound recovery. Existing API1 wrappers must not call legacy-only writers on
native state or relabel API1 as API2. Legacy-disk API2 insertion, tombstone,
compound batch/purge and manual maintenance are separately inventoried and owned;
native API1 support does not establish them. No fresh unowned startup entrypoint
was found in this bounded review. Runtime closure remains to be demonstrated.

## Minimal ordering

1. Prepare and validate actual native writer/adapter leaves on owned fixtures;
   keep original request identity and all known receipt/publication failures.
2. Join original API1 linked-store/batch and graph/feedback/catalog consumers;
   Huygens supplies their real API1 projections. Feynman enables ordinary native
   API1 startup only on that validated closure. Recompute the old plan's ancestry
   table; it is not a request to import every leaf or a broad checkpoint.
3. Independently, the next A2 similarity child needs only published PR133 and
   PR132 (shared PR119 ancestor), not moving writer/startup branches. Validate
   real baseline failures and explicit V2 selection/exposure on owned fixtures.
4. Huygens activates public API2 only after startup, actual A2 consumers, original
   origin forwarding and per-operation legacy/native support are validated.
   Capability declarations must not infer support from disk format or envelopes.

The separate PR119 V1 representation-order probe is not an e78 incremental
publisher comparison. It does not change this review's V1 compatibility scope;
future incremental promotion needs its own actual publisher parity proof.
