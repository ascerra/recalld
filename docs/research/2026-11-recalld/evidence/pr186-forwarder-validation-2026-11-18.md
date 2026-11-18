# PR186 completion forwarding correction

Candidate base: `073ca8a690067848bb60abb52411c51e03618862`.
Source: `/private/tmp/recalld-implementation/pr186-completion-forwarder`.

Removed the pure async `complete_observing` forwarding layer; both callers now call `complete_observing_bounded` directly with the same arguments. Independent source review by Wegener found no semantic changes. This demonstrates a correction for the reproduced regression, not a measured general performance improvement or a complete explanation of compiler stack usage.

Validation used an offline linux/amd64 Docker container, Rust 1.99.1 image `rust@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c`, default stack, no RUSTFLAGS, one build job, incremental disabled, 2 CPUs, 4 GiB RAM and 8 GiB memory-plus-swap. No personal data, sockets, or credentials were mounted. The previously held diagnostic003 was not run.

- `cargo test --offline --locked --all-features --lib performance::tests::writes::batch::legacy_batch_actual_partial_stdio_requires_parent_and_child_facts -- --exact --nocapture`: PASS, 1 test, 0.53 seconds; rebuilt in 1m43s. The unchanged base had previously aborted with stack overflow under this compiler/default-stack profile.
- Fresh executable filter `derived_completion::tests::`, two test threads: PASS, 12 tests, 2.27 seconds.
- Fresh executable filter `storage::native_purge_tests::api1_consumer::`, two test threads: PASS, 20 tests, 56.57 seconds.
- `git diff --check`: PASS.

SHA256 provenance:

- `src/derived_completion.rs`: `4f11ebc02e03586b83fdf771164c0e469a49094620406c00498d482caa3791cd`
- `src/derived_completion/native_purge.rs`: `14c49c50c9800e609d045bf0a6b3a858099fa791f101f2f333bcd8688e59bda2`
- rebuilt `recalld-f7864ad1d33c9234`: `803e198ea66a2dd414f53fb01479825ffbfcda7c16bb5dfff6c31634c239995c`

Remaining: compose with current main, run current-head CI, merge prerequisite #186 before #189. Main CI for `4ad3cb0f0d0af3477861a576b90f068ba7c3564f` passed (run 44255106721).

Cleanup: both validation containers used `--rm` and exited. No new full build cache or datasets were copied. Existing x86 target and registry remain owned by the integration effort for follow-up validation; source worktree is retained for the PR update. No existing data deleted, zero reclaimed-space claim.
