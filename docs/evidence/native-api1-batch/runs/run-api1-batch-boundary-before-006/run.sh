#!/bin/sh
cd /work
rustc --version > /evidence/rustc.txt
cargo clean --offline -p recalld > /evidence/clean.log 2>&1
cargo test --offline --all-features --lib api1_batch_malformed_remote_evidence_never_claims_commit_or_counts -- --nocapture > /evidence/before.log 2>&1
status=$?
printf '%s\n' "$status" > /evidence/before.exit
cat /sys/fs/cgroup/memory.events > /evidence/memory.events.final
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak.final
exit "$status"
