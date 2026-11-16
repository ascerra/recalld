#!/bin/sh
set -eu
docker run --rm --network none --cpus 2 --memory 4g \
 -e RUSTUP_TOOLCHAIN=1.94.1 -e CARGO_BUILD_JOBS=2 -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target \
 -v /tmp/recalld-implementation/native-batch-binding:/work:ro \
 -v /tmp/recalld-implementation/native-batch-binding/.validation/cargo:/cargo \
 -v /tmp/recalld-implementation/native-batch-binding/.validation/target:/target \
 -w /work recalld-ci:1.94 \
 sh -c 'cargo test --offline --all-features && cargo fmt --all -- --check && cargo clippy --offline --all-features --all-targets' \
 > .validation/native-full-final.log 2>&1
python3 .validation/source-manifest.py > .validation/source-after-final.json
cmp .validation/source-before-final.json .validation/source-after-final.json
