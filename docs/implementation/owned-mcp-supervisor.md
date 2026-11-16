# Frozen private-control synthetic consumer

`serialized-owned-mcp-synthetic-diagnostic-v1` is an independently named internal diagnostic on published92 `0084ad30d768747b13c2815d838be8f0feaee284` plus published99 `534d9be9c4df54a95b5fca440f163b4a703a3238` (runtime `6ba411ecc0d0333177127f81fc83d76cfe816d4a`). No102 dependency. Older study artifacts remain pinned to their recorded source and binaries; these invocations do not reproduce their evidence.

ManifestVersion1, sourceMappingVersion1 and traceSchemaVersion1 are independent of publicContractVersion1/privateControlVersion1. Canonical schema remains null (`unattested-via-public-v1`). Strict source/artifact/executable pins and compiled scorer provenance are checked before episode output/provider/store creation. Generated synthetic fixtures have no claimed upstream revision, license or scorer semantics.

Build the exact frozen source offline with bench enabled and compile the scorer with `RECALLD_A6_SOURCE_COMMIT`, `RECALLD_A6_SOURCE_TREE_SHA256` (SHA256 of sorted, indented sourceFilesSha256 JSON plus newline), and `RECALLD_A6_IMAGE_ID`. Supply the matching sourceCommit/sourceFilesSha256/containerImageId in build.json. The validation driver records these values, immutable source mount, isolated target, resource bounds and all executed binary hashes.

```
cargo build --offline --locked --features bench --example owned_evaluator --example score_mcp_episode -j1
python3 scripts/public-mcp-evaluation.py prepare --track serialized-owned-mcp-synthetic-diagnostic-v1 --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --build /work/output/build.json --out /work/output/inputs
python3 scripts/public-mcp-evaluation.py run --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --source-root /work/source --manifest /work/output/inputs/manifest.json --out /work/output/episode-0
python3 scripts/test_owned_mcp_episode.py --source /work/source --inputs /work/output/inputs --baseline /work/output/episode-0 --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --out /work/output/canaries
```

The supervisor owns two private control pipes and the evaluator schedule. Each actor process inherits only MCP stdio, receives only its current step after prior ACK, and checks both actual control descriptors are closed. Its copied standalone code imports no fixture factory or evaluator module. This is trusted-process isolation, not a security sandbox. Actor/runtime environments are explicit allowlists; fresh private roots, loopback cache-only embeddings, disabled sweeps, no ambient credentials/config/socket. Cache misses fail, never generate.

Actual capabilities must match frozen controlVersion1 exactly. After each delivery, inspect obtains actual committed G; barrier must acknowledge matching lineage, at least target G, all four derived names at its committed G and zero pending work. A second inspect verifies the serialized checkpoint. Raw control attempts/outcomes and readiness receipts remain evaluator-only. Public trace references a post-delivery barrier, never a per-response ReadView. Mismatched ACK, EOF, timeout, stale/missing/duplicate view or unsupported capabilities invalidate the episode; no retry or recovery fiction.

The fixture stores a primary, unlinked records and foreign decoy, then adds a same-namespace neighbor using an acknowledged parentId. Limit1/depth1/compact=false searches require the primary and linked neighbor exactly once in returned UUIDs; unlinked/foreign records excluded. Observed get, feedback and observing graph search must advance G; invalid feedback/false search/cache miss must preserve it. Every checkpoint must recover readiness through actual production completion. Exact per-record exposure-event counts are not exposed by public-v1/control-v1; owner Rust tests and source audit cover deduplication, not this public runtime oracle.

This named synthetic track scores primary memories followed by serialized graphContext.neighbors as the returned rank list; it makes no upstream scorer equivalence claim.

Golden label perturbation compares actual actor actions after explicit per-run UUID mapping, returned source ranks and actual provider vectors; environment hashes/cache identities intentionally change. Future-question canary is absent from actor code/inputs/wire/cache requests before its current delivery. Scorer reports scored, zero-label-unscored and unknown annotation cases separately; source ledger retains seen/attempted/accepted/rejected statuses.

FullA6, publicv2, whole-response generation, deterministic reset, snapshot, logical hash, controlled clock/IDs/RNG remain unavailable and reject in preflight. Frozen launcher accepts only a fresh absent root, so this track makes no restart/reset claim. Newer owner control work is excluded.
