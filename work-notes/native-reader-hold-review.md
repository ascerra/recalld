# Native reader integration review during A5 hold

2026-11-14. Native core PR79 remains frozen at `6a70174`. The separate reader
worktree is `impl/native-record-readers`, actual parent join `d1d916e` of that
core and PR69 `eefc987`. It is not a frozen validated handoff yet.

The actual native writer/publication/HTTP/MCP/reopen and failed-forget-publication
regressions passed (2 tests). The first complete joined run ended normally with
**917 passed, 3 failed out of 920**; cargo stopped before CLI tests. Raw output
is preserved at `.test-scratch/native-reader-full.log`. Its source before these
corrections is reproducible from `d1d916e` for the two files changed below; the
native reader changes and tests remain in this separate worktree.

Light review prepared these corrections without a rerun during the A5 hold:

- Scoped forget returned the correct `StorageError::NamespaceMismatch`, but the
  generic mutation error mapper flattened it to a storage error. Preserve the
  typed bridge error and existing daemon code `-32007`; do not accept a generic
  error in the regression.
- The namespace-deletion fixture expected a foreign orphan posting to survive.
  The actual purge coordinator now rebuilds from canonical rows and correctly
  removes noncanonical postings. Use a real foreign record and the same UUID's
  stale target-namespace posting. Keep exact foreign-posting and row assertions,
  while requiring the target posting to disappear.
- The namespace-key fixture's absolute receipt count omitted two real catalog
  creation receipts. Measure its existing receipt/key baseline, then require
  exactly two additional receipts and keys. Cross-operation key conflicts and
  independent namespace keys remain unchanged.

No new Docker/build/test/format/archive/compression/cache-copy jobs will run
until explicit END. The existing full job drained and main received the ACK.
These corrections are prepared, not validated or committed as a final result.

## Sampling window and post-END validation

Sampling began at `2026-11-14T15:27:38.000Z`; the hard end is
`2026-11-14T15:47:38.000Z`. Wait for explicit END before scheduling anything
heavy. The deadline alone is not permission to start a job.

After release, run these sequentially in the existing private Docker setup:

```text
cargo fmt --all -- --check
cargo test --offline --lib scope_assertions_remote_round_trip_and_old_daemon_fail_closed -- --nocapture
cargo test --offline --lib namespace_deletion_clears_orphan_entity_postings_only_in_its_scope -- --nocapture
cargo test --offline --lib namespace_keys_are_independent_and_shared_across_operation_kinds -- --nocapture
cargo test --offline --lib storage::metadata::native_records::tests::readers -- --nocapture
cargo test --offline
cargo clippy --offline --all-targets
```

Use `recalld-ci:1.94`, `RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_BUILD_JOBS=2`,
`--network none --cpus 2 --memory 4g`, and only this worktree plus its private
`.test-scratch/cargo` and `.test-scratch/target` mounts. No source-mount switch or
cache copy is needed. If formatting needs correction, do it only after release,
then rerun its check before tests. Preserve the first full-run log; write new
focused/full/lint logs to distinct paths. Do not claim CLI results unless the
full command actually reaches and passes them.

Only after successful validation: freeze the minimal child, archive source/logs
and executable identity, mirror and hash-verify the evidence in the actual
work-notes vault, then send the tested tip and API seams to main/Feynman/Halley/
Huygens. Ordinary schema-2 activation stays closed.

## Release

Main explicitly ended the sampling hold after all 15 arms exited successfully.
Post-END formatting, all three focused correction tests and both native reader
regressions passed. The final full-suite/lint results and frozen source identity
are recorded separately in `native-readers/REPORT.md`; the original failed full
log remains evidence, rather than being overwritten by a successful rerun.

PR79's existing HOME evidence mirror was independently rechecked against all 14
files in the frozen native-records worktree. The mirror is
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-records-6a70174`.
Its added `COPY-MANIFEST.json` has SHA-256
`36979199e21e4c5aae697f81556e32eee04412e94b19259d292f9df4da548400`.
The original artifacts and experiment notebook template were not modified.
