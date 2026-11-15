# Native explicit-sweep consumer — validated, unpublished

The new named synthetic track consumes the actual #135 native sweep through the
#143 shared owned core, checkpoint/hash/restore and public false-read actor.
Necessary exact parents are #143 `0f34cd30e48b59dd655b3285d00f8b99e92c32d1` and
#135 `24da9206ba141493cdb9ca988188be6d5690a814`; mechanical merge `e4ed874` is
documented in `PLAN.md` and `join-conflicts.patch`. No ordinary-startup, public
writer, new graph, physical-compaction or moving owner branch was added.

Runtime and actual consumer source: `e1d4cea92f3ae8bed6fc9e1865a5ce05d89bea38`.
All validation completed successfully, including the native sweep consumer,
existing-track regressions and invalid-manifest CLI checks. Both measured runs
used this same source; all three example binaries are identical. This is a local
evidence seal for publication review, not a published or merged change.

| Check | Completed result |
| --- | --- |
| Rust sweep filter | 45 passed, 0 failed/ignored, 1189 filtered; includes actual native manual1/autonomous2, stale/partial-publication/receipt/worker cases and new wrong-schema control admission rejection |
| Python preflight/actor boundary checks | 23 passed |
| Full library | 1234 passed, 0 failed/ignored/filtered, 536.93 seconds |
| CLI and scorer | 2 CLI tests and 3 scorer example tests passed |
| Formatting / clippy | Both exit0; clippy985 library warnings,1058 test warnings (951 duplicates),1 binary warning |
| Actual native suffix | 17 public actions, three real sweeps over four migrated records |
| Fresh-owner restores | Three complete suffixes exactly equal the original; no field normalization |
| Label/future canaries | Two further actual suffixes; evaluator label changes do not affect actor behavior; future changes do not affect preceding requests/vectors/actions |
| Constructor/control negatives | 10: old owner, actual blocked provider worker, unsupported clock, existing/symlink destination, bad manifest digest/source, unbound/wrong-model/wrong-revision profile |
| Malformed sweep controls | Seven rejected per suffix before canonical/runtime change |
| Existing native-prefix regression | Three restores,15 actions per suffix, two canaries and10 negatives; original sweep rejection preserved |
| Legacy sweep regression | Three exact fresh-owner suffix replays and7 negatives |
| Actual invalid-manifest CLI | 10 rejected with nonzero status and no output directory, including false origin/writer/compaction/multiplier/RNG claims and source/binary hash mismatch |
| Provenance | Immutable source manifest equal before/after; example hashes equal before/after execution; offline pinned image, 2 CPU, 4 GiB, `-j1`, one reused private cache |

Each pass reports four actual transitions and advances canonical generation by
four. All four derived components equal that generation with pending work zero.
The sequence is Full → Summary → Ghost → hard deletion. False public get verifies
actual phase and retained/erased text; recall verifies namespace confinement and
absence of erased UUIDs. RecordAccess omitted/default-true/explicit-true and
public writers remain rejected before dispatch. Actual replay-cache misses error;
the blocked provider worker is used only by the supervisor's negative case.

The existing manual clock is advanced once by the schedule. The controller calls
`execute_sweep_with_origin_work(..., 1)` explicitly: schema2 storage does not
choose original API version. #135's autonomous original2 producer is retained
and covered by its same-source focused tests. No new phase formula or clock was
introduced. Reports retain exact exposed transition retrievability; public
scores/order are not described as internal BM25 or composite-score evidence.

First run phase hashes (initial, Summary, Ghost, deleted) are respectively:

```
1e104d0b99869671d28c860e07a84a102a1db032c9c573a04ed2988847f2347d
cbf6ead2c1b9b60e9a969390993dc7d3bdc7feca6288af93f371ac26866fbfbd
30cc5ce4298aa0ea81bde7fbc00737bfbb97181c93c38e0b93ca92bf0214f2c0
232fb6f388789430114246eecd7253099f294eca52ba38fdd9afda6777a8f742
```

