# Actual source-grouped public MCP evaluation consumer

**PASS — bounded synthetic contract diagnostic.** Frozen evaluator source:
`6af05d7544ea634960ce198f67ce5649ce427419`. Direct necessary parent is published
#90 `003d857dd99acca9eb33801fa6c5d7b137d124a2`; no additional runtime parent.
No #92/native/control/sweep/compaction ancestry was added for convenience.
The subsequent evidence seal adds documentation/output auditing only.

The existing production MCP stdio runner executed 15 episodes / 225 acknowledged
actions: seven baseline cohort members, seven with changed evaluator labels, and
one old-profile episode. All passed. The two grouped runs contain 56 measured
query events. Targeted checks passed: 14 Python tests (including seven inherited
preflight tests), five Rust scorer/example tests, fmt, actual binary build and
scorer-target clippy. No broad library suite was rerun. There was no unexpected
candidate failure; all 12 command-level negative outcomes below were expected,
asserted and retained. Preliminary host Python passes are retained separately.

## Change and compatibility

`grouped-mcp-evaluation.py::{registry,maps,prepare,validate,run}` supplies a consumed
cohort dispatch for the existing `public-mcp-evaluation.py::run_episode`.
Seven synthetic source histories produce five deterministic connected components:
exact-history duplicates episode0/1, declared shared-history aliases episode2/3,
and three independent histories. Typed user/conversation/history/alias relations
and exact ordered role/namespace/content fingerprints select membership. Source
IDs/timestamps are excluded from this exact-history fingerprint but retained
unchanged in source mappings. This is not semantic near-duplicate detection.

Groups are built before cache/label construction. The full cohort is revalidated
before the first output directory, provider or store: source/binary pins, all
artifacts, recomputed groups and whole-group split assignments, including the
last episode. Correctly rehashed split/duplicate changes cannot bypass this gate.
Sorted SHA256(`a6-split-v1` + group ID), floor60%/floor20%/remainder produces three
train, one development and one test group. Synthetic execution covers every split;
there is no training, tuning or pristine-holdout claim.

`score_mcp_episode.rs::score_grouped` consumes the actual runner's raw score inputs,
checks member/query rosters and map/input hashes, binds each episode environment
to its cohort/member/query roster, then reuses the existing unique-source scorer.
Canonical group/episode/query accumulation yields stable aggregate output under
input permutations. Missing/failed/duplicate episodes and omitted queries cannot
silently shrink denominators. Every planned episode remains in the status ledger,
including unattempted members following failure. Failure-ledger accounting is
proved by an injected Python unit failure; no actual engine failure is invented.

New track: `serialized-mcp-grouped-synthetic-diagnostic-v1`; independent
manifest/sourceMapping/trace versions remain1, with groupMapVersion1,
splitMapVersion1 and aggregate profile `synthetic-grouped-unique-source-v1`.
The old named profile/fixture remains valid on the same binary, while grouped
fields under the old track reject. Historical studies stay pinned to their old
source/executables. Production `src/`, Cargo files and `public-mcp-actor.py` are
byte-identical to #90. The CLI uses ordinary isolated direct fallback, with
sweep/autolink/RIF disabled, false-access recall and a synthetic offline provider.

## Actual denominators and source semantics

The source-derived map and split assignments are byte-identical and semantically
identical under label perturbation. The output auditor verifies actual source rows,
all four acknowledged source memberships per member, context group/split IDs,
accepted-event ledgers, trace group IDs and the complete q0–q3 roster in both runs.
It retains each run's real public UUIDs; it does not rename IDs or rewrite outputs.
All actual limit-one queries return the source0 mapping, not the foreign decoy.

| Baseline split | Queries | Scored | Zero-label | Unknown | Groups / scored / unscored | Recall@5 | All-required rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| train | 16 | 4 | 6 | 6 | 3 / 1 / 2 | 0.375 | 0.25 |
| development | 8 | 4 | 0 | 4 | 1 / 1 / 0 | 0.625 | 0.5 |
| test | 4 | 1 | 0 | 3 | 1 / 1 / 0 | 1.0 | 1.0 |

Episode5's wholly unknown group and episode6's zero-label-only group each remain
present with four events, zero scored observations and null means/all-required
rates. They do not enter the macro denominator. Each baseline split happens to
have one scored group, so its observed query-micro and group-macro values agree.
The nonvacuous unequal-weight distinction is separately proved in the actual Rust
scorer test: three perfect queries in one group plus one empty query in another
give micro0.75 versus macro0.5, with another wholly unknown group excluded.
All-zero-label development and all-unknown test splits have null macro means.
Inherited hand-calculated tests cover perfect/empty/partial/duplicate/unknown
unique-source Recall/nDCG/all-required and ledger pending/rejected/no-op/truncated.

