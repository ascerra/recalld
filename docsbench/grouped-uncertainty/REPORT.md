# Actual paired whole-group uncertainty consumer

**PASS — synthetic public-v1 contract diagnostic only.** Final executed source
`427c57e9e00bfa32f09edff22a479b31e656d269`, necessary direct parent published
#161 `6265d05ae9fd52177dc0603d179c7c082d82b755`. No trace/native/reset/A5 parent
was added. The evidence seal changes documentation only.

The existing runner executed fourteen final episodes /210 acknowledged actions,
seven members per arm, with56 measured query events. Each arm has28 queries:
14scored,7zero-label unscored,7unknown annotations. All planned members passed;
none were intersected away. Eight manifest negatives reject before run output,
provider or store creation. Seven actual Rust scorer negatives reject without
comparison output. Permuting arms, episodes, roster and query rows yields exactly
the same statistics and draw hash. `final-tail/actual/validation.json` records the
checks; full wire, source mappings, score inputs, status and negative logs are retained.

## Actual consumer and isolation

New named track `serialized-mcp-paired-grouped-synthetic-diagnostic-v1` consumes
both arms through existing `grouped-mcp-evaluation.py::run` and
`public-mcp-evaluation.py::run_episode`. `score_mcp_episode --paired-input` consumes
raw per-episode scores, group/split maps, both manifests, shared contract and
complete status/query rosters; this is not a detached statistics helper.

Before either arm runs, the common contract and both full manifests freeze source,
labels, schedules, vectors, group/split identities and actual binary/source pins.
Whole-pair preflight validates every member, including the last candidate cache.
`frozen-before-outcomes.json` is captured before execution and checked unchanged
afterward. Groups are computed from source histories before labels/cache assembly.
The inherited exact-history grouping limitation remains: no semantic duplicate detector.

The only intervention is real public `recall_memories.limit=1` versus2, with
namespace default,depth0,recordAccessfalse. The second source has predeclared
vector[0.8,0.6,0,0]. Every actual query returns mapped source0 for baseline and
source0,source1 for candidate, with the foreign decoy excluded. All four current
queries execute, including zero-label and unknown queries. Labels remain evaluator-only.

For all seven paired members, actor code/current-step inputs and provider
inputs/vectors match. Every actor attempt/call is compared in order, including
replay-miss: its declared per-arm limit is asserted first, then only that limit
field is removed for request equality. No other request field is dropped.
Provider comparisons exclude environment/cache-key identity because arm manifests
necessarily differ; they compare actual provider requests, responses and vectors.
This is not a claim of identical cache identities or independently generated UUIDs.
Existing future-query and evaluator-label isolation checks pass on all episodes.

The intentional uncached query produces the actual provider400/tool error and
fails closed in each episode; it is not a generation/fallback call. Old single and
grouped fixture definitions remain pinned and unchanged. Existing version absence
and ordinary control gates remain: public1, canonical schema unattested/null,
no ReadView/barrier/reset/clock/ID guarantee. Statistical RNG is not runtime RNG.

## Statistical contract and actual denominators

| Split | Groups eligible | Queries per arm | Scored /zero /unknown per arm | Replicates executed | Interval |
|---|---:|---:|---|---:|---|
| train |3|16|8 /4 /4|10,000|synthetic diagnostic only|
| development |1|8|4 /2 /2|0|null: insufficient groups|
| test |1|4|2 /1 /1|0|null: insufficient groups|

Within a split, canonical group IDs order the population. Each replicate samples
whole groups with replacement; one draw-weight vector is reused for both arms.
Query-micro weights all scored queries in sampled groups; group-macro gives each
sampled group's scored mean equal weight. Unknown/empty annotations are never
zeros in score denominators. Entirely unscored groups retain their summaries and
an exclusion reason. Zero eligible groups return null point/interval; one returns
a point but null interval. These boundary cases are focused Rust proof; the actual
fixture has all five groups eligible, so no actual all-unknown group is claimed.

Training Recall@5/10/20 query-micro baseline0.375,candidate0.875,effect0.5;
group-macro baseline0.4583333333333333,candidate0.9166666666666666,
effect0.4583333333333333. Both diagnostic percentile intervals are[0,0.75].
Extraction-retention effect is0 with interval[0,0] and explicit
`zeroObservedDispersion:true`. All-required and nDCG values are retained verbatim
in `final-tail/actual/paired/paired-scores.json`. Three synthetic training groups
cannot support quality/promotion claims; every output declares promotionfalse.

