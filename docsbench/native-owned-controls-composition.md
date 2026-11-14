# Native prefix checkpoint consumer: actual composition

2026-11-14. Implementation preparation; no joined runtime validation yet.

The source plan was `dc210443ab9553ba4014119fed16ae04acd0a938`, based on PR128 `5deb840a21ad5f1f80006e0d05a6372919292acf`. Exact validated PR138 `05e0d55aa3a92483e294b2a5e79cf490ce270609` supplies actual124 owned allocation plus134 original-request context and native catalog/policy receipt allocation. Its conflict-free join is `8ce6bf6`.

Actual published PR130 `e571104a2fc551a29314f6361b9b67503acd1fdb` is also necessary: PR128 alone does not contain the115 checkpoint/hash/restore implementation or the actual serialized supervisor consumer. PR130 supplies that existing consumer, its actual115/117/125 ancestry, and the128 sweep integration. Join `5d3f14f` preserves both native batch and exact-scan test modules and combines the seed/ghost fixture helpers without weakening either case. No broad validation checkpoint, A2 selection candidate, unfinished insertion adapter or ordinary startup selector was imported. The exact parents and reasons were communicated before these joins; this consolidated record follows the joins.

Feynman assigned the shared `Recalld::with_owned_storage(config, clock, engine)` extraction to this worker and will not edit `system.rs` until handoff. Existing legacy construction opens its engine then forwards. The private native-prefix constructor validates an existing schema2/default/profile and opens with owned IDs before recovery, then forwards to the same initializer/engine/registry. Ordinary native selection/bootstrap remains Feynman's separate work.

## Frozen PR138 public API1 route inventory

All line references below are from `git show 05e0d55`, not moving worktree lines. These are source observations, not runtime results.

| Operation | Actual route | First native-prefix scope |
| --- | --- | --- |
| Store | `src/mcp/bridge_adapters.rs:1467,1292` → `src/derived_completion.rs:101` → `src/storage/engine/insertion.rs:133` | Legacy mutation guard; unsupported. |
| Feedback | `src/mcp/bridge_adapters.rs:1676` → `src/decay/storage_adapter.rs:116` → `src/storage/metadata/observation.rs:209` | Legacy mutation guard; unsupported. |
| Forget | `src/mcp/bridge_adapters.rs:1618` → `src/derived_completion.rs:194` → `src/storage/engine/tombstone.rs:41` | Legacy mutation guard; unsupported. |
| False fetch | `src/mcp/bridge_adapters.rs:1477,1501,1578` | Real record/text read; observation branch skipped when false. Validate actual serialized behavior. |
| False search | `src/mcp/bridge_adapters.rs:229,330` | Real coherent query/payload; observation branch skipped when false. Native canonical read transaction exists at `src/storage/metadata/publication.rs:150`. Validate actual rows/projections, not only generation stamps. |

The shared legacy guard rejects native ownership at `src/storage/metadata/work.rs:95` → `src/storage/metadata.rs:348`. Private native write methods existing elsewhere do not activate these public routes. This consumer must reject writes and sweep before canonical work, rather than invoke private substitutes and call them public actions.

## First executable scope

Named synthetic private native-prefix checkpoint track, public API1 false get/search through the real MCP serialization, explicit disabled sweep/autolink/RIF, same owned clock/ID source, existing typed native hash and115 capture/clone guards. Compare actual suffix outputs, provider trace, canonical hash/generation and source cursor across three fresh-process restores. Declare read-only capabilities and reject unsupported actions before provider/canonical work. No independent native genesis, ordinary serving, public API2, full A6 or semantic-quality claim.

This is the first native checkpoint consumer, not final A6 closure. Extend real writes and explicit sweep after their necessary Huygens/Curie/Pascal adapters are validated; do not permanently define the wider task as unsupported. Preserve source/destination/ancestor locks, cancellation and old-worker exclusion from115. Use one fresh bounded private cache and remove completed raw runs after PR/results publication.

## First focused source validation

Source `65450ae504ee3347fb1e266c3690590d4e48ca19` failed compilation before runtime: the two new query tests incorrectly assumed `SearchQuery: Default`. The query model instead supplies serde defaults. A duplicate `AccessRecorded` receipt-validation match arm from the conflict-free parent merge also emitted an unreachable-pattern warning; the duplicate is removed without changing semantics. Raw evidence is retained under `/tmp/recalld-owned-native-controls-validation/run-65450ae5/output/`. The next focused build explicitly enables `bench` to compile the actual owned constructor and checkpoint consumers as well as query tests. This is not a runtime failure claim.
