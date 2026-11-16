# Native physical-sweep consumer — validation report

This extends the existing synthetic native sweep runner over actual published
#149 `25dd555db4d23253714a656dcd804210f937cb02` and #147
`50da71abee66cea159988a203d286f570f091718` only. #147 already contains #145/#135;
no additional convenience parent or moving owner branch was imported. Merge
`b3be1b9` resolved one mechanical test-module conflict, retained in
`join-conflicts.patch`. The consumer adds no `system.rs`, ordinary startup, profile resolver, public
writer or config-adoption changes; production physical-sweep behavior comes
from the exact #147 parent.

Frozen implementation/consumer source: `d516b263f0f980f8d940a77489236da6449d3b02`.
The report/evidence commit is a documentation-only descendant of that source.
Publication requires separate branch-specific review; this seal does not claim
publication or integration.

## Compatibility and actual consumer

The internal diagnostic track advances to
`serialized-native-sweep-synthetic-diagnostic-v2`, with checkpoint track
`isolated-native-sweep-checkpoint-v2`. Manifest/source-mapping/trace/control/
checkpoint formats stay 1, public contract 1, canonical schema 2, manual sweep
origin 1, physical-compaction internal contract 1. Diagnostic v2 is not public API2.
The manifest explicitly declares physical compaction and fixtureVersion 2.

Published #149's recorded v1 files remain byte-for-byte unchanged. The actual
negative runner uses those successful historical artifacts, verifies their
hashes, and substitutes the current correct source/image/executable/profile
pins. It rejects at the unsupported track/profile boundary with no output
creation. Independently, the actual restore process rejects a same-binary valid
checkpoint changed only to the old track, with its digest correctly recomputed;
stderr identifies `unsupported checkpoint track`, no destination is created,
and no provider call occurs. Neither case fails merely on stale hashes or
emulates the old no-compaction behavior.

The actor continues to receive only its current serialized public false-read
step. Omitted/default-true and explicit-true recordAccess reject before dispatch;
public store/feedback/forget remain rejected. Sweep is a supervisor-only private
control calling the existing original 1 producer. No disk-layout inference,
second phase formula, clock, compaction algorithm or hash walker was added.

## Results

| Check | Result |
| --- | --- |
| Focused Rust filters | 46 sweep + 32 compaction passes, zero failures/ignored; filter counts overlap and are not added as unique coverage |
| Python checks | 23 passed; repeated in the full run |
| Full library | 1252 passed, zero failed/ignored/filtered; 565.95 seconds |
| CLI/scorer | 2 CLI and 3 scorer tests passed |
| Format/clippy | Both exit 0; clippy 1001 lib warnings, 1076 lib-test warnings (967 duplicates), 1 binary warning (also emitted for its test target) |
| Actual native physical-sweep | Two independently migrated lineages; each 24 public actions per suffix, 3 exact fresh restores, 2 real canaries, 11 constructor negatives and 7 malformed sweep-control rejections per suffix |
| Existing native-prefix track | Same binary: 15 actions/suffix, 3 exact restores, 2 canaries, 10 constructor negatives; sweep remains unavailable |
| Existing legacy-sweep track | Same binary: 3 exact restores and 7 constructor negatives |
| Manifest/builder boundaries | 13 malformed manifests and 5 malformed builder fixtures rejected by actual executables/CLI with no output root/report |
| Compatibility | Real accepted #149 v1 manifest with valid current pins rejected; same-binary old checkpoint track with correct digest rejected before destination/provider |
| Provenance | All 331 source inputs match Git and host pre/post and successful-run container pre/post digests; focused/full example binary hashes identical |

The real fixture keeps three default-namespace records at global multiplier 1
and a fourth foreign-namespace record with actual numeric override 0, set during
original acknowledged ingestion before migration. This is fixtureVersion 2;
fixtureVersion 1 behavior and its accepted input remain unchanged. Five actual
builder negatives cover v1 policy presence/null, wrong/missing v2 policy, and
unknown fixture version, with no output root/report created.

Each sweep scans four records and transitions the three default records. The
Full→Summary and deletion passes select new native text generations, return
real complete `nativeTextGenerationSelected` receipts, retain one full-text
payload and report cleanupPending=false. The Summary→Ghost pass has no physical
receipt. Canonical G advances 4/3/4 (initial 8 → 12 → 15 → 19); graph, vectors, FTS
and entities are all ready at each actual G with pending work 0. Public false
get checks the foreign Full phase, UUID, namespace, summary and exact full text
after every phase and across fresh restores. Foreign search returns that UUID;
default search excludes it and returns no erased UUID after Ghost/deletion.

Each of three genuinely fresh-owner restores matches the original suffix's
complete sweep/physical receipts, state/readiness, all four logical hashes,
public outputs/order/exposed scores, provider requests and vectors exactly.
No field, UUID, score or hash normalization. Episode trace IDs remain distinct;
no complete-trace byte equality is claimed. Label and future-question canaries
exercise real actor inputs/codepaths. Evaluator-label changes intentionally
change environment identity/cache keys, while actor behavior stays equal;
future questions do not change preceding actor requests/vectors/actions.