Pinned statistical policy: `rand_chacha`0.3.1 explicit ChaCha12Rng,
`rand`0.8.6 unsigned64 `gen_range`, seed7 encoded unsigned64 little-endian followed
by24zero bytes, streams train0/development1/test2. Draw hash concatenates sampled
zero-based unsigned64 little-endian indices in replicate order. Nearest-rank2.5%
and97.5% use sorted indices249/9749 of10,000 effects. Training draw SHA256:
`11d835c8b5a8155e86573bafd30b8f167eec48b79f27f6f5e71a40ee70d76eea`.
Final Cargo.lock SHA256:
`a6f73ed9223bb1d6a7a02f893067d8c537c77aa515b5837d33a041271400669e`.
`statistical-policy.json` pins final Cargo manifests and cached implementation
source hashes. Only an exact example/test `rand_chacha` dependency was added;
its already-locked package version is unchanged. No package/model/dataset download.

## Validation and preserved failures

- `edc2394b89cf21c67dee0c4c36bf90380ef180a9`: fmt and Python14+8 passed;
  scorer compilation failed because the pinned SHA digest array lacks LowerHex.
  No test/episode ran after that failure. Original steps include old cache-glob
  binary hashes; those are explicitly historical inventory, **not executed proof**.
- `93598b713feb8da18a0df7ccbe3da61f5ecd1970`: bytewise digest encoding fixed the
  compile error. Eight focused Rust scorer tests(three new,five inherited),
  Python22unique checks,fmt,actual CLI/scorer build and scorer Clippy passed.
  Fourteen actual episodes and paired scoring passed. The outer audit checker
  then failed because it removed limit differences only for `query:*`, omitting
  the real intentional `replay-miss` recall. Exact wire diff is retained; this was
  a checker scope error, not an engine/scoring failure.
- Final `427c57e9`: only that Python checker changed. All Rust/Cargo hashes equal
  93598. Reused focused Rust/fmt/Clippy evidence; reran Python22 and rebuilt the
  actual scorer for changed embedded source pins, then executed a fresh14episode
  pair and all negatives/permutation checks successfully. No broad library repeat.

Failed-arm ledger proof is an injected Python first-episode fault: all14 members
remain,1failed/13unattempted, candidate unattempted, no comparison. The scorer also
rejects a correctly rehashed failed-candidate status, missing/duplicate arms or
members, an omitted unknown query and mismatched labels. These are deliberate
contract tests, not claims that an actual engine episode failed.

All239 tracked inputs match committed Git, host snapshots and inside-container
pre/post hashes in all three runs. Actual final executable hashes are checked
before/after the consumer:

- Production CLI: `05a8384b5444a18cae2e4fdd8ddb2a894a209b4240379ad79091bb4f94fed2e3` (byte-identical93598→427c57).
- Final scorer: `d3549cb2488d1e3304b6a1f2cc0a6ac50e223ff061bdc896b797b30c13f6e74f`.
- Executed93598scorer-test binary: `be7d0d10b1f899e9e3494358300cd4ea006188f2f9cd9250df0efdaa53806fe1` (captured after execution, exact Cargo run path).

Pinned Rust1.94.1 image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`;
networknone,2CPU,4GiB memory/swap,-j1,read-only source/root,one reused private
Cargo/target and shared registry read-only. No ambient HOME/config/credentials/
personal daemon/socket/store. Initial compile-failure peak2,509,955,072bytes;
93598peak4,294,967,296 with memory.max events2601,OOM/kill0; final-tail
peak4,269,314,048 with max/OOM/kill0. These are correctness checks under memory
pressure, not performance/headroom evidence. Existing library warnings remain;
no new paired-scorer Clippy warning. Containers were removed after capture.

## Limits and cleanup

This closes a consumed paired-uncertainty diagnostic gap, not full A6. No upstream
benchmark/scorer/rights semantics, training, pristine holdout, native/public2
activation, deterministic runtime reset or semantic quality is claimed. Actual
pinned tokenizer/context assembly/enforcement remains the next owned gap; no
byte/character/token estimate or fabricated production cost substitutes for it.

Three raw runs remain protected pending resulting PR and recorded publication;
then the central cleanup owner will delete those exact roots under the retention
policy. Source/worktree/reports/HOME and the one reusable cache remain protected.
Separately, central batches37–39 removed12 released completed scratch paths,
bringing the ledger to421 exact actions; observed batch free-space changes were
177,725,440,62,722,048 and572,858,368bytes, not exclusive allocation. Latest batch39
free space was1,240.155GiB. No unconfirmed/active path was deleted.
