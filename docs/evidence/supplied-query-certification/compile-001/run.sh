#!/bin/sh
cd /work
rustc -Vv > /output/rustc.txt
cargo clean -p recalld > /output/clean.log 2>&1
cargo test --offline --locked --all-features --lib supplied_query_certification -- --nocapture > /output/focused.log 2>&1
echo "$?" > /output/focused.exit
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum {} \; > /output/executables.sha256
cat /sys/fs/cgroup/memory.events > /output/memory.events
cat /sys/fs/cgroup/memory.peak > /output/memory.peak
