set -eu
rustc --version --verbose
cargo fmt --all -- --check
rustfmt --edition 2024 --check src/search/payload_tests.rs src/search/similar_selection_tests.rs
cargo test --offline --lib search::pipeline::read_view::tests::similar -- --nocapture
