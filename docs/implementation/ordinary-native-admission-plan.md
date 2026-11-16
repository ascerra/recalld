# N1: ordinary native startup, separate from public API2

2026-11-16. Owner: Feynman (A1). Source-only plan on exact main181
`33865ca0d19599a13ba999d31a1ec736c89db233`; no runtime changes, execution,
activation, or permission to edit queue branches. Halley releases exact validated
parents and shared hunks before implementation. This document narrows the earlier
six-closure inventory to dependencies actually used by ordinary construction/API1.

## Concrete source boundaries

| Boundary at this source | Actual dependency / action |
| --- | --- |
| `system.rs::with_runtime_sources` always calls `open_with_ids(..., false, ...)` | Ordinary format selection is missing. Reuse the existing native open and same `StoreOwnership`, immutable inspection and strict recovery. Do not try a writable legacy open and then retry native; do not infer format from API, filenames alone, or a failure message. Empty roots keep legacy creation. Unknown required formats reject before canonical mutation. |
| `with_owned_storage` already supports native ownership, but requires disabled sweep/RIF/autolink and an existing schema2 default | Preserve this private constructor contract while establishing ordinary admission independently. No automatic schema/lifecycle migration, default creation, policy adoption, or vector certification. Explicit unsupported configuration must fail before ordinary serving, never silently turn features off. |
| Global provider/profile loop at system469 requires every namespace compatible with one provider | Replace only after actual per-operation semantic guards and heterogeneous warmup proof are verified on the final source. Unknown/other-profile namespaces must not block metadata/FTS/structural reads of otherwise valid stores. Generated queries still require profile compatibility; supplied vectors still require certification and exact canonical scope/G. |
| Native startup already uses `VectorIndexOwner::from_namespaces`; completion rebuild supports heterogeneous partitions | No new backend or registry. Audit startup loader's counters/skips against final published view stamps: current error condition at system556 is tied to `effects_rebuild`. Any native partial vector load must not become a complete published view, even without pending effects. Corrupt referenced payload is an error, not an empty partition. This is a source hypothesis pending actual fixture, not an executed defect. |
| Production direct MCP and daemon factories construct `McpStorageAdapter` without native policy; HTTP storage/manual sweep factories likewise omit it | Supply the same resolved configured `NativePolicyInputs` through existing setters, preserving original API1 context and the owned clock. Manual native sweep explicitly rejects missing policy at api/adapters1461. Inspect lifecycle API1 final policy selection before asserting a particular failure. Existing fixture-only setter calls do not establish production wiring. |
| API1 namespace creation at bridge_adapters2673 always calls `create_legacy_namespace` with schema None | Actual format-aware API1 catalog adapter remains required. Consume the validated native catalog/policy producer (103), map existing defaults and optional selectors without changing API1 hash identity, and preserve full known receipt/error semantics. Do not invent a general setter or bind arbitrary weights from provider configuration. |
| API1 native purge dispatch already exists in main183 | Reuse its receipt/frame-admission/unknown behavior; no new purge implementation or dependency on private prepared reads. Test through ordinary production construction after wiring. |
| Native store, batch, graph, feedback/forget, exposure and maintenance producers exist in main181 | Establish an actual production API1 route matrix, including configured autolink and batch semantics. A native engine opening successfully is insufficient. Original API1 remains API1 in all retained hashes/proofs; no forced context2. Keep prework unsupported cases explicit until their actual consumer is present. |
| Manual sweep failure aggregation is under E1 review, with Pascal sole runtime owner | Consume its validated correction if ordinary manual-sweep acceptance exercises those paths. Do not duplicate collector, frame-budget or completion edits in N1. Background counter-only behavior is separate from HTTP evidence. |

## Minimal parent decisions

Start runtime work from a literal validated main/owner handoff, not this moving
queue snapshot. Main181 already contains heterogeneous owner, certification,
typed insertion/graph/maintenance/purge and full receipt corrections. The actual
103 catalog producer/adapter seam is a dependency where API1 creation consumes it;
its new full result is queue evidence, not independent proof by this owner.

65 is needed only for actual profile-provider/catalog admission calls used by the
ordinary route. Confirm its concrete interfaces and final main proof with Huygens;
do not merge merely because it appears in an inventory. 164 is needed only where
its format-aware compound API1/legacy compatibility is genuinely consumed. The
existing native API1 batch route must be tested, rather than inferred from API2.

