#!/bin/sh
cd /work
rustc -Vv > /output/rustc.txt
cargo clean -p recalld > /output/clean.log 2>&1
for filter in supplied_query_certification native_query_admission similar_v2 similar_v1 strict_query; do
 cargo test --offline --locked --all-features --lib "$filter" -- --nocapture > "/output/$filter.log" 2>&1
 echo "$?" > "/output/$filter.exit"
done
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum {} \; > /output/executables.sha256
cat /sys/fs/cgroup/memory.events > /output/memory.events
cat /sys/fs/cgroup/memory.peak > /output/memory.peak
