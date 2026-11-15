# Actual paired grouped uncertainty — source-only plan

No implementation or experiment in this plan. Necessary direct parent: published
PR161 `6265d05ae9fd52177dc0603d179c7c082d82b755`. It contains the consumed group/split
preflight, complete episode/query rosters and actual grouped scorer. PR163 trace
accounting, native/reset controls and A5 instrumentation are not prerequisites.
Both published branches remain frozen with Halley; their fixture/evidence pins
must not change. New isolated branch/worktree: `impl/a6-grouped-uncertainty` at
`/tmp/recalld-implementation/a6-grouped-uncertainty`.

## Priority and actual missing consumer

A6 §6 requires paired whole-history-group resampling, seed7,10,000 replicates,
paired effect and percentile95% intervals, alongside micro/group-macro counts.
PR161's actual `score_grouped` emits `bootstrap:null`; no uncertainty consumer
exists. Its current baseline has only one scored group per split, so that fixture
cannot support a meaningful interval and must not be relabeled as one.

Source-only local tokenizer inventory found no installed tiktoken/tokenizers/
transformers/sentencepiece and no tokenizer assets/dependency or answer-context
assembler in the actual runner. This is inventory, not a permanent blocker or a
claim no suitable tokenizer exists. Actual pinned tokenizer/context enforcement
remains a priority follow-up requiring version/assets/hash and a consumed context
seam. Bytes/characters and synthetic protocol usage0 will not substitute for
measured model tokens. There is no production-cost claim without priced usage.

The next concrete offline slice can consume PR161's existing paired source units
now, without model assets, paid calls, datasets or a second harness.

## Frozen two-arm fixture before outcomes

Add a distinct `serialized-mcp-paired-grouped-synthetic-diagnostic-v1` track with
explicit pair/uncertainty policy versions, preserving independent manifest1,
sourceMapping1,trace1,groupMap1 and splitMap1. Old161 single-cohort behavior and
historical reports remain unchanged. Both complete arm manifests and their shared
source, label, query schedule, cache, group and split identities freeze before any
arm executes. Whole-pair preflight validates both arms and every member before
output/provider/store side effects; a bad final candidate member cannot follow a
successful baseline run unnoticed.

Reuse the existing seven source histories/five groups and3/1/1 split assignment.
Source grouping remains independent of annotations/future questions. Introduce
only a separately named paired synthetic fixture, with these predeclared labels:

| Episode | q0 relevant source IDs | q3 relevant source IDs |
|---|---|---|
|0|turn0,turn1|turn1|
|1|turn1|turn3 (foreign)|
|2|turn0|turn1|
|3|turn0,turn1|turn0,turn3 (foreign)|
|4|turn0,turn3 (foreign)|turn1|
|5|turn0|turn0|
|6|turn1|turn0,turn1|

Actual IDs retain their existing hyphenated `turn-0` form. Every episode's q1 has
explicit empty labels and q2 lacks annotations. Both arms use these exact same
annotations, questions, source rows and offline vectors. The new fixture gives
source1 vector[0.8,0.6,0,0], while source0/query remain[1,0,0,0], source2 orthogonal
and foreign source3 a decoy. Freeze this fixture before outcomes; it is synthetic
contract data, not upstream semantic labels or a learned/optimized policy.

The only arm difference is actual public `recall_memories.limit=1` versus2, both
`recordAccess:false`, `depth:0`, namespace default. Frozen161 source confirms
`src/mcp/tools.rs:493–510` exposes this limit and `bridge_adapters.rs:212–216`
forwards it to the real query engine (with its existing min_score0). Do not invent
an extra recall minScore argument or private action. Current-step actor projection
receives only its declared retrieval limit, never pair/group/split labels or the
future schedule. The same existing production CLI/MCP and scorer execute both
arms, with separate fresh isolated stores and no retry of unkeyed operations.

Expected fixture behavior to verify, not assume as runtime proof: source0 then
source1 form strict returned order; the candidate adds evidence without selecting
the foreign decoy. Train has three scored groups with different paired effects,
allowing a nondegenerate computation check. Development/test each have one scored
group and must report insufficient eligible groups, not a meaningful interval.
Actual UUIDs, timestamps and independent startup states are retained; no reset,
startup identity or causal quality claim follows from this paired synthetic run.

## Pairing, failures and denominators

The pair manifest declares baseline/candidate roles, exact controlled policy
difference, complete member/query rosters and shared artifact/group/split hashes.
Reject missing/duplicate/unknown arms, episodes or queries, failed/incomplete arm
outputs, different source/label/query universes, cross-split pairing and mismatched
policy/version/provenance. A label-perturbed161 run is **not** a valid comparison
arm; correctly rehashed label changes must reject rather than manufacture effect.

