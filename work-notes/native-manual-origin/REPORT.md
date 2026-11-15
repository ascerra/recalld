# Manual and autonomous lifecycle origin validation

Runtime `f86b244e75ecd3e9d6c64a61d94d5eadcda6a689`, branch
`impl/native-manual-origin`, direct [PR107](https://github.com/calebevans/recalld/pull/107)
`785dfec4`. No feedback/exposure/compound/batch parent.

## Results

**4 original-contract +7 native-sweep +11 native-maintenance focused tests passed**.
Full **1027 library +2 CLI passed**,4 doctests ignored. Format/check passed;
Clippy exit0 **with737 library/804 test warnings (717duplicates)**.
All231 source/Cargo/test/toolchain inputs were sealed before tests and verified
unchanged afterward. No failed validation stage occurred. Full444.18s is a suite
duration, not a performance claim. Exact commands/exits/timestamps in results.json;
complete source archive, logs and executable SHA256/size/mtime_ns retained.

## Actual consumers

The existing runner now receives a required explicit original number. Rust/manual
compatibility chooses1; startup/timer/internal notification chooses2. Both actual
legacy and native arms carry it to the canonical transaction. Namespace matching,
physical writer admission, expected record/policy/graph state, native ownership,
publication and retained timestamps remain guarded. Shared effects finalization
signature/child proof is unchanged. No schema migration is required just for API2.

Real tests run both producer modes through Full→Summary→Ghost→harddelete, comparing
request hashes against original1 and2 and proving a single clock sample/pass.
Legacy API2 phase/delete receipts strict-reopen and preserve every previous API1
receipt byte. Those prior1 rows come from the actual current compatibility writer,
not a historical binary fixture. Existing native old2 compatibility/malformed
witness/export/fork/recovery tests remain and passed. Unknown3 harddelete rejects
before lookup/write for both existing and missing IDs; exact meta.db/control stay
unchanged. Unknown/missing native policy rejects before clock; configured multiplier
is consumed from the same native policy field/builder used in PR126.

The actual startup queue/accepted-root and fresh-readiness completion are used,
not a second scheduler. Native maintenance still retains anchors and raw timestamps,
uses clamped markers, rechecks donor/fan/policy and one phase per pass, and preserves
full receipts/unknown identity on partial failure. Existing focused proofs cover it.

## Reproduction

Offline Docker recalld-ci:1.94, RUSTUP_TOOLCHAIN1.94.1, jobs2,networknone,2CPU/4GiB.
Mounts only own worktree/privatecargo/privatetarget; no HOME/credentials/socket or
personal data. Private caches cloned from completed owned feedback validation;
clean-p recalld ran on this own target before compiling switched source. See
validation-command.txt, first/run.py and seal.py. No source edited during tests.

## Limits and joins

Private trigger_sweep_with_origin supplies the existing configured native policy
and clock; public contract routing remains Huygens's separate work. Physical legacy
text compaction retains its existing internal housekeeping domain; this slice
propagates origin for per-record lifecycle effects, not housekeeping receipts.
No native legacy compaction, keyed sweep, durable keyed skipped/noop result,
performance readiness or ordinary native/public2 activation is claimed.
Internal stale/exempt/nochange/missing-summary remains unkeyed diagnostic only.
Future adapters cannot present a supplied-key skipped result as durably bound.
