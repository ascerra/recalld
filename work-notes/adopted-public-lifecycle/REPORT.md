# Adopted policy through public API1 lifecycle — validation report

Status: validated bounded composition, ready for publication review. Not published
or integration-READY; exact published-head full CI is still required.

## Actual parents and source boundaries

Exact PR159 `b71793910b0fc5593bb9644317a1f514ee6a303e` (adoption runtime3e5fd806)
plus PR153 `a2a8866dd898dff7eae01dd49b23d8cd18532980` (public lifecycle runtime1364b272).
Normal merge `7be2d3019b4f52a6582756f08e141421e040eed1`; no other parent or duplicate
cherry-pick. Planning commit858587 precedes that merge on159. Full-suite candidate
`3fbdcb09b275e47844577abfc5020913711bd8dd` changes tests only after the merge.
Final test-only tail `db2d14f2e930b8ed72af45c5da9cc395daf43436` corrects
two fixture/consumer boundaries exposed by that completed full run.
No new production policy/receipt codec or request semantics were introduced.

Fourteen merge-conflicted files were reconciled per hunk, preserving159 typed
history proof and153 original1/receipt/nonretry behavior. `storage/control.rs`
and `main.rs` remain exactly159. The shared direct bridge preserves the same
clock and153 query-engine injection; the pinned reader retains159 native query
admission and153 RejectExposure; the observer and reader share the same completion
handles. Feynman and Huygens each source-reviewed these substantive seams without
a blocker. Their review is not execution evidence. Details are in
`docs/implementation/adopted-public-lifecycle-join.md`.

## Four bounded consumer cases

1. A real migrated native namespace with absent inherited policy rejects public
   feedback while public structural forget succeeds. The test never adopts that
   unknown fixture. Rejected feedback preserves canonical record/control/receipt/
   ledger bytes and takes zero mutation clock samples. Accepted work may be
   registered and completed; this is not a no-ticket claim. Forget retains unknown
   inheritance, produces original1 terminal history, samples once and survives
   strict inspect/reopen. Actual HTTP, MCP and negotiated Remote-backed MCP lanes.
2. Actual offline close/preview/adopt/reopen enables public feedback. Adoption
   itself changes only permitted revision/tokens, preserving anchors, timestamps,
   phase and payload. A changed global rejects fresh feedback without writes or
   mutation clock sampling. Real re-adoption enables the new inputs; old receipt
   bytes remain exact. Structural forget still works under a mismatched configured
   global. All three public lanes preserve original1 and complete accepted work.
3. Explicit Some(1) overrides the unused changed global, without adoption or
   namespace revision churn. Actual public feedback is compared to the existing
   shared planner using that override. This proves policy/adapter agreement,
   not an independent mathematical oracle or a new quality/phase matrix.
4. Historical keyed replay uses the existing explicit original1 private lifecycle
   RPC, separately from unkeyed public API1. After a second offline adoption,
   the original request with stale expected G replays its exact full receipt,
   operation and historical G before current-policy/CAS/clock checks. Different
   quality conflicts. Canonical state/receipt bytes do not change. No API1 retry
   key or cross-copy/global key guarantee is invented.

Fixtures use actual legacy insertion, canonical/schema and lifecycle migration,
profile attestation and operator adoption. No raw schema/default blessing, new
clock, second observer, parallel storage, provider call or duplicate scheduler.
Preview samples are accounted separately from public mutation samples. Public
HTTP retains its existing simple BAD_REQUEST input errors; known committed and
unknown errors remain covered by affected153 tests, not replaced with a new envelope.

## Preserved attempts and exact attribution

- `891d9489`: compile-only E0369 in new test comparing NamespaceConfig without
  PartialEq. No tests/full ran. Corrected to canonical encoded-byte comparison.
- `5b28b898`: new tests0PASS/4FAIL. Three stopped on a test snapshot trying to
  JSON-encode raw byte-map keys; corrected to ordered exact key/value entries.
  Explicit-override test reached the Remote branch but the test called an old
  low-level method, receiving its honest native nonparticipant rejection.
  Corrected to actual public MCP dispatch over the negotiated Remote adapter;
  this was not a production routing defect. Existing public tests16PASS/2FAIL
  exposed intended-positive fixture conflicts: selected legacy-lifecycle schema
  remained unknown, and multiplier2 conflicted with an adopted1. Only those two
  positives gained actual offline adoption. Genuine unknown/history cases remain.
  Inherited-policy11PASS; legacy lifecycle21PASS/1ignored. A native decoder filter
  matched zero tests and is explicitly excluded from coverage; later runs fix it.
- `46bfb6bb`: new tests2PASS/2FAIL. Explicit override and actual private keyed
  history replay passed. Unknown/global-change cases received the correct public
  HTTP400 BAD_REQUEST, but the test incorrectly expected an observation envelope.
  Corrected assertion preserves existing API1 shape. Public18, inherited11,
  legacy21/1ignored and actual native decoder3 all passed. No full ran.
- `3fbdcb09`: all focused groups passed. Driver explicitly verifies nonzero
  minimum counts4/18/11/21/3 before allowing full. Their57 passing invocations
  cover53 distinct tests (the public/legacy groups overlap); one legacy test is
  ignored. The actual full library run then finished **1324 passed, 2 failed,
  1 ignored** in919.36s (exit101). Cargo stopped before CLI/examples; fmt/clippy
  were not reached. These are preserved failures, not a full pass:
  - `native_exposure_per_namespace_failure_publishes_preceding_receipts_without_terminal_access`: expected committed, got notCommitted. The fixture had remigrated into a fresh unknown binding before its intended first positive.
  - `native_record_context_replays_genuine_before_api2_bytes`: fresh capture rejected the genuine historical fixture's unknown inherited policy before the test reached replay.
