# Proposed bounded continuation, no source change

Review basis: runtime22bc5f0c, run006 exit124, exact preserved library executable SHA256ecd1db3f588751cce45170b39e824e0fe1a06fdf6928eb42ee483d615ed5059e. This is a coverage continuation, never a claim that run006 passed.

Use one new owned output root007, but read-only reuse005/source and the same protected target. No compile, package clean, source copy or target clone before execution. Keep Rust1.94.1/aarch64/default stack, 2CPU/4GiB/jobs1/test-threads1, no network, HOME=/tmp/home and no personal mounts. Verify all14 test and4 subprocess executable hashes before execution; preserve them through completion.

Library command is a structured argv array: `[exact_executable, "--exact", "--test-threads=1", ...remaining_library]`, where `remaining_library` is the277 literal full names in `continuation-plan.json`. First run the identical filters with `--list --format=terse` and require exact set/cardinality equality, no duplicates or extra cases. This catches wrong filter semantics before any test execution. Nested-child output does not define coverage: the final harness summary must report277 successful cases, no failures/ignores, and exact selected-list identity. A20-minute command cap covers this remaining library subset.

Then execute each of the13 other preserved test harnesses directly, with `--test-threads=1`, a combined10-minute cap, and recorded argv/output/status. Their inventory has25 tests total:10 CLI integration and15 example cases; zero-test harnesses must also exit0. CLI source uses compile-time `env!(CARGO_BIN_EXE_...)` paths, so the unchanged `/work/source` and `/work/target` mounts preserve the actual invoked binaries. Do not force ignored fixture generators or omit the final unconfirmed case.

After successful tests and executable-hash verification, run separate `cargo clippy --all-targets --all-features --locked --offline` with a10-minute cap. This is lint/compilation, not all-feature test execution. Formatting already passed on the unchanged source; no repeat needed solely for documentation.

Final report must retain006TIMEOUT plus007 results separately. Only exact inventory reconciliation may state that default all-target cases were covered across the two runs; never report a single successful full command. Any failure/timeout remains immutable, with no automatic source change, assertion relaxation, test exclusion or repeated full run.
