# Certified selection consumer join — validated

Actual parents are certification PR175 `7283a700c4eca5f8d1d8907b33c867b1d1e3a6bb`
and selection demand PR169 `1b7fd01b810c15c62ef2fe4e01f6cf6263b789a0`.
Local merge `b8f0a3b5` is separate from new test changes; no other parent was added.
The production selector is byte-identical to169. Relative to175, the two actual
single/reserved-batch callers pass full frozen intent to capture and preserve
175's private-sealed generated object. Supplied input remains distinct. No core,
codec, receipt/hash, certification-helper or public routing edits were required.

The child expands the real consumer demand tests to generated and supplied
inputs in all five modes. The provider's raw [2,0,0,0] survives selection's working
normalization and verifies at final commit; stored bytes are [1,0,0,0]. Generated
receipts bind the actual document and certification, Supplied receipts bind the
explicit profile and make no provider calls. Strict reopen/replay returns the
exact original receipt without fresh provider/profile/clock/selection work.
The existing real certification fixture is used only for fresh positive setup;
original uncertified/historical fixtures are unchanged.

Selection-only counts with two eligible peers: off(0scan,0vector), entity and
temporal(1scan,0vector), semantic/advisory(1scan,2vectors). Exact edge kind/target/
weight, original receipt identity and advisory intent remain asserted. No latency
or throughput claim follows from these counts. All-off uncertified actual single
and batch reject before work, with unchanged canonical file bytes/G. Semantic
and advisory corruption still reject after provider entry: no child record or
receipt, exact committed batch reservation/child ID retained, durable pending
state preserved, and strict reopen rejects corruption.

## Executed validation

- `focus001`, sourcee109ce9e: test-only Tag From<&str> compilation error. No tests
  executed and no executable is attributed to that source. All queued filter
  attempts failed the same compile error; original logs/source remain unchanged.
- `focus002`, source`eca9e0edd7c6303a703b3bd776169d1c4e67b8b4`:33focused test
  executions pass:5newdemand,5existingselection,16publication-failure,5unknown-child,
  1freshadmission and1checked-empty advisory. Allfeatures enabled. Exact source
  unchanged; test executable SHA256
  `40730671542e10066fd7ee2e6a4c0407fd75ec146c0bcc405672486851a5da9e`.
  Cgroup maxevents1044,oom0/oom_kill0. One-line Tag::new correction only after001;
  no production change.
- `full003`, same frozen source: **1,420 library passed,0failed,2ignored**
  (1177.64s), **5CLI tests and10example tests passed**, allfeatures/alltargets.
  This includes the existing offline_eval example tests. Three binary harnesses
  and two example harnesses contain zero tests. This command does not run doctests.
  Fmt exit0; all-feature/all-target Clippy exit0 with warnings (25.76s).
  Driver0; memory peak3,960,991,744bytes, maxevents0/oom0/oom_kill0.
  All388 recorded source inputs stayed unchanged. The library test binary matches
  focus002. Thirteen actual Cargo test harnesses plus three CLI applications used
  by their integration tests were hashed before further builds/cleanup.

`VALIDATION.json` records exact source/parents/counts. `RAW-MANIFEST.json` binds
original raw logs and metadata; compact excerpts retain all test output and final
statuses. Repetitive Clippy warnings are omitted from its excerpt, not claimed
absent. The failed initial compilation has no executable attribution. Stored
command files preserve the actual first-run script; afterward the runner gained
an early stop on compilation errors to avoid redundant filtered compilations.

Rust1.94.1, pinned recalld-ci image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Docker networknone,2CPU/4GiB/jobs2, debug0. Read-only source, synthetic temporary
stores, no host HOME/credentials/daemon/socket mounts. One existing private
cargo/target only. Own package clean before the source-root switch removed2.3GiB;
no other cache was cleaned. Actual run commands/source/executable/memory evidence
are sealed under this directory, with original raw logs under protected child
.validation until publication/results cleanup. No raw HOME archive is made.

No ordinary native/public2 activation, whole-store CAS, full-spec completion,
power-loss durability or new certification of historical Bound vectors is
claimed. Standalone semantic graph, query and compaction consumers remain separate.
No parent pushes, deployments or main merges. Tracked changes after the validated
runtime are documentation/evidence only; no redundant local rerun. The child is
ready for exact-head publication review, with suggested GitHub base175
`impl/vector-certification` and required additional169. Parent branches stay
frozen; this branch has not been pushed.

## Changed files and remaining scope

Production: `src/graph/native_selection.rs`,
`src/derived_completion/native_store.rs`, `src/mcp/native_store_batch.rs`.
Tests: `src/storage/metadata/native_records/selection_demand_tests.rs`,
`selection_tests.rs`, `store_batch_tests.rs`, `store_producer_tests.rs`.
Relative to the pure approved merge, only selection_demand_tests has new code.
The other test differences preserve the two parents' actual APIs and explicit
certification setup. The shared fixture helper and all core/codec files are
unchanged. Plan/implementation note, work note, runner and compact evidence are
the remaining child documentation changes.

Standalone semantic graph certification, query/constructor and physical
compaction joins remain separate. No backend-comparison/default-choice evidence
or broader A1 acceptance is supplied by this bounded consumer integration.

## Resource handoff

All owned containers exited and were automatically removed. The active raw root
`/tmp/recalld-implementation/certified-selection-consumer/.validation` remains
protected until the child PR and results are recorded; only the existing central
cleanup owner may remove it. The sole private cache/target under
`/tmp/recalld-curie-active-build` stays protected for the assigned next standalone
semantic-producer consumer; no per-run cache clones. Prebuild own-package clean
removed2.3GiB (subsequently rebuilt), not a claim of net recovered disk space.
Old centrally released164/169 raw paths were not recreated.
