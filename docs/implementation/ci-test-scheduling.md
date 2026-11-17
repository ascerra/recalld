# CI test scheduling

The PR test critical path is library execution, not compilation or linting.
This change partitions that execution into four ordinary Cargo/libtest jobs per
OS and retains the required `Test (ubuntu-latest)` and `Test (macos-latest)`
checks as strict collectors. No baseline test is removed or newly ignored.

## Actual baseline and first patch

Source: `32ab3b3f754e778403a53d400b36f249a3f9fd2c` (main after PR190).
Measured workflow: [44151177990](https://github.com/calebevans/recalld/actions/runs/44151177990).
First patch: `dd6b8107d44e24e03a65d78e31abc63fd140c7c1`; final runner
`1fac0977ed19a0747a7a198a058d78063cce9ac0` on
`impl/ci-test-scheduling`; no production, Cargo, compiler, stack, or Rust test
changes. Halley reviewed the file scope and collector design; queue owns
publication/integration. PR189 and all current CI runs remain untouched.

| Measured stage | Ubuntu | macOS |
| --- | ---: | ---: |
| Test job including setup | 49m04s | 45m52s |
| `cargo test --all-features` step | 48m40s | 45m09s |
| Cargo compilation reported by Cargo | 2m05s | 4m24s |
| Library harness execution | 2765.83s | 2413.14s |
| Library passed / ignored | 1763 / 6 | 1762 / 6 |
| CLI integration tests passed | 10 | 10 |
| Documentation tests ignored | 4 | 4 |

Clippy took62s including setup (38s command), Docker527s and release build323s.
The workflow installs1.94, but the repository's unchanged stable override made
the actual Cargo/cache compiler1.99.1 on both OSes. This patch deliberately
preserves that selection, including the default-stack regression exposure; it
does not pin away the held E1/PR186 failure or retry/reroute Hume's diagnostic.

The timing evidence identifies expensive real setup, not an arbitrary slow-test
skiplist. GitHub logs provide completion times and60-second warnings, not exact
per-test start times. The following are lower bounds from warning-to-completion
plus60s; concurrent harness execution means they cannot be summed as wall time.

| Concrete test suffix | Ubuntu lower bound | Source / reason to retain |
| --- | ---: | --- |
| `batch_wire_historical_oversize_completed_pending_retained_bytes_and_settings` | 350.09s | batch_wire_tests.rs702: genuine pending/completed archives, two link settings, key/ID retries, actual daemon/MCP/HTTP, unchanged source and full receipts |
| `similar_explanation_display_cap_preserves_true_max_in_both_iteration_orders` | 285.54s | similar_selection_tests.rs1206: four fixtures with70 origins, ties/order/display cap and unchanged bytes |
| `public_matrix_feedback_matches_resolved_planner_and_original_one` | 153.86s | public_matrix_tests.rs236: layout/transport/multiplier/quality matrix, original-contract hashes, receipt retention and reopen |
| `similar_v2_actual_chain_64_resolves_65_is_unresolved_history_stays_raw` | 123.54s | chain boundary and retained-history behavior |

The shared native fixture performs real legacy insertion, schema migration,
lifecycle migration, explicit policy adoption and, where needed, fresh
re-embedding/validation. Repeating this setup is costly, but replacing it with
synthetic rows or globally sharing mutable stores is outside this CI-only fix.
The display-cap matrix is a possible later broad-matrix scheduling candidate;
it is not moved now because it also checks ordering and source immutability.
The user's permission to prune low-value tests is retained, but partitioning
offers a larger immediate improvement without first weakening these checks.

## Coverage and failure behavior

- Build using the baseline `cargo test --all-features --no-run` target selection.
  Cargo JSON artifacts must match the root package's eligible library, binary
  and integration-test metadata. All baseline examples must appear as compiled
  artifacts; examples remain compile-only, with no new example test execution.
- Enumerate actual emitted harnesses and their ignored tests. Sort library
  names and assign index modulo4. Execute exact names in one ordinary Cargo
  invocation per selected harness; keep default libtest threads/stack and Cargo
  environment. No nextest, per-test subprocess wrapper or new testing service.
- Assign each non-library harness, including empty binary harnesses and all
  CLI suites, to one shard. Doctests remain a normal Cargo doc run on shard0.
  No broad `--tests` selector that accidentally reruns the library.
- Collect full inventories, ignored states, actual result names/statuses,
  compiler/target, source SHA and executable SHA256. A binary changing between
  enumeration and execution fails. Only assigned executables are hashed, avoiding repeated reads of other shards' binaries. Each shard writes its report only after all
  assigned work and source-immutability checks succeed.
- The stable required checks run with `always()`. They require every shard job
  to succeed and exactly four reports per OS, matching source/compiler and
  inventories, exact disjoint assignment/result union, complete CLI/binary/doc
  evidence and example compilation. Failure, cancellation, missing artifacts,
  duplicates, foreign-source/OS results and inventory drift fail collection.
  Since the matrix is one dependency, failure on either OS currently makes both
  collectors fail; neither can silently turn green after a partial run.

Actual logged library inventories partition into443/442/442/442 on Ubuntu and
442/442/442/442 on macOS. Platform-specific tests are enumerated independently.
Existing legacy_batch tests are included; future merged tests are discovered
automatically rather than maintained in a stale name allowlist. Intentional
ignored fixture generators stay ignored and must be reported as such.

## Expected benefit and cost

The ideal four-way library execution is roughly10–12 minutes plus2–4.5 minutes
of build/setup per OS. A **16–22 minute critical-path estimate** allows for
uneven cases and overhead; it is not a measured speedup or a guarantee about
runner queue time. Both OSes require four available runners. Repeating the
observed compilation adds roughly6.25 Ubuntu runner-minutes and13.2 macOS
runner-minutes over one build, plus setup/collection overhead; actual cache and
runner contention may change this. The existing cache action is reused.

Hosted CI on the exact published head must establish real duration and coverage
before any speed claim. No local recalld full rebuild, benchmark, paid service,
compiler switch, stack enlargement, retry or cancellation was used here.

## Bounded validation and retained failures

Final runner validation: six checks passed in0.871s in an isolated offline
`recalld-ci:1.94` container, Rust1.94.1,2CPU/1GiB/jobs2. This is a tiny
dependency-free runner fixture, **not the recalld suite or proof on CI1.99.1**.
It exercises real Cargo artifact enumeration, all four partitions, exact
overlapping names (`a`/`ab`), should-panic and ignored tests, a CLI that uses its
Cargo-provided executable path, an empty binary harness, ignored docs, and an
example whose main/test panic if wrongly executed. Collector negatives cover
missing/duplicate/foreign/failed reports, inventory/compiler drift, extra or
missing results, unexpected ignored states, absent binaries and docs.

The first smoke failed closed because libtest's **terse** listing omits the
count footer. Enumeration was corrected to ordinary **pretty** listing; the
strict footer/count assertion remains. A second container failed with EACCES
because its tmpfs was noexec; the same source passed after mounting only the
private synthetic temporary directory executable. The final smoke also includes
the before/after executable hash check. These failures were retained, not
reported as green or used to relax tests.

Final source hashes:

| File | SHA256 |
| --- | --- |
| `.github/workflows/ci.yml` | `339b73e670aad427ebadff3ee369ca7e383cd4cca4f76f3a70b7906019dfaf00` |
| `.github/ci/test_shards.py` | `164e8bafc7d895c2bb833c6b11737a24d583c292572724f66dc12278bd68bf5c` |
| `.github/ci/test_partition.py` | `cb74ab6ddff4df9eebc43022ec5d15edf790854f15da56a4437dd21338ec3456` |
| Final smoke log | `8665da6b44ab9db47b6ec3696d758a6691bfb37a0d97828bf2e6b82292a2e427` |

Compact baseline timings/inventory digests accompany this report in
`docs/evidence/ci-test-scheduling/baseline-summary.json`. Local collection-only
checks also passed; they skip the Cargo smoke when Cargo is absent. `git diff
--check` passes. Hosted workflow validation remains required; no full local Rust
test/lint claim is made for this CI-only patch.

Cleanup: all own smoke containers used `--rm` and completed. The integrated
unit smoke deletes its synthetic crate/target on exit. About28MiB of initial
smoke data/logs and baseline inputs remain protected at
`/tmp/recalld-implementation/ci-test-scheduling/.ci-analysis` until publication
and central cleanup handoff; no existing shared cache was mounted or copied.
No bytes from other owners or personal stores were removed.

References: [Cargo test target selection](https://doc.rust-lang.org/cargo/commands/cargo-test.html),
[libtest filtering](https://doc.rust-lang.org/rustc/tests/index.html),
[Actions dependencies and always](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idneeds).
