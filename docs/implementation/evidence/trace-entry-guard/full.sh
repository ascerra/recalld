set -eu
printf 'SOURCE_PREFLIGHT_BEGIN\n'
sha256sum --check --strict /source.sha256
printf 'SOURCE_PREFLIGHT_OK\n'
trap 'result=$?; printf "SOURCE_POSTCHECK_BEGIN\n"; sha256sum --check --strict /source.sha256 || exit 99; printf "SOURCE_POSTCHECK_OK original_exit=%s\n" "$result"; exit "$result"' EXIT
rustc --version --verbose
cargo fmt --all -- --check
cargo test --offline --all-targets
cargo clippy --offline --all-targets
