# Owned allocator HOME seal and cleanup handoff

Validated source `0a175afd9167e1147a3fc12b70eaa0fb055db75f`; production change
`73dd4c0410a1147fb40124783dd50f80e92df829`. Evidence commit
`057f86f0e519b1deb4b3f42e4f000fdc6ce2cab6` changes no runtime/build inputs.
All-feature1171 library+2CLI, five focused groups, fmt/Clippy with warnings,
owned evaluator, nine public episodes and seven harness checks passed. See the
[full report](owned-native-catalog-ids.md) for exact scope and both failed runs.

Actual HOME artifact root:
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-owned-native-catalog-ids-0a175af`.

- 68 repository evidence/report/preparation files copied and individually verified.
- `COPY-MANIFEST.json` SHA256:
  `73d9767eb4dddb8623f53c0e36b7a51b571b878d8a42cd103404329f5b528fb7`.
- Three exact source snapshots and five actual executed library/server/evaluator
  binaries were archived before the user's later retention override. Archive
  total143273248 bytes; `ARCHIVE-MANIFEST.json` SHA256:
  `b1a9fd1ef15df2908df53eaf5304f44899b388e13a78912e5a977277d47343f0`.
- The existing `Recalld correctness and retrieval research.md` notebook has a
  dated entry with source/runtime IDs, actual results, failures and HOME links.

No additional Rust run was performed for evidence or this pointer document.
The snapshot manifest deliberately describes evidence commit057f86f, not this
later pointer. It does not recursively include its own HOME-copy manifest.

## Current retention rule and exact cleanup scope

The user subsequently overrode archive-before-cleanup: completed experiment plus
resulting PR plus recorded results permits deletion of raw run data, including
failed candidate data. No archive/hash prerequisite remains; no further HOME
archives should be created merely to delete temporary data. Earlier evidence
manifests record what had already been done, not a continuing retention rule.

The dedicated cleanup owner alone deletes. No owned build/test/container is
active and no new large job starts while the disk-pressure hold remains. This
agent has deleted **0 bytes directly**; actual removed-byte counts belong to the
cleanup owner's report and are not inferred from release authorization.

Whole completed historical standalone roots released, with tracked results:

| Root under /tmp | Resulting PR | Preserved report worktree/path |
| --- | --- | --- |
| recalld-native-catalog-policy-validation |87 | native-catalog-policy/docs/evidence/native-catalog-creation/README.md |
| recalld-native-numeric-policy-validation |94 | native-numeric-policy/docs/evidence/native-numeric-policy/README.md |
| recalld-native-purge-validation |105 | native-purge/docs/evidence/native-purge/README.md |
| recalld-native-mixed-history-validation |111 | native-mixed-history/docs/evidence/native-mixed-history/README.md |
| recalld-native-replay-binding-validation |121, actual helper consumer | native-replay-binding/docs/evidence/native-replay-binding/README.md |
| recalld-native-replay-consumers-validation |121 | native-replay-consumers/docs/evidence/native-replay-consumers/README.md |
| recalld-native-context-validation/after and /unanchored |134 | native-request-context/docs/implementation/native-request-context-status.md |

The worktrees above are under `/tmp/recalld-implementation` and are **preserved**.
The two allocator failed targets are also released now:
`/tmp/recalld-owned-native-catalog-ids-red-2d_jox1q/target` and
`/tmp/recalld-owned-native-catalog-ids-full-qejenk3v/target`.
Their complete standalone roots can be released after this consumer PR exists;
recorded results and optional previously-created archives already exist.

Explicit future inputs retained:

- `/tmp/recalld-native-context-validation/before/fixtures` and
  `/tmp/recalld-native-context-validation/before/target/debug/deps/recalld-b2844bb1c4a63074`:
  genuine pre-context four-case fixtures and old test executable for future joins.
- `/tmp/recalld-owned-native-catalog-ids-full-xk182ybj/target`: the single requested
  reusable owned build cache. No additional per-run target is retained for reuse.
- `/tmp/recalld-unit-tests-l__ja1yo/cargo/registry`: shared dependency input.
- All code/worktrees/.git/tracked reports, and personal data.

No other agent's native-profile, batch, insertion, public-conformance or
similarity roots are implicitly released by these mappings. Cleanup paths are
ownership assertions, not permission to delete a containing worktree or an
active mounted input. Keep releases coordinated with the dedicated owner.
