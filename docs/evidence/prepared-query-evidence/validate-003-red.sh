#!/bin/sh
cd /work
cargo clean -p recalld > /evidence/clean.log 2>&1
cargo test --locked --offline --all-features --lib prepared_evidence_rejects_operator_rif_ -- --nocapture > /evidence/focused.log 2>&1
result=$?
printf 'focused %s\n' "$result" > /evidence/status.txt
printf '%s\n' "$result" > /evidence/exit-code
cat /sys/fs/cgroup/memory.events > /evidence/memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
exit "$result"