Write planned/attempted/passed/failed/unattempted status for all14 arm-members.
Retain partial outputs and exact failures. No successful comparison/interval may
silently intersect surviving members or omit a failed arm. Scorer validation must
consume and bind actual raw per-episode score inputs, not trust hand-entered means.
Reuse the existing unique-source per-query scorer and group map; no detached
statistics framework or second episode loop.

Within each split, preserve all group/member/query counts, scored observations,
zero-label unscored and unknown annotations. Pair eligibility is determined by
common frozen scored-query annotations, never observed effect. Entirely unscored
groups remain listed and excluded from the statistical denominator with reasons.
Strict same annotations/query roster ensures both arms share the same eligibility;
a discrepancy rejects, rather than silently dropping an arm's query.

Report candidate-minus-baseline point effects for query micro and scored-group
macro separately. Resample eligible **whole groups** with replacement; a selected
group contributes all its scored member/query events, using exactly the same draw
indices in both arms. Do not independently resample arms, split groups into queries,
or mix train/development/test. Stable group/episode/query order fixes floating
accumulation; record draw/index digest and effective denominators for reproduction.

## Deterministic statistical policy and insufficient cases

Requested replicates10,000, seed7, per-split independent deterministic sampler and
explicit percentile convention (nearest-rank2.5%/97.5%). Use an explicitly named
ChaCha12 generator, not unqualified StdRng or the engine's runtime RNG. Current
parent lockfile already pins rand0.8.6, rand_chacha0.3.1, rand_core0.6.4; cached
`rand_chacha0.3.1/src/chacha.rs:344` defines ChaCha12Rng. Add only the direct
example/test dependency on the already cached exact rand_chacha version if needed.
No model download or new statistical package is required.

Pin seed encoding as little-endian u64 followed by zero padding to32bytes, and
bounded u64 index draws using the pinned rand0.8.6 distribution implementation.
Use the actual ChaCha `set_stream` with fixed train0/development1/test2 stream IDs;
these streams are separate evaluator state, not a runtime RNG/control guarantee.
Record exact Cargo.lock/build/algorithm policy; unknown versions/seed encoding or
replicate count reject. This statistical RNG is evaluator computation only; it
must not falsely set the manifest's runtime deterministicRng capability to true.
The reviewed parent Cargo.lock SHA256 is
`2027c0623c819d8aa903a2e0189b4f99be8aae118daac04de48207c5862ca440`;
the inspected cached chacha.rs SHA256 is
`dfd79ed4762e8267148d1776381c71b898808014a4069cfafbc78177247d5fe9`.
Actual implementation must pin its final lockfile, not reuse a pre-change hash.

Zero eligible groups: point effects/intervals null, requested10000/executed0,
explicit unscored reason. One eligible group: finite point effect where defined,
interval null, executed0 and insufficient-eligible-groups reason. Two or more:
execute10000 whole-group paired replicates; identical effects may yield a
mathematically degenerate interval, labeled zero observed dispersion rather than
independent confirmation. Keep the actual three-group synthetic train computation
explicitly insufficient for statistical-quality/lifecycle promotion. A6's real
rights-cleared sample/holdout/power gates remain unmet. No bootstrap count can
create more independent history groups than actually exist.

## Validation and remaining work

- Hand-calculated paired zero/positive/negative effects, unequal group sizes,
  micro versus macro, partial evidence and duplicate source mappings; unknown/
  zero-label/all-unscored, single-group and zero-dispersion cases. Test same-index
  pairing, exact fixed-seed repeat/permutation invariance and policy rejection.
- Actual both-arm manifests/episodes/scorer output, strict source/label pairing,
  nonempty returned results and actual public limit difference. Compare requests
  to the declared single policy change; preserve label/future isolation.
- Correctly rehashed changed labels/splits, missing members/queries, failed arm,
  swapped roles/policy, unknown versions and malicious outcome-dependent eligibility
  cannot produce an interval. Injected failure-ledger tests are labeled as units.
- Targeted Python/scorer-example checks, required formatter/lint and actual episodes;
  no broad production-library rerun for evaluator-only changes. Freeze source and
  actual executable hashes; offline2CPU4GiB, one retained cache, no clones.

Tokenizer/context assembly remains real implementation follow-up, as do upstream
scorer adapters, richer A6 traces/native writer joins and priced cost accounting.
This source plan authorizes no paid calls, data download, training, benchmark
window, default activation, native/full-A6 or semantic-quality promotion.
