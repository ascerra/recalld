# Validation commands and scope

This is correctness validation, not a performance experiment. No quiet/exclusive host interval was requested. The image and actual owned-container environment are recorded in `runtime-environment.txt` and `VALIDATION.json`.

Final runtime: `8a0f2fb3781f12ebb18fa9c8b80f048dd936e80b`. Export all committed `src/`, `tests/`, `examples/`, `benches/`, `.cargo/`, Cargo manifest/lockfile, toolchain file and build script (where present) into a staging directory, verify against worktree bytes, then atomically rename before mounting read-only. `source.json` and `source.sha256` identify every exported file. The host worktree was never mounted. No source writes/renames occurred in the mounted snapshot during a run.

Actual final container command (log redirection is outside the container):

```sh
docker run --name recalld-hume-producer-green \
  --network none --cpus 2 --memory 4g \
  -e RUSTUP_TOOLCHAIN=1.94.1 -e CARGO_BUILD_JOBS=2 \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target \
  -v /tmp/recalld-implementation/coherent-v2-similar/.validation/cargo:/cargo \
  -v /tmp/recalld-implementation/a2-conformance-inventory/.validation/target-entity:/target \
  -v /tmp/recalld-implementation/online-producer-counters/.validation/online-producer-green/source:/work:ro \
  -v /tmp/recalld-implementation/online-producer-counters/.validation/online-producer-green/source.sha256:/source.sha256:ro \
  -v /tmp/recalld-implementation/online-producer-counters/.validation/online-producer-green/validate.sh:/validate.sh:ro \
  -w /work recalld-ci:1.94 sh /validate.sh
```

`validate.sh` performs inside-container SHA verification before any compiler/formatter call, installs an exit trap for final SHA verification (including failures), records rustc, cleans only package `recalld` in the single owned target, checks formatting, runs all-target tests and clippy offline. Focused gates run `cargo test --offline --lib online_producer_ -- --nocapture` and `cargo test --offline --lib performance::tests -- --nocapture` on the same immutable source. The latter overlaps the former; their counts must not be added as unique tests.

Before each package rebuild, record the prior relevant executable hash. After the final run, verify snapshot, current working source and committed blobs against the manifest, and hash every executable named in the test log. This source-switch procedure prevents an earlier executable from being reused as evidence for later source. Container exits, source/log hashes, failed attempts, test results and warning summaries are in `VALIDATION.json`; no failed attempt is relabeled as green.

No home directory, credentials, personal database, daemon socket or Docker socket was mounted into tests. All owned test containers are removed after their exit evidence is recorded. One private cargo/target is retained for this agent's active work. Active raw roots remain protected until the PR/results handoff, then central cleanup may delete them under the user's retention override. No raw archive or build output belongs in Git, and no new HOME raw archive is required.
