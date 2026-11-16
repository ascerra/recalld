#!/bin/sh
cd /work
result=0
cargo test --locked --offline --all-features > /evidence/full.log 2>&1
result=$?
printf 'full %s\n' "$result" >> /evidence/status.txt
if [ "$result" = 0 ]; then
 cargo fmt --all --check > /evidence/fmt.log 2>&1
 result=$?
 printf 'fmt %s\n' "$result" >> /evidence/status.txt
fi
if [ "$result" = 0 ]; then
 cargo clippy --locked --offline --all-features --all-targets > /evidence/clippy.log 2>&1
 result=$?
 printf 'clippy %s\n' "$result" >> /evidence/status.txt
fi
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum {} \; > /evidence/executables.sha256
cat /sys/fs/cgroup/memory.events > /evidence/memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
printf '%s\n' "$result" > /evidence/exit-code
exit "$result"
