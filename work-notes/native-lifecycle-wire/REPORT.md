# Native lifecycle wire validation

Runtime: `d00cebfdfc6ca67586c9fbb5d329243028b1c6e0`. Branch `impl/native-lifecycle-wire`.
Actual prerequisites: [PR121](https://github.com/calebevans/recalld/pull/121)
`08914a4a8478d83e8124ed07fdbff4a87ff15cd1` (runtime75bc809; actual114/110/111),
and [PR88](https://github.com/calebevans/recalld/pull/88)
`3961f6653d6c3076bdf9b5b8c27fa6b01a8f0b2e`.
No PR119, native batch-wire or broad activation parent.

## Results

- Final focused: **6 passed**, 0 failed.
- Full: **1136 library + 2 CLI passed**, 0 failed; 4 doctests ignored.
- Formatting and formatting check: exit0.
- Clippy all targets: exit0 **with warnings**: 890 library, 954 test
  (865 duplicates), one binary warning (also reported as duplicate for binary).
- Earlier six focused cases passed before the final additional forget socket
  replay assertion; its exact source and binary hashes are retained separately.
  No failing test run was discarded or retried in this slice.

`run.json` records exact commands, return codes and start/end timestamps.
The full suite's reported584.65s is a validation duration, not a latency claim.
`executed-binaries.json` records SHA256, byte length and nanosecond mtime for all
8 executed library/binary/CLI artifacts, including CLI subprocess executables.
No executable, build output or Cargo cache is committed or copied HOME.

All **266 source/Cargo/test/toolchain files** match the committed runtime.
The pre-run263-file code/Cargo manifest was checked again after each stage;
the three unchanged tracked integration-test/toolchain files were additionally
verified byte-for-byte against the same Git commit. `source.tar.gz` contains the
complete verified inputs, with `source-sha256.json` as the content inventory.
Inventory file SHA256: `30506270ddbb00fa861927177f2c314c4ca1f6c13eeaa3025cf748d4ab325dd1`.

## Reproduction and isolation

Image `recalld-ci:1.94`, `RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_BUILD_JOBS=2`,
`CARGO_HOME=/cargo`, `CARGO_TARGET_DIR=/target`. Docker flags
`--rm --network none --cpus 2 --memory 4g`; working directory `/work`.
Only this assigned worktree, its private cargo directory and its private target
were mounted. No home, credentials, daemon socket or personal data was mounted.
Private caches were cloned from the previous owned native-exposure validation;
`cargo clean -p recalld` ran on this target before changing source roots.
The actual driver is `validate.py`; its outputs are gzip-preserved unchanged.
Run it from the source root after creating its `.test-scratch/final/input.json`
code/Cargo hash map as in the driver. No source was edited during final validation.

## What the focused tests establish

Real migration/native-entry fixtures drive prepared MCP, REST body/header and
RemoteStorageAdapter/DaemonClient/framed daemon handlers. The shared backend uses
one accepted root, existing native transaction/planner and caller-configured
policy/clock. Tests cover omitted quality3, strict malformed fields and scope,
CAS and changed-payload conflicts, matching replay before freshCAS/clock,
feedback and forget cross-transport replay, terminal feedback, current readiness
separate from historical receiptG, actual publication failure/full receipt,
actual post-commit unknown identity/reopen, and retained terminal by-ID scope
plus explicitID after namespace purge/reopen. The old-handler fixture rejects
the exact new aliases and cannot silently fall back to a legacy mutating RPC.

Public MCP/REST contract2 selectors are separately proven to reject before work.
These positive prepared/private transport tests do not claim live public2,
API1 writes on native stores, ordinary native serving, whole-query readGeneration,
public keyed search, deterministic IDs, complete A3/A4 or performance readiness.
Failure readiness is omitted pending Huygens's actual observer/null-stamp join;
known receipts survive, and failed coordinator paths never self-drain.
API1/native original-context propagation remains a separate coordinated child.
See `docs/implementation/native-lifecycle-wire.md` and `changed-files.txt`.
