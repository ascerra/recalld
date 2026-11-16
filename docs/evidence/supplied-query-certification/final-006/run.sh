#!/bin/sh
cd /work
rustc -Vv > /output/rustc.txt
cargo clean -p recalld > /output/clean.log 2>&1
failed=0
for filter in native_writer_publication_http_mcp_and_reopen_keep_exact_labels_and_read_only_bytes supplied_query_certification; do
 cargo test --offline --locked --all-features --lib "$filter" -- --nocapture > "/output/$filter.log" 2>&1
 result=$?; echo "$result" > "/output/$filter.exit"
 if [ "$result" != 0 ]; then failed=1; fi
done
if [ "$failed" = 0 ]; then
 cargo test --offline --locked --all-features > /output/full.log 2>&1
 echo "$?" > /output/full.exit
 cargo fmt --all -- --check > /output/fmt.log 2>&1
 echo "$?" > /output/fmt.exit
 cargo clippy --offline --locked --all-features --all-targets > /output/clippy.log 2>&1
 echo "$?" > /output/clippy.exit
fi
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum {} \; > /output/executables.sha256
cat /sys/fs/cgroup/memory.events > /output/memory.events
cat /sys/fs/cgroup/memory.peak > /output/memory.peak