The sparse scorer remains unchanged: four judged query events, two scored,
one zero-label unscored and one missing-annotation unknown. The deleted query
stays scored with zero recall; no annotation is removed to improve the result.
The source ledger records acknowledged migrated-prefix provenance, not live
public re-ingestion. No upstream dataset/scorer semantics or quality claim.

## Resources, failures and provenance

The initial 4 GiB focused build failed during GNU ld linking, before any test
ran. Log `oom-4g/step-0.log` records signal 9; Docker events identify the same
container `f08b6c4c2b177d3141518fb32ec9914058cc33bb97a5a324528a690107980f4e`
with an OOM event, then exit 101. Its automatic removal precluded recovering that
container's cgroup counters. This is a confirmed build-resource failure, not
an observed runtime failure.

The coordinator approved a bounded 6 GiB adjustment under the existing
isolated-testing authorization, with unchanged source/flags/profile; the full
correctness run retained that cap. Same single private Cargo/target cache,
2 CPUs, `-j1`, pinned image, offline network, immutable read-only source. The
focused successful run's cgroup peak was 5,997,666,304 bytes (5.59 GiB), with
zero oom/oom_kill/max events. The full run peaked at 5,294,837,760 bytes (4.93 GiB), also with zero
oom/oom_kill/max events; exact counters are in `validation.json`.
No linker-flag or build-profile changes, cache clone, arbitrary retry loop or VM restart.
These checks are not comparable performance measurements.

All 331 source/artifact inputs match frozen Git objects and host pre/post hashes;
successful runs additionally hash the immutable inputs inside the container
before compilation and after execution. The actual executed library path is
identified from its test log and hashed; example executables are hashed before
and after the actual consumers. Full and focused example identities match.
Commands, all failed/success logs, exact inputs, control/actor wire, receipts,
scorer outputs and comparisons are retained in this directory. Raw logs retain
their original compiler whitespace; whole-evidence `git diff --check` reports
those spaces/blank lines. Runtime source and Markdown checks are clean.

| Executed binary | SHA256 |
| --- | --- |
| Library `/target/debug/deps/recalld-7025a4141187b374` | `64b34f60d32dc17b9e76ba08edd99b39d626702dfd72c9bb25b32ebae08aa53a` |
| `owned_evaluator` | `0611c3198ce9eb2d6410c2b62baa0f360b0cfb2edb5d99dbd1532e767d7f4fb4` |
| `native_prefix_fixture` | `a16a866dbb23b0cdefe97c971bb01904ff61696682b100a13a00401a00e2ff7b` |
| `score_mcp_episode` | `7b81be6a07c23c84b12f0733d2b9175d416ad9fca63643e88a75206abc97ad5f` |

## Limits and retention

Equality is within each real migrated-prefix lineage. Independent native
genesis/startup equality remains a separate gate; independently created prefixes
may have different genesis IDs and hashes. Public exposed scores are not internal
BM25/composite-score proof. Post-step barriers are not response ReadView metadata.
No ordinary native serving, public native writer/API2, full A6, all-RNG guarantee,
secure erasure of retained history/checkpoints, power-loss test or performance
claim. Existing broader trace/grouping/scorer-adapter/token/public-writer work
remains open; there were no datasets, paid calls or training.

Completed validation containers are removed. These unpublished raw run roots
remain protected until this consumer has a PR/results: native-compaction-d516b263,
native-compaction-6g-d516b263 and native-compaction-full-d516b263 under
`/tmp/recalld-owned-native-controls-validation`. Then central cleanup may delete
those exact disposable runs without a raw HOME archive, retaining source/Git,
recorded reports and the one private Cargo/target cache for assigned follow-up.
During this task central cleanup 22–26 removed 36 owner-released PR150/151/96/153/152 paths,
bringing the ledger to 377 exact actions. Observed concurrent filesystem deltas
are not unique byte savings; the original-repo cleanup ledger retains each guard
and measurement. HOME receives only the concise report/provenance note.

## Exact phase hashes

Each row lists initial, Summary, Ghost and deleted hashes. Each lineage passed
three exact restores; these two independently migrated lineages are deliberately
not asserted equal.

focused-actual-6g:

```
a04e951888ab369d019acbf2bee8a00f5f5a90ba6dcdddcd405c24559382badd
cb83ceb5c8d14084413af6d32742a75a19961417f837e67cbcd71d9ec07e3db0
0a9866c47a1824ecf1d5981a979303bd5fff6dd60f4cb66b94136c333f169d8e
e4e8933357d378aa632d042bcd55df35dc5a7ef3ae0cec01f7f6130fc8a8c836
```

full-6g:

```
b9e6cfc01d6e20f9ea028ca478d46fc746c77b449f89a7adffa319c159f4373b
f2e05ec8a1bdb3f963934fb5aa68225e72d93b9bd8467b7f6aa1b3a622351261
86e7c1c76b74e5336f6b9c2179f9e2d98b9a340f0496401782f7faa9b247e206
2fb7bb63d2e64471708ec5f27d52e0f1f858531e345c4a68b56c7e44236abd1d
```
