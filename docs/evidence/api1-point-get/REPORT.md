# Ordinary API1 point-get consumer — frozen handoff

Final tested source: `33fd10af99cfe545bd19356e9e5979fceaab7ca3` (runtime behavior from `cc29c1dc`, formatted at `7c4e2f9b`; subsequent changes are tests/imports only), branch `impl/api1-point-get`, actual parent `2925a7289055b27ffda2583a81b656442f347632`. This is the actual ordinary API1 consumer child. The original O1 private prepared-get work at `18d32ffce275fbb691cd1a696c90214d8ad48282` is already frozen with the integrator; historical diagnostic `1d9fe7b` is not the current task. No convenience parents, schema/history changes, HTTP change or public2 activation.

## Behavior and ownership

Actual MCP get routes through the configured serving owner. Native point capture uses the existing QueryEngine with original API1, clock, policy, accepted root, canonical hydration and observation. Legacy shares its prior capture/observation and preserves Ghost payload/event semantics, Tombstone behavior, old aliases, namespace assertions, nil/missing lookup and owning-timezone rendering. Successful public API1 responses remain historical MemoryRecord. Typed capable failures retain exact committed receipts or unknown operation identity; the final Observed admission error is retained before legacy Display conversion.

The independent private `api1GetV1` capability uses `get_api1_v1` with a fixed era/origin/allowance and owning-mode/timezone tag. Valid old-peer capability absence selects compatibility before send. Malformed negotiation or later capability downgrade never falls back. Observing calls, including the old observing alias, are never automatically retransmitted after a lost reply; old read-only retry remains tested.

Before any owned receipt reconstruction, the production borrowed visitor validates the whole compact input, counts every candidate (including invalid and repeated aliases) against 128 and a checked byte allowance, and never reserves from peer size hints. It then validates canonical receipt bytes, deduplicates exact raw aliases preserving order and conflicting evidence, and retains raw strings through the final public frame. Recursive evidence is rejected. Failed decoding cannot promote received evidence into a current receipt, operation ID or readiness. O1 strict decode predicates and generic receipt validation are unchanged; only decoder visibility was widened.

A1 source-ACKed the API1-only `cc29c1dc8a698135675d1cf2e7ccb764a91702ba` mode check after strict decoding: every known Native observation receipt is LifecycleExposureRecorded, every Legacy receipt ExposuresRecorded, for both success and failure. Empty-receipt outcomes keep the existing checks. The formatting-only final candidate preserves that implementation. Huygens independently source-ACKed exact `7c4e2f9b3b8e541939b908a34ff25f8088525eae`: DTO privacy, pre-typed borrowed count/byte admission, raw evidence retention, get-only routing, capability mutex recheck and observing old/new no-retry. These owner reviews are source review, not independent test execution or activation clearance.

Halley formally released the listed get-only runtime hunks to Curie on parent2925 after A1/Huygens source review. D1 ToolInfo/schema/session/get-description, shared Python evaluation runners and HTTP remained outside this ownership. See [reviewed runtime handoff](../../implementation/api1-point-get-runtime-handoff.md).

## Evidence and attribution

| Source/run | Actual result |
| --- | --- |
| Original017 / `1316bced` | Three actual ordinary-consumer failures plus one read-only control. Preserved in [before](before/REPORT.md); not a failure of the already completed O1 visitor. |
| 018–020 diagnostics | Rejected intermediate Value-only receipt carrier; numeric spelling did not preserve canonical raw bytes. Actual failing diagnostics remain recorded and explicitly ignored in the final suite. No report relabels them green. |
| 022 / `738b1cde` | Five raw-carrier checks passed; [report](raw-carrier/REPORT.md). |
| 024 / `90b086b4` | Seven raw-carrier/borrowed visitor/timezone diagnostics passed; [report](raw-visitor/REPORT.md). Original021/023 compilation failures retained. Diagnostic-only runtime remained2925. |
| [025](025-api1-runtime-focus/SHA256.json) / `767d8ae2` | Original three actual failures now pass unchanged (3/0/0), 9.19s. |
| [026](026-api1-consumer-and-peer-before/SHA256.json) / `452e0932` | 9 passed, 2 failed, 0 ignored, 25.13s: actual peer acceptance of Ghost fullText and missing legacy summary. Other actual consumer controls passed. |
| [027](027-api1-corrected-focus/SHA256.json) / `e6430f17` | 18 passed, 0 failed/ignored, 43.04s after producer-shape validation and additional consumer controls. |
| [028](028-api1-mode-before/SHA256.json) / `17ff301c24e628e790a2c9483a9e3b45e3c59076` | 1 passed, 3 failed, 0 ignored, 8.01s. Real legacy success/failure retagged Native and native failure retagged Legacy were accepted. Native success retagged Legacy already rejected and is a passing control, not a fourth reproduced defect. |
| [029](029-api1-mode-corrected/SHA256.json) / `cc29c1dc8a698135675d1cf2e7ccb764a91702ba` | 22 passed, 0 failed/ignored, 52.33s after the exact owning-mode predicate. Includes actual RPC, cancellation, publication, unknown reconciliation and production emitter/count checks. |
| 031 / `7c4e2f9b3b8e541939b908a34ff25f8088525eae` | Formatting-only final candidate. All-targets build passed. The all-features/all-targets command stopped after the library: 1831 passed, 6 failed, 9 ignored in2316.26s; later CLI/example targets were not executed. Format, included-file format, Clippy and docs passed (docs0 passed/4ignored). Full failure, all19 executable hashes and502 source hashes are preserved in [031](031-frozen-full/SHA256.json). No full-pass claim. |

