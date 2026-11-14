# Owned native catalog/policy allocator evidence

Actual parents124 `a5865da02969631a5031f65c6e43269701d7a254` and134
`b62d1e2f4b3f516b5889946605946e274f82286c`. Final tested source0a175af;
production correction73dd4c0. See ../../implementation/owned-native-catalog-ids.md.

`before` is the actual four-failure allocator baseline. `recovery-expectation-failed`
is four passing catalog/policy groups plus the author's incorrect test expectation
about automatic purge recovery. `corrected` contains the exact full passing run.
The latter test correction does not change production code or relax guards.

Each run's gzipped manifest records exact source files/origins, image, command,
exit, and hashes. `.log.gz` files decompress to the original raw logs. The source
binary manifest links executable hashes to source. The public harness is a frozen
separately-owned input, explicitly identified in the source manifests. The public
and owned evaluator fixtures are synthetic and bounded; they do not establish
ordinary native admission, publicAPI2, A2 quality, or whole-state reset.

The two `prior-context-cleanup-*` manifests record completed PR134 cache release
and the supplemental verified HOME executable archives. No runtime or frozen
PR134 branch was changed. The designated cleanup owner performs deletions only
after checking exact paths and active mounts. Worktrees and unarchived failures
are never released.
