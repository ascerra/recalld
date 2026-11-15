#!/bin/sh
cd /work
rustc -Vv > /evidence/rustc.txt
cargo clean -p recalld > /evidence/clean.log 2>&1
cargo test --offline --locked --all-features --lib catalog_owner_create_then_false_recall_probe -- --nocapture > /evidence/probe.log 2>&1
status=$?
echo "$status" > /evidence/probe.exit
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum {} \; > /evidence/executables.sha256
cat /sys/fs/cgroup/memory.events > /evidence/memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
exit "$status"
