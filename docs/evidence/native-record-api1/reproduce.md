# Reproduce native plain-record API origin checks

Use a new scratch checkout of `f19655b5e886e364a1f0d471412ff82202bec554` and
private Cargo/target directories. The fixture is bundled; no fixture environment
variable is needed. Tests create temporary stores only.

```sh
docker run --rm --network none --cpus 2 --memory 4g \
  -e RUSTUP_TOOLCHAIN=1.94.1 -e CARGO_BUILD_JOBS=2 \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target \
  -v "$PWD:/work" -v "$PWD/.validation/cargo:/cargo" \
  -v "$PWD/.validation/target:/target" -w /work recalld-ci:1.94 \
  bash -c 'cargo fmt --all --check &&
    cargo test --offline --all-features native_forget_scope_contract_and_stale_cas_reject_before_clock_or_bytes -- --nocapture &&
    cargo test --offline --all-features native_record_ -- --nocapture &&
    cargo test --offline --all-features &&
    cargo clippy --offline --all-features --all-targets'
```

If a target was copied or used under another source root, run `cargo clean -p
recalld` in that private target before building. Do not reuse a BEFORE executable
as AFTER proof. The recorded experiment package-cleaned its own copied target,
kept one source root throughout, and separately archived/hashed every source and
executable. No dependency fetch occurs during these test runs.

The preserved BEFORE experiment uses test-only71721fa on unchanged121 runtime,
sets `RECALLD_WRITE_RECORD_CONTEXT_BEFORE=/work/.validation/before-fixture`, then
runs `native_record_context_before_fixture` and `native_record_api1_`. The latter
has three expected failures. `source-before.tar` and `before-fixture.tar` retain
that source and output. The bundled ZIP members match the original two fixture
files byte-for-byte; they were not regenerated with the corrected runtime.

Archive mapping: `source-before` is71721fa; `source-focused` is the first
intermediate AFTER source; `source-final` isb3a21e6 (failed full run, obsolete
negative fixture); `source-shipping` isf19655b (test-only follow-up). Their JSON
manifests, logs and individual library SHA files are retained separately. The
initial focused result is not substituted for the final full-suite proof.
