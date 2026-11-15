set -eu
rustc --version --verbose
cargo clean -p recalld --offline
cargo fmt --all -- --check
rustfmt --edition 2024 --check src/search/payload_tests.rs src/search/similar_selection_tests.rs
cargo test --offline --lib search::pipeline::read_view::tests::similar -- --nocapture
cargo test --offline --lib by_id_scope_and_record_policy_mismatches_fail_closed -- --nocapture
cargo test --offline --lib by_id_selected_phase_scope_or_missing_row_fails_before_observer -- --nocapture
cargo test --offline --lib similarity_threshold_namespace_and_returned_only_exposure -- --nocapture
cargo test --offline --all-targets
cargo clippy --offline --all-targets
