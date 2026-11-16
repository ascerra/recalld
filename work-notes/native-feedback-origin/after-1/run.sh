set -eu
cd /work
python3 .test-scratch/seal.py verify .test-scratch/after-1
cargo fmt --all
python3 .test-scratch/seal.py seal .test-scratch/after-1
cargo test --offline --lib original_api -- --nocapture
cargo test --offline --lib native_unsupported_contracts -- --nocapture
cargo test --offline --lib native_lifecycle_wire -- --nocapture
python3 .test-scratch/seal.py binary .test-scratch/after-1
