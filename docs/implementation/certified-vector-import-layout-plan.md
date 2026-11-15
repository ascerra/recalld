# Certified vector import layout — source/format proposal, not implementation approval

Owner: Feynman. Parent join `a3c7010c50809bb231e220a7f4000dfd2802979e`
is actual159 `b7179391` +158 `9c90f820`. Main requested this review after
actual remedy runs008/009. The first unvalidated transport draft was started
before that request, then removed from the active runtime tree. It is retained
at `docs/evidence/vector-certification/placement-draft-unvalidated.patch` and
`placement-draft-unvalidated.rs.txt`; it is not the complete proposal below and
has no passing-test or publication claim. No further hint implementation until
review. The independent certifier/validator work continues.

## Reproduced need and rejected inference

Run008 passed the provider-failure immutable-source/fresh-destination negative.
Its other test completed genuine148 wrong-input re-embedding, exact actual
provider-document checks, original receipt-byte preservation, keyed replay,
native reopen and canonical-vector snapshot. Logical import then failed
`retained native record disagrees with its creation commitment`: transport
normalizes slots to zero and import allocates new slots, whereas retained typed
creation/cutover references bind the original positions. This is one failed
test, not a claimed complete positive suite.

Run009 tried reconstructing layout from historical native creation/replacement
references. Both tests failed while importing the genuine BEFORE fixture,
`linked creation retained state mismatch`. Reserving native slots displaced an
unwitnessed legacy parent whose reference was retained in peer history. That
approach is removed; no historical receipt or peer proof will be rewritten.
Both immutable run sources/logs remain at the corresponding remedy008/009 roots.

## Proposed format and exact compatibility

Prefer an explicit **logical export format2** for certified history, rather than
silently expanding format1. Header `exportFormatVersion` is already checked
before chunks in `logical_export::scan_stream`. The new reader accepts exactly
1 or2; unknown versions fail. Format1 retains its existing encoding and behavior
and must reject vector-placement fields. The exporter emits format2 when strict
validated certification summary contains a live or retired binding; otherwise
it continues emitting format1. Disk schema2, lifecycle versions, mutation
contracts, receipt hashes and logical-hash algorithm stay independent.

Format2 `LogicalMemory` adds `vectorPlacement: u32` only for an actually present
vector in a **currently certified namespace**. Serde uses default +
skip_serializing_if=None + deserialize_present: absence is allowed where
specified, explicit null is rejected, unknown fields and duplicate fields fail.
Mixed format2 exports retain ordinary namespace/record encoding with the field
absent for uncertified namespaces. Tombstones and rows with no retained vector
never have a placement. Ghost/ineligible rows with retained vectors do.

The chunk chain checksum includes placement. `LogicalHash::item` hashes the
same logical memory representation with placement absent, just as it already
excludes physical text catalog/offsets. A roundtrip must match exact prior
logical SHA and all retained f32 bits/provenance/bindings/history; this is not a
new semantic payload hash or authority. Receipt/root validation still binds
actual physical references at final reconstruction.

Compatibility probes will serialize a format1 LogicalMemory before/after the
new optional field and compare bytes, including absence rather than null, and
import the genuine tracked148 JSONL unchanged. Genuine old export readers reject
format2 at the header **before any hint handling or staging**. Separately, the
actual159 old storage binary must reject the existing namespace required feature
before writable open; the earlier sentinel probe is retained but is not proof
of a new valid certificate. A deliberate header1 copy of a format2 stream tests
old namespace-feature rejection before it reaches the first Memory hint; this
is an invalid downgrade probe, not a supported format1 export. The new parser
also rejects that downgrade. No claim that a genuine format2 old-reader error
comes from namespace decoding when its earlier header fence rejects it first.

## Complete first-pass checks before staging or vector allocation

Extend the existing full `scan_stream` prepass, which precedes destination
ownership, staging directory creation and vector-file allocation. A layout plan
is only returned after header, every chunk sequence/hash, footer chain, counts,
logical SHA and EOF succeed. No partial plan is applicable.

- Namespace IDs/names/dimensions/schema and required features retain strict
  validation. Namespace count, examined record count and encoded namespace/record
  bytes are bounded (100,000 rows each, 256MiB combined accounting); line parsing
  retains its existing16MiB limit. Explicit limit errors, never truncated success.
- Every record, including excluded/Ghost/Tombstone/unrelated rows, counts. IDs
  stay globally ordered and unique. Each record must name one decoded namespace.
- Retained vector presence, schema reference presence and payload cardinality
  agree; dimension equals namespace dimension; raw f32 bits are finite and satisfy
  the declared normalization; SHA matches the logical reference. Logical slot
  remains zero. No vector means no hint. Certified vector means exactly one hint;
  uncertified vector forbids a hint. Format1 forbids all hints and certified state.
