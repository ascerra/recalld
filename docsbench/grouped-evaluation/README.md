# Synthetic grouped public MCP diagnostic

This is an extension of the existing #90 serialized public runner and scorer,
directly based on `003d857dd99acca9eb33801fa6c5d7b137d124a2`. Production library,
CLI handlers and the separate current-question actor are unchanged. It is not an
upstream dataset/scorer adapter, training experiment, quality result or full A6.

Inside the existing isolated offline container, with source-pinned build metadata
and matching production CLI/scorer binaries:

```sh
python3 scripts/public-mcp-evaluation.py prepare \
  --track serialized-mcp-grouped-synthetic-diagnostic-v1 \
  --binary /target/debug/recalld --scorer /target/debug/examples/score_mcp_episode \
  --build /output/build.json --out /output/grouped-inputs
python3 scripts/public-mcp-evaluation.py run \
  --binary /target/debug/recalld --scorer /target/debug/examples/score_mcp_episode \
  --source-root /work/source --manifest /output/grouped-inputs/manifest.json \
  --out /output/grouped-run
```

The generated seven-episode fixture has five source-derived groups. Typed shared
user/conversation/history/alias relations and exact ordered role/namespace/content
fingerprints form connected components. Fingerprints exclude source IDs and
source timestamps; those fields remain intact in source mappings. This handles
exact history duplicates and declared relations only, not semantic near-duplicates.
Questions, answers, labels, returned results and cache hits cannot select groups.
Whole-cohort preflight recomputes group/split maps and validates all artifacts,
including the final episode, before creating output, a provider or a store.

Whole groups are assigned by sorted SHA256(`a6-split-v1` + group ID), floor60%
train/floor20% development/remainder test: three/one/one groups here. All splits
execute synthetic contract checks; there is no training or pristine holdout claim.
The current manifestVersion1 is distinct from groupMapVersion1, splitMapVersion1
and the named `synthetic-grouped-unique-source-v1` aggregate profile. Canonical
schema remains null/unattested via public API1. Public2, native readiness, reset,
clock/RNG control, snapshots, barriers and coherent ReadViews remain unavailable.

`episode-status.json` retains every planned member, attempted/failed/unattempted
status and counts. No successful aggregate is emitted if an episode fails.
`grouped-score-input.json` binds each raw episode score input and expected query
roster to the cohort environment and map hashes. Missing/duplicate episodes or
queries and a valid input moved from another member fail closed in the actual
scorer. The scorer reuses the existing unique-source per-query metrics and emits
per-split query micro, scored-group macro and per-group summaries. Unknown and
zero-label observations remain unscored with explicit counts; wholly unscored
groups stay present but do not enter the macro mean. Empty denominators are null.
Canonical group/episode/query ordering fixes aggregation order. No bootstrap or
confidence interval is claimed.

The old default `serialized-mcp-synthetic-diagnostic-v1` keeps its fixture and
output semantics. Historical frozen studies retain their old source/executable
pins; they must not be rerun with this source under their original evidence name.
The same-binary old-profile positive and new fields on the old profile negative
are included in validation.

Targeted checks are `scripts/test_grouped_mcp_evaluation.py`, `cargo test --example
score_mcp_episode`, fmt/clippy, and `scripts/check_grouped_mcp_evaluation.py` against
actual serialized episodes. The last checker runs baseline/label-perturbed cohorts,
an old-profile episode, rehashed map/split negatives, missing roster/query cases
and scorer input permutations. It compares actor code, current-step inputs,
requests and provider vectors exactly; environment/cache identities deliberately
change when evaluator labels change. Independent public UUIDs/timestamps/outputs
are retained, not normalized or asserted equal without owned reset.
