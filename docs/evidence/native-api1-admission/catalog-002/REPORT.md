# N1 catalog producer focused validation

Frozen runtime and tests: `04406c8b5fbc4fcaa78b527a65815b40e8919f3f`.
Run: `/tmp/recalld-native-api1-admission-validation/002`.
UTC: 2026-11-16T21:27:35Z–21:29:21Z.

Default library compilation passed (58.21s, 72 warnings); `cargo fmt --all -- --check` passed.
Actual `native_catalog` binary filter: **27 passed, 0 failed, 0 ignored**, 1648 filtered out, 46.44s.
This is focused validation, not a full-suite, all-feature, Clippy or transport-frame proof.

Both new tests passed for original contract versions 1 and 2: candidate collector/callback refusal rolls back key, namespace counter and canonical generation before file preparation; candidate construction consumes exactly one clock sample and operation ID. Actual replay retains its original receipt, bypasses fresh provider/budget checks, and rejects changed-contract reuse. Existing catalog/reopen/publication/context/owned-ID/purge controls also passed. Intentional worker-panic diagnostics in passing fault tests are retained in `tests.log`.

Docker used Rust1.94.1, 2 CPUs, 4GiB memory/swap cap, jobs1, network none, read-only source and registry, one reused private target, no personal credentials/data/daemon or socket mount. Only the owned package was cleaned before rebuilding this source. Executable SHA256: `99cf77c89d35097aaaf0d8b77d88709a0fac8ca4902eafebffb2955285725e0d`.

Cgroup peak4294967296 bytes; memory.max4006, oom0, oom_kill0: reclaim pressure, not OOM. Compile/fmt/tests/container exit0. All source-manifest hashes independently rechecked after completion. Container `recalld-feynman-n1-catalog002` removal verified with exact-name docker ps -a query. No additional heavy job started.

Later documentation commits cdc2348e/c6b15671 are outside this frozen source; no runtime changes. Emitter architecture has source review only. Explicit local response policy, dispatcher capability, malformed-Remote evidence handling and actual route/frame tests remain required. Ordinary startup/public2 activation remains closed. Raw root remains protected pending consumer needs/publication; no cleanup authorization is implied by this report.
