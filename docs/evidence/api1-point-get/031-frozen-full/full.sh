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
status=0
run_stage full cargo test --locked --offline --all-features --all-targets || status=1
run_stage fmt cargo fmt --all -- --check || status=1
run_stage include-fmt rustfmt --edition 2024 --config skip_children=true --check src/daemon/api1_get.rs src/daemon/api1_get_before_tests.rs src/daemon/api1_get_tests.rs src/daemon/bridge_adapters.rs src/daemon/client.rs src/daemon/mod.rs src/daemon/prepared_get.rs src/daemon/server.rs src/mcp/bridge.rs src/mcp/bridge_adapters.rs src/mcp/tools.rs src/search/api1_point_get_before_tests.rs src/search/api1_point_get_tests.rs src/search/get_wire.rs src/search/get_wire_tests.rs src/serialization/api1_get_budget_probe.rs src/serialization/api1_raw_get_budget_probe.rs src/serialization/get_api1.rs src/serialization/get_rpc_budget_probe.rs src/serialization/get_tests.rs src/serialization/mod.rs || status=1
run_stage clippy cargo clippy --locked --offline --all-features --all-targets || status=1
run_stage doc cargo test --locked --offline --all-features --doc || status=1
echo "$status" > /evidence/driver.exit
exit "$status"
