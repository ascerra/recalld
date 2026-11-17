#!/bin/sh
cd /work
cargo clean -p recalld > /evidence/clean.log 2>&1
code=$?
printf 'clean %s\n' "$code" > /evidence/status.txt
if [ "$code" -eq 0 ]; then
 cargo test --locked --offline --all-features --lib live_metadata_schemas_ -- --nocapture > /evidence/focus.log 2>&1
 code=$?
 printf 'focus %s\n' "$code" >> /evidence/status.txt
fi
if [ "$code" -eq 0 ]; then
 cargo test --locked --offline --all-features --lib session_context_resolves_each_bridge -- --nocapture > /evidence/session.log 2>&1
 code=$?
 printf 'session %s\n' "$code" >> /evidence/status.txt
fi
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum {} \; > /evidence/executables.sha256
cat /sys/fs/cgroup/memory.events > /evidence/memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
printf '%s\n' "$code" > /evidence/exit-code
exit "$code"
