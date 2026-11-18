# Prepared query exposure → public API1 feedback chronology

Status: fully validated bounded composition, ready for publication review.
Not published or integration-READY; exact published-head CI remains required.

## Actual dependencies and boundaries

Runtime is the normal merge `d2c705d49154c8b9284d7564f4366d2f2e3ba23a` of exact
PR151 `2b2560516e1e8e4548c9511f296a72862110caa9` into PR166
`3f3ca05d160cad917b716758dd360412a652e9d9`'s approved source-plan child.
166 already supplies actual159+153.151 supplies actual146+88, including the132
recorder. No unused156 matrix,150 RIF,158 store,162 sweep-clock, certification WIP
or broad integration parent. Public API2/ordinary native activation remains gated.

Eleven conflicts were resolved per hunk, recorded in
`docs/implementation/query-feedback-chronology-join.md`. Huygens reviewed committed
query/observation/prepared-dispatch interfaces and Feynman reviewed inherited
admission, pinned clock/policy and lock/proof seams. Both found no source blocker;
neither review is execution proof. Canonical control is byte-identical166/159,
query_wire.rs byte-identical151, inherited/native lifecycle/publication/system
unchanged from166 where noted. No production source changes follow the merge.

## Four actual chronological cases

The same helper executes native-schema lifecycle v1/v2 for two lanes: prepared
MCP query→public API1 MCP feedback, and prepared REST query→public API1 HTTP
feedback. These invoke actual adapters and handlers, not a direct recorder or
mock query. Prepared query version2 is distinct from live outer routing, which
remains closed. Both policy cases use native schema; lifecycle v1 here is not a
claim that the fixture uses legacy disk format. Feedback preserves original1;
the query emits original2 receipts independent of lifecycle policy.

Day0 uses actual existing internal insertion, schema migration/attestation,
lifecycle migration and explicit inherited adoption, not a public store producer.
Two copies come from one actual logical export/import with identical record
identities. Each is strictly inspected/opened and finishes real owner rebuild/
publication before its baseline. This is isolated paired synthetic state, not
production multi-writer authorization, restore promotion or a NativeV2 label
provenance upgrade.

- At T0+7days, actual metadata-only false query is byte-identical across owned
  canonical metadata/edges/catalog-selected text/vectors, has no exposure receipt
  and leaves no pending/unresolved access work. It captures one shared clock time.
  Accepted ticket sequences and cache accounting are not claimed immutable.
- The true query limits two eligible seeded records to one final result. Its
  actual returned UUID receives exactly one exposure; the omitted record's exact
  bytes/token remain unchanged. Full receipt, original2 request hash, original
  readGeneration, actual captureGeneration, sampled time and current readiness
  are checked. The recorder does not resample the configured clock. An empty
  metadata filter commits no exposure/receipt; no durable keyed no-op is claimed.
- In explicit_feedback_v2, day7 exposure preserves anchor/stability/feedback and
  observation provenance. In legacy_access_v1 it resets the anchor to day7,
  without stability growth. Phase, text/vector/summary/tag payload and retained
  label provenance remain intact.
- At T0+8days, actual unkeyed public API1 quality3 feedback runs in observed and
  untouched no-query copies. Both get one clock sample and original1 request
  hash/full persisted receipt. V2 final stability/difficulty equal the no-query
  control despite retrieval. V1 retains its day7 reset and differs from the
  no-query control as the existing planner predicts. This comparison verifies
  adapter/policy agreement, not an independent formula oracle.
- Strict inspect/reopen preserves records, retained payload and exact receipts.
  retrieved_at and reinforced_at remain distinct; observed_at is immutable.
  The existing provider panics on embedding/profile/dimension access, so those
  accesses cannot silently satisfy this metadata-only fixture. No ID/RNG/full-state hash,
  cross-origin generation ordering or public keyed-query guarantee is invented.

One existing ManualClock is shared through the test adapters and recorder,
wrapped only to count samples. Test helper clock arguments now accept the
existing Clock trait, and query/feedback share the configured policy inputs.
No second runtime policy source, scheduler or storage subsystem was introduced.

## Preserved validation attempts

- `0101b38401da453f1437c2c19c67b38b4ec36473`: compile succeeded. All four new
  chronological tests passed. Existing strict-query group 12 passed / 2 failed;
  public 18, adopted-public 4, inherited-policy 11, legacy-lifecycle 21 / 1 ignored,
  native decoder 3 and genuine historic record-context 2 passed. The driver
  correctly blocked the full suite. On unchanged source, existing similar_v2
  diagnostics passed 2 / failed 7 with explicit inherited-policy-unknown errors.
  The two prepared failures projected that underlying rejection as 503; they were
  not classified from envelope text alone. All 354 source files and 15 emitted
  executable hashes were verified before correction.
