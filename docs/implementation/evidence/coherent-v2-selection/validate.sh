#!/bin/sh
set -eu
rustc --version --verbose
cargo clean -p recalld --offline
cargo fmt --all -- --check
rustfmt --edition 2024 --check src/search/payload_tests.rs
cargo test --offline --lib coherent_v2 -- --nocapture
cargo test --offline --lib coherent_payload_http_never_exposes_unserialized_graph_neighbors -- --nocapture
cargo test --offline --lib
cargo test --offline --test lifecycle_cli --test text_migration_cli
cargo clippy --offline --all-targets
