# Prepared point get: actual main164 composition

All driver stages passed on frozen `2925a7289055b27ffda2583a81b656442f347632`. The normal merge has parents
`18d32ffce275fbb691cd1a696c90214d8ad48282` (sealed O1) and
`6f064789473eea310be28e004de38fd52d2aa3d5` (actual main164).
Curie, Huygens and Feynman source-reviewed the exact merged runtime before execution.
Three conflicts retained independent get/page/query/exact-pair capabilities,
private module exports and server dispatch/ping. No parent source was overwritten.

## Exact results

- All-feature/all-target test command: **1801 library +10 CLI +15 example tests passed**, zero failures. Seven historical library probes remain ignored; exact names and root harness results are in [FULL-SUMMARY.log](FULL-SUMMARY.log).
- Prepared-path focus: **78 passed, one historical ignore**. These overlap the full suite and are not added to its count.
- Doctests:0passed,4historical ignores. Cargo formatter, explicit included-test rustfmt checks and all-feature/all-target Clippy exited0. Clippy emitted warnings.
- Actual public CLI: **9 episodes**,3 fresh replays each over stdio MCP, HTTP MCP and REST; **54 true-access cases**. False public/durable state, scope/defaults, selected malformed requests, returned-only exposure and restart checks passed. Fixture version3; controlled embedding stub, no external model calls. Synthetic fixture metrics are not a quality improvement claim.
- Formal actual emitted query cases **63**, page cases **32**, with schema runner mutation negatives, using pinned offline jsonschema4.25.1 wheels. No fixture edits during validation.
- All **539 source input hashes** matched before/after. Fifteen full test executable paths were hashed and reverified after all stages. Four CARGO_BIN_EXE binaries were pinned after the full harness and before the later bench-feature CLI rebuild; this is not a separate before/after CLI-harness hash claim. The actual public CLI was pinned before/after its two public checks.

Library executable SHA256: `2c167cd5dcbb13e921ae8dfbec6c513388855aa806b1414f75af1d1e2924c068`.
Public bench-feature CLI SHA256: `7b133089f072d804970e1a84985222dc5aee6b6b393ad288c6838a6dd28cd09b`.
Library validation duration1677.03s is not a performance result.

## Isolation and source provenance

Image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`; actual Rust1.94.1
(e408947bfd), Cargo1.94.1(29ea6fb6), aarch64-unknown-linux-gnu, RUSTUP_TOOLCHAIN1.94.1.
RUST_MIN_STACK and RUSTFLAGS unset; host process stack rlimit8MiB, not a raised test-thread stack.
Offline Docker,2CPU/**6GiB**/jobs1/tests2, read-only allowlisted source,
private HOME/Cargo/target/output, no home/credential/socket mounts. This authorized6GiB validation
is not4GiB resource conformance and does not clear unrelated x86/compiler failures.
One queue cache was reused with `cargo clean -p recalld` before changing source roots.
No dependency cache clone or source switching during the run.

Cgroup memory peak `6442450944`bytes at the6GiB ceiling;
max counter `10336`,
OOM/OOM-kill0. This records reclaim pressure, not an OOM or a product-performance claim.
Both run containers were retired by the driver. See [driver.py](driver.py),
[commands.json](commands.json), [toolchain.json](toolchain.json),
[build.json](build.json), [EXECUTABLES.json](EXECUTABLES.json),
[steps.json](steps.json) and [summary.json](summary.json).

## Limits and retained histories

The prepared point consumer pins actual canonical payload, clock/policy and observation intent;
typed private transport preserves exact known receipts, unknown current-write attribution and
unverified received evidence under checked bounds. Canonical point reads do not require unrelated
search-view readiness. Private get capability remains independent of query and page capabilities.

Ordinary public API1 MCP/old daemon get routing is **not** replaced by this change.
Separate test-only child1316bced later reproduced raw-versus-projected native strength and an old-peer
lost observing reply causing two exposures; those failed017 artifacts are preserved by Curie and
remain a follow-on consumer gate. HTTP behavior is not declared defective by that finding.
Public2, ordinary native startup, universal outer MCP-ID response bounds and full A6 determinism
remain unactivated/unproved. Public9 here is the declared legacy diagnostic, not native public conformance.

Original O1 owner full1764/10/15 belongs to0e3338e3, not this composition. Earlier owner
numeric carrier, stale-wrapper and terminal-race expectation failures remain recorded in
`docs/evidence/prepared-get/rpc-final/REPORT.md`; they are not retroactively relabeled. This exact
2925 integration run passed without a source edit/retry. D1/PR190 remains an independent schema
consumer; no speculative D1/N1 parent was imported. Exact published-head and main CI remain merge gates.

Cleanup: containers removed;0raw/cache bytes deleted by queue at this seal. Passing run
`.queue/validation-o1-2925a728` remains protected until publication/results and exact central release.
The one queue cache `.queue/validation-pr22-a20ab593/{cargo,target}` remains active future input.
Failed owner runs and code/Git worktrees remain protected. No HOME raw archive was created.
