# Native same-insertion links — validation evidence

Frozen runtime `4ad7965be6f6c51e3288edac0cce318996cab644`, direct PR97 parent `292fb290e24736f5ff86baf345f5b2794d17aa9d`.
No batch, producer, maintenance, catalog, or broad checkpoint ancestry.
[Interface and remaining scope](../../implementation/native-store-links.md).

## Result

**9 focused tests, 949 library tests, and 2 CLI integration tests passed.**
Formatting passed; clippy completed with warnings (exact counts in the manifest).
Four doctests remain ignored. Default test parallelism, Rust 1.94.1, offline Docker
`recalld-ci:1.94`, 2 CPU / 4 GiB and CARGO_BUILD_JOBS=2. No serial CI workaround.

Source manifest: `80fd6f52f48d0fc633e9e53675be612bcbba21cfb6e39bc70b7ab57e4603e085` (215 files).
Library test binary: `aaa9ab5d858bf433cba222cf522688fbbe31ac74f2ec89eb84505a7321dd8800`.
Validation log: `ef54fe6219d6e64d77bf4095ff5f1069271ddb11587d71a67b06ff90d79ea1ca`.
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

The nine-test count includes the subprocess child helper. This is core engine
validation, not a public MCP/HTTP/native-serving proof. Actual disabled-provider
zero-call and reserved batch-v3 tests belong to Curie's consuming producer join;
this core does not claim those results or provide a parallel provider wrapper.

## Preserved attempts and limitations

The first all-target check failed on a private helper call; the second attempt
failed compiling two UUID conversions in the subprocess fixture. No tests ran in
those attempts, and both exact sources/logs are preserved. Run 3 passed seven engine tests. Run 4 (`08707c7`) added changed-settings replay
and canonical foreign/terminal controls and passed eight focused, 948 library and
two CLI tests. That green source and HOME seal remain preserved independently;
it predates the dimension-binding finding described below. No failure is represented as a pass,
no cache was cleaned outside this task, and there was no unrecorded retry.

The 1,024 limit bounds actual insertion edges, not all examined candidates,
allocation or process RSS. Existing full graph/history scans, canonical candidate
reads and peer text commitments may be linear. No performance, quality, public
activation or complete A4 claim is made. Canonical success still requires the
actual producer publication boundary before a ready response. Typed standalone
graph no-op receipts and maintenance/purge history joins remain separate work.

## Semantic dimension binding correction

Feynman's source finding reproduced in the actual deletion-aware restore fixture:
changing only a retained foreign linked receipt's embeddingDim from 4 to 5 left
its profile/state well formed, and startup incorrectly accepted it. The new
regression failed 0 pass / 1 fail before the correction. Source manifest
`029dcc8233ed95b4dc3db02cb71584857c51a5e3789232e07ebd59bc4c79a5e5`;
test binary `22af30968799402728597904ed46818ad7f87e46e988e5969a91b9af687a4259`.

The narrow correction binds proof dimension to the validated namespace dimension
inside the existing all-origin namespace/profile check. Supported-range validation
and a well-formed profile ID alone are insufficient. Retired namespace witnesses
must supply this validated dimension in later purge joins. The regression now
checks startup and export rejection with original bytes unchanged. It does not
claim to validate unjoined historical outcomes owned by other writers.

Run 6 failed formatting before tests because the snapshot was copied before the
scratch formatter completed. Its exact source/log are retained. Run 7 uses the
completed formatter output and is the corrected full green result above. No API,
request hash, persisted receipt shape or dependency change was needed.

## Evidence retention

All seven source freezes, logs, scripts and source/binary verification records are
hash-verified in actual HOME work notes at:

`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-store-links-4ad7965`

[COPY-VERIFICATION.json](COPY-VERIFICATION.json) records each copied file's path,
size and SHA-256. Raw evidence, caches and binaries are not in Git; binaries are
identified by hash, not copied. Task-local originals remain under
`/tmp/recalld-implementation/native-store-links/.validation`.
