# Native document preprocessing and original request context

Status: local validation sealed for publication review; exact-head full CI is
still required. No final full-suite PASS is claimed.
Final test source is67ca18e27b0ce5a1ec75d1708467e10c45a7af1b. Production code is
unchanged from the full-run source f6b0b1649e7443d1b3c6b2018a98560a47d00554. Actual dependencies are PR141
605f172748c0986803c0a0b72ebfba1ec2a3bee8, PR131
974421582f56641222f78ba18f8c652e93d5b352 and PR134
b62d1e2f4b3f516b5889946605946e274f82286c. PR123 is already an ancestor of PR141;
no moving wire work, PR138 or broad checkpoint is required. The original API
consumer and focused proof are described in [the implementation note](../../implementation/native-insertion-origin.md).

## Actual failure and correction

The declared `recalld-document-v1` recipe is summary, optional two newlines and
full text, then optional space and joined merged tags. Raw tags retain their
order and duplicates. Typed entity/topic/emotion labels append in that order,
using existing prefixes and lowercase conversion; a value already present in
the accumulated tag list is not appended again. Provider document mode and
literal provider prefix handling are unchanged.

Actual test-only source d9e629c4b875b40baea55ec9d642a7fd80372991 produced **2 failures,
0 passes in4.53s**. Both actual `store_native_single_by_profile_id` and ordered
`run_batch` committed successfully and then the provider-spy assertion failed:

```text
actual:   body
expected: header\n\nbody raw entity/alice raw entity/bob topic/rust emotion/calm
```

This is a provider-input/profile conformance bug, not a reported bit-corruption
or data-loss failure. The old native producer selected full text or summary and
omitted the other declared components. The profile already advertised document-v1.

Runtime16c1cb163db807d70413f3e5d65fbb569eb790bd adds one pure
`NativeInsert::document_v1` helper. All four fresh generated paths consume it:
owned single same-insert producer, linked batch, private plain batch, and explicit
offline `insert_generated`. Every path retains its matching-replay check ahead
of composition/provider work. Caller input, supplied-vector bits, request hashes,
receipt encodings, generated outputs in historical records and proof versions
are not rewritten. No new profile ID or provenance field is invented.

Focused validation at16c1cb1: **6 passes in5.49s**, exit0; five actual tests plus
an intentional no-op fixture-export entry when its control environment is absent.
The two original probes become green, a genuine old2 single/batch export replays
with exact receipts and unchanged raw receipt rows/G and zero provider/profile/
clock calls, plain batch2 with explicitly disabled unsupported auxiliaries uses
the same recipe, and offline plain API1/API2 generated insertion uses the recipe.
Online linked API1 and originating batch API1 are covered by the later focused
consumer proof below; this earlier six-test result alone did not cover them.

## Genuine historical fixture

Test-only source2550da536db05c1c430d45b506b728fbb52c302f uses unchanged old producer
runtime and passed its real exporter in2.14s. The provider spy explicitly saw
`body` twice. Its139,690-byte synthetic JSONL/receipt fixture is bundled under
`src/storage/metadata/native_records/fixtures/document-recipe-legacy*` so normal
CI performs the old-receipt replay test. These are test inputs, not a raw archive.
The fixture and source-specific executable hashes are recorded in MANIFEST.json.

## Important activation and history boundary

Older experimental native generated vectors are **not certified for document-v1
recipe conformance or retrieval quality**. This correction affects fresh provider
inputs only. It does not repair, re-embed, relabel, silently bless or rewrite
historical vectors/receipts. Personal legacy data and the personal daemon are
untouched. A valid vector commitment proves stored bytes, not the previously
omitted provider-input composition. Native activation requires this validated
recipe companion alongside actual writer/consumer closure. No ordinary-native,
public2, full-storeCAS or LoCoMo closure is claimed here.

## Validation environment and retention

Isolated Docker Rust1.94.1/recalld-ci:1.94, networknone,2CPUs/4GiB/jobs2, no HOME,
credential or socket mounts. One existing private Cargo/target was renamed to
`/tmp/recalld-curie-active-build` and reused. Only this crate was cleaned before
switching source roots; BEFORE and subsequent source executable hashes were
captured separately. Source stays frozen during each run. No new raw HOME
archive. After completed PR and recorded results, central cleanup may remove
released raw runs; the genuine fixture and compact reports stay tracked.


## Typed numeric boundary follow-up

Source0f1fc692514c690abea81102c59aff765e3fe52d distinguishes
`CallerVectorInvalid` in the shared normalization helper while keeping the old
legacy diagnostic text. Existing legacy HTTP422, MCPInvalidInput and indexed
batchINVALID_INPUT mappings are preserved; provider invalid remains503 and
canonical ownership/control corruption remains500. Six focused cases passed
across five filters (1+2+1+1+1,0.51/0.24/1.50/1.66/0.00s), exit0. Actual insertion
and selection tests assert no mutation, retained original float-bit identity,
and exact numeric error type. The final native single wire adds only the typed /embedding422 mapping,
proved against actual HTTP/MCP below. Generic InvalidMutation is not remapped.

## Original API consumer and typed wire baseline

