set -eu
cd /source
cargo test --offline --all-features api1_public_ -- --nocapture > /validation/affected-public.log 2>&1
cargo test --offline --all-features scope_assertions -- --nocapture > /validation/affected-scope.log 2>&1
cargo test --offline --all-features decoder_native_ -- --nocapture > /validation/affected-decoder.log 2>&1
cargo test --offline --all-features native_lifecycle_wire_ -- --nocapture > /validation/affected-wire.log 2>&1
cargo test --offline --all-features legacy_api2_ -- --nocapture > /validation/affected-legacy.log 2>&1
cargo fmt --all -- --check > /validation/fmt.log 2>&1
cargo clippy --offline --all-targets --all-features > /validation/clippy.log 2>&1
