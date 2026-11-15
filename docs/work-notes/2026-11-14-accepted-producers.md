# 2026-11-14: accepted producer registration

Question: can the existing accepted-work owner see actual queued producers before
file/provider preparation and keep their descendants visible through publication
while new root admission is closed?

Parents: PR70 af20d7331410feef8ea1d2a8803e6a0772bb5d99 and PR78
bec3bb5aa4e9525efed3170320e29c5413575d56. Worktree:
`/tmp/recalld-implementation/accepted-producers`, branch `impl/accepted-producers`.
No personal store, provider, daemon, credentials, socket mount or deployed service.

Initial focused run used `recalld-ci:1.94`, installed toolchain 1.94.1,
`--network none --cpus 2 --memory 4g`, `CARGO_BUILD_JOBS=2`, private cargo/target.
The copied target was package-cleaned before first compilation; no before/after
source switching reused an executable. Eleven new focused tests passed. A graph
test initially asserted a peer counter for an edge type outside that counter's
semantics; it was corrected to inspect the canonical persisted edge and published
graph. This was a test-expectation correction, not a before/after engine defect.

The next A5 hold was acknowledged after the focused container exited 0. No heavy
job remained and none was started afterward. Light review added queued-scheduler
shutdown abandonment, direct graph-node admission, and same-owner publication
repair with the original receipt tests. These additions await
validation after explicit END; do not treat the earlier 11 passes as validation
of those later edits.

After explicit END, all eight producer integration tests passed, including the
three light-review additions. The final read-only run passed 906 library and two
CLI tests; four doctests were ignored. The script then exited 101 requesting an
`offline_eval` example absent from the actual 70+78 ancestry. That failure is
retained. No unrelated dependency was added and no passed full suite was rerun.
Separate format checks passed and Clippy exited 0 (554 library warnings, 614 test
warnings including 541 duplicates, one binary warning), on the unchanged source.

All 198 runtime files have matching before/after hashes. Final source/binary
hashes, exact commands and integration diff are retained in
`docs/evidence/accepted-producers`. That final frozen run includes all fourteen
new regressions; earlier development logs are not substitutes for its provenance.
Scope and integration seams are in `docs/implementation/accepted-producers.md`.
No whole-core readiness, public CAS, native-v2 activation, OS deadline or power-loss
conclusion follows. No deploy or main-branch merge is authorized.

Frozen runtime: `209ce38ab6949c1231d54c099a98f48c9239c4b5`. See the evidence manifest for exact source and
test-executable hashes. Publication is now explicitly authorized as a minimal PR
against `impl/canonical-writer-guards` (PR70), with PR78 as an additional prerequisite.
