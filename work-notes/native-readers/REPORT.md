# Native record publication and reader validation, 2026-11-14

Frozen runtime: `6a712482d91c1c22d9c5d7ffa72082ea7fdd9db0` on
`impl/native-record-readers`. Actual merge parent `d1d916e` joins PR79
`6a70174` and PR69 `eefc987`. Publish on PR79 with PR69 as an additional
dependency; neither batch PR78 nor writer-guard PR80 was imported. The eight
changed runtime/documentation files are recorded in `source-files.json`, with
the patch against the actual merge parent and a source archive alongside it.

Native insertion already committed canonical effects, but the existing derived
acknowledgment rejected the native owner through its legacy-only mutation guard.
The narrow checkpoint now uses the actual native guard and work ticket, validates
pending effects, and consumes only canonically applied rows after the existing
coordinator rebuilds the real views. It changes no canonical records, mutation
receipts or generation. Uncertain commits still poison the owner. Clean-outbox
reopen uses `DerivedCompletion::rebuild_all`, the same implementation as normal
completion, without a fabricated outbox operation.

The real HTTP/MCP readers exercise exact native UTF-8 labels, empty native labels
without tag inference, migrated legacy label provenance, supplied-vector and
filtered metadata-only queries, zero provider calls, byte-identical false-mode
reads, and reopen. The second test forces an actual publication failure after
native tombstoning, requires reads to remain blocked, then retries successfully
and verifies removed payload/vector visibility and persistent terminal state.
Ordinary observing routes remain rejected on the explicit native owner.

The scope-error adapter now preserves `NamespaceMismatch` and daemon `-32007`.
The joined full suite's two other corrections use a real foreign canonical
entity owner during purge and count observation receipts relative to actual
namespace-creation receipts. Assertions were retained rather than relaxed.

## Validation

All Rust commands ran in `recalld-ci:1.94`, with `RUSTUP_TOOLCHAIN=1.94.1`,
`CARGO_BUILD_JOBS=2`, Docker `--network none --cpus 2 --memory 4g`.
Only this assigned worktree and its private `.test-scratch/cargo` and
`.test-scratch/target` were mounted at `/work`, `/cargo`, `/target`.
No home, credentials, socket or personal data was mounted. The native-core
target was not reused for execution: this child received a private COW cache
copy and its own package clean before the initial compile. The source mount did
not switch during these runs.

```text
cargo fmt --all -- --check
PASS
cargo test --offline --lib scope_assertions_remote_round_trip_and_old_daemon_fail_closed -- --nocapture
1 passed
cargo test --offline --lib namespace_deletion_clears_orphan_entity_postings_only_in_its_scope -- --nocapture
1 passed
cargo test --offline --lib namespace_keys_are_independent_and_shared_across_operation_kinds -- --nocapture
1 passed
cargo test --offline --lib storage::metadata::native_records::tests::readers -- --nocapture
2 passed
cargo test --offline
920 library passed; 0 failed
1 lifecycle CLI passed; 1 text migration CLI passed
4 doctests ignored
cargo clippy --offline --all-targets
exit 0; warnings remain: 585 lib, 622 lib-test including 583 duplicates
```

The full run's recorded library duration is 376.00 seconds; this is a validation
run, not performance evidence. `test-binary.json` records the actual tested
executable SHA-256, size and mtime. Its hash is
`5d7229544eeff6558abbb4c62e5b3f13381cb7d8ff28c79ae573fdee306c1d05`.
The executable itself is retained in the private target, not committed.

## Failed evidence retained

`native-reader-before2.log` is the actual native checkpoint failure. Its source
archive and previously captured binary/source identities are preserved as
`before-source.tar.gz` and `before-manifest.json`. The preceding compile failure
and intermediate harness corrections (MCP response shape, HTTP route, explicit
metadata filter) remain separate raw logs. `native-reader-after4.log` records the
initial two-test pass.

`native-reader-full.log` records the first complete joined run: 917 passed,
3 failed out of 920, with no CLI run after the library failure. Those failures
are the typed scope mapping, orphan entity fixture, and receipt baseline
described above. They were preserved throughout the A5 hold. Its two corrected
source files before correction are available at `d1d916e`; no separately
captured executable hash is claimed for that full failed run. The earlier
checkpoint binary manifest belongs only to its own reproduction.

Main explicitly ended the sampling hold before the final focused/full/lint
commands. No failure log was overwritten and no retry is presented as an
unchanged-source success.

## Remaining scope and handoff

This is private native writer and existing-reader composition. Ordinary
schema-2/public contract-v2 serving remains closed. It establishes no whole-query
ReadView, generation watermark, deterministic IDs, or complete native writer
participation. Halley owns the actual registry/read-lease join; its forced
rebuild logic should absorb this same coordinator path. Feynman owns additive
catalog/config, phase-loss/hard-delete, purge and batch proof extensions.
The A3 anchor/policy/graph-lock review is recorded in
`../native-writer-interface-review.md`; no duplicate writer is implemented here.

PR79's existing actual HOME mirror was reverified: all 14 files match the frozen
source at `/tmp/recalld-implementation/native-records/work-notes/native-records`.
Destination:
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-records-6a70174`.
Its `COPY-MANIFEST.json` SHA-256 is
`36979199e21e4c5aae697f81556e32eee04412e94b19259d292f9df4da548400`.
