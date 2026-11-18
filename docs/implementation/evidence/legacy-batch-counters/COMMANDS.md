# Isolated validation commands and attribution

Parent PR165: `3cfa164453d5e1c0f149e123acbc2331eca384ab`. Pre-code plan: `9514ac59cac58de176544869c80837a5558a16ab`. Source ownership: `/tmp/recalld-implementation/legacy-batch-counters`, branch `impl/legacy-batch-counters`.

The completed committed-source export includes tracked `src/`, `tests/`, `examples/`, `benches/`, `.cargo/`, Cargo manifests/lock, toolchain and build script where present. Git blobs are read from the pinned commit, compared with working files, written to a fresh staging directory, then atomically renamed before mounting. `source.json` and `source.sha256` bind every exported file. Sources are mounted read-only, never edited or renamed during a run. `validate.sh` verifies the manifest **inside the container before compilation/formatting**, and its EXIT trap repeats verification even when a test fails. Final host verification compares the snapshot, working tree and pinned Git blobs again.

All runtime containers use `recalld-ci:1.94`, recorded immutable image ID in `VALIDATION.json`, `RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_BUILD_JOBS=2`, `--network none --cpus 2 --memory 4g`. No home, credentials, personal data or Docker socket mounts. Tests create disposable isolated stores. No host Cargo test, stack override, benchmark window, global cache clean or cache clone.

The only writable build mounts are this agent's retained cargo `/tmp/recalld-implementation/coherent-v2-similar/.validation/cargo` at `/cargo` and target `/tmp/recalld-implementation/a2-conformance-inventory/.validation/target-entity` at `/target`. The complete frozen source is `/work:ro`, manifest `/source.sha256:ro`, driver `/validate.sh:ro`. Each source switch preserves existing executable evidence, then package-cleans only `recalld` in this owned target. Dependencies remain cached. A failed compilation is recorded without claiming a new executable/test outcome.

Focused commands after inside-source verification, package clean and fmt:

```sh
cargo test --offline --lib legacy_batch_ -- --nocapture
cargo test --offline --lib performance::tests -- --nocapture
```

The genuine preimplementation RED used `cargo test --offline --lib legacy_batch_actual_partial_stdio_requires_parent_and_child_facts -- --nocapture`. Earlier fixture failures and the intended missing-counter failure are separately attributed. The first implementation compile stopped before any test. The corrected focused sets overlap and must not be summed as unique tests.

Final commands are exactly [validate.sh](validate.sh): `cargo fmt --all -- --check`, `cargo test --offline --all-targets`, and `cargo clippy --offline --all-targets`. Local scope is **all-targets, not all-features**. Warnings are retained; there is no warning-free claim. Test execution/compilation durations are not service latency, throughput, overhead or performance-promotion measurements.

`VALIDATION.json` records source, driver, log, image, container-exit and executable hashes; `RESULTS.txt` retains compact outcomes and failed-attempt identities. Raw logs/snapshots/binaries remain outside Git. Following resulting PR/results recording, exact completed roots are released to the central cleanup owner under the user retention override; no redundant raw HOME archive is required. The live single cache/target remains protected for subsequent work.