112 metadata-only pages are not an ordinary startup prerequisite unless a selected
API1 adapter actually calls them. 184 private prepared results and the proposed
Remote prepared-query RPC are separate from existing API1 search/similarity.
172 is a test-only chronology composition, not a constructor implementation.
Public API2 capability/discovery activation belongs to Huygens and follows N1;
N1 must not depend on that activation. E1 remains a separate actual producer/error
dependency, not permission to absorb all queue leaves.

### A4 source handshake, 2026-11-16

Huygens confirmed no frozen API1 catalog wire consumer supersedes this finding.
His source check of 103 candidate `ee3e10924ad3f9944cf53bbdc18523e748b2f943`
found both API1 namespace adapters still on legacy creation. Feynman's subsequent
read at queue head `086f827c7c721ca3304df3f22e9a641f4c12f726` confirms
`NativeCatalogAdapter::create_accepted` explicitly constructs context2. Its
`create_native_namespace_with_provider` producer already accepts an explicit
context and checks replay before provider profile/dimension sampling. Reuse that
producer and accepted-work/publication lifetime; do not invoke the context2
wrapper for API1. These are source checks, not new validation results or frozen
runtime-parent handoffs.

The concrete consumer plan must preserve original1, fixed numeric defaults and
optional dynamic selectors, existing NamespaceInfo success shape, typed unknown
and full known-receipt failures through actual Remote frames. A raw namespace
error string is not sufficient after a native commit. Halley subsequently assigned
Feynman sole N1 runtime ownership including this API1 catalog consumer; Huygens
retains A4 wire review and makes no parallel catalog edits. Exact validated-parent
and runtime-hunk release remain pending. R1 prepared Remote reads are separate.

65's `creation_dimension` is specifically the legacy creation input/provider
width behavior (1..8192, optional profile). It allows a missing profile in the
legacy lane and therefore cannot authorize a native Bound namespace. Use it only
where that legacy behavior is required; native creation retains its actual
descriptor/provenance producer and cannot bless old vectors or arbitrary weights.

### Proposed catalog hunks for the next source review

* `native_catalog_adapter.rs`: make the existing accepted creation worker consume
  an explicit original `MutationContext` plus `NativeNamespaceInput`; the current
  v2 wrapper constructs exactly its existing context2, while an API1-only entry
  constructs original1. Reuse root/child, worker-held lease, provider replay order
  and completion. No new canonical producer or parser-version coercion.
* `mcp/bridge_adapters.rs::McpNamespaceAdapter::create_namespace` and
  `api/adapters.rs::NamespaceRegistryAdapter::create`: dispatch by actual owned
  format; preserve legacy route, numeric defaults and omitted selectors. Use the
  actual native outcome to recover the newly created namespace values, retaining
  the receipt on every fallible postcommit projection. Existing API1 requests have
  no new key/header semantics invented by this change.
* Factory wiring in `direct_bridge`, `daemon/server.rs::new`, and the HTTP system
  assembly: install actual103 native catalog provider/policy/clock through its
  existing setter. Keep native lifecycle-policy wiring explicit and shared with
  the constructor's configured inputs.
* Error surfaces: reuse existing `map_catalog_create_error`,
  `tools::handle_create_namespace` PartiallyApplied JSON carrier and HTTP
  `namespace_create_error` where compatible. They already preserve structured
  legacy known failures; do not replace them with generic strings or claim the
  mechanism is absent. Audit native `CatalogError` conversion and recursive
  unknown precedence before extending them.
* Daemon namespace-create dispatch needs actual peer frame admission before a
  native canonical write. Pin a provable full receipt/envelope bound from actual
  namespace/profile/proof limits, or a final candidate-size gate, before runtime
  release. Existing `client.call` lost-response/reconnect behavior also requires
  a typed transmission audit. Reuse protocol machinery after literal shared-hunk
  agreement with Huygens R1; do not add a parallel transport, assumed8MiB guarantee
  or receipt reference. This unresolved bound is not an implementation approval.

These are proposed source hunks, not a released runtime patch. Their bounded
diagnostics must first show actual API1 native rejection on the current parent,
then genuine local/Remote full known/unknown receipt preservation. Keyed replay
coverage remains required at the shared context producer; it must not be claimed
as a newly supported public API1 creation option.

