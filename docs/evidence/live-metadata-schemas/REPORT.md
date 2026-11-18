# Live v1 metadata output schemas and session guidance

Local validation complete; exact-head CI and final integration remain separate gates.

Runtime: `16b2aa17d0f94e7963e8021db9e3df62eaf6b754`.
Final test source: `13ff718d5fafcf7637a298e8b30a671ea63cfb46`.
Actual sole parent: main `6f064789473eea310be28e004de38fd52d2aa3d5`.
Owner Huygens; Halley released the implementation hunks and reviewed the narrow
runtime and test-only corrections. No O1 get or N1 catalog dependency.

## Results

- Full all-feature command: **1763 library and10 CLI tests passed**, zero failures.
  Six historical library probes remain ignored; four doctests remain ignored.
  Exact names are retained in [full-summary.log](full-summary.log). This command
  is not a claim of all-target example-test execution.
- Focused3 metadata +1 session test passed;13 emitted outputs passed the formal
  schema validator and mutation negatives. Final full-run fixtures were checked
  again after the full suite. These repeated tests/cases are not added together.
- Formatter and all-feature/all-target Clippy exited0 with warnings.
- All490 build inputs (17,791,063bytes) matched after execution. All14 actual
  test/CLI executable paths were hashed; the4 CARGO_BIN_EXE binaries matched
  before/after their harnesses. Focused/full library SHA256 is identical:
  `23e1c3469c0cef53ee2d6619cb0afad822a1745c5b876c606def2dca599ffaa1`.
- Library duration2142.07s is a capped-resource validation duration, not a
  product performance measurement. Combined lint/full cgroup peak4,294,967,296B,
  max9214, OOM/kill0: reclaim pressure, not an OOM.

See [RESULTS.json](RESULTS.json), [source](SOURCE.json),
[after-run verification](SOURCE-AFTER.json), [executables](EXECUTABLES.json),
[toolchain](TOOLCHAIN.json), and [actual container mounts](CONTAINER.json).
The image is `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Rust1.94.1 (`e408947bfd`), Cargo1.94.1 (`29ea6fb6`),
`aarch64-unknown-linux-gnu`, `RUSTUP_TOOLCHAIN=1.94.1`.

## Behavior

Modern tools/list advertises outputSchema for the existing namespace_stats and
list_tags object results. The schemas describe actual u64 counts, required fields,
nullable global scope/counts and bounded per-bucket result arrays, without
inventing bounds on label strings or total vocabulary. Legacy tools/list omits
outputSchema and legacy calls retain text-only payloads. Tool result handlers,
array-valued list_namespaces, public version selectors and v1 defaults are unchanged.

The shared MCP session/REST capabilities document adds truthful by-ID scope,
recordAccess defaults, API/disk-version independence and conditional API1 store
profile guidance. Legacy supplied vectors without a profile remain unverified;
explicit profiles are rejected on that lane. Native REST supplied vectors need
the namespace's exact Bound profile ID. This is conditional guidance, not a claim
that ordinary native startup is enabled. MCP v1 single embedding remains ignored.
The get tool description now matches the existing true recordAccess default.
Instructions direct clients to recalld://session. Public catalog caching stays
static; resolved context remains in the private/no-store context document.

## Evidence and preserved failures

Run001 (182ff803) was a source snapshot with Docker formatter exit0, but no test
or build execution. A source review caught
the inherited false get-description claim before the first validation run.

Run002 (16b2aa17) passed the actual storage-backed metadata scope/count/truncation
control, then failed two fixture setups: modern HTTP tools/call lacked the
existing required Mcp-Name header (400 rather than expected200); the REST fixture
used a current-thread Tokio runtime where the actual resolver requires
block_in_place. These are fixture failures, not demonstrated production defects.
The source, log and633,086,632-byte executable were preserved and hash verified
before rebuilding. The correction changes only the two tests: use the real header
and multithread runtime, retain strict200/output assertions, and explicitly assert
the missing-header400 rejection. No routing or serializer assertion is relaxed.

Run003 focused:3 metadata tests plus1 existing session-context test passed.
Formal Draft202012 checks accepted13 actual emitted outputs paired with schemas
read from actual tools/list, then rejected required-field/type/count/unknown-field
and array-cap mutations. These are13 output cases, not13 independent Rust tests.
Actual storage-backed fixtures include two defaults/namespaces, live/tombstoned
rows, scoped/global capped tag vocabulary and empty HTTP stats. MCP modern and
legacy payloads, HTTP MCP requests, REST context/stats and explicit2 refusal are
exercised. Record bytes/control state stay unchanged; this is not a claim that
all scratch directory coordination files are immutable. The metadata tests use
actual storage/namespace adapters with unused stub search/provider, not a full
native writer-to-reopen/Remote daemon conformance claim.

## Reproduction and scope

Offline recalld-ci:1.94, Rust1.94.1,2CPU/4GiB/jobs1. Read-only490-input source
snapshot; one private Cargo cache and target. No source switch between corrected
focus, formal checks, lint and full suite. No personal store, home/credentials or
socket mounts. No raw HOME archive. A compact notebook entry links this repository report.

Focused: cargo test --locked --offline --all-features --lib
live_metadata_schemas_ -- --nocapture; then the same command with
session_context_resolves_each_bridge. Formal: python3
tests/live_tool_schema_conformance.py FIXTURE_DIR, using the offline test-only
jsonschema validator and actual emitted fixtures. Full: cargo test --locked
--offline --all-features. Lint: cargo fmt --all --check and cargo clippy --locked
--offline --all-features --all-targets.

This closes only two live output-schema consumers and the stated guidance slice.
Other tools' complete schemas/error unions, remaining prepared operations,
ordinary native startup, public2 activation, full transport conformance and A5
performance gates remain separate. No readiness/generation/profile-attestation
capability is invented. Exact-head CI and final integration remain required.

Cleanup remains protected pending publication/results: raw001NOT-RUN, failed002,
corrected003 and the sole active cache/target. No deletion or recovered-byte claim.

The compact excerpts are labeled; RAW-MANIFEST.json pins complete protected logs.
Run002 source/failed executable remained intact through this seal. No frozen
queue branch was edited and no parent proof is attributed to this source.