These are within one genuinely migrated prefix lineage. A fresh independently
constructed prefix has legitimate different genesis/IDs; this track makes no
independent-native-startup hash claim. Comparison includes all four phase hashes,
complete sweep reports, final state/readiness, public outputs and actual provider
requests/vectors. Trace episode IDs remain distinct; no full-trace byte claim.

Evaluator-label variants intentionally change manifest/environment identity and
replay-cache keys. The consumer verifies those keys are disjoint, while comparing
actual actor inputs/outputs and provider requests/vectors for unchanged behavior.
This is channel-isolation evidence, not a claim that evaluator changes preserve
cache identity. Canonical phase hashes are compared without normalization.

The actual scorer preserves sparse labels: q0 is scored, q1 is zero-label unscored,
q2 annotations are unknown, and deleted q3 is scored with zero retrieval recall.
No labels are changed to make deletion look successful. The four query events
produce two scored, one zero-label and one unknown event. The source ledger's
four accepted turns describe acknowledged migrated-prefix provenance, not public
re-ingestion or semantic preservation.

## Executed provenance

`validation.json` summarizes the exact two measured lineages, results and hashes;
`focused-actual-e1d4cea9/` and `full-e1d4cea9/` retain commands, source manifests,
logs, control/actor wire, mappings, scorer outputs and complete comparisons.
All321 copied source inputs match committed Git objects and match before/after
execution. Source hashes were recorded on the host around the immutable read-only
container mount; this is not a claim of an additional in-container precompile
hash probe. Full-library executable identity is taken from the actual executed
path in the test log and hashed immediately after the test completed.

| Executed binary | SHA256 |
| --- | --- |
| Library `/target/debug/deps/recalld-7025a4141187b374` | `cfd330c0ae9e1d5e5e5bbce7b127c8e415ef27172c4aed8d315ac9b4076ddd63` |
| `owned_evaluator` | `67dc8dac7dd42701af0d662c542e99841f069ebd40a812096dd4964b3580d413` |
| `native_prefix_fixture` | `a0be95280872dfe921c5f89b3dd10bede332b2f0f4931ecc2b2132226a8b7506` |
| `score_mcp_episode` | `a7d2af83b566170fb0a30b75c96a86380a0c50cba2e49224dcbcf5a108af58fe` |

The actual example binaries were hashed before and after consumer execution.
No unexpected build/test/consumer failure occurred in these two runs; expected
negative subprocess exits and their logs are retained. No runtime changes or
additional broad suite repeat followed the full1234 pass. Own validation
containers exited and were removed.

The second measured native lineage independently built another migrated prefix;
its initial/phase hashes differ legitimately from the first lineage and are
retained in `validation.json`. Each lineage independently passed all three exact
restores. No cross-genesis equality assertion was weakened or removed.

## Explicit limits and next dependencies

`nativePhysicalCompaction=false` throughout. Actual #135 skips physical native
compaction; the separate validated #145 plus sweep caller integration is future
work and is not part of this evidence. The actor remains API1 false get/search
only: native store/exposure/feedback/forget/correction/keyed replay require actual
validated public adapters, not private-action substitutes. The original native
prefix track still refuses sweep; the new manifest/checkpoint track admits it
explicitly and preserves that admission on restore.

No full A6, ordinary native serving, public2, all-RNG control, response ReadView,
upstream adapter/golden, quality, reward or training claim. Broader trace,
grouping, scorer-adapter and public-writer gaps remain implementation work as
recorded in the A6 acceptance inventory. No datasets, paid calls or training.

## Retention

Both validation containers are gone. Unpublished raw roots
`/tmp/recalld-owned-native-controls-validation/native-sweep-e1d4cea9` and
`native-sweep-full-e1d4cea9` remain protected until this slice has a resulting PR;
then the central cleanup owner may delete them without a raw HOME archive.
Preserve this source worktree, recorded results and the one private `cargo`/
`target` pair under `/tmp/recalld-owned-native-controls-validation` for the next
assigned consumer. During validation central cleanup completed owner-released
PR58/145/146 scratch batches17–19 (6 paths; observed concurrent deltas retained
separately); cumulative333 central exact path actions. These are not333 distinct
experiments or uniquely attributable allocated-byte savings. The separate
original-repo resource-cleanup ledger records all guards and before/after space.
