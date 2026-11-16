# Public API1 single-store validation

Runtime candidate: `14441bb9ee56649d77211a65520d770c16c28065`.
Actual parents: PR148 `508d98c65b0f824bbcc0290812c63e0dea426edd` and
PR153 `a2a8866dd898dff7eae01dd49b23d8cd18532980`. No query, catalog/list or
startup convenience merge. Final local validation is complete. Publication and
READY handoff remain pending final publication review; exact published-head CI
will be required separately. The seal changes documentation/evidence only.

## Behavior and proof boundaries

Public MCP/REST API1 selects the actual owned physical format. Native stores use
original1 atomic insertion, exact UTF-8 labels, canonical profile binding and the
corrected generated document recipe. REST successful MemoryResponse and MCP
StoredMemory shapes remain API1. Native REST missing profile422, malformed/null400
and mismatched Bound409 are explicit. Legacy REST no-profile supplied vectors,
legacy displayed entity capitalization, and ignored MCP single embedding/profile
inputs retain their old behavior.

The native public HTTP fixture verifies exactly one generation advancement with
its parent link and recomputes the retained request hash for original1; original2
must differ. Provider spies verify actual composed input. Native Remote exercises
the real daemon/client handshake, creation, label persistence and reopen. Raw old
store RPC on native and undersized/unnegotiated new peers reject before provider,
clock or canonical bytes change; the same old alias succeeds on legacy storage.

Publication failure retains the exact full receipt through API1 projection and
reopen. Missing success identity fails closed while preserving in-hand receipt,
readiness and warnings. A malformed new-method response yields unknown without
inventing a receipt or sending a legacy mutation. Capability cross-products and
reconnect downgrade prove independent store/lifecycle state on one held connection
lock. Scripted capability peers prove protocol behavior, not historical native
producer acceptance. Genuine PR148 production provides the separate public-route
BEFORE failures below.

No-allocation conclusions are limited to the explicitly exercised fresh-invalid
and predispatch paths and their code boundaries. Provider/unknown failures may
consume owned reservations. This report does not claim every error is rollback.
No public2 or ordinary native startup activation, no certification of historical
Bound vectors, no batch API1 projection or full A4 completion is claimed.

## Run attribution

| Run | Source/result | Interpretation |
| --- | --- | --- |
| before001 | Actual PR148 + added fixtures; compile failure | Wrong test helper arity; no behavior evidence. |
| before002 | Actual PR148 + corrected arity; 2 failed | MCP legacy Unicode rejection; REST fixture stopped at pending-effects503. REST is not a store-route rejection claim. |
| before003 | Actual PR148 + actual owner completion; 2 failed | Both public MCP and REST reject canonical Unicode labels through legacy validation. Production unchanged. |
| draft004 | Local producer/projection; compile failure | Missing test-only readers::bytes import; no behavior evidence. |
| draft005 | Before153 join; 3 passed, 2 failed | Native public creation/full-known-receipt positives; wrong zero-vector HTTP expectation and legacy test runtime flavor. |
| join006 | Actual148+153 + store transport; 39 passed, 2 failed | Actual Remote/reopen/capability positives; wrong initialStability status and legacy display expectation. |
| join007 | Added raw-alias guard/transport fixtures; compile failure | Missing Arc import and wrong tuple accessor in tests; no behavior evidence. |
| final008 | Frozen runtime above; 43 focused + 1332 library + 2 CLI passed | Zero failures; 2 library/4 doctests ignored. fmt and clippy exit0; warnings retained. |

[Exact PR148 source excerpts](expectation-source.md) explain fixture corrections.
The 400 initialStability mapping is inherited prepared-native taxonomy, not a
claim that the old public native store route worked. Legacy `Alice` display comes
from the unchanged legacy handler/MemoryResponse/tag parser. No production code
was changed to satisfy these two expected values.

## Isolation and provenance

All execution used `recalld-ci:1.94`, toolchain1.94.1, network none,2CPU,4GiB,
CARGO_BUILD_JOBS2, immutable RO source and one private Cargo/target. No personal
store, daemon, home/credential/socket mount or paid provider calls. Tests use real
explicit migrated/attested native fixtures; they do not enable ordinary serving.

Every source switch used `cargo clean --offline -p recalld` in the owned target
after executed-binary capture. Per-run pre/post source hashes and executed binary
hashes are retained with logs. Compile-only failures have zero executed binaries.
Raw scratch is eligible for central cleanup only after PR/results recording;
tracked code/reports/manifests and the one continuing private cache stay protected.
No HOME raw archive is required under the user's retention override.

Run006 reached memory.peak4294967296 and memory.events.max10356 with oom0 and
oom_kill0. Final008 reached the same 4294967296-byte peak and max31011, also with
oom0/oom_kill0. These are reclaim-pressure counters, not OOM failures. No
resource-cap increase or target clone occurred.

## Final immutable proof

Final008 ran `cargo test --offline --all-features --lib api1_` (43 passed,
0 failed, 1291 filtered), then `cargo test --offline --all-features` (1332 library
passed, 0 failed, 2 ignored; two CLI integration tests passed). The full library
run took 904.92 seconds. `cargo fmt --all --check` and
`cargo clippy --offline --all-targets --all-features` both exited 0. Clippy reported
1054 library warnings and 1128 library-test warnings (1014 duplicates); this is
not a warning-free claim.

The ignored library tests are the genuine pre-correction legacy lifecycle fixture
generator and the dense native-single receipt probe requiring immutable BEFORE123
artifacts. Neither was executed by this run. Four documentation examples are
also ignored. Existing parent evidence is not attributed as a fresh rerun.

All 355 frozen source files match their pre/post SHA256 manifest and the runtime
worktree at seal time. Nine Cargo-produced executable hashes/sizes were captured
before any target cleanup. The final run's 18 evidence files were copied and
SHA256-verified in [the copy manifest](runs/run-api1-store-final-008/COPY-VERIFIED.json),
including exact source, exits, complete logs, source manifests, executable hashes,
container mounts/image/resources, rustc version and cgroup counters. The immutable
source corresponds to `14441bb9ee56649d77211a65520d770c16c28065`; no runtime changes
or full rerun accompany this documentation seal.

Publication is proposed on PR148 with PR153 as the additional actual dependency.
The raw run roots remain protected until publication/results are recorded, then
central cleanup may release them under the user retention override. The continuing
private Cargo/target and source worktree remain protected.
