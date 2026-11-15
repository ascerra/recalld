# Native explicit sweep consumer

Necessary parents: published #143 `0f34cd30e48b59dd655b3285d00f8b99e92c32d1`
(actual #130 + #138 shared native owner, profile admission, hash and restore) and
published #135 `24da9206ba141493cdb9ca988188be6d5690a814` (native sweep and explicit
manual/autonomous origin). Merge `e4ed874` retains the native receipt/module
additions already present in #143 and both parents' test modules; original four
mechanical conflicts are retained in `join-conflicts.patch`.

New manifest track `serialized-native-sweep-synthetic-diagnostic-v1` uses
`isolated-native-sweep-checkpoint-v1`, explicitly selected by
`owned_evaluator --native-prefix --native-sweep`. The old native-prefix track
still rejects sweep. Restore admits the checkpoint's exact named capability.
The actor still has only explicit `recordAccess=false` API1 get/search; omitted,
true, writes and API2 fail before dispatch. No public writer activation is implied.

The inherited controller samples its existing owned clock and calls the actual
#135 runner with explicit original contract1. Disk schema does not choose API
origin. Background sweeps, autolink and RIF remain disabled; multiplier1 is pinned
in this track. No second planner, clock, phase formula, engine or registry.
Physical native compaction remains false: #135 deliberately skips it. Feynman's
validated compaction companion is a separate dependency, not claimed here.

Four acknowledged migrated synthetic prefix records provide full/summary/ghost/
hard-delete public payload observations. The evaluator supplies clock advance
and three real sweeps; each report, phase hash, post-step barrier and public
response is saved before assertions. Three fresh-owner restores must match exact
hashes, output ordering/exposed scores, clock/ID state and sweep reports. Existing
source/destination/old-worker negatives, provider-profile rejection and label/
future-query canaries remain. Missing annotations remain unknown; zero labels
remain unscored; deleted annotated queries are not normalized away.

Validation completed; see REPORT.md and validation.json for measured evidence. No
independent native genesis, full A6, all-RNG control, ordinary native serving,
native public writer, semantic quality or physical compaction claim.
