# Public native API1 batch insertion validation

Frozen corrected runtime: `28ada7bca1c3ae803982c9bc0ae812653d0c23fc`.
Earlier frozen source `4ee0d093b7ad49ec15adb98287eb96d44e191e5d` is preserved as007.
Direct parent: published PR158 `9c90f8201d476918077d3d08920fdb989269c205`.
PR148 and PR153 are already ancestors of158; this child adds no separate join,
codec, query, catalog, startup or public2 activation dependency.

Final008 validation is complete:1349 library tests and2 CLI tests passed, with
zero failures,3 library tests and4 doctests ignored. The existing private2 parity
regression,17 API1 focused tests, historical budget case, formatting and clippy
also passed. Clippy warnings are retained. Publication and exact-head CI remain
pending review; this documentation seal changes no runtime source.

## Behavior

MCP store_memories and REST POST /memories/batch select the actual owned physical
format. Native execution uses the existing accepted native3 compound owner with
Some(original1), atomic linked children, full receipts and final current-generation
readiness. Legacy success shapes and no-profile supplied-vector lane remain.
Native supplied vectors require an explicit matching profile ID; generated vectors
use the actual corrected document recipe. The single-MCP ignored-vector exception
does not apply to batches, which already supported supplied vectors.

REST keeps its typed validation/defaults and createdAt/initialStability controls.
The implementation computes unchanged old item/caller hashes first, then changes
only the ephemeral execution plan. A retained timestamp rejection replays unchanged;
changed timestamp under the same key conflicts. It does not rewrite historical
None/original2/private3 request bytes or receipt proofs.

api1StoreBatchV1/store_batch_api1_v1 uses the existing single-connection handshake,
independent of other capabilities. New alias requires the full frame before work;
old raw insertion on native refuses before reservation/provider. Old peers receive
only the exact representable legacy request, selected before sending. Lost or
malformed replies never cause a second mutation. Mixed or committed-unready batches
retain each indexed outcome and full receipt, HTTP207/MCPisError; final barrier
failure after completed children is repaired and replayed without new generation,
provider work or sampled time.

Malformed remote aggregates/display do not become successful batch projections.
receivedEvidence preserves decoded receipts and identities without aggregate counts,
mutable display or readiness, with commitment unknown. Normal remote outcomes
check the reservation against original caller hash/store/resume identity and native
child receipt proof/scope. Wrong receipt version/store/scope/hash or changed request
never upgrades received evidence. A validated reservation-only failure explicitly
uses commitEvidenceScope=reservation; this proves no child or atomic-batch success.

## Bound and fixture classification

[Implementation behavior and bound derivation](../../implementation/native-api1-batch.md)
describe the enforced byte/count/closed-field limits. The derived compatibility
allowance is11248bytes/item. The serializer-only maximal escaped display fixture
measures5338bytes; that observation alone is not a proof over all possible strings.
Actual field/count limits and counting serialization are the admission argument.
Full receipts are immutable and occur once, alongside bounded compatibility copies.

Final007 measured MCP36001, REST16572 and daemon11949bytes for the entire packets.
Corrected008 measured MCP35997, REST16570 and daemon11947bytes for its actual
receipt fixture; these are measured examples, separate from the enforced bound.
The real canonical receipt plus expanded advisory fixture counts the entire REST,
MCP text+structured JSON-RPC and daemon packets. It is a valid committed receipt
with deliberately authored display strings, not a native persisted maximal store.
The older maximal typed serializer stress is explicitly synthetic. The prior106
oversized original2 fixture exercises API1 projection only, not original1 historical
acceptance. Fresh oversized input refuses before reservation/provider/clock; old
oversized outcomes retain the small explicitly scoped reservation evidence and no
pretend empty completed batch or truncated item receipts.

The tracked [genuine fixture](../../../tests/fixtures/native-api1-batch-before158/README.md)
is20955compressed bytes, six exact closed-store files, extracted total7389377bytes.
It was produced by actual158 production plus a test-only private original1 driver.
It proves historical private Some1 createdAt rejection, not prior public acceptance.
The two separate public BEFORE failures establish the old public route behavior.
Default CI extracts this fixture without environment setup; archive SHA, exact
names/count, unique relative paths, no symlinks/directories, compressed/extracted
limits and every extracted file SHA are checked. Its original source/executable
hashes and extraction hashes live in PROVENANCE.json. This is necessary future
input and must survive raw-run cleanup.

## Run attribution

