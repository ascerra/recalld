#!/bin/sh
cd /work
cargo fmt --all --check > /evidence/fmt.log 2>&1
fmt=$?
printf 'fmt %s\n' "$fmt" > /evidence/full-status.txt
cargo clippy --locked --offline --all-features --all-targets > /evidence/clippy.log 2>&1
clippy=$?
printf 'clippy %s\n' "$clippy" >> /evidence/full-status.txt
cargo test --locked --offline --all-features > /evidence/full.log 2>&1
full=$?
printf 'full %s\n' "$full" >> /evidence/full-status.txt
find /target/debug -type f -perm /111 -not -path '*/incremental/*' -exec sha256sum {} \; > /evidence/full-executables.sha256
cat /sys/fs/cgroup/memory.events > /evidence/full-memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/full-memory.peak
if [ "$fmt" -eq 0 ] && [ "$clippy" -eq 0 ] && [ "$full" -eq 0 ]; then
 echo 0 > /evidence/full-exit-code
 exit 0
fi
echo 1 > /evidence/full-exit-code
exit 1
