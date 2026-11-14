#!/bin/sh
set -eu
# Execute only after main's explicit END. Source stays read-only for proof.
python3 .validation/source-manifest.py > .validation/source-before.json
docker run --rm --network none --cpus 2 --memory 4g \
  -e RUSTUP_TOOLCHAIN=1.94.1 -e CARGO_BUILD_JOBS=2 \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target \
  -v /tmp/recalld-implementation/accepted-producers:/work:ro \
  -v /tmp/recalld-implementation/accepted-producers/.validation/cargo:/cargo \
  -v /tmp/recalld-implementation/accepted-producers/.validation/target:/target \
  -w /work recalld-ci:1.94 \
  sh -c 'rustc -Vv && cargo fmt --all -- --check && cargo test --offline --all-features && cargo test --offline --all-features --example offline_eval && cargo clippy --offline --all-features --all-targets' \
  > .validation/final.log 2>&1
python3 .validation/source-manifest.py > .validation/source-after.json
cmp .validation/source-before.json .validation/source-after.json
