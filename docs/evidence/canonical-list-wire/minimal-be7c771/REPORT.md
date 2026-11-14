# Canonical metadata pagination: minimal composition validation

Frozen runtime: `be7c77103208d7f9ed9ee8d5c627beb3d93e7d5c`.
Actual parent join: `4fd324d9774d730a7707d3d0b52380459dc4df67`, joining PR88 `3961f6653d6c3076bdf9b5b8c27fa6b01a8f0b2e` and the actual PR82 startup recovery correction `fb804a0a72ccf7dde0a6ee7eaea2e91a6b7ba2db`. PR93/87 catalog creation is not consumed. The two cherry-pick conflicts were catalog dispatcher/test-reexport context; those unrelated catalog additions were excluded.

## Shipping evidence

- Six focused canonical-list regressions passed in 3.50s.
- Normal parallel `cargo test --offline --all-features`: **981 library tests passed in 381.30s**, plus **2 CLI tests** (2.38s and 1.29s). Four existing doctests ignored.
- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --offline --all-features --all-targets`: exit 0, with 612 library and 678 library-test warnings (603 duplicates), 14.30s. This is not a warning-free claim.
- Rust1.94.1 in `recalld-ci:1.94`, image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`; Docker network none, 2 CPUs, 4 GiB, CARGO_BUILD_JOBS=2. No home, credentials, daemon socket or production-data mounts.
- A separate immutable git archive was mounted read-only at `/work`, outside the writable log directory. All **540 archived files** were hashed before execution and verified in the container before and after checks, then independently verified from the host. All **6 executed test binaries** were hashed.
- Before switching source roots, the previous run's binaries were sealed, then `cargo clean --offline -p recalld` ran only in this agent's private target. Dependencies were retained. No stale BEFORE/AFTER executable was accepted as proof.

Artifact root: `/tmp/recalld-canonical-list-validation/minimal`; immutable input root: `/tmp/recalld-canonical-list-source-be7c771`. `manifest.json` records exact artifact hashes. External durable copies and their copy manifest accompany the experiment notebook entry. No target/cargo cache or build binaries are committed.

## What the tests establish

The native fixture uses real legacy storage, explicit schema/lifecycle migration, native inserts and tombstones, and strict native reopen between pages. It does not author schema-v2 rows into an open legacy store. Unicode/multiword labels (`José`, `John Smith`, `Alice's`) round-trip unchanged; `NativeV2` and `LegacyRetainedOnly` remain distinct. Canonical empty labels do not fall back to compatibility tags. A separate actual migration without attestation proves `UnknownLegacy` metadata is readable without a provider/profile/vector requirement.

The actual inner MCP dispatcher and REST handler agree on envelopes; MCP text and structured JSON agree. A wrong frozen MCP default fails rather than scanning all namespaces; explicit canonical/case-alias scope succeeds. Actual public contract2 requests remain rejected before work. Native and legacy metadata reads preserve canonical files or encoded records/generation, with no provider/clock work or retained work ticket. Reopen continuations, mutation-stale cursors, live ghosts/tombstones, exact totals, empty exhaustion, filter/namespace/UUID-anchor validation, and changing `limit` on a cursor are exercised.

A real legacy store with wide summaries/tags exceeds the declared page-byte budget and returns a whole error. Smaller retry succeeds without dropping matches. The actual MCP text+structured JSON-RPC serialization and daemon framed writer/reader stay within the frame limit. Actual REST serialization is exercised by native transport parity. Closed admission rejects new list work and normal admission resumes after release.

These are private contract2 metadata consumers, not ordinary native-serving activation or whole-payload ReadView proof. There are no vector copies, embedding calls, derived readiness stamps, exposure receipts, fabricated generations, hidden candidate truncation or server-side cursor jobs in this implementation.

## Earlier attempts, kept separate

`earlier-runs.json` records hashes for the retained exploratory and diagnostic logs:

1. Initial compilation failed on three missing error conversions; no tests executed.
2. Two formatter invocations immediately after host edits observed incomplete-file parses, while subsequent host reads showed complete files. Logs are preserved; this observation does not establish a production defect or any behavior-test result. Mounted-input hash verification and frozen read-only archives were used for final validation.
3. Exploratory focused runs passed 3 tests, then 5 tests. These are not substituted for final immutable-composition evidence.
4. Runtime `2ea143bca71063f67024d7fcd1381f5bb1479900` on the unnecessary PR93 parent passed 6 focused tests (3.81s), **993 library tests** (450.99s), **2 CLI tests** (2.62s,1.54s), fmt and clippy. Its 649 archived input hashes and six binary hashes were sealed. It is diagnostic only; it was not published as the list branch.
5. The minimal branch retains identical list runtime implementation to `2ea143b`; only the page-limit continuation assertion and parent documentation differ. The minimal composition was rebuilt and validated separately rather than borrowing the diagnostic pass.

The publication-lock style observation did not justify a speculative fix: the normal MutationGuard panic path sets sticky unavailability before releasing publication, and reads check it. No raw-lock-only production claim or lock weakening was introduced.

Catalog PR93's earlier float fixture history remains unchanged: typed MCP and REST already agreed, the incorrect `to_value` expectation was corrected to canonical serialized bytes, and the unnecessary normalization attempt was reverted. This list change does not claim a production float-parity fix.

## Remaining scope

Public contract2 activation, dedicated RPC consumers, bounded tag/namespace catalogs and remaining native mutation/payload schemas are not complete. The operation matrix in `docs/implementation/canonical-list-wire.md` tracks those gaps. Metadata paging scans O(N) rows per page with bounded retained allocation; no A5 performance acceptance claim is made.

The next separate child consumes actual PR93+94+96, with no list ancestry: native numeric policy wire and catalog create/replay completion under the same accepted parent. The actual coordinator must publish current generation after changed policy, keyed no-op and replay; completion failure must retain the original receipt. No generic stamp advance or premature whole-native readiness claim is authorized.
