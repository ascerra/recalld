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
run_stage focus-01 /target/debug/deps/recalld-8b31964f64b2cfc4 coherent_by_id_actual_postcommit_failure_preserves_receipt_and_hides_payload --nocapture --test-threads=2 || status=1
run_stage focus-02 /target/debug/deps/recalld-8b31964f64b2cfc4 public_postcommit_admission_faults_retain_receipts_and_exactly_once_history --nocapture --test-threads=2 || status=1
run_stage focus-03 /target/debug/deps/recalld-8b31964f64b2cfc4 scoped_read_only_controls_survive_http_mcp_and_framed_daemon --nocapture --test-threads=2 || status=1
run_stage focus-04 /target/debug/deps/recalld-8b31964f64b2cfc4 native_forget_failed_publication_stays_blocked_until_real_retry_without_resurrection --nocapture --test-threads=2 || status=1
run_stage focus-05 /target/debug/deps/recalld-8b31964f64b2cfc4 public_matrix_summary_extreme_clocks_sticky_permastore_and_forget --nocapture --test-threads=2 || status=1
run_stage focus-06 /target/debug/deps/recalld-8b31964f64b2cfc4 native_writer_publication_http_mcp_and_reopen_keep_exact_labels_and_read_only_bytes --nocapture --test-threads=2 || status=1
run_stage focus-07 /target/debug/deps/recalld-8b31964f64b2cfc4 native_exposure_deferred_selection_never_writes_and_unsupported_recorder_stays_closed --nocapture --test-threads=2 || status=1
run_stage focus-08 /target/debug/deps/recalld-8b31964f64b2cfc4 native_maintenance_accepted_publication_failure_replays_full_receipt_then_removes_content --nocapture --test-threads=2 || status=1
run_stage focus-09 /target/debug/deps/recalld-8b31964f64b2cfc4 ordinary_point_native_old_alias_and_preobserve_budget_do_not_write --nocapture --test-threads=2 || status=1
run_stage focus-10 /target/debug/deps/recalld-8b31964f64b2cfc4 native_exposure_invalid_configured_policy_fails_without_writes_and_empty_has_no_work --nocapture --test-threads=2 || status=1
run_stage fmt cargo fmt --all -- --check || status=1
run_stage include-fmt rustfmt --edition 2024 --config skip_children=true --check src/api/coherent_by_id_tests.rs src/daemon/api1_get.rs src/daemon/api1_get_before_tests.rs src/daemon/api1_get_tests.rs src/daemon/bridge_adapters.rs src/daemon/client.rs src/daemon/mod.rs src/daemon/observed_join_tests.rs src/daemon/prepared_get.rs src/daemon/read_only_tests.rs src/daemon/server.rs src/mcp/bridge.rs src/mcp/bridge_adapters.rs src/mcp/tools.rs src/search/api1_point_get_before_tests.rs src/search/api1_point_get_tests.rs src/search/get_wire.rs src/search/get_wire_tests.rs src/serialization/api1_get_budget_probe.rs src/serialization/api1_raw_get_budget_probe.rs src/serialization/get_api1.rs src/serialization/get_rpc_budget_probe.rs src/serialization/get_tests.rs src/serialization/mod.rs src/storage/metadata/native_records/public_matrix_tests.rs src/storage/metadata/native_records/reader_tests.rs src/storage/metadata/native_records/wire_tests.rs || status=1
run_stage clippy cargo clippy --locked --offline --all-features --all-targets || status=1
echo "$status" > /evidence/driver.exit
exit "$status"
