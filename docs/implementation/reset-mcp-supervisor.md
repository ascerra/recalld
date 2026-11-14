# Fresh-owner serialized checkpoint diagnostic (preparation)

This consumer is prepared on published109 `9027b5307dce09bee88aa57baf9f50cc60b2c5d6` for Halley's owned checkpoint constructor. The owner-confirmed interface was inspected at `f9f75e0d297a49ae7cd0aa416c782f4760374b3e`; the validated final owner parent is still pending. This document is an implementation plan, not runtime proof. No unvalidated owner parent is imported or published.

The new track `serialized-owned-reset-synthetic-diagnostic-v1` retains independent manifest/sourceMapping/trace versions1, publicContractVersion1, privateControlVersion1 and checkpointVersion1. Canonical schema remains null/unattested. It requests only the owned lifecycle clock, canonical allocation stream, snapshot and fresh-owner checkpoint constructor. Reset RPC, all-clock/IDs/RNG determinism, whole-response ReadView, publicv2 and fullA6 remain unsupported and fail preflight.

`public-mcp-evaluation.py prepare --track serialized-owned-reset-synthetic-diagnostic-v1` emits the strict manifest and checked-in synthetic fixture artifacts. `run` consumes that manifest and the actual `owned_evaluator` example through inherited control pipes and public MCP stdio. The runner owns subprocess termination/join and current-step actor delivery; the core owns checkpoint capture, canonical validation/hash/copy, allocation/clock/cache restoration, locks and derived rebuild. It never opens another store for an oracle or emulates reset by copying files.

Sequence: actual prefix stores and two warm repeated queries; inspect/barrier/snapshot; original suffix; close private control input, wait for process exit and join every actor; construct three sequential fresh sibling roots using `--restore-checkpoint PATH --checkpoint-sha256 HEX`. Each restored child must match prefix runtime state and actual checkpoint logical hash, generation and all-four-view readiness before actor admission. The actual public suffix advances lifecycle time, stores a replacement, observes/feeds back, recalls, forgets, reads the content-free tombstone, records invalid quality and an offline replay miss. Complete semantic tool results, actual IDs, runtime state, readiness, logical suffix hash and provider requests must match the original suffix without normalization. Distinct vectors, depth0 and explicit limit avoid an all-legacy-ties claim.

Negative canaries exercise active old owner, a real blocked provider worker in a restored owner, existing and symlink destinations, wrong checkpoint digest and foreign source. The supervisor's positive constructor path rejects an unjoined prior owner before launching anything; separate deliberate negative constructor attempts exercise actual core rejection. Unsupported checkpoint clock must reject snapshot before publishing a manifest. Original source/checkpoint bytes and destination sentinels remain verified; these byte hashes are not substituted for the core's logical hash.

The actor script remains unchanged from109, standalone and evaluator-free. It inherits only MCP descriptors; both actual private supervisor descriptors are checked closed. Each subprocess receives only its current step after prior ACK. Snapshot paths, future query schedules, labels, runtime/seed/cursor state and controls remain evaluator-only. This is trusted-process isolation, not a security sandbox.

Scorer and source ledger consume actual public ACK mappings and returned IDs. Prefix mappings are explicitly inherited from the acknowledged checkpoint prefix and hash-referenced. Label perturbation runs the real complete consumer again; actor actions/IDs/vectors and source ranks must remain stable while environment/cache identities and evaluator metrics change. Future question canary is absent until delivery. Repeated queries must reach the cache-only fixture; production owned provider caching is disabled by the owner constructor. No downloads, paid calls, training or upstream scorer/license/revision claims.

Validation commands after the owner supplies a validated frozen parent:

```
python3 scripts/test_public_mcp_evaluation.py
python3 scripts/public-mcp-evaluation.py prepare --track serialized-owned-reset-synthetic-diagnostic-v1 --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --build /work/output/build.json --out /work/output/inputs
python3 scripts/public-mcp-evaluation.py run --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --source-root /work/source --manifest /work/output/inputs/manifest.json --out /work/output/episode-0
python3 scripts/test_reset_mcp_episode.py --source /work/source --inputs /work/output/inputs --baseline /work/output/episode-0 --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --out /work/output/canaries
```

All runtime validation uses immutable source, a new source-specific target, pinned offline Docker, 2CPU/4GiB/-j1 and source/executable hashes. Prior109 study evidence stays pinned; its old bounded capability handshake intentionally does not accept this expanded endpoint.
