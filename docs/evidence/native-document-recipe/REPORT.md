# Native document preprocessing and original request context

Status: focused evidence, NOT READY for publication. The final original-context
consumer still needs the actual validated insertion-wire parent and PR134.
Current preparation uses published PR123 d7c4d5f plus PR1319744215 joined locally
at915a60a. It does not consume moving wire work or PR138.

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
Online linked API1 and originating batch API1 are still pending the context work.

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
and exact numeric error type. The native single wire's specific /embedding422
mapping is still required after the actual PR141 join; no blanket generic
InvalidMutation mapping is proposed.
