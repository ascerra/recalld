#!/bin/sh
cd /work
finish() {
 find /target/debug/deps /target/debug/examples -maxdepth 1 -type f -name 'recalld*' -perm /111 -exec sha256sum {} \; > /evidence/executables.sha256
 cat /sys/fs/cgroup/memory.events > /evidence/memory.events
 cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
}
rustc -Vv > /evidence/rustc.txt
cargo clean -p recalld > /evidence/clean.log 2>&1
for filter in creation_adapters_reject_unusable_lifecycle_policy_before_persistence catalog_completion namespace_creation owned_namespace_clock queued_catalog_adapters; do
 cargo test --offline --locked --all-features --lib "$filter" -- --nocapture > "/evidence/$filter.log" 2>&1
 code=$?
 echo "$code" > "/evidence/$filter.exit"
 if [ "$code" != 0 ]; then finish; exit "$code"; fi
done
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum {} \; > /evidence/focused-executables.sha256
cargo test --offline --locked --all-features > /evidence/full.log 2>&1
echo "$?" > /evidence/full.exit
cargo fmt --all -- --check > /evidence/fmt.log 2>&1
echo "$?" > /evidence/fmt.exit
cargo clippy --offline --locked --all-features --all-targets > /evidence/clippy.log 2>&1
echo "$?" > /evidence/clippy.exit
finish
