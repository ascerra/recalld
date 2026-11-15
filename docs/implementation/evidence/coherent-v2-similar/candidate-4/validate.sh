set -eu
rustc --version --verbose
cargo clean -p recalld --offline
cargo fmt --all -- --check
rustfmt --edition 2024 --check src/search/payload_tests.rs src/search/similar_selection_tests.rs
cargo test --offline --lib search::pipeline::read_view::tests::similar -- --nocapture
cargo test --offline --lib similarity_threshold_namespace_and_returned_only_exposure -- --nocapture
cargo test --offline --all-targets
cargo clippy --offline --all-targets