Changing labels to q0→foreign source3 for every episode yields seven scored and
21 unknown queries, Recall@5=0 across splits, while all source/group/split maps,
actor code/current-step inputs, actual requests, provider inputs and returned
embedding vectors stay equal. The environment hash and replay-cache keys change,
as required by changed evaluator provenance. Raw result UUIDs/timestamps are
retained without independent-startup/output equality claims. Both real cohorts
withhold the future-query canary until delivery4; evaluator label canaries remain
absent from actor inputs/code/wire/provider/trace. There is one real fail-closed
provider miss per episode, with no generate/fallback path.

## Negative coverage and retained evidence

Eight actual runner preflight negatives: final-member missing cache entry,
correctly rehashed duplicate-history split, rehashed assignment mismatch,
unsupported version, missing member, duplicate member, unsupported reset claim,
and grouped fields under the old track. Each rejects before its run-output path
exists. Rehashed duplicate/split cases assert the specific semantic map error,
not a generic bad digest. Four actual scorer CLI negatives reject without output:
missing episode, duplicate episode, omitted unknown query and a correctly hashed
raw input moved from another member. Reversed episode/roster/query input order
produces exactly equal aggregate JSON. Malformed content hashes, relation chains,
registry permutations and failed/unattempted ledger counts have targeted unit proof.

Evidence: `evidence/actual/receipts.json` retains every command/exit classification;
`evidence/actual/{baseline,changed-labels}` retains actual wire, traces, source
mappings, ledgers, per-episode/aggregate scorer input/output and canary comparisons.
`evidence/output-audit-final.json` records the semantic audit and hashes every
actual input file. `audit_outputs.py` reproduces it against retained outputs.
`evidence/step-2.log` identifies the actual executed Rust test binary. The broader
cache file list in `steps.json` includes other retained executables; those are
not counted as executed coverage. `validation.json` selects the executed one.

## Provenance, resources and limits

All 235 source inputs match the frozen Git blobs and host/container pre/post
hashes. Immutable read-only source, network-none Docker, no ambient credentials,
personal store or daemon, two CPUs, 4GiB memory/swap cap, cargo offline/locked/-j1,
and one reused private Cargo/target cache with the shared registry mounted read-only.
Image: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
The run's cgroup peak reached4,294,967,296 bytes; `max=3673`, `oom=0`, `oom_kill=0`.
This records memory-limit pressure, not headroom or performance evidence. No retry
or flag/resource change occurred. Build emitted12 existing library warnings;
clippy completed0 with587 library warnings. No new broad lint-clean claim.

Executed SHA256:

- Production CLI: `603bfa58f26c2e5eb66c3dc73372d5ed910ed014e087593a7f78e8aa1ccdfcf8`.
- Actual scorer: `caa723e3f786b87cfa4861e7797a814f7433117692a2763ca00dd66d48a71adb`.
- Executed scorer test: `c8c2937e3b35cc11eb23c051c06fee24359f56113a99dca5b7d095ae5b840b2d`.

Actual CLI/scorer hashes are checked before and after episodes. The documented
extra output audit reads recorded evidence only and does not change source or
runtime results. Full command/source/executable metadata is retained, along with
all expected negative logs; no raw database or binary archive is needed.

This is a synthetic legacy public API1 comparator, canonical schema null and
unattested. It proves no native/ordinary startup readiness, public2, coherent
ReadView/barrier/reset, controlled clock/RNG, independent startup identity,
upstream dataset/license/scorer semantics, semantic quality or full A6. Deferred
work includes rights-cleared adapters/goldens, semantic dedup, richer trace/token
accounting, group-paired bootstrap/uncertainty, lifecycle promotion samples and
actual native writer consumer coverage. These are not silently converted to
experiment-only deferrals, and no paid models/datasets/training were used.

## Ownership and cleanup

Branch `impl/a6-grouped-evaluation` is unpublished pending coordinator review.
Published PR154 remains frozen with Halley. The one current raw validation root
`/tmp/recalld-owned-native-controls-validation/grouped-6af05d75` stays protected
until publication/results, then central cleanup may remove it under the retention
override. Keep the single future Cargo/target cache; no large clone was created.
Compact report/results are copied to HOME with a verified copy manifest and a
notebook Log entry. Central cleanup29/30 removed15 separately owner-released paths
and now totals400 exact successful path actions; observed batch free-space deltas
were317,575,168 and577,171,456 bytes, including concurrent/APFS effects, not exclusive
or per-path reclaimed allocation. No active worktree/source/report was removed.
