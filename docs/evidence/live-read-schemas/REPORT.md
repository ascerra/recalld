# Live list, recall and similarity output schemas

Local validation complete; exact-head CI and final integration remain separate gates.

Frozen runtime/test source: `fb1278232df0581f777bef8ed50af9350a83c4b1`.
Sole parent: PR190 `295fdac03b6bdf93d59c2dd68876c18393b5086f`.
Owner Huygens. Halley released this child and source-reviewed the limited
schema/test changes before validation. Hume reviewed only the similar-tool
scope description and its existing actual compatible-profile control.
Coordinator approval is under the user's standing implementation authorization.

## Results

- Full all-feature suite: **1766 library +10 CLI PASS**, zero failures; six
  historical library probes and four doctests ignored. This does not claim
  example-test execution. Library duration2080.65s is a bounded-resource
  validation duration, not a product performance measurement.
- Eight focused Rust tests passed. Formal47 read outputs/specimens and13 metadata
  outputs passed, including mutation negatives. The final full-run fixtures
  passed the same validators; these repeated cases are not added together.
- Format and all-feature/all-target Clippy exited0 with warnings. All492 source
  inputs (17,823,957bytes) matched after execution. All19 pinned executable paths
  matched before/after; focused/full library hash is identical.
- Full-run cgroup peak4,294,967,296B, max22976, OOM/kill0: reclaim pressure, not OOM.
  No failed test/compile run occurred on this child.

See [RESULTS.json](RESULTS.json), [full-summary.log](full-summary.log),
[source](SOURCE.json), [source verification](SOURCE-AFTER.json),
[executable verification](EXECUTABLES-AFTER.json), and [container](CONTAINER.json).
All13 regenerated metadata fixtures are byte-identical to PR190
([comparison](METADATA-FIXTURE-PARITY.json)); its existing tracked fixtures
are reused rather than duplicated. New47 final output cases are retained in
[read-fixtures](read-fixtures). [RAW-MANIFEST.json](RAW-MANIFEST.json) pins
complete raw logs; compact evidence does not claim they are reproduced in full.

## Behavior and limits

Modern tools/list now advertises schemas for list_memories, recall_memories and
both modes of find_similar_memories, in addition to the two metadata tools from
PR190. Compact recall remains the default. Its schema and the full-result schema
use anyOf because both emit the same valid empty result. Optional arrays omitted
by their actual serializers are nonempty when present; canonical labels retain
all three arrays, including authoritative empty arrays, and exact snake_case
provenance. No handler, ranking, clock, observation, provider or serializer path
changed. The sameNamespace description now states the actual compatible-profile
cross-namespace behavior of explicit false, without changing v1 semantics.

List counts and offsets remain numeric; arrays/limit use the actual list cap.
Search scores/weights do not acquire invented unit-interval restrictions.
Duplicate entries remain id/summary only. Known scan coverage has actual counts;
old/custom bridges have null counts/truncation. Both remain complete:false, with
the existing 200-row sample and 11-candidate cap. This schema work does not turn
the sampled scan into the exact-pair cursor or establish an A5 performance gate.
Schemas describe shapes, not cross-field equality or generation consistency.

Legacy MCP still omits outputSchema and structuredContent. list_namespaces
remains a bare JSON array in text. The nine other tools still have no advertised
object outputSchema; eight need their actual complete success/error unions, and
list_namespaces deliberately remains an array. Plain-text errors have no
structured object. Prepared API2 admission remains closed. No O1/get, N1/catalog,
writer or other convenience parent is included.

## Focused proof

The frozen source passed three new read-schema tests, three original metadata
schema tests, the existing real sameNamespace:false compatible-profile regression
and the existing session-context test. These are eight Rust tests, not sixty.
The offline formal validators separately accepted 47 new emitted read outputs/
specimens and the original 13 metadata outputs, then rejected their strict
required/unknown/nested/type/count/provenance/coverage/cap mutations.

The actual legacy-owner tests use real storage, query/vector/graph adapters and a
fixed 4D test provider. They cover empty/nonempty lists, offsets and maximum limit,
tombstone exclusion, provider-free list calls, compact/full recall, actual graph
context, single/scan similarity, both MCP eras, recordAccess:false record/control
state equality, and a real 201-row tied-vector truncated scan. They do not claim
whole-directory immutability or new true-access observation acceptance.

Unicode, both label provenances, absent labels and authoritative empty labels
are explicitly authored typed DTO specimens. They pass through the real HTTP
MCP handler/ToolCallResult serializers, paired with schemas read from tools/list.
Old-bridge unknown coverage uses the actual trait's default fallback over a
labeled stub. These are not native writer-to-read/reopen/serving proofs, and no
schema rows were injected into a legacy store. Existing native gates remain.

Focused executable SHA256:
`98fc7f776d4d5cf5532e5dc223864db0f05c6687cb9ec754c254a1579d3ee5a2`.
All492 source inputs were verified before and after the full run. Focused
cgroup max31742, OOM0/kill0 describes reclaim pressure, not an OOM.

## Reproduction

Rust1.94.1 in recalld-ci:1.94; isolated offline Docker, 2CPU/4GiB/jobs1,
read-only immutable source, one private Cargo cache/target, no home/credentials,
sockets or personal data. Own package was cleaned before the source switch.
No source switch occurs between focus, formal, lint and full validation.

Focused cargo test --locked --offline --all-features --lib filters:
live_read_schemas_; live_metadata_schemas_;
similar_v2_actual_legacy_false_flag_uses_bound_scope_without_current_rewrite;
session_context_resolves_each_bridge. Formal scripts:
tests/live_read_schema_conformance.py and tests/live_tool_schema_conformance.py,
using their separate emitted fixture directories and an offline test-only
jsonschema installation. Full command: cargo test --locked --offline
--all-features. Lint: cargo fmt --all --check; cargo clippy --locked --offline
--all-features --all-targets. A preceding --no-run records executable hashes;
it is not another test execution or an all-target example-test claim.

Scratch remains protected pending publication/results handoff to central cleanup:
/tmp/recalld-huygens-active-validation/live-read-schemas-001-fb127823.
The sole target and Cargo cache remain active. No deletion or recovered bytes
are claimed. Completed PR190 raw roots were separately released to central
cleanup; this child neither accesses nor depends on them.

PR190 subsequently merged as32ab3b3f; publication can target main without
changing this locally validated source or adding unrelated parent history.
Only report/notebook evidence may follow the frozen runtime/test commit.
