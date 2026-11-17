#!/bin/sh
set -u
finish() {
  find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum '{}' '+' > /evidence/binaries.sha256
  find /target/debug/deps -maxdepth 1 -type f \( -name 'lifecycle_cli-*' -o -name 'text_migration_cli-*' \) -perm /111 -exec sha256sum '{}' '+' >> /evidence/binaries.sha256
  cat /sys/fs/cgroup/memory.events > /evidence/memory.events
  cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
}
trap finish EXIT
cargo test --offline --all-features --lib store_batches -- --test-threads=2 > /evidence/native-batch.log 2>&1
status=$?; printf '%s\n' "$status" > /evidence/native-batch.status
[ "$status" = 0 ] || exit "$status"
cargo test --offline --all-features --lib legacy_compound_origin -- --test-threads=2 > /evidence/compound.log 2>&1
printf '%s\n' "$?" > /evidence/compound.status
cargo test --offline --all-features --test lifecycle_cli --test text_migration_cli -- --test-threads=2 > /evidence/cli.log 2>&1
printf '%s\n' "$?" > /evidence/cli.status
cargo test --offline --all-features --bins > /evidence/bins.log 2>&1
printf '%s\n' "$?" > /evidence/bins.status
cargo test --offline --all-features --doc > /evidence/doc.log 2>&1
printf '%s\n' "$?" > /evidence/doc.status
cargo fmt --all -- --check > /evidence/fmt.log 2>&1
printf '%s\n' "$?" > /evidence/fmt.status
cargo clippy --offline --all-features --all-targets > /evidence/clippy.log 2>&1
printf '%s\n' "$?" > /evidence/clippy.status
