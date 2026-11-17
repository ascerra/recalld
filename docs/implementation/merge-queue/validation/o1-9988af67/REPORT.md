# Prepared get plus actual main190: bounded final composition

Frozen source `9988af67439a19813decb8c01ddddb97b91fc09b` is the normal merge of published O1 `8039cc2b` and actual main190 `32ab3b3f`. The only conflict was get-description wording; resolution retains exact D1 text and `output_schema:None`. Huygens reviewed the exact delta. Get core/decoder/client/Cargo blobs remain unchanged from O1; added metadata schemas, negotiated-era stripping and session guidance are exact main190. No speculative dependency or public activation.

## Actual validation

- **109 unique library tests passed across111 executions**,0 failed; one historical stale-point BEFORE probe ignored. Prepared78, strict-query14 (two overlaps), metadata3, session1, legacy/direct pages9, native pages6. Exact groups and ignore appear in TEST-SUMMARY.log.
- Cargo fmt and all-feature/all-target Clippy passed; warnings retained. No new local full-suite/CLI-test/example-test claim. Full1801library+10CLI+15examples remains source2925 only.
- Actual public fixture3: **9 episodes** (three fresh replays each stdio MCP, HTTP MCP, REST) and **54 true-access cases** passed, including false public/durable invariance, returned-only exposure, malformed inputs and restart. This is legacy diagnostic conformance, not native ordinary/public2 activation or quality evidence.
- Formal actual emitted schemas: **63query,22page envelopes,13live metadata outputs**, plus mutation negatives, all passed using pinned offline jsonschema4.25.1. The earlier full run's32page count belongs to2925; this bounded run covers22.
- All **542 source hashes** verified before/after; library executable `46479a60c3a3bda06ad6301593daee667fc47de716916a4e8784f3190b5829fa` unchanged across every selected test and both supplemental tails. Public CLI `614b2248d23701d733bc6cdaee8a332af3abed0c0d8aafad9ba4a8ac10b55e83` pinned before/after public checks and reverified at seal.

## Two retained driver failures

The first driver completed all nine contained stages successfully, then its host-side fixture-count assertion stopped the schema phase: selected filters emitted6page files. Supplemental canonical-list/catalog module tests passed4+5 on the identical executable but emitted only8page files, again stopping before schema validation. Both host AssertionErrors are preserved in driver-failure.json and completion-tail/driver-failure.json, with original drivers/log hashes. These were insufficient filter selections, not runtime test failures. No assertion was weakened. The second supplemental tail ran the actual native list/catalog transport modules2+4, producing22page fixtures; all three formal suites then passed. No Rust source edit, rebuild or repeated full suite occurred. Original build.json retains its pre-host-failure running status; RESULT.json and completion-native-tail/completed.json carry the final aggregate outcome, not a fictional single-clean-driver result.

## Isolation and limits

Actual Rust/Cargo1.94.1, aarch64-unknown-linux-gnu; RUST_MIN_STACK/RUSTFLAGS unset. Offline Docker2CPU/**6GiB**, jobs1/tests2, read-only allowlisted source, private HOME/cache/target/output, no personal store/socket/credential mounts. Same-executable tails mount target read-only. Schema-only container4GiB. Reused one queue cache after own-package clean (cargo reported12.3GiB removed); no clones or switched-source stale executable. Initial cgroup peak6442450944bytes, max7990, OOM/OOM-kill0: reclaim pressure, not4GiB resource conformance or a performance result. No x86/compiler clearance is transferred.

Ordinary public API1 point routing/retry fixes remain Curie's separate child; oldO1 known limits remain explicit. Arbitrary public MCP-ID budgets, ordinary native startup and public2/fullA6 activation remain gated. Main190 CI and exact newly published-head CI are required before merge.

Cleanup: all four owned validation containers retired;0raw bytes deleted at seal. Passing/resulting-run raw may be released centrally after PR update/results recorded, with both driver failures preserved in repository. One queue cargo/target remains active future input. Registered worktree/Git and unrelated failed evidence remain protected; no HOME raw archive.
