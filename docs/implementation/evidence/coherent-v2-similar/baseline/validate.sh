set -eu
rustc --version --verbose
cargo clean -p recalld --offline
cargo fmt --all -- --check
cargo test --offline --lib --no-run
set +e
for probe in similar_baseline_phase_before_cap similar_baseline_current_terminal similar_baseline_explicit_cross_scope_is_ignored; do
 cargo test --offline --lib "$probe" -- --nocapture > "/evidence/$probe.log" 2>&1
 code=$?
 printf '%s %s\n' "$probe" "$code" >> /evidence/exits.txt
done