Counts are separate executions, not additive unique coverage. Each compact package pins source, exact executable SHA, actual container limits, logs and source/executable recheck. The same source root/private target is reused with a rebuild for each changed source, never a source-root swap. Old executable hashes are recorded; this does not claim every old executable is archived after incremental rebuild.

The actual consumer controls cover direct and real daemon native/legacy points; one clock and one canonical exposure; false whole-store invariance; owner timezone distinct from client; old native alias rejection before work; oversized rejection before exposure; capability downgrade; no observing retransmission; exact known receipt after publication/final-admission failure; unknown operation identity plus unresolved root and reopen reconciliation; cancellation at actual graph wait; raw evidence preserved by the final protocol writer/reader. Fault injection and task/reopen tests are not power-loss evidence.

The production output check observes a maximum admitted backslash payload at324967, allowance1383973 and completed private tag1307423 bytes under the pinned shape. The fixed80KiB completion allowance is derived for the actual singleton recorder and bounded error constructors, not arbitrary custom observer output. Arbitrary outer public MCP IDs still require their own remaining-budget integration; no universal public frame guarantee follows from the fixed-u64 daemon proof. No performance, latency or retrieval-quality claim.

## Resources and remaining scope

One protected cache/target at `/tmp/recalld-curie-o1-build/{cargo,target,runs}`. Container tests use `recalld-ci:1.94`, Rust1.94.1, offline/networknone,2CPU/4GiB,jobs1/tests2, readonly worktree and no home, credentials, personal data or daemon socket mounts. No cache clones, personal recalld access, deployment, push or production merge. The failed full run and source-only owner reviews remain distinct from the targeted correction. Final targeted and format/Clippy checks are complete; exact-head integration CI remains required before shipping.

Central cleanup remains sole deletion owner. Zero host bytes released during active validation; completed containers retire via --rm. Compact proof is repository-owned; no new HOME/raw archive required. Release exact completed run roots only after publication/results recording, retain the one bounded cache only for approved upcoming work, and never delete worktrees/Git metadata or recreate previously released paths.

This consumer does not establish ordinary native startup, complete A1–A6, public2, general whole-store CAS, arbitrary custom-owner completion bounds or universal public frame safety. Final integration must preserve current D1/schema work and obtain exact-head CI; this branch has not inherited unrelated current-main composition merely for convenience.

## Full-run failures and bounded test-only correction

The six exact diagnostics are retained in [FAILURES.txt](031-frozen-full/FAILURES.txt). Two earlier MCP tests searched for an old `{\"commitState\"` text prefix and `receipts` field; actual new point failures contained the full typed INDEX_NOT_READY envelope, current receipt/observationReceipts, operation ID and no payload. One fake old peer asserted a scoped read as the first frame, but the new client negotiates first. Two native reader positives omitted the coherent QueryEngine from their adapter when observation capability was disabled. One lifecycle matrix called the intentionally refused old native get alias. These are not six silently accepted runtime defects or six tests removed to obtain green.

The reviewed correction parses only MCP-point JSON in its actual shape, asserts exact error code, receipt alias and canonical receipt equality, and preserves the older HTTP/search/similar formats and all payload/event/reopen checks. The peer explicitly answers a valid capability-absent ping before the unchanged seven alias assertions. Native fixtures share their existing actual engine/completion owner for point capture; recorder native-exposure capability remains conditional and the HTTP storage adapter is unchanged. The matrix exercises the actual MCP get consumer before/after reopen. Native old-alias rejection is retained as a separate runtime control. The only non-test source tail moves an unused CommitState import into its test module; production behavior is unchanged.