| Run | Result | Meaning |
| --- | --- | --- |
| before001 | 1 private generator passed; 2 public failures | Actual158 production. Private timestamp rejection exported; MCP and REST public native Unicode paths fail before this consumer. |
| draft002 | Compile failure | Test-only accepted runner needed its new optional surface argument; no runtime evidence. |
| draft003 | 9 focused passed, 1 generator ignored | Initial routes, private replay, final barrier, mixed outcomes and transport checks. |
| final004 | 15 focused passed, 1 ignored; budget6 passed/1 failed | Historical API1 projection expected replayed=false incorrectly; retained reservation was replayed=true. Other receipt fields remain exact. No full/lint run followed this failure. |
| final005 | Compile failure | Test SHA digest helper used unsupported LowerHex. No tests/full/lint executed. |
| boundary-before006 | 1 genuine failure | Production300f1786 plus test-only helper/probe: impossible succeeded99 still upgraded malformed reply to committed. Exact source and executable preserved. |
| final007 | 17 focused passed, 1 ignored; budget1 passed; fmt/clippy0; full1348 passed/1 failed/3 ignored | Frozen4ee0d093. Actual routing regression intercepted explicit private writer2. Default tracked genuine fixture passed without override. |
| final008 | Parity1, API1 focus17, budget1,1349 library+2 CLI passed; fmt/clippy0 | Frozen28ada7bc. No failures;3 library/4 doctests ignored. One-condition dispatch correction; unchanged existing private2 parity assertion. |

The malformed aggregate correction is a real runtime fix on this child. The replay
bit and SHA helper corrections are fixture changes. The007 parity failure is a
second real runtime regression: tools.handle_durable_batch sent every insertion
to the API1-only Request validator, including the existing explicit private
batchMutationVersion2. At batch_tests.rs378, deserializing its absent batch field
failed with "invalid type: null, expected struct BatchOutcome". The correction
selects the new path only for Insert && mutation_version1; explicit2 keeps the
original mutate_batch route. The parity assertion is unchanged; there is no
error-triggered fallback or new public2 activation. The source-manifest
[007-to008 delta](delta-007-008.json) contains only src/mcp/tools.rs; tests are
unchanged. Run007 stopped after the library failure, so no CLI pass is attributed
to it. These findings are not conflated.

## Isolation and evidence

All execution uses recalld-ci:1.94 (image sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef),
Rust1.94.1, network none,2CPU/4GiB, CARGO_BUILD_JOBS2, immutable read-only source,
one private Cargo cache and one private target. No home/credential/socket/personal
store or daemon mounts, no paid providers. Every source switch captures executed
binaries first and cleans only this package in the owned target. Separate pre/post
source manifests and executable SHA/size evidence prevent stale-executable claims.

Small logs, exits, source manifests and executable hashes are copied under runs/
with per-run COPY-VERIFIED manifests. Raw databases/source copies/cache are not
archived to HOME. After publication and recorded results, central cleanup may
release completed raw roots. Tracked fixture/code/reports and the continuing one
cache/target remain protected. Publication and exact-head CI are still pending.

No ordinary native startup or public2 activation, historical vector certification,
full A4 acceptance or performance-gate claim is made by this slice.

## Final immutable proof and limits

Final008 ran the exact inherited parity test (1 passed,3.56s), api1_batch_ filter
(17 passed,1 historical generator ignored,13.11s), historical_oversize filter
(1 passed,62.99s), cargo fmt --all --check and cargo clippy --offline --all-features
--all-targets (both exit0), then cargo test --offline --all-features. The full
library result is1349 passed,0 failed,3 ignored in871.88s; both CLI tests passed.
Clippy reports1058 library and1132 library-test warnings (1018 duplicates), plus
one binary warning. This is not a warning-free claim.

The three ignored library tests are the inherited genuine legacy-lifecycle
exporter, the native-single dense receipt probe requiring separate BEFORE123
artifacts, and this child's BEFORE158 private-history exporter. Default CI does
execute the new archived BEFORE158 replay regression; no environment variable or
external raw run directory is required. The four doctests are inherited examples.

All362 frozen source files match their pre/post SHA256 manifest and the runtime
worktree. Nine executed Cargo-produced binaries were hashed before any further
package cleaning. The final run's24 copied evidence files are SHA256-verified in
[runs/run-api1-batch-final-008/COPY-VERIFIED.json](runs/run-api1-batch-final-008/COPY-VERIFIED.json)
(the copy manifest is the25th file). These retain full logs/exits, container
configuration/image, rustc version, cgroup observations, exact source and binary
hashes. The original158 source/executable and extracted-file hashes remain in the
tracked fixture PROVENANCE.json, separately from corrected-source evidence.

Run007 reached memory.peak4294967296bytes and memory.events.max26964; final008
reached the same peak with max34025. Both reported oom0 and oom_kill0. These are
reclaim-pressure counters, not OOM failures. Resources were never increased and no
Cargo/target clone was made. All owned validation containers exited normally.

The original malformed aggregate RED,007 routing failure, compile-only attempts
and mistaken replay-bit assertion are retained in the report/run evidence. No
runtime edit or additional local test run accompanies the final documentation
seal. Exact published-head CI remains a separate required integration check.
