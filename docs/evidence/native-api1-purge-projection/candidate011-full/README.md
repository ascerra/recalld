# Frozen full011: default all-targets PASS

Exact test source90938aa5abc0bd35c6dc784311bea05bb8189679; runtime369d8e92 unchanged.
One approved run on the same011 source snapshot and target, no repeated focused
execution or source switching. Pre-run source and existing focused executable
hash checks passed. Default `cargo test --locked --offline --all-targets` exited0:
1378libraryPASS/0FAIL/1ignored,3CLIintegrationPASS,12example/harnessPASS.
The ignored case is the genuine pre-correction legacyAPI2 fixture generator, not
an ignored runtime regression. Library602.33s; whole container command03:18:39Z
to03:29:19Z,2026-11-16 (1800s timeout did not fire).

OfflineRust1.94.1,2CPU4GiB/jobs2/testthreads2, no personal/credential/socket mounts.
Cgroup peak3,937,464,320B, memory.max0/OOM0/kill0. Warnings are retained in raw full.log;
this is not warning-free compilation. Actual11 Cargo target executable hashes are
recorded in executed-targets.json, plus the captured recalld executables (including
CLI-spawned daemon binary) in executables.sha256. Existing focused library hash
and full library hash match:a1a7424c34c1ac28b6dcdaccfbd6f998eb76495b40754c8dbd6ec30ef7329a7c.

No all-feature test-execution claim. The separate checks evidence records fmt and
all-feature/all-target Clippy. No activation/publication claim. Complete raw root
remains protected until PR/results and explicit release.
