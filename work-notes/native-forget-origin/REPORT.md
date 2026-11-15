# Original-contract native forget consumer validation

Runtime/test-source `0c82d18cd7adcd051f9c44aaeed206ed2e729302`, branch
`impl/native-forget-origin`. Actual parents: [PR129](https://github.com/calebevans/recalld/pull/129)
`f504db1a0e98c985bfd5db8b8e760a2f7705e48e` and [PR131](https://github.com/calebevans/recalld/pull/131)
`974421582f56641222f78ba18f8c652e93d5b352`. Their real two-parent join is
`bd1b07bd90a493a3d4c9924b4ae7ffda56b51957`; no duplicated cherry-pick, compound,
batch, exposure or manual parent. Both parent branches remain frozen for Halley.

## Results

Final frozen third source passed **1150 library +2 CLI**,4doctests ignored.
Focused filters passed: forget_origin5 (four new cases plus one existing parent),
original_api5, request_context5, native_forget7. These are overlapping filter
runs, not22 unique new tests. Formatting/check passed. Clippy exit0 **with890
library/954 test warnings (865duplicates)**. All277 source/Cargo/test/toolchain/
fixture inputs match the source commit and were verified unchanged during the run.
Full599.05s is a validation duration, not a performance claim.

First source failed compilation: three uses of an existing FixedClock-only test
helper with NeverClock. Corrected by using the actual NativeLifecycleSession API;
no executable was produced (empty first/binaries.json), and log/source are retained.
Second source compiled and passed4/failed1 in the forget filter. Forgetting the
only live record left no vector to trigger the intended dimension fault; no
runtime defect was established. The corrected fixture inserts a real surviving
native record and rebuilds before changing the vector dimension. Runtime code
was unchanged across construction fixes. Second source/binary/log retained.

Every stage includes source archive, per-input hashes, commands/exits/timestamps,
raw logs (gzip) and available binary SHA256/size/mtime_ns. Third's executable
manifest describes the three private target artifacts; executed paths are in logs.
No executable, cache or personal data is in the evidence bundle.

## What is actually joined and tested

PR129 already carries original1|2 through explicit private Request/RPC/executor;
PR131 removes the redundant native-record2-only guard while retaining the actual
native ownership/schema/scope validation. This child adds no parallel storage,
planner, runtime wrapper or public selector. It verifies the actual combined
caller reaches the existing canonical record transaction.

New tests use real RemoteStorageAdapter/client framing/private daemon for both
origins; staleCAS and wrongscope reject before clock/bytes; cross-origin keys
conflict; matching replay precedes staleCAS/changedpolicy/NeverClock and strict
reopen. Canonical terminal identity resolves by ID without session default or
unnecessary new namespaceID. Known publication failure retains full receipt and
repairs on replay. Actual effects postcommit failure returns unknown operation ID,
poisons the owner, and reopens/replays the same operation without a second clock.
Known publication failure reports committed plus the full receipt and no claimed
readiness. The injected uncertain commit reports unknown plus its operationID,
without a receipt; only reopening/reconciliation recovers the stored receipt.
Neither becomes notCommitted or an ordinary safe blind retry.
Real namespace purge followed by reopen proves implicit unique terminal scope
and explicitID retries, and wrong-name failure retains the known committed receipt.

Both genuine historical API2 fixtures from the parents run in normal tests;
no old receipt/source/publication provenance is rewritten. The old-handler
no-fallback proof remains explicitly a current strict2 parser emulation, not
execution of a historical daemon binary.

## Isolation and scope

Offline Docker recalld-ci:1.94, RUSTUP_TOOLCHAIN1.94.1, jobs2,networknone,2CPU/4GiB;
only own worktree/privatecargo/privatetarget. No HOME/credentials/socket mounts.
A private clone was prepared before the user's central-cleanup directive; own
packageclean ran before first compilation. No next Rust task is assigned at this handoff, so after HOME/repo sealing both
the disposable target and private Cargo directory are released only to central
cleanup. A future identified task may retain one needed shared cache, never
repeat per-run copies. Source, .git and evidence remain protected.
See validation-command.txt, stage run.py/results.json and seal.py.

Huygens owns public contract routing/API1 outward compatibility/shared failure
observation. This private consumer does not enable ordinary native/public2
serving, public keyed-query semantics, name-reuse transport parity, deterministic
IDs or full A3/A4 activation. Existing same-origin current receipt checks remain;
no generation comparison across store IDs is introduced. See implementation doc.
