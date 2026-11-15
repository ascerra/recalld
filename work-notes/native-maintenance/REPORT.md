# Native maintenance validation

This bounded child uses corrected PR82 `bb1a0863b2ab0eba940e767941ca4e85fe9b8e77`
(runtime `fb804a0a72ccf7dde0a6ee7eaea2e91a6b7ba2db`) and the minimal profile-shape
helper `1f4eb18c24a3d3f512030bbee0f9c4adcecfe656`, merged at
`7ff372c7a176d3641073860b0a2cecf3d98f875e`. No graph, catalog, numeric-policy,
batch or legacy maintenance parent is imported. Validated runtime `86ad2743edc0f797825dcd9e2b871564bf2f1546`; tree
`3cc1df19b871b33549c3ce98a6c791fbb00f3387`.

## Implementation and limits

The owning native session and accepted-parent publication consumer delegate to
the existing metadata final transaction. It materializes strengths and at most
one phase per captured ID, preserves lifecycle anchors/provenance/history, strips
retained content atomically with the ledger/receipt/outbox, retains Ghost vectors,
and hard-deletes eligible Ghosts with exact edge/peer/vector ownership effects.
Projection rechecks current canonical donor state and fan-out using the original
sampled clock under the final publication guard. No cached bonus authorizes loss.

Typed, content-free deletion witnesses bind identities, namespace/profile,
predecessor state, bounded strict history, exact incident edges and peer token/count
changes. Native copy-recovery admits only the deleted primary as an absent edge
endpoint of an exact validated unapplied pending deletion. Float bits are exact.
Other missing endpoints, nil identities, unknown fields and wrong namespace/profile
bindings fail closed. Source validation precedes writable open.

See [implementation boundary](../../docs/implementation/native-maintenance-plan.md)
for entry APIs and required future joins. Existing migration/publication receipts
and native tombstone semantics are unchanged. New receipt kinds are rejected by
older readers. Ordinary schema-2 serving remains closed. No full A3, whole-store
readiness, deterministic UUID, or performance claim is made. Internal skipped
maintenance outcomes do not reserve keys or promise public keyed-noop semantics.

## Reproductions and preserved failures

- `harddelete-recovery-before.log`: actual after-metadata-commit process exit91
  followed by native reopen failed with dangling/inconsistent edge. The metadata
  deletion was committed while separate edge replay remained pending. Exact
  source patch archive and binary SHA/mtime are in `harddelete-recovery-before/`.
  The fix permits only the validated pending deletion, not arbitrary dangling edges.
- `signed-zero-before.log`: real legacy insertion and edge write with +0, actual
  schema/lifecycle migration, native phase losses, then deletion exit91. Changing
  both physical edge indexes consistently to -0 was wrongly accepted as exact.
  `signed-zero-before/` preserves source and failing binary identity. NativeEdgeProof
  comparison now includes float bits. This is a correctness result, not timing evidence.
- `focused-1.log`: fixture assertion failed because Supersedes edges have zero
  spreading weight. The corrected fixture uses real parent feedback and a new
  canonical fan-out child while the target token is unchanged; it proves both
  phase-loss and strength-only final projection rechecks at the original clock.
  Initial source/binary identity is in `initial-failure/`.
- `publication-focused.log`: fixture used Tokio current_thread while the existing
  HTTP adapter requires multi_thread for block_in_place. The corrected fixture
  uses two runtime workers. `publication-focused-2.log` passes. This setup failure
  is not attributed to runtime implementation. Its initial executable was not
  separately sealed; signed-zero manifest merely records this earlier log hash.
- `check-initial.log`: initial missing HashMap import compile error, corrected.
  No failed run was hidden or recast as a successful validation.

## Validation protocol

All execution uses synthetic data inside Docker `recalld-ci:1.94`,
RUSTUP_TOOLCHAIN=1.94.1, CARGO_BUILD_JOBS=2, `--network none --cpus 2 --memory 4g`.
Only this worktree and its private cargo/target scratch directories are mounted,
with no home, credentials, personal data or Docker socket mounts. The copied
cache was package-cleaned before this worktree built; no later source mount switch
occurred. `final-source-manifest.json` and `validated-source.tar.gz` identify the
full runtime/test inputs; final binary SHA/mtime is recorded separately.

The final container runs: `cargo fmt --all`, then
`cargo test --offline --lib storage::metadata::native_records::tests::maintenance -- --nocapture`,
`cargo fmt --all --check`, `cargo test --offline`, and
`cargo clippy --offline --all-targets`. Focused accepted-publication checks ran
separately with `cargo test --offline --lib native_maintenance_accepted_publication -- --nocapture`.

The 13 focused maintenance test entries include one process-child harness.
Actual checks include original clock/current donor/fan projection, one-phase passes,
anchor versus compatibility access, stale feedback and policy, migration replay,
missing summary, permastore and terminal exemptions, threshold boundaries, ledger
operation identity, slot reuse, process exit before/after commit, abandoned and
ambiguous transactions, exact missing-endpoint validation, signed zero, and
export/import/restore with 14 malformed historical-proof cases that leave source
bytes unchanged. The accepted publication test exercises committed failure, full
receipt replay, HTTP visibility, retained Ghost vector, deletion, strict reopen
and work completion. The full suite also retains actual historical tombstone
receipt export/fork/reopen and preflight ownership/copy-repair tests from PR82.

## Completed results

- Focused final: 13 maintenance entries passed, including one process-child harness; accepted publication 1 passed.
- Full: 940 library tests and 2 CLI tests passed, zero failed; 4 doctests ignored.
- Formatting: exit 0. Clippy: exit 0 WITH 623 library and 661 library-test warnings (621 duplicates), plus one binary warning (also reported once as a duplicate for its test target).
- Final container exit 0. All 210 runtime/test/build input hashes match the frozen source manifest after the full run and Clippy.
- Full-suite old-proof checks passed: actual original phase receipt reopen/export/fork, malformed phase rule immutable rejection, native preflight process recovery and malformed repaired-copy immutable rejection.

`validation.json` records executable identities and raw-log hashes. Logs record test duration only; this work provides no performance measurement. No additional full run is needed for this evidence-only tail.
