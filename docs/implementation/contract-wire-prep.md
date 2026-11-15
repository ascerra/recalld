# A4 wire preparation, v2 still unavailable

This child uses PR69 `eefc987` (canonical label readers) and PR13 `1b23532`
(modern structured MCP results). It does not take unrelated integration history,
new native writers, catalog policy changes, or a work tracker as a prerequisite.
The local prep commit is intended to fold into the first actual public contract
consumer, not require an independent PR or an unnecessary stack for unused types.

## Actual transport behavior

MCP tool arguments accept an optional integer `contractVersion`. Omission or 1
uses the existing v1 handler. Every published tool schema lists only version 1.
The generic dispatcher and direct bridge dispatch reject explicit 2 and malformed
selectors before handler/provider/storage work. Missing `arguments` still defaults
to `{}`; explicit null remains invalid. Transport protocol negotiation is separate.

The REST router accepts omission or exactly `Recalld-Contract-Version: 1`.
Version 2, other values, and duplicate version headers return the existing HTTP
400 error shape before body extraction or handler work. This header selects the
REST application contract; MCP-over-HTTP uses the MCP argument and dispatcher.

V1 successful payload shapes and unknown-field tolerance remain unchanged.
Legacy MCP eras retain text-only results. Modern MCP includes identical parsed
`structuredContent` for object payloads; arrays/scalars remain text-only as in
PR13. Existing entity/topic/emotion arrays and optional canonical labels retain
PR69 behavior. No result acquires a v2 envelope or synthesized read generation.

## Pure helpers, not active writer or read APIs

`serialization::contract` contains strict version selection, unknown-field JSON
Pointers, key/header agreement and lineage-option parsing, and envelope/error/
readiness serialization types. Decimal counters reuse A1 `Generation`, which is
a canonical string in 0..9223372036854775807. Retry options reuse A1 context
validation: keys are 1–128 printable ASCII bytes; key/CAS requires explicit
non-nil storeId. Parsing preserves caller input, key bytes, and array order;
it does not reserve receipts, sample time, generate IDs, or compute request hashes.

These helpers are not wired into v1 mutations. No public key/CAS support, retry
guarantee, version-2 output schema, capability resource, or discovery readiness
is introduced. Typed envelopes are serializer fixtures only until actual backend
interfaces can populate them. A value typed as GenerationRef is not proof that
selection and hydration used a coherent view.

Batch classification follows CONTRACTS: committed-but-unready counts as success;
all ready is 200/ok, mixed or committed-unready is 207/partial, all final failures
is 422/error, and zero successes with unresolved items is 503/error. This is
classification only, not a manifest/executor or a claim of atomic batching.
No aggregate committed generation is constructed.

## Owned dependencies for activation

Feynman (A1) owns the eventual coherent ReadView after guard/offline work. His
interface is not frozen. It must pin canonical lineage/generation, namespace,
profile/lifecycle and required vector/FTS/graph views through selection and
hydration with real derived watermarks. Halley owns the accepted-work/freeze
consumer (PR73 `05cd254`). Neither draft is a parent of this preparation.
WorkTracker IDs, effects epochs, latest control counters and
MetadataFreeze are not substitutes for that ReadView. Cursor/backend children
will consume a frozen actual API, not an invented or mock-only generation.

Curie's PR72 `e51cb67` (frozen runtime `b7c1ddc`) provides `mutate_batch(BatchInput)` and
`BatchOutcome`, with ordered indexed outcomes, parent/child receipts and unknown
bookkeeping reconciliation. A later minimal wire child must adapt that API and
its actual transport tests, including MCP isError unless every item is ready.
Pascal owns native insertion/tombstone and lifecycle writers. Actual native
acceptance must be tested end to end with those writers and the all-writer
barrier before activation. The separate native UTF8 join must prove real
write/publication/read/reopen and exact provenance, including malformed-proof
rejection; PR69's authored reader fixtures do not provide that evidence.

Remaining A4 scope includes actual keyed-write adaptation, strict per-operation
v2 schemas/mode unions, native caller profile selectors, coherent-generation
pagination and exact pair coverage, readiness/error adaptation, and truthful
session discovery/output schemas. This preparation does not complete A4.

## Next actual duplicate-pair consumer (not a generic list cursor)

The minimal child should take this prep content plus Feynman's frozen canonical/
vector snapshot and post-comparison validation, and only their real prerequisites.
This scan-only snapshot is not a complete query ReadView. Hume confirmed the existing `search::dot_product_simd` kernel is already the
flat-index exact baseline, so no A5 benchmark parent is required. Inputs must
be normalized, profile-compatible, finite and same-dimensional before scoring;
the kernel does not validate them. Pair enumeration uses UUID lexical order,
normalizes signed zero, and does not group near scores with an epsilon.
Determinism is for the same build/SIMD target, not cross-architecture bit identity. Neither batch PR72
nor work-tracker PR73 is an independent parent merely for pagination. The
required canonical/vector writer guards belong in the actual snapshot dependency. No child starts
cursor/backend implementation against a synthetic generation API.

