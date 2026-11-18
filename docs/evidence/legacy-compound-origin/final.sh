#!/bin/sh
set -u
finish() {
  find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum '{}' '+' > /evidence/binaries.sha256
  cat /sys/fs/cgroup/memory.events > /evidence/memory.events
  cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
}
trap finish EXIT
cargo test --offline --all-features --lib legacy_compound_origin -- --test-threads=2 > /evidence/focused.log 2>&1
status=$?; printf '%s\n' "$status" > /evidence/focused.status
[ "$status" = 0 ] || exit "$status"
cargo test --offline --all-features -- --test-threads=2 > /evidence/full.log 2>&1
status=$?; printf '%s\n' "$status" > /evidence/full.status
cargo fmt --all -- --check > /evidence/fmt.log 2>&1
printf '%s\n' "$?" > /evidence/fmt.status
cargo clippy --offline --all-features --all-targets > /evidence/clippy.log 2>&1
printf '%s\n' "$?" > /evidence/clippy.status
