#!/bin/bash
set -eu
rustc --version > /evidence/environment.log
cargo --version >> /evidence/environment.log
printf 'fmt started\n' > /evidence/status.log
cargo fmt --all -- --check > /evidence/fmt.log 2>&1
printf 'fmt passed; tests started\n' >> /evidence/status.log
cargo test --offline --all-features > /evidence/tests.log 2>&1
printf 'tests passed; clippy started\n' >> /evidence/status.log
cargo clippy --offline --all-features --all-targets > /evidence/clippy.log 2>&1
printf 'clippy passed; final checks complete\n' >> /evidence/status.log
