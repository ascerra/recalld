# Native same-insertion links — validation evidence

Frozen runtime `08707c7a1d61213504534ae55e00597598d285ed`, direct PR97 parent `292fb290e24736f5ff86baf345f5b2794d17aa9d`.
No batch, producer, maintenance, catalog, or broad checkpoint ancestry.
[Interface and remaining scope](../../implementation/native-store-links.md).

## Result

**8 focused tests, 948 library tests, and 2 CLI integration tests passed.**
Formatting passed; clippy completed with warnings (exact counts in the manifest).
Four doctests remain ignored. Default test parallelism, Rust 1.94.1, offline Docker
`recalld-ci:1.94`, 2 CPU / 4 GiB and CARGO_BUILD_JOBS=2. No serial CI workaround.

Source manifest: `d9d86275daa54250dbefb06c6f46b8ed33fb07f352345ea3222781c305adfcb7` (215 files).
Library test binary: `2b0efc23cbd10e650bd712609042ad5fd74b3d81744831ed41e9d299fe05f484`.
Validation log: `9332a7cd0ce1fb143e5dbea270feb9eb5894873d6e3a44cfb51c73afbd0af9a3`.
The mounted allowlisted source and working source match before and after the run.
All Cargo-reported executable hashes, actual CLI hash and runtime settings are in
[MANIFEST.json](MANIFEST.json). Each source switch used package clean in this task's
private target; dependencies were copied without writable hard links. No home,
credential or Docker socket mounts. No source edits during a validation run.

## Actual fixtures

- Five explicit/semantic/entity/temporal edges committed with one insertion,
  primary token 1, one parent-token increment and absolute outgoing counts.
- One clock sample; temporal selection uses caller creation time, UUID ties and
  overflow-safe time differences. Edge timestamps use the same sample.
- Matching replay returns the historical outcome without writes or another clock,
  even with fresh current settings Off/caps=0/window=0 and unavailable generated
  output. The old five edges and advisory survive; checkDuplicates changes conflict.
- Advisory disabled versus checked-empty remains distinct. Later deletion does
  not rerun selection or recreate content.
- Actual canonical foreign/terminal peers, stale G, invalid temporal times,
  malformed/duplicate selections and generated versus supplied vector errors
  reject without allocation/state changes.
- Actual child process exits before metadata commit, after metadata commit, after
  edge commit and after canonical acknowledgement. Reopen/retry recovers all five
  edges exactly once with the original primary token; completed replay does not
  regenerate links.
- Logical export/import and deletion-aware backup restore preserve the additive
  proof. Foreign-lineage retry conflicts, later tombstone state survives, and an
  actual wrong-profile historical receipt rejects without changing original bytes.
- Strict proof negatives cover tokens, counts, edge bits, advisory presence/scores,
  capture G and caps. The historical two-edge insertion validator still rejects
  widening to this new outcome's five edges.

The eight-test count includes the subprocess child helper. This is core engine
validation, not a public MCP/HTTP/native-serving proof. Actual disabled-provider
zero-call and reserved batch-v3 tests belong to Curie's consuming producer join;
this core does not claim those results or provide a parallel provider wrapper.

## Preserved attempts and limitations

The first all-target check failed on a private helper call; the second attempt
failed compiling two UUID conversions in the subprocess fixture. No tests ran in
those attempts, and both exact sources/logs are preserved. The third run passed
seven engine tests; the final freeze adds changed-settings replay and canonical
foreign/terminal controls and passes eight. No failure is represented as a pass,
no cache was cleaned outside this task, and there was no unrecorded retry.

The 1,024 limit bounds actual insertion edges, not all examined candidates,
allocation or process RSS. Existing full graph/history scans, canonical candidate
reads and peer text commitments may be linear. No performance, quality, public
activation or complete A4 claim is made. Canonical success still requires the
actual producer publication boundary before a ready response. Typed standalone
graph no-op receipts and maintenance/purge history joins remain separate work.

## Evidence retention

All four source freezes, logs, scripts and source/binary verification records are
hash-verified in actual HOME work notes at:

`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-store-links-08707c7`

[COPY-VERIFICATION.json](COPY-VERIFICATION.json) records each copied file's path,
size and SHA-256. Raw evidence, caches and binaries are not in Git; binaries are
identified by hash, not copied. Task-local originals remain under
`/tmp/recalld-implementation/native-store-links/.validation`.