Frame audit finding: `EmbeddingProfile::validate` in model/schema.rs checks
identity/version, dimension and nonempty model ID but has no byte limit on model
ID/revision or preprocessing prefixes. `NativeNamespaceCreated` retains the full
profile. Therefore current schema validity does not imply a fixed1MiB or8MiB
receipt limit. The runtime proposal needs actual candidate-size admission or an
actual descriptor count plus proven bounds for all remaining fields; never impose
retroactive history/profile limits. Original replay still precedes fresh admission.
`map_catalog_create_error` also currently places `source.to_string()` in the
failure body and uses `json!` receipt conversion; human diagnostics must be bounded
separately and exact serialized representation counted. No receipt truncation.

`NativeCatalogAdapter::complete_receipt` nests the own receipt around completion
errors. Completion can return another pending receipt or an unknown outcome, so
mapping only the outer receipt does not close the contract. Coordinate the exact
guarded completion/evidence design with Pascal's E1 review; consume a shared seam
only if genuinely required and validated, not as a convenience parent. These
source findings are not executed REDs or an approval to broaden E1 ownership.

## Single-owner interfaces and handoffs before edits

Feynman owns ordinary engine format selection, constructor admission and startup
publication proof. Existing ownership/metadata/publication state remain the only
authorities. No parallel registry, dirty flag pretending to certify vectors, or
per-query full inventory scan.

Request Halley's literal release for `system.rs`, ordinary `main.rs` and daemon
factory hunks; Huygens reviews API1 catalog/profile and direct/Remote dispatch;
Curie reviews batch/store/autolink producer wiring; Pascal reviews lifecycle input
and clock forwarding. E1 completion/HTTP hunks stay Pascal's. Produce the exact
route table and resolve absent consumers before runtime changes; a broad branch
merge is not an interface handoff.

The format-detection design must transfer the same source owner into live open.
Pin actual inspection helper behavior, required-feature validation and recovery
classification before proposing signatures. Preserve current genuine old-store
opening, native repaired-copy negative immutability, locks and pending-effect
recovery order. Do not expose native metadata as a second read authority.

## One composed acceptance suite, then appropriate final checks

1. Genuine legacy unchanged open/creation and API1 bytes; real migrated native
   strict reopen; malformed/unsupported format and repaired-copy negatives keep
   canonical source unchanged and release ownership correctly.
2. Heterogeneous namespaces with different dimensions, incompatible and unknown
   profiles, uncertified/certified vectors, empty scopes and retained Ghosts.
   Metadata/FTS/structural operations remain usable where independent; all actual
   semantic consumers (generated/supplied query, similarity, duplicates/autolink)
   enforce the same canonical scope/G admission. No skipped requested links.
3. Ordinary direct MCP, daemon and HTTP API1: store, batch, feedback, forget,
   namespace creation/purge, get/search/similar, graph, manual sweep and configured
   autonomous sweep. Assert actual original context, hashes, IDs/clock, receipt
   replay and changed-request conflicts, required policy bindings and truthful
   unsupported cases. Cover configured RIF explicitly rather than silently
   disabling it; activation requires its actual guarded writer path.
4. Strict startup rebuild publishes all actual views at the canonical generation;
   failure never publishes a falsely complete view. First/later known/unknown
   commit and restart/pending repair retain full available receipts, and do not
   double-apply exposure or free reused slots. E1 supplies aggregate sweep proof.
5. Real provider/profile mismatch and inherited-policy restart changes reject
   dependent semantics without automatically certifying/adopting or preventing
   independent metadata inspection. Explicit actual remedy/adoption remains the
   operator boundary. Native absence/history negatives are not globally upgraded
   by positive fixture helpers.

Run focused actual BEFORE diagnostics first, preserve failures, then one immutable
final runtime composition/full/lint run as appropriate. Reuse one owned cache,
network-none bounded Rust1.94.1 Docker; no personal daemon/data. Queue exact-head
CI is separate. No new heavy jobs have been started for this source plan.

This plan is not a claim that all APIs are activated, that the six queue closures
are all N1 dependencies, or that startup can ship with permanently unowned gaps.
The remaining work is production format admission plus the explicitly identified
API1 constructor/adapter consumers and their composed proof.
