#!/bin/sh
cd /work
cargo fmt --all --check > /evidence/fmt.log 2>&1
code=$?
printf 'fmt %s\n' "$code" > /evidence/status.txt
if [ "$code" -eq 0 ]; then
 cargo clean -p recalld > /evidence/clean.log 2>&1
 code=$?
 printf 'clean %s\n' "$code" >> /evidence/status.txt
fi
for filter in live_read_schemas_ live_metadata_schemas_ similar_v2_actual_legacy_false_flag_uses_bound_scope_without_current_rewrite session_context_resolves_each_bridge; do
 if [ "$code" -eq 0 ]; then
  cargo test --locked --offline --all-features --lib "$filter" -- --nocapture > "/evidence/$filter.log" 2>&1
  code=$?
  printf '%s %s\n' "$filter" "$code" >> /evidence/status.txt
 fi
done
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum {} \; > /evidence/executables.sha256
cat /sys/fs/cgroup/memory.events > /evidence/memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
printf '%s\n' "$code" > /evidence/exit-code
exit "$code"
