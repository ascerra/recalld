# Live API1 record schemas: frozen affected validation

2026-11-17, Huygens. Runtime/schema/test source:
`cde04db8f8806fb59c7298dc2fb23592b67276f1`. The final tail changes only docs,
compact evidence and the notebook reference. No additional full run is claimed.

## Outcome and actual dependencies

Modern tools/list now includes output schemas for get_memory, store_memory,
store_memories and forget_memories: 11 of 14 tools. Actual emitter behavior,
API1 defaults, serializers, mutation attribution, receipt bytes and public2 gates
are unchanged. Generic retained-error receipts cover the complete 33-variant
Rust codec; shape is not canonical validity or commitment proof. Explicit
unverified evidence, raw receipt strings and declared compatibility Value fields
keep their existing boundaries.

The exact composition is normal merge `f77acf0a1413b1b5208d408a960436c72113443f`
of PR195 `c4a62065aac6192f8b3b30ca6c3bd63fd48c7416` and PR196
`10d58c70bc18d936a0e2369e02c8440c4d2dc288`. These are actual point-get
producer and shared lifecycle schema dependencies. No catalog/N1 or unrelated
writer composition is added. Parent branches remain frozen/queue-owned.

Halley released this schema/test/docs scope. The coordinator approved bounded
validation under the user's standing authorization; no claim that the user
personally reviewed each test selection. Feynman's bounded source ACK compared
all 33 outcomes and their payload fields against Rust definitions. It was on
pre-format receipt file SHA256
`58a5207171e89231973eded47134002913b8633cb31673c40f09788fd74f6c97`
and record file SHA256
`53a20e71ab4a719533455f7b8988ac8b1098572357af517629a46b58da730170`,
recorded in his review `f8a0a4cd`; it is not independent executed test proof.
The compiled source hashes are in source-sha256.json.

## Results

| Check | Exact result |
| --- | --- |
| Offline all-feature lib compilation | PASS; 65 warnings |
| Affected Rust regression filters | 19 passed, 0 failed, 0 ignored across 18 filters |
| Draft 2020-12 formal schema checks | 50 captured output cases across all four tools, plus meaningful malformed-shape negatives; PASS |
| cargo fmt --all --check | exit0 |
| cargo clippy --offline --all-features --all-targets | exit0 with warnings |
| Immutable source pre/post verification | all516 files match |
| Executed library test binary pre/post verification | one identical SHA256, below |

Executable `/target/debug/deps/recalld-8b31964f64b2cfc4`, 653369648 bytes:
`bde0824806a446eaa26c2364ac02e2cba7b0699ae7a380d4a62ee0bae1346f12`.
It was checked again after Clippy. Before building this different source root,
`cargo clean -p recalld --offline` cleaned only our package in the owned target;
dependencies were reused. There was no stale before/after executable reuse.

The 50 cases count separately captured text and structured outputs and shared
projection cases, not 50 independent writer histories. Each fixture records its
provenance. No new failing test occurred in this candidate. Parent195's reported
full-suite failures and parent196's earlier formal numeric RED remain attributed
to their own reports; this affected pass does not erase or rerun them.

## Meaningful exercised boundaries

- Actual native and legacy get success, Ghost payload, capable Remote parity,
  known publication failure, unknown operation and original raw receipt evidence.
  Existing history, receipt identity, pending work and no-fallback assertions remain.
- Actual native Unicode single store and legacy compatibility, plus known full
  receipt publication failure. The shared REST failure projection is labeled as
  such, not represented as an independent MCP invocation.
- Actual native/legacy insertion batches, mixed invalid items, final-barrier
  failure and replay repair, malformed Remote evidence and reservation binding.
  Complete parent/bookkeeping/child receipts and existing commitments remain.
- A genuine separately committed BatchReserved receipt wrapped in an authored
  single-error peer reply. Actual decode/emitter retains its full shape with
  unknown current-write attribution; malformed wrapper moves it to explicitly
  unverified evidence. No extra provider call, clock tick or source byte change.
- Actual durable legacy mixed-forget results and the existing stub short-array
  failure. The latter is explicitly a compatibility projection, not a writer.
- Actual tools/list modern/legacy registry expectations. Formal tests validate
  emitted text and structured Values separately without float normalization,
  tolerance, receipt/hash rewriting or new global serde behavior.

The new shapes are defined in tool_record_schema.rs/tool_receipt_schema.rs;
registry changes are in tool_schema.rs. Existing regressions gained an optional
artifact sink in test_support.rs. Two focused fixtures live in
native_records/live_record_schema_tests.rs; tests/live_record_schema_conformance.py
is the formal checker. Production mutation/query implementations were not changed.

## Reproduction and resource facts

Source is an immutable read-only archive of the exact commit, including all
needed fixtures, with the file manifest retained here. Image recalld-ci:1.94:
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Rust `RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_BUILD_JOBS=2`, Docker `--network none
--cpus 2 --memory 4g`. No home, credentials, daemon socket or personal data mounts.
One private Cargo cache and target were reused; no cloned build cache.

Build: `cargo test --offline --all-features --lib --no-run`.
Focused execution: focused.py invokes the exact built binary for the 18 recorded
filters with `--test-threads=2`; `RECALLD_LIVE_RECORD_SCHEMA_FIXTURES=/evidence/fixtures`
enables captures. Formal: Python3.11 with the existing offline jsonschema cache,
`python3 tests/live_record_schema_conformance.py /evidence/fixtures`.

Both build and checks peaked at 4294967296 bytes. Build memory.events max37997;
checks max2526. Both had oom0/oom_kill0/oom_group_kill0. These show reclaim
pressure, not an OOM failure or a performance result. Lint warnings were retained;
no unrelated warning cleanup was included.

## Handoff, cleanup and remaining scope

Exact final-head CI and integration remain Halley's gates. This schema-only child
uses affected tests/formal/lint rather than another backend full run, as directed.
No further local tests are needed for the evidence/docs-only tail. Catalog
create/delete schemas await their actual final producers; list_namespaces remains
an array. Activation, remaining conformance and measurement gates are not claimed
complete by these schemas.

Active raw root: `/tmp/recalld-huygens-active-validation/live-record-001-cde04db8`.
Source-only formatter: `/tmp/recalld-huygens-active-validation/live-record-format-001`.
After publication plus recorded results, release both centrally; do not delete
other owners' data or protected prior failures. Retain one private Cargo/target
for assigned future work. No raw HOME archive is required. The compact notebook
append and verification are recorded in NOTEBOOK-COPY.json. Cleanup recovery is
not yet measured; no recovered-byte claim.
