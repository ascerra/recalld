set -eu
trap 'result=$?; sha256sum -c /logs/input.sha256 > /logs/source-after.log 2>&1 || result=91; find /target/debug/deps -maxdepth 1 -type f -name "recalld-*" -perm /111 -exec sha256sum {} \; > /logs/executables.sha256; echo "$result" > /logs/status; exit "$result"' EXIT
sha256sum -c /logs/input.sha256 > /logs/source-before.log 2>&1
rustc -Vv > /logs/rustc.txt
cargo clean --offline -p recalld > /logs/package-clean.log 2>&1
cargo fmt --all -- --check > /logs/fmt.log 2>&1
cargo test --offline --locked --all-features --lib native_single_ -- --nocapture > /logs/single.log 2>&1
cargo test --offline --locked --all-features --lib native_single_wire_genuine_before_dense_full_receipt_fits_all_transports -- --ignored --nocapture > /logs/genuine-before.log 2>&1
cargo test --offline --locked --all-features > /logs/full.log 2>&1
cargo clippy --offline --locked --all-features --all-targets > /logs/clippy.log 2>&1
