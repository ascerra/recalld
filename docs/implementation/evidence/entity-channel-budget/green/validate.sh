set -eu
rustc --version --verbose
cargo clean -p recalld --offline
cargo fmt --all -- --check
cargo test --offline --lib entity_total_budget_counts_vector_fts_overlap_before_union -- --nocapture
cargo test --offline --lib diagnostics_capture_overlapping_sources_before_dedup_and_only_returned_ids -- --nocapture
cargo test --offline --lib entity_recall_filters_namespace_and_metadata_before_cap -- --nocapture
cargo test --offline --all-targets
cargo clippy --offline --all-targets
