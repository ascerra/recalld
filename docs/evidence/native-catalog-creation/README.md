# Native catalog creation validation

Runtime `e06f7455df1097cd3658d89170d57e327a967731`, tree `feecef3aa76aa5afacabffc141d0b280e44dc0ed`; minimal actual PR82 phase-corrected parent `a69865b706beb958881b6b808fe6977222e7d1a9`. Branch `impl/native-catalog-policy`. No broad checkpoint parent.

The exact command in final-manifest.json completed with exit0: formatting check, 6 focused native catalog tests, full 929 library tests and 2 CLI tests, then all-target Clippy. Four doctests ignored. Clippy exited zero WITH warnings; all warning counts and individual test summaries are retained in result_lines. No warning-free claim.

Actual red/green proof: 498f0ed produced 4PASS/2FAIL (fixed-default replay conflict and invalid inherited policy creation). 689c57a passed all6. Later a69865b joined without changing the native catalog interface. The expanded real export/import/fork regression first failed compilation at1c13eaa due to test-only private-field access; e06f745 uses the sanctioned owning offline snapshot and passes. Original failures and immutable binary/source/log manifests are retained.

The final source-specific target never switched source roots. Offline image recalld-ci:1.94/Rust1.94.1, 2CPUs/4GiB, CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=2, registry read-only, network none, no home credentials or sockets. Cargo dev/test debug info disabled. Executable inventory records binaries named by Cargo; it is not a claim each contained nonzero tests. All-target Clippy covers examples, not their execution.

The [implementation report](../../implementation/native-catalog-creation.md) distinguishes canonical metadata commit, recoverable file publication and actual remaining writer/consumer gates. New native creation uses legacy_access_v1 by default. Numeric policy, maintenance/phase-loss, purge, Curie's native batches and whole-query readiness remain required; ordinary native serving is still disabled.

Final raw log SHA256 `581c132b887eec00b93ecddb3187f4614e61c984f0674430cddfc37b424e88ce`; final manifest SHA256 `906600fd975e95dcdb4952ea7620fede692cc255013901b7763ba414f87b3457`. Source/binary inventories include unchanged historical receipts and strict offline consumer regressions in the actual final run. No personal data, external provider calls, live daemon or remote publication was used.
