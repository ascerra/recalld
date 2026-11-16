
### 2026-11-16: N1 locked format-selection focused validation

Source `085a93a3fda52ea5bad4fdcb74c6fba2b38cf262`, branch `impl/native-api1-admission`, exact parents actual103/main `b374f07e` + frozen E1 `6ebfd250`. Four `storage::engine::format_open_tests` passed, zero failed/ignored, 1669 filtered, 3.41s. Compile and fmt exited 0. This is a focused internal format-selection proof, not ordinary native admission, full-suite or transport readiness. Later bounded catalog WIP was excluded.

Actual empty-namespace schema migration, both ownership locks through full native preflight, unchanged source hashes on unknown/partial/malformed metadata rejection, existing/fresh legacy defaults, and missing-metadata refusal passed. Ordinary legacy open remains gated for native schema. No vector certification or record-payload equivalence claim follows from the empty fixture.

Offline Rust1.94.1, 2CPU/4GiB/jobs1, one private target with package-only clean; immutable 446-file/16,974,961-byte source verified after run. Executable SHA256 `574de0d6bfa9cd1546d94dddaaedde37fee97a42344b02a0ec48db26b684b271`. UTC21:04:51 to21:05:56. Cgroup peak4,294,967,296 bytes, memory.max7166, OOM0/kill0: reclaim pressure. Owned container retired. No personal data, daemon, credentials or network mounts.

Report: [format-001 REPORT](/tmp/recalld-implementation/native-api1-admission/docs/evidence/native-api1-admission/format-001/REPORT.md), evidence commit `3d8d8d40`. Exact command/source/raw-log hashes in adjacent RESULTS.json. Raw `/tmp/recalld-native-api1-admission-validation/001` remains protected during active implementation; no raw HOME archive. Still required: actual API1 catalog/bounded wire consumer, same-owner ordinary constructor route closure, final composed tests/CI. No push or PR yet.
