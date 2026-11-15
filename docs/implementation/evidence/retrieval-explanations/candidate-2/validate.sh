set -eu
rustc --version --verbose
cargo clean -p recalld --offline
cargo fmt --all -- --check
cargo test --offline --lib explanation_tests -- --nocapture
cargo test --offline --lib v2_inheritance_preserves_min_score_temporal_boost_and_stronger_own_evidence -- --nocapture
cargo test --offline --lib coherent_v2_final_hydrated_exposures_reach_actual_recorder_once -- --nocapture
cargo test --offline --all-targets
cargo clippy --offline --all-targets
