# PR186 integrated default-stack correction — 2026-11-18

Before: `23965332a72ca9f527f4bb97f48b41a7270fa33f`, incorporating main
`4ad3cb0f0d0af3477861a576b90f068ba7c3564f`. CI merge preview
`d40aee77b5cf3c9dcbcca862c412c8cba181f339` has the identical Git tree.
CI44350712087/job134965251516 reproduced stack overflow/SIGABRT in
`performance::tests::writes::batch::legacy_batch_actual_partial_stdio_requires_parent_and_child_facts`
at 2026-11-18T16:47:39Z. Earlier wrapper-elision proof was pre-main and did
not clear this composed failure. It remains attributed in the earlier report.

The unchanged integrated source reproduced the same failure locally (Cargo101,
SIGABRT) with ordinary Cargo/libtest on offline Rust1.99.1 x86_64/default stack.
Then the sole additional runtime change replaces inline `pin!` with `Box::pin`
for the large batch async body in `run_batch_with_surface`. This keeps that
body out of enclosing transport futures; all callers share the correction.
The body, receipt/unknown handling, locks, work tickets, tracing, cancellation,
readiness and replay logic are unchanged. Cost: one heap allocation per batch.
No raised stack, compiler workaround, ignored failure, test weakening, or
performance/general stack-safety claim.

Exact unchanged regression: PASS1,0failed,0ignored,0.54s after1m55s rebuild.
Related filters: `performance::tests::writes::batch::`,
`mcp::bridge_adapters::batch_adapters::tests::`, `derived_completion::tests::`,
`storage::native_purge_tests::api1_consumer::`, `api1_batch_`.
PASS71,0failed,1existing historical fixture ignore,98.66s,2testthreads.
The exact regression is included in71; counts are not additive unique coverage.
Formatting PASS using installed recalld-ci:1.94; x86 regression image lacks
rustfmt (recorded setup limitation, not a formatting pass). git diff --check PASS.
Results retain actual test names and the historical ignore reason.

Regression command: cargo test --offline --locked --all-features --lib
performance::tests::writes::batch::legacy_batch_actual_partial_stdio_requires_parent_and_child_facts
-- --exact --nocapture

Environment: pinned rust@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c,
linux/amd64,Rust1.99.1,networknone,2CPU,4GiB RAM/8GiB RAM+swap,jobs1,
incremental0,no RUSTFLAGS or RUST_MIN_STACK. Read-only current source;
no personal home/data/credentials/daemon socket mounts. Dependencies for the
current main lockfile were fetched separately into a new queue-owned registry.
The user transferred the idle x86-target cache to this queue; no old diagnostic
run was changed or executed. Held diagnostic003 remains untouched.

Fresh current-head CI and successful current-main CI are still merge gates.
Local focused tests are not a full CI pass. Queue is sole pusher/merger.
All containers auto-removed; cache is retained for PR189 follow-up until
completion, then cleaned. Worktrees, source, failed evidence and this report
remain protected. No original-root uncommitted documents changed.
