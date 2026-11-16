set -eu
printf 'SOURCE_PREFLIGHT_BEGIN\n'
sha256sum --check --strict /source.sha256
printf 'SOURCE_PREFLIGHT_OK\n'
trap 'result=$?; printf "SOURCE_POSTCHECK_BEGIN\n"; sha256sum --check --strict /source.sha256 || exit 99; printf "SOURCE_POSTCHECK_OK original_exit=%s\n" "$result"; exit "$result"' EXIT
rustc --version --verbose
cargo clean -p recalld --offline
cargo fmt --all -- --check
cargo test --offline --lib trace_provenance_actual_mcp_after_unsubscribed_first_callsite -- --nocapture
