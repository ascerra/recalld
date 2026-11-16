# Public lifecycle chronological matrix

Status: local targeted validation sealed; exact published-head full CI remains
required. No production change, activation or complete A3 acceptance claim.

Sole parent: PR153 `a2a8866dd898dff7eae01dd49b23d8cd18532980`.
Final test source: `915d160c2d82f632c7edfb70c362d05449c0c30e`.
Worktree `/tmp/recalld-implementation/public-lifecycle-matrix`, branch
`impl/public-lifecycle-matrix`. PR153 stays frozen under Halley's ownership.

## What the tests exercise

Existing real legacy insertion/phase-loss, schema/attestation and lifecycle
migration fixtures feed actual MCP dispatch, HTTP router and MCP→Remote→daemon
API1 consumers. The matrix includes physical legacy, native legacy_access_v1,
and native explicit_feedback_v2. Original API1 is independent of disk and policy.
Test-only fixture wiring accepts the existing Clock trait and reuses the existing
regressed-clock counter. There is no new production framework, storage subsystem,
observer, scheduler, public time endpoint or separate ID allocator.

- Resolved multipliers0/1/2, missing quality3 and qualities1–4: public numerical
  outputs match the shared planner under the same canonical policy/time; one
  history event/receipt/clock sample per successful unkeyed feedback; original1
  request hashes and all prior receipt bytes are preserved. Twenty-seven
  layout/transport/multiplier combinations each run five feedback calls.
- Actual Chrono MIN_UTC and MAX_UTC milliseconds are representable. Feedback
  remains finite; clock regression clamps native anchor/reinforcement/updated
  values while preserving observation/retrieval provenance and phase marker.
  No artificial invalid Clock was introduced.
- Summary content is stripped by a real canonical phase-loss transaction before
  migration. Feedback retains Summary/no full text through actual rebuild,
  canonical read, strict reopen and another rebuild/read. The final strengthened
  fixture starts below permastore threshold5, quality4 crosses it, and quality1
  falls below it while exemption remains set. Explicit forget still removes it.
  Native forget preserves raw anchors and clamps its phase marker. Nine
  layout/transport combinations exercise this trace.
- Ghost feedback and existing malformed quality entry validation make no record,
  generation, receipt or clock change. Existing API1 plain-text parser errors
  remain plain text; typed lifecycle failures retain their existing envelope.
- A real StorageAccessRecorder exposure at day7 precedes public feedback at day8.
  Native policy2 produces equal stability to a separately seeded no-exposure
  comparator; legacy policy retains its exposure-driven anchor behavior.
  Retrieved and reinforced timestamps remain distinct. The recorder is explicitly
  setup, not proof of public search/get routing. Eighteen stores cover the three
  layouts, three transports and exposed/unexposed variants.

Planner comparison proves adapter/policy/clock agreement, not an independent
formula oracle. Operation UUIDs differ across independent unkeyed actions; no
complete-state-hash equality or independent-genesis/A6 determinism is claimed.
No lifecycle/default/RIF utility or performance promotion follows from these tests.

## Exact validation attribution

| Source / execution | Result |
| --- | --- |
| Parent153 runtime1364b27 | Prior full1209 library +2CLI PASS; separate baseline, not a child rerun |
| fa91554 first matrix | 3 PASS /1 FAIL,57.00s: new test helper wrongly JSON-decoded an existing plain-text MCP validation error |
| 741528ec corrected matrix | 4 PASS,76.74s, including Summary reopen before forget |
| 741528ec affected fixtures | public18,scope5,decoder3,wire6,legacy17 PASS (49 total); one historical fixture generator ignored |
| 741528ec fmt/clippy | exit0;949 library/1012 lib-test warnings (920 duplicates),1 bin warning |
| Final915d160c threshold-strengthened Summary matrix | 1 PASS,20.78s; nine layout/transport traces |
| Final915d160c fmt/clippy | exit0;949 library/1012 lib-test warnings (920 duplicates),1 bin warning |

The only delta741528ec→915d160c changes this new Summary test: threshold0.01→5
and assertions proving the crossing/lapse branch. Production code and all other
matrix/affected tests are unchanged. No production defect was reproduced.
The first failure is test-helper attribution, not a historical runtime BEFORE.
Source and all nine emitted executable hashes were verified after the completed
741 and915 checks; first-run source was also verified unchanged before correction.

Per the explicit bounded-validation instruction, no automatic local full-suite
repeat was performed for this test-only child. Exact-head full CI is required
before integration. Parent153's exact a2a8866 head independently has all six CI
checks green (run43665367480); that is not this child's CI result.

Synthetic offline Docker only: recalld-ci:1.94, Rust1.94.1,2CPU/4GiB/jobs2,
networknone, no home/credentials/socket mounts. One private Cargo cache and one
new reusable target, no per-run cache cloning. Manifests retain all nine emitted
executables, including production CLI binaries compiled by cargo; filtered runs
executed no CLI test, so no new CLI pass is claimed. Detailed log hashes, results,
source manifests and exact driver scripts accompany this report.

## Ownership and remaining boundaries

Huygens owns public query/store routing and the store capability extension;
Curie owns remaining legacyAPI2 insertion/graph/compound closure; Feynman owns
inherited-global adoption, legacyAPI2 catalog/purge audit and published147
maintenance/compaction producer. Pascal reviewed the narrow nonprojecting
forget/structural-graph binding exception source-only, without editing his files.
The A6 helper confirmed no overlapping fixture/harness work. There was no
identified unowned legacyAPI2 lifecycle route requiring another production fix.

No135/147/owned-core/adoption parent was needed for these actual consumers.
Add one only for a concrete subsequent sweep, compaction, owned reset or public
adoption test. Ordinary native startup/public2 remain gated. Joined public query
exposure/read-only/RIF traces, full mutation participation, global-adoption
consumer proof, fault/durability and empirical promotion remain separate gates.

## Resource handoff

Retain one reusable `/tmp/recalld-a3-matrix-target` and one private Cargo cache
`/tmp/recalld-api1-lifecycle-validation/cargo`, coordinated with central cleanup.
The old target was already being deleted when retention guidance arrived; it was
not recreated. The new target is separately protected. Source/worktree/.git and
tracked evidence remain protected.

Raw `/tmp/recalld-a3-matrix-validation` may be released centrally after publication
and recorded results; until then protect it. No raw HOME archive is required.
This agent has deleted0 bytes and does not race central cleanup.

Small HOME results-only copy and verified COPY digest are recorded in `home-seal.json`; no raw cache, binary or log archive is copied there.
