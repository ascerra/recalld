#!/bin/sh
# Source/executables are frozen and separately hashed before execution.
run_stage() {
    stage="$1"
    shift
    date -u +%Y-%m-%dT%H:%M:%SZ > "/evidence/$stage.started"
    "$@" > "/evidence/$stage.log" 2>&1
    result=$?
    echo "$result" > "/evidence/$stage.exit"
    date -u +%Y-%m-%dT%H:%M:%SZ > "/evidence/$stage.finished"
    cat /sys/fs/cgroup/memory.events > "/evidence/$stage-memory.events"
    cat /sys/fs/cgroup/memory.peak > "/evidence/$stage-memory.peak"
    return "$result"
}
run_stage affected /target/debug/deps/recalld-8b31964f64b2cfc4 prepared_get_rpc_actual_ --nocapture || exit 1
status=0
run_stage full cargo test --locked --offline --all-features --all-targets || status=1
run_stage fmt cargo fmt --all -- --check || status=1
run_stage include-fmt rustfmt --edition 2024 --check src/search/get_wire_tests.rs src/serialization/get_tests.rs src/serialization/get_rpc_budget_probe.rs || status=1
run_stage clippy cargo clippy --locked --offline --all-features --all-targets || status=1
run_stage doc cargo test --locked --offline --all-features --doc || status=1
echo "$status" > /evidence/driver.exit
exit "$status"