- Detect duplicate `(namespaceID, slot)` placements before staging. Fixed-width
  vectors cannot overlap within a namespace without sharing that slot. Identical
  numeric slots in separate namespace files are valid. Duplicate record UUIDs,
  conflicting namespace declarations and duplicate/null hint fields reject.
- For each certified namespace, derive exact required extent from the **current
  export rows**, never historical creation slots. Compute maxSlot+1, stride,
  growth-chunk rounding and file-header bytes with checked arithmetic. Bound
  aggregate reconstructed vector-file capacity across all namespaces at256MiB,
  including existing allocator growth slack. Thus an extreme sparse u32 slot
  fails before file creation, even if it carries only one tiny vector.
- Include uncertified namespaces in aggregate capacity accounting using their
  current payload count and the existing allocator growth rule. Bound placement
  map cardinality at100,000; no loop or allocation proportional to an unchecked
  slot. Include the complete capacity estimate in the existing free-space check.
- Placement count is exactly current certified retained-vector payload count;
  it is NOT compared naively with the original cutover count, which can shrink
  through valid later ledger losses. Retained history/loss validation is described
  below and is never replaced by this cardinality check.

Negative probes recompute checksums where necessary to isolate each structural
failure. A separate corrupt checksum/footer probe must fail even with a valid
layout. For each rejection hash source bytes, assert destination absent or
unchanged, and assert no new import stage/vector allocation (existing owner lock
files are separately accounted for, not canonical data).

## Private reconstruction, retry and free-list behavior

Only the existing checksummed Importer consumes the completed layout. In a
certified namespace it reserves the bounded extent as zero placeholders using
the existing VectorStore allocator, preserving valid on-disk free-list/header
state at every durable boundary. An internal importer-only fill method writes
actual retained f32 bytes to the declared occupied slot. It cannot be invoked
through public Rust/transport APIs, ordinary insertion or benchmark escape.
Normal writers continue allocating new slots. No new vector backend.

Uncertified namespaces use their existing sequential import path, with no slots
reserved from native histories and no relocation of unwitnessed peers caused by
another namespace's plan. This proposal does not claim to repair all preexisting
uncertified sparse-native import limitations. The genuine mixed BEFORE fixture
must remain byte/logically compatible with its prior successful import.

After all record chunks commit, free only zero placeholders not referenced by
any current canonical record, using existing free_slot zeroing/header logic.
Live declared slots remain allocated. Holes become ordinary free-list capacity;
free-list order and the next physical slot are not promised identical to the
source. Subsequent allocation must obey current UUID/reference/allocation-epoch
ownership, and delayed frees must not release a reused live slot. Test actual
post-import insertion, deletion and reuse, not just a header count.

The existing `.recalld-importing` marker excludes ordinary opens throughout.
Existing import progress/exportID/checksum/prefix owns resumability. A repeated
unacknowledged payload write fills only this import's prevalidated slot with the
same input bytes; no unrelated live store can be targeted. Tests cover crashes
at layout reservation, payload synced before metadata chunk, chunk committed,
placeholder reclamation and final publication. No second work/generation registry.

## Authority and final proof

A hint never creates/changes certification, namespace binding, receipt, source
origin, allocation epoch or canonical G. It provides only a destination location
for already retained bytes. Before publication, existing strict canonical
validation checks every actual vector hash/slot/epoch/owner, typed creation and
cutover before/after evidence, inventory root/count, ancestry, namespace/profile/
dimension binding, and exact validated ledger losses. The new certifier validator
is shared with reopen/export/fork, not bypassed for import.

Unique but wrong placements with recomputed transport checksums may reach private
staging; final typed-reference/root validation must reject them before publication.
This distinction is deliberate: early structural/cardinality/budget checks are
not a replacement for the complete history proof. A hint alone, missing anchor,
stripped feature, fabricated certificate, omitted root row or altered f32 bits
must fail. Source remains unchanged and invalid private stage never becomes an
ordinary serving store. No arbitrary total-rewrite authenticity claim.

## Source seams and validation gate

Proposed files: `storage/logical_export.rs` (Header1/2, gated optional memory
field, logical-hash exclusion, prepass), private
`storage/logical_export/vector_layout.rs` (bounded current layout),
`storage/vectors.rs` (private reserved-slot fill only), and actual import tests.
Existing certification/offline validation and creation/cutover receipt bytes
remain authoritative. No main/integrator branch, system.rs, API activation or
Curie producer-demand hunk changes.

Before publication: genuine old serialization/import proof, actual old reader
header/feature fence, mixed certified+uncertified namespaces, duplicate/overlap,
sparse extreme, missing/extraneous/null hints, payload/ref mismatch, complete
checksum rejection with no stage, exact logical/f32/history roundtrip, foreign
fork and post-loss/purge proof, interruption/replay and subsequent slot reuse.
Only then appropriate full/fmt/clippy checks on the final source. Current008/009
are failed candidates and remain explicitly distinguished from future results.
