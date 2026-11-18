#!/bin/sh
set -u
date -u +%Y-%m-%dT%H:%M:%SZ > /evidence/build.started
cargo clean --offline -p recalld > /evidence/clean.log 2>&1 || exit 1
cargo test --locked --offline --all-features --lib --no-run --message-format=json > /evidence/build.jsonl 2> /evidence/build.log
status=$?
echo "$status" > /evidence/build.exit
date -u +%Y-%m-%dT%H:%M:%SZ > /evidence/build.finished
cat /sys/fs/cgroup/memory.events > /evidence/build-memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/build-memory.peak
exit "$status"
