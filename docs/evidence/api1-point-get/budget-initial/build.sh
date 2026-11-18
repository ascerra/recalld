#!/bin/sh
date -u +%Y-%m-%dT%H:%M:%SZ > /evidence/build.started
cargo test --locked --offline --all-features --lib --no-run --message-format=json > /evidence/build.jsonl 2> /evidence/build.log
status=$?
echo "$status" > /evidence/build.exit
date -u +%Y-%m-%dT%H:%M:%SZ > /evidence/build.finished
cat /sys/fs/cgroup/memory.events > /evidence/build-memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/build-memory.peak
exit "$status"
