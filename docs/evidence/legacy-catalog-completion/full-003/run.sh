#!/bin/sh
cd /work
rustc -Vv > /evidence/rustc.txt
cargo test --offline --locked --all-features > /evidence/full.log 2>&1
echo "$?" > /evidence/full.exit
cargo fmt --all -- --check > /evidence/fmt.log 2>&1
echo "$?" > /evidence/fmt.exit
cargo clippy --offline --locked --all-features --all-targets > /evidence/clippy.log 2>&1
echo "$?" > /evidence/clippy.exit
find /target/debug/deps /target/debug/examples -maxdepth 1 -type f -name 'recalld*' -perm /111 -exec sha256sum {} \; > /evidence/executables.sha256
cat /sys/fs/cgroup/memory.events > /evidence/memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