Test-only joined source12b33ce9eaa828ec45475c382009f08e1555a660 produced **0 passes,
2 failures in1.59s**. Actual linked API1 single creation hit the old native wire
API2 guard. Actual HTTP with a valid-norm supplied vector of the wrong canonical
width returned503 STORAGE_UNAVAILABLE instead of caller422. The failure log is
retained unchanged during development, with a compact excerpt and executable
hash in this evidence package. This is a new actual baseline, separate from the
earlier document recipe failures.

Runtime90765cf665d04f0616e9951cf78cb33930c267ec accepts the existing closed
MutationContext1|2 on native linked single entry, retaining namespace/profile/
owner guards. It threads the common RetainedRequestContext through the actual
ordered runner, parent manifest and reserved child. Native codec2/3 absent context
continues historical API2; legacy codec1 absence continues historical API1.
Unsupported versions, explicit null, redundant Some2, foreign/stripped context
and altered reservation CAS reject. Context is checked against the immutable
parent request hash even when no child is valid. Child identity is reloaded and
checked in the final canonical transaction. API1 and API2 using the same key
conflict; matching original replay precedes current settings/provider/clock/CAS.

Nine focused tests passed at90765cf in27.40s. Test-only follow-up
f6b0b1649e7443d1b3c6b2018a98560a47d00554 extends the actual unknown-commit and
accepted-descendant cases to API1 and exercises API1 reserved forget. Its focused
run passed **11 origin tests in35.59s**, **6 recipe tests in5.85s**, and **1 genuine
historical batch compatibility test in2.86s**. The last test verifies real old
private formats1/2/3 request, manifest and receipt byte encodings, reopen and
replay. The recipe six include the intentionally inactive export entry; the
bundled historical replay executes normally. These are synthetic process tests,
not power-loss proof or retrieval quality evidence.

The API1 publication-failure tests compare the full committed receipt and recover
publication without another provider invocation. Unknown child identity stays
separate from the parent, remains unresolved until reopen, and replays its
reserved operation. Process exits around child commit preserve one identity and
original reservation CAS. API1 logical export/import and retained restore preserve
historical receipts/context; a new fork rejects the old storeId before replay.
The all-invalid manifest test proves rejection of stripped context before there
is any successful child receipt to anchor it.

The first all-features library run atf6b0b164 finished **1,246 passed, 1 failed,
1 ignored in820.07s**, exit101. Its sole failing test was inherited
`native_single_profile_id_invalid_pair_or_binding_has_no_provider_or_mutation`:
it expected generic InvalidMutation for the zero-norm and wrong-width cases,
while the approved runtime now returns CallerVectorInvalid. The first failing
assertion printed `CallerVectorInvalid("embedding must have a finite positive norm")`.
The full failure log remains unchanged; full-first-excerpt.txt preserves the
failure and exact count. No production correction or weaker assertion was needed.

Commit67ca18e27b0ce5a1ec75d1708467e10c45a7af1b adds only five test lines to assert
CallerVectorInvalid specifically for those two numeric cases. Invalid pair and
profile mismatch cases retain their earlier specific checks. The exact failed
test now passes **1/1 in1.65s**. Per the user's explicit validation direction,
there is no automatic second local full run for this test-only delta. Final
origin/HTTP, recipe and CLI checks plus format/Clippy run on67ca18e; full exact-head
CI remains required after publication. No final full-suite PASS is claimed.
No public parser/API1 outward projection, ordinary native startup, legacy-disk
API2 insertion/compound, whole-store CAS or LoCoMo ingestion closure is claimed.
The public API1 projection remains the wire owner's dependent consumer.

## Local composition notes

The actual parents were joined only in the assigned implementation branch. The
initial record/profile join retained the strict131 control, lifecycle migration,
purge and startup validation while adding actual linked/batch modules. The final
141 join kept its wire/profile-ID cases and module layout, retained131's original
request-context tests, and removed only a duplicate NativeMemoryInsertedWithLinks
alternative in the missing-effects-schema match. Its remaining alternative still
rejects that receipt without effects schema. The common134 consumer merged without
a source conflict. No shared-codec edits, broad checkpoint or public activation
were introduced. Ancestor declaration/match ordering may differ from141 without
changing canonical encodings; genuine historical fixture tests check the bytes.

## Final local validation seal

Final test source67ca18e27b0ce5a1ec75d1708467e10c45a7af1b passed the exact failed
profile test (1,1.65s), origin/actual HTTP422/provider0/fault/accepted-work suite
(11,31.88s), recipe suite (6,5.15s, including one inactive exporter entry), and
both CLI integration tests (1 each,2.46s and1.49s). Format passed. Clippy finished
in8.08s with warnings, exit0. The final targeted Docker command exited0.

The earlier full run remains1246pass/1fail/1ignored atf6b0b164. There are no
production changes after it; the final delta is the five-line exact typed test
assertion. Per user direction, exact-head CI must run the complete suite after
publication. This package does not claim that pending CI has passed.

VALIDATION.json maps every source, command, result and per-source executable
hash. SOURCE-MANIFEST.json seals314 final inputs; SOURCE-MANIFEST-FULL.json seals
the original full-run inputs. All314 inputs were rehashed unchanged after final
targeted validation. The genuine fixture manifest remains MANIFEST.json. All
owned validation containers use --rm; no raw HOME archive or cache clone was
created. Current raw run remains protected until publication/results handoff to
central cleanup; the sole cache is retained for the next assigned writer task.
