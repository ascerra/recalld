set -eu
rustc --version --verbose
cargo fmt --all -- --check
cargo test --offline --lib entity_total_budget_counts_vector_fts_overlap_before_union -- --nocapture