Current MCP scans sample IDs and use ten-neighbor clustering; REST scans sampled
pairs. Keep those v1 result shapes and non-exhaustive coverage. Add one shared
exact comparator for v2 MCP scan and REST duplicates, consuming the actual
ReadView. Resolve namespace/profile once, enumerate canonical live non-tombstone
records (including valid Ghost vectors), validate profile/vector eligibility,
record exclusions and phase counts, and sort UUIDs. Compare lexicographic i<j
pairs directly with exact cosine, never nearest-neighbor top-k. Checked total
pairs is n*(n-1)/2. Each page spends at most pairBudget (default 10000,
range 1..100000), including comparisons that produce no edge.

Return pair edges plus exact aggregate coverage, not page-local clusters.
A cursor binds version, lineage, immutable namespace ID, generation, profile,
threshold, eligibility policy and next pair position. Validate encoding, request
bindings and positions before comparing. A changed generation during a page
invalidates the entire page; a changed generation between pages or after restart
returns CURSOR_STALE. Unrelated writes also invalidate it. Terminal complete is
true only after all pairs and zero excluded namespace records; exclusions end
with null cursor, false complete and a warning. No provider call or access write
is needed. List pagination remains a separate UUID/filter-bound consumer.

Required actual transport oracles: at least 201 records with matching pairs
across the former sample/page boundary; dense groups larger than ten neighbors;
tied scores and disconnected components; budget 1 and multi-page union exactly
equal to the exhaustive unordered-pair oracle with no repeats or gaps. Cover
empty/singleton scopes, all-nonmatching pages that still advance, Ghost versus
tombstone/foreign namespace, missing/invalid/profile-mismatched vectors and
checked overflow. Restart must continue unchanged-generation scans and reject
changed lineage/generation/profile/threshold/scope/position bindings. Inject a
write during comparison and assert no partial page escapes as valid. These
must use an actual published native store and coherent read API, not PR69's
authored reader fixtures or a metadata-only freeze.

Separate actual consumers still required: strict per-operation unknown-field,
null/type/range and single/scan union schemas; versioned public profile selectors
and keyed writes using native/Curie APIs; output schemas and envelope/error/
readiness adaptation; truthful session discovery with a brief tool guide,
resolved default provenance and credential-free active profile. Activation waits
for their real transport/native acceptance tests and all-writer readiness.

### Approved narrow snapshot plan, API not yet frozen

The user approved Feynman's proposed `CanonicalVectorSnapshot` for the exact-pair
consumer alone: one blocking capture holds storage/publication guards and one
metadata read transaction while resolving the actual GenerationRef, namespace
schema and ordered records and copying their vectors. It returns owned rows with
Available/Missing/Invalid vector states. Record/byte bounds must fail explicitly,
not truncate. This can supply the scan's coherent canonical/vector needs without
claiming a coherent FTS/graph/query ReadView. Its actual implementation, all
participating writer guards and native acceptance are still prerequisites.

A4 additionally requires a guarded generation/policy/lineage check after comparison:
checking the expected generation only during capture would miss writes during
page computation. Such a change must discard the whole page as CURSOR_STALE.
Typed missing/invalid proof states must not hide I/O/corruption failures. Exact
native profile binding, phases, stable UUID ordering, overflow and restart
continuation need real storage tests. The proposed API is not yet frozen or
consumed by this prep; no mock snapshot or latest-counter substitution is added.

### Consumer decision and prep validation

The user chose the intended MCP/REST consumer, not a new offline CLI. Implement
comparator/cursor, strict operation schemas and session/tool guidance together
with the actual canonical/vector backend, keeping activation closed until the
native owner and schema-serving path are correct. Pure exhaustive-oracle and
legacy compatibility tests may proceed now; actual native transport acceptance
waits for Pascal's writer/serving join. There will be one actual consumer PR
after joined proof, not a separately published unused-helper dependency.

Prep validation passed eight focused wire tests and the original omitted-tool-
arguments regression, then 833 library and two CLI integration tests under
normal parallel execution (214.88 seconds for the library suite). Four doctests
were ignored. Formatting passed; Clippy exited 0 with 431 library and 455 test
warnings (431 duplicates). Docker used network none, Rust1.94.1, 2 CPUs/4GiB,
two Cargo jobs and this agent's private cache. The private target was cleaned
for package recalld before switching source roots. Per-source executable hashes
and initial failure logs are preserved in the accompanying evidence.