- `db2d14f2`: test-only correction, no production guard/codec change. The exposure
  fixture performs actual offline adoption only for the intended first-success
  namespace; the second terminal namespace explicitly remains unknown. The old
  API2 zip remains unchanged and unadopted. Both original2 replay and contract1
  conflict use the actual `insert_generated` consumer, whose receipt lookup
  precedes fresh capture/provider. Full original receipt rows, canonical file
  bytes, control/G, zero clock samples, provider fail_if_called/calls0, and final
  None are asserted. Existing terminal forget replay is unchanged. Curie reviewed
  this consumer boundary before edits; that review is not executed proof.
  Final checks passed: **68 focused invocations / 64 distinct tests**, with one
  pre-existing fixture-generator test ignored; **3 CLI tests and 10 example
  tests passed**. Both formerly failing tests executed and passed. Nonzero gates
  enforce all seven groups: adopted_public4, api1_public18, inherited_policy11,
  legacy_lifecycle21, decoder_native3, native_exposure9, native_record_context2.
  Formatting passed; clippy exited0 with warnings:1076 library,1153 library-test
  (1041 duplicates),1 binary plus1 duplicate in its test target. Not warning-free.
  The run container exited0; no source edits occurred during the run.

There is deliberately no second local full suite for this two-test-only tail.
Exact published-head full CI remains mandatory before integration; the failed
3fbd local full is never represented as a green final-head suite.

Failed sources, build diagnostics, emitted-executable manifests and actual
failure/result excerpts are retained per attempt. No failure is relabeled as
runtime proof or an exploit. There has been no production correction after the
reviewed normal join; subsequent commits correct test encodings/envelopes, actual consumer selection,
and the individually identified intended-positive fixtures. Genuine unknown and
historical fixtures remain unadopted.

## Environment and resource boundary

Isolated synthetic Docker recalld-ci:1.94, Rust1.94.1, networknone, 2CPU/4GiB,
CARGO_BUILD_JOBS2, no personal data/home/credentials/socket mounts. One private
Cargo and reusable target. On the first source mount switch only this package's
outputs were cleaned (144 files/685.6MiB reported), retaining dependencies.
Subsequent attempts used the same target. Frozen source is read-only during runs.
All **338 source/config/data files and 15 emitted executables** verified against
their recorded hashes after final checks. All five source manifests also match
their exact committed Git blobs. Emitted count does not mean15 nonempty test
targets. The genuine API2 ZIP SHA256 is
`d9f61e87b45de34ae8622dbb3145fd577fe45333943d7777294bf5aea5a4af78`,
identical in153, the normal merge,3fbd anddb2;159 did not contain this fixture.
The initial provenance helper incorrectly included159 in that comparison and
failed git-show; it was corrected to the actual fixture-bearing ancestry, without
changing source/test outcomes. Final source manifest SHA256:
`370736192af3acdeba25a4d8055bc1872ef719f600a5d35932c26f6180277dd0`.
Executable, source and log hashes are retained in the adjacent manifests.
No HOME raw archive, full source copy or executable copy is produced.
No additional Rust run for an evidence-only tail. Small results/HOME notebook
copy is separate from disposable raw runs; no raw archive requirement.

Protected raw root: /tmp/recalld-adopted-public-validation. Release it through
central cleanup only after resulting PR+recorded results. Retain the protected
/tmp/recalld-a3-matrix-target and /tmp/recalld-api1-lifecycle-validation/cargo for
next assigned work. Never recreate released public-sweep-clock/matrix raw paths.
Owned container `recalld-adopted-public-final` is removed after successful exit and
provenance capture. Reclaimed bytes are not measured. Raw roots remain protected
pending publication/results and the central cleanup ownership handoff.

## Remaining acceptance and owners

This closes only actual adoption→public API1 feedback/forget composition and a
separately labeled existing keyed private replay path. It does not enable ordinary
native startup or public2, and does not prove public query exposure→feedback.
Huygens owns validated query/wire activation and remaining batch/store composition;
Feynman owns certification/profile proof and catalog/purge/binding codec work.
Existing156 chronology/retained-phase evidence is not copied here.
Full logical trace equality needs the actual owned IDs/reset harness, not these
unkeyed UUIDs. RIF/persistent-loss/permutation acceptance, A3 completeness, A6
quality/RL/default promotion and performance gates remain separately evidenced.
No merge/deployment or blanket integration acceptance is claimed.

## Review and integration handoff

Validated Rust/source head: `db2d14f2e930b8ed72af45c5da9cc395daf43436`.
Runtime composition: normal merge `7be2d3019b4f52a6582756f08e141421e040eed1`.
Everything after that merge changes tests or documentation/evidence only.
Six changed Rust test files after the merge: `native_records/{adopted_public_tests,
exposure_consumer_tests,public_api1_tests,request_context_tests,tests,wire_tests}.rs`.
The two final full-failure corrections are isolated in db2. Existing canonical
control and main.rs remain byte-exact159. No production guard was weakened.

Publish as actual159 child with additional153 dependency disclosed, after review.
Do not add156/158/162 or a general integration checkpoint. Exact final-head CI
must pass before a READY/freeze handoff to Halley; this report alone is not READY.
The small HOME result copy and notebook entry are recorded by `home-copy.json`.

Evidence layout: each candidate has its immutable source manifest, driver,
emitted executable manifest when compilation succeeded, compact result/failure
excerpts and hashes of the retained raw logs. Candidate1 is compile-only; its
compiler JSON diagnostics are preserved as text. Candidate4 retains the actual
full failure block. Candidate5 has the final post-run verification. No zero-test
filter is credited as coverage. Raw data can be discarded centrally after PR
and recorded results under the retention override; committed reports remain.
