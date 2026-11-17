# Canonical namespace and tag continuation validation

Runtime: `fa71caf9f6255b79017c7b129190eff77ddeab35`.
Sole implementation parent: PR100 `ff577809f8f4b9575ad465ae677ab4bab2b13195`.
PR93/catalog creation and PR103/policy publication are not dependencies.

## Evidence boundary

The implementation reads actual canonical generation, namespace configuration,
records and vocabulary in one redb transaction under the existing owning engine
and accepted work. Native tests use real legacy stores, explicit schema/lifecycle
migration, native writes and strict reopen, including actual UTF-8 labels and
retained-only provenance. They do not insert invented v2 rows into open legacy
stores. The native fixture owner is `NativeLifecycleSession`; this does not
prove ordinary native Recalld/daemon startup admission.

MCP and REST proof uses the real inner contract2 dispatcher/handler and storage
adapters. Public contract2 remains rejected before work. The frame regression
uses the real MCP text+structured/JSON-RPC serializers and daemon framed
writer/reader; this is serialization proof, not a new dedicated remote catalog
RPC implementation. Existing v1 successful shapes and defaults remain unchanged.

## Results

- Seven focused catalog tests passed in 4.54s.
- Normal parallel `cargo test --offline --all-features`: **988 library tests**
  passed in 437.95s, plus **2 CLI tests** (2.48s and 1.35s). Four existing
  doctests ignored.
- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --offline --all-features --all-targets`: exit 0 with 613 library
  and 678 library-test warnings (603 duplicates), plus one binary warning.
  This is not a warning-free result.
- All **549 archived input files** passed pre/post hash checks and independent
  host verification. All **6 executed test binaries** were hashed. The focused
  and full library executable hashes match. All three owned containers exited
  without OOM; the initial compile-only failure was exit101, successful runs0.

Logs: `/tmp/recalld-catalog-page-validation/{initial,focused,final}`.
Immutable inputs: `/tmp/recalld-catalog-page-source-{b7d87d8,fa71caf}`.
`manifest.json` records the artifact hashes. Durable copies accompany the
experiment notebook entry; no cargo/target cache or build binary is committed.

All validation uses Rust1.94.1, `recalld-ci:1.94`, Docker network none, 2 CPUs,
4 GiB and CARGO_BUILD_JOBS=2. The only mounts are immutable source read-only,
private cargo/target and logs. No home, credentials, socket or production data.
Image: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Source hashes are checked before and after execution inside the container and
independently from the host. Executed test binaries are hashed. The initial
failed source was replaced only after a package-only clean in this agent's own
target. Focused and full successful runs share the same immutable source root
and library executable hash; no BEFORE/AFTER target reuse is accepted as proof.

## Focused coverage

- Strict parsers reject unknown/repeated fields, invalid types/nulls/ranges,
  unsupported cursor ordering and changed scope shape.
- A real 203-record legacy vocabulary spans all four buckets and three pages,
  with count-descending order and stable exact-label ties. Changing page size
  succeeds; wrong namespace, forged anchor and intervening mutation reject.
  A different namespace's records do not enter the selected vocabulary.
- Namespace entries use immutable ID order and exact live counts, include empty
  namespaces and resume after reopen. Closed admission rejects work; no ticket
  remains after known successful reads.
- Real native labels `José`, `John Smith` and `Alice's` survive page/reopen,
  deduplicate per record, preserve NativeV2 provenance and never leak old raw
  structured tags when canonical labels are empty. Actual migrated retained
  labels remain LegacyRetainedOnly, including UnknownLegacy profile stores.
- Metadata catalog requests do not consult providers or clocks. Cached actual
  provider configuration reports exact profile compatibility; a different model
  with the same dimension reports mismatch. MCP and REST typed envelopes agree.
- A real wide legacy vocabulary exceeds the whole-response byte budget and
  fails without source mutation. Smaller retry succeeds; complete MCP text plus
  structured result and actual daemon frame remain below 8 MiB. Native REST
  serialization is exercised separately by transport parity.

Canonical bytes are compared around reads and rejected requests. The original
fixtures remain intact; tests do not run against personal stores or a daemon.

## Retained failure

Initial `b7d87d895c0de6191ddfefad19f555e3b570548e` failed compilation because the
new frame test passed three arguments to the existing two-argument
`DaemonResponse::success`. No tests ran. `fa71caf` removes the extra argument and
unused test imports; no runtime workaround was introduced. Immutable source and
failed logs are retained and hashed separately. Formatter output is retained
without treating it as behavior evidence.

## Scope and remaining work

This closes the bounded namespace/tag catalog consumer slice. It does not
complete A4 or activate public contract2. Each page scans canonical rows and
sorts vocabulary; declared row, aggregation and serialized-size limits do not
bound examined rows, peak RSS or latency. No A5 performance acceptance is claimed.

Dedicated RPC, public outputSchema/discovery and actual ordinary native startup
remain required. Native mutation/partial-batch wire, coherent A2 semantics2
selection and accepted native exposure writes remain separate prerequisites.
Pascal owns accepted exposure after feedback, Curie/Hume its query consumers,
and A4 the wire contract. API1 on native-schema stores needs explicit tested
owner routing while preserving omitted-version/default behavior; disk schema2
does not imply API2. Existing contract1 observation is not a native
true-mode fallback. Coherent payload and read-only success alone do not prove
semantics2 or true-mode readiness. See the implementation activation inventory.
