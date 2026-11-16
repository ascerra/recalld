#!/bin/sh
cd /work
status=0
cargo fmt --all -- --check > /evidence/fmt.log 2>&1
result=$?; printf 'fmt %s\n' "$result" >> /evidence/status.txt
if [ "$result" -ne 0 ]; then status=$result; else
 cargo test --locked --offline --all-features > /evidence/full.log 2>&1
 result=$?; printf 'full %s\n' "$result" >> /evidence/status.txt
 if [ "$result" -ne 0 ]; then status=$result; fi
 cargo clippy --locked --offline --all-features --all-targets > /evidence/clippy.log 2>&1
 result=$?; printf 'clippy %s\n' "$result" >> /evidence/status.txt
 if [ "$result" -ne 0 ]; then status=$result; fi
fi
printf '%s\n' "$status" > /evidence/full-exit-code
cat /sys/fs/cgroup/memory.events > /evidence/full-memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/full-memory.peak
exit "$status"
