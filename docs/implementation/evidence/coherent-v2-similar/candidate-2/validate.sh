set -eu
rustc --version --verbose
cargo clean -p recalld --offline
cargo fmt --all -- --check
rustfmt --edition 2024 --check src/search/payload_tests.rs src/search/similar_selection_tests.rs
cargo test --offline --lib similar_v2 -- --nocapture