- `b0eae0adef3d24b9c5d73720b7aa566fbd7d1c51`: test-only correction after Huygens'
  exact-helper review. The older similar_fixture now actually adopts only its
  intended-positive default+other namespaces after real migration and before
  owned open. Pre-adoption None and resolve_bound rejection are asserted. The
  mismatch/unknown namespaces retain their entire encoded configuration and
  None binding, including existing exclusion/race negatives. No production guard
  was relaxed and no historical ZIP was blessed. All nine focused gates passed: 87 invocations/83 distinct tests, plus one
  existing ignored fixture generator. The first full all-targets suite passed on this unchanged source: **1368 library
  tests, 3 CLI tests and 10 example tests**, with 1 existing ignored library test.
  The library portion took 890.76s; this is correctness validation, not a performance
  measurement. Fmt passed. Clippy exited 0 with warnings: 1093 library, 1157 library
  test (1044 duplicates), 1 binary plus 1 duplicate in its test target. Not warning-free.
  All four chronological tests passed both focused and in the full run.

Candidate1 failure sources, compiler/executable provenance, raw diagnostic text
and exact failed test names remain retained. Full never ran on that candidate;
the corrected candidate has the actual first successful full run. No failure
is relabeled as a full failure or omitted. No production behavior was corrected
after the normal merge; all later changes are test code or evidence.

## Validation environment and resource ownership

Synthetic offline Docker image recalld-ci:1.94/Rust1.94.1; network none, 2 CPUs/4 GiB,
CARGO_BUILD_JOBS=2, immutable source mount, no home/credentials/socket mounts.
One private Cargo and reusable /tmp/recalld-a3-matrix-target. On the source-root
switch only this package's outputs were cleaned after 166 provenance was sealed:
242 files/1.1 GiB reported, dependencies retained. No cache clones/released-path reuse.

Raw root /tmp/recalld-query-feedback-validation stays protected until resulting
PR+recorded results, then central cleanup can discard it under the retention
override. Code/reports/Git and one reusable cache/target remain. No HOME raw archive.
All 354 source/config/data files and 15 emitted executables verified after full
checks/lints, and both source manifests match their committed Git blobs. 15 is an
emitted-artifact count, not 15 nonempty test targets. Final source manifest SHA256:
`6c0f2cc14f6b1cff2cb3a30d514266e3796bb608c68a7ac856e3cec640659952`.
Actual image:`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
No source edits occurred during frozen runs. Final container exited 0; all three
owned run/diagnostic containers are removed. Reclaimed bytes are not measured.
Compact results/manifests and the actual HOME notebook entry are recorded by
`home-copy.json`; no raw source/executable/archive copy is needed.
No further Rust tests for an evidence-only tail.

## Remaining acceptance

This closes the bounded prepared selection→native observation→public API1 feedback
chronology. It does not prove ordinary public2 activation, Remote prepared queries,
public insertion, full ID/batch/graph/replacement read-only coverage, all-namespace
exposure, complete RIF/permutation behavior, deletion/sweep races, all restore/
rollback invariants, deterministic UUID/reset traces, A3 completeness, quality/RL
or performance/default promotion. Existing156/166 and other owners' evidence stays
separate; no duplicate matrix or convenience parent is added. Huygens retains
public routing/query and experimental RIF joins; Feynman retains certification/
profile/binding proof; Hume's frozen150 false-only RIF proof is not imported here.

## Exact review handoff

Branch:`impl/query-feedback-chronology`; worktree:
`/tmp/recalld-implementation/query-feedback-chronology`.
Validated final source:`b0eae0adef3d24b9c5d73720b7aa566fbd7d1c51`; runtime
merge:`d2c705d49154c8b9284d7564f4366d2f2e3ba23a`. Subsequent publication-tail
commit contains documentation/compact evidence only. Proposed PR base166 with
additional actual151 dependency. No push/PR or READY handoff before review;
exact-head CI is required before integration.166 itself is already green and
READY/frozen under Halley's ownership; this child has not changed its branch.

Post-merge Rust changes are five test files: native_records reader/wire helpers,
adopted-public helper visibility, the new query_feedback_chronology_tests module,
and search/similar_selection_tests positive-fixture adoption. No query/proof
runtime hunk was changed after owner review. Shared-boundary verification records
exact unchanged parent files; the source correction patch and both attempts'
source/binary manifests, raw-log hashes and failure/result excerpts are adjacent.
Source plan and merge resolution notes remain in docs/implementation.
