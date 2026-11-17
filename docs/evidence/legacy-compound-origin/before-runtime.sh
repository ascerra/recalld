#!/bin/sh
set -u
cargo test --offline --all-features --lib legacy_compound_origin_actual -- --test-threads=2 > /evidence/before.log 2>&1
printf '%s\n' "$?" > /evidence/before.status
RECALLD_LEGACY_COMPOUND_BEFORE=/evidence/legacy-compound-before.zip cargo test --offline --all-features --lib legacy_compound_origin_export_genuine_old_one -- --ignored --test-threads=1 > /evidence/fixture.log 2>&1
printf '%s\n' "$?" > /evidence/fixture.status
find /target/debug/deps -maxdepth 1 -type f -name 'recalld-*' -perm /111 -exec sha256sum '{}' '+' > /evidence/binaries.sha256
