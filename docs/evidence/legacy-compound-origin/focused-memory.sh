#!/bin/sh
set -u
cargo test --offline --all-features --lib legacy_compound_origin -- --test-threads=2 > /evidence/focused.log 2>&1
printf '%s\n' "$?" > /evidence/focused.status
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum '{}' '+' > /evidence/binaries.sha256
cat /sys/fs/cgroup/memory.events > /evidence/memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
