#!/bin/sh
cd /work
rustc --version > /evidence/rustc.txt
trap 'cat /sys/fs/cgroup/memory.events > /evidence/memory.events.final; cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak.final' EXIT
cargo clean --offline -p recalld > /evidence/clean.log 2>&1 || exit $?
run() {
  name="$1"; shift
  "$@" > "/evidence/$name.log" 2>&1
  status=$?
  printf '%s\n' "$status" > "/evidence/$name.exit"
  if [ "$status" != 0 ]; then exit "$status"; fi
}
run parity cargo test --offline --all-features --lib native_batch_mcp_daemon_parity_invalid_items_replay_and_current_publication_receipt -- --nocapture
run focused cargo test --offline --all-features --lib api1_batch_ -- --nocapture
run budget cargo test --offline --all-features --lib historical_oversize -- --nocapture
run fmt cargo fmt --all --check
run clippy cargo clippy --offline --all-features --all-targets
run full cargo test --offline --all-features