Correction source51b4f504 failed compilation (2 errors, zero tests): an unqualified `mcp` name resolved to the lifecycle helper taking Operation. Source25feec922bd43e6d794cf03f3989463aedf67a10 qualifies the existing ordinary helper twice; no new helper or runtime change. The original compile failure is retained. On25fe, the ten selected tests produced8PASS/2FAIL; format/included-file format/Clippy passed. The original failure logs and source/executable identity are in [033](033-fixture-focus/SHA256.json). The two failures were in this test-only correction: the new canonical receipt assertion tried to read a deliberately poisoned handle before reopen, and unconditional fixture owner binding changed the intended legacy HTTP503 control to500. Neither was accepted by weakening a runtime guard.

Per the user's explicit validation direction, no second full library suite is scheduled for these test/import-only changes. Run the six failures and the actual unsupported-observer, invalid-policy, old-alias/prewrite-budget and maintenance-recovery controls, then final format/Clippy. Existing22 focused RPC/receipt/unknown/bounds tests and O1 full-run regressions retain their original source attribution rather than being repeatedly rerun. Exact-head CI remains an integration gate, and no corrected-source local full-pass claim will be made.


## Final correction and validation

Final33fd scopes the fixture's `point_owner` independently of recorder observation capability. Original `readers()` and native-exposure dispatch controls keep their previous owner gating. Point-read positives and the public fixture explicitly opt into the existing real owner. The forget test verifies original HTTP503 and unchanged canonical bytes first, drops those legacy adapters, then binds the point owner. The exact poisoned-handle receipt assertion now closes/reopens the real tempstore before lookup. Production behavior remains unchanged; no second registry, storage plane, error remap or guard weakening.

Final [034](034-final-tail/SHA256.json) executes exactly three tests: the two remaining failures plus the unsupported-observer control. **3PASS/0FAIL/0ignored**, in5.41s/4.80s/4.14s respectively. Format, explicit included-file format and all-features/all-targets Clippy exit0 (warnings retained). Source502 files and executable SHA256 `0fd859566e2855e446470fbe7c991b455763c3399a7014389eeb4f62a2d00e48` were checked before/after. All final-stage exits0; no full suite was repeated.

| Selected case | Passing execution |
| --- | --- |
| Coherent postcommit exact receipt / hidden payload / reopen |34 focus01,33fd |
| HTTP/MCP/daemon postcommit receipt, generation, event and reopen |33 focus02,25fe |
| Scoped false reads with valid old-peer handshake and seven aliases |33 focus03,25fe |
| Native forget publication failure, repair, original HTTP503, no resurrection |34 focus02,33fd |
| Summary/extreme-clock/sticky-permastore matrix with actual MCP point reads |33 focus05,25fe |
| Native publication labels/read-only bytes before/after reopen |33 focus06,25fe |
| Deferred selection / unsupported observer remains closed |34 focus03,33fd (also33 focus07) |
| Accepted maintenance publication failure/replay/removal |33 focus08,25fe |
| Native old-alias and pre-observation-budget no-write controls |33 focus09,25fe |
| Invalid configured policy / empty no-work control |33 focus10,25fe |

These are ten selected cases with attributed passing executions, not ten tests all executed on33fd. Likewise the earlier22 ordinary-point tests passed oncc29 and in the7c4 full run; no final-source full PASS is claimed. Full7c4 remains **1831PASS/6FAIL/9ignored**. The all-targets build compiled19 executable artifacts, but the failed library stopped later CLI/example execution; required integration CI must run the complete target matrix. Documentation tests on7c4:0PASS/0FAIL/4ignored. Huygens/A1/Pascal source reviews are not independent execution.

All captured OOM/oom_kill counts are0. The full-test peak was846422016bytes; full-stage Clippy cumulative peak3905900544bytes. Final targeted/lint cumulative peak3784216576bytes; build peaks reached the configured4294967296byte limit without OOM. Limits stayed2CPU/4GiB/jobs1/tests2. Completed containers retired normally; zero host bytes released. The one existing private cache and run evidence remain protected pending integrator/PR handoff; central cleanup alone receives exact completed roots after publication/results recording. No additional HOME archive.
