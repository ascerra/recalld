# Native preflight recovery correction, 2026-11-14

Minimal child of published PR82 `360c4531ef2fcfa54d268017ed0d38fdff8e2583`.
No batch or catalog ancestry is included. Frozen runtime `fb804a0a72ccf7dde0a6ee7eaea2e91a6b7ba2db`;
source/executable identities are recorded in `manifest.json`.

## Defect and bounded repair

A real native insertion that exits at the existing `before_metadata_commit`
process hook leaves an unclean edge database. The unchanged parent startup
rejects it with `RedbDatabase(RepairAborted)` before reaching writable recovery.
The independent BEFORE reproduction failed 0 passed / 1 failed. The source and
binary identity were sealed before changing the two production inspection calls.

Native preflight now invokes the existing disposable-copy inspector with repair
allowed for metadata and edges. All existing complete canonical validation still
runs before writable opens of the originals. Offline snapshot and import-stage
inspection retain repair disabled. No recovery effects are applied in preflight.

The test-only callback at the actual post-preflight boundary checks stable store
ownership, the directory lock, and all original file bytes on success and error.
Ownership remains held through actual writable recovery and engine lifetime.
A separate child process verifies an already-owned store cannot be opened.
The byte-immutability evidence excludes pending restore journals: ownership
acquisition can complete that separate recovery protocol before preflight.

## Actual regression scenarios

The fixture uses real legacy insertion, schema attestation and explicit lifecycle
migration, then the actual native insertion caller. It does not manufacture valid
native rows. The test executable launches itself as a separate process.

- Exit 91 before metadata commit: reopen recovers, target is absent, keyed retry
  commits once, and a second retry replays. One clock sample for the new write.
- Exit 91 after metadata commit: reopen recovers, target is present, keyed retry
  replays the original receipt with no clock sample. Both cases check the exact
  expected generation, one parent edge, two records and another strict reopen.
- Five negative child processes first perform a valid native insertion, then
  deliberately corrupt schema, pending effects, text, vector or edge state and
  exit 94 with unclosed redb handles. Repair of disposable copies succeeds;
  complete native validation rejects each malformed store without changing any
  original bytes. Raw writes are used only to construct malformed negatives.
- At least one database in every scenario actually reports `RepairAborted` when
  copy repair is disabled. The other may use a valid allocator checkpoint.
  The strict offline consumer still rejects the unclean store without mutation.

`preflight-focused-final.log` identifies both crash points and all five errors.
There are three test entries: the process helper and two scenario-driven tests.
These are process-exit tests, not power-loss durability or full recovery claims.
Ordinary schema-2 serving remains gated.

## Provenance and validation environment

All Rust execution used `recalld-ci:1.94`, `RUSTUP_TOOLCHAIN=1.94.1`,
`CARGO_BUILD_JOBS=2`, `--network none --cpus 2 --memory 4g`. Only the assigned
`/tmp/recalld-implementation/native-preflight-recovery` worktree and its private
`.test-scratch/cargo` and `.test-scratch/target` were mounted. No home, credentials,
sockets or personal data were mounted. Private COW cache copies were followed by
`cargo clean -p recalld` on this child's target only before compilation. No source
mount switched during these runs. Logs and executable/source hashes are retained;
no executable, target directory or dependency cache is committed.

BEFORE executable SHA-256:
`3900d9ffdb00c76f08b2804eaf6127c3eb08c2234a0851848f50e0c8f193fda9`.
`before/manifest.json` records its size, mtime, source hashes and failing log hash.
`before/source.tar.gz` preserves the exact reproduced source.

Initial `preflight-before.log` contains test-construction compile errors, and
`preflight-before-runtime.log` contains an incorrect test assumption that both
redb files must require repair. These are preserved separately and are not the
runtime defect evidence. `preflight-before-runtime2.log` contains the actual
RepairAborted failure with the corrected at-least-one-repair assertion.

Final command sequence inside the same isolated container:

```sh
cargo fmt --all -- --check
cargo test --offline --lib storage::metadata::native_records::tests::recovery -- --nocapture
cargo test --offline
cargo clippy --offline --all-targets
```

Final results: focused **3 passed**; full **926 library + 2 CLI passed**, zero
failures; 4 doctests ignored. Formatting passed. Clippy exited 0 **with warnings**:
585 library, 624 lib-test (584 duplicates), and one binary warning (duplicated
for the binary test target). The complete chained container exit status is 0.
Full-suite duration 368.78 seconds is validation metadata, not a performance claim.
Artifact-only sealing does not change the tested runtime.

## Coordination and scope

Curie supplied the original batch reproduction and minimal two-call proposal;
this branch independently proves the defect without importing batch code.
Feynman reviewed the ownership boundary before edits and the final bounded diff
plus focused logs with no blocker. That is source/log review, not a claim of his
independent execution. Shared runtime owners receive the frozen runtime before
artifact-only sealing. Native phase-clock receipt compatibility is unchanged.
No catalog mutation, new storage plane, public serving activation, merge or
deployment is included.
