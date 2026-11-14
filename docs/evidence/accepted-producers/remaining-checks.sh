#!/bin/sh
set -eu
docker run --rm --network none --cpus 2 --memory 4g \
  -e RUSTUP_TOOLCHAIN=1.94.1 -e CARGO_BUILD_JOBS=2 \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target \
  -v /tmp/recalld-implementation/accepted-producers:/work:ro \
  -v /tmp/recalld-implementation/accepted-producers/.validation/cargo:/cargo \
  -v /tmp/recalld-implementation/accepted-producers/.validation/target:/target \
  -w /work recalld-ci:1.94 \
  sh -c 'cargo fmt --all -- --check && cargo clippy --offline --all-features --all-targets' \
  > .validation/remaining-checks.log 2>&1
