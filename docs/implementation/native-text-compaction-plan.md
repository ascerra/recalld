# Native text compaction: proposed proof and consumer boundary

Plan only, 2026-11-14. Actual implementation parent is PR138
`05e0d55aa3a92483e294b2a5e79cf490ce270609`. No source changes or executions
are claimed here. Feynman owns this slice. The controls helper owns the shared
`system.rs` initializer extraction and private native fixture ingress; this slice
will not edit that file. Pascal's legacy-disk API2 lifecycle/forget work and
Curie's graph/linked/batch original-context work remain separate.

## Source-confirmed boundaries

The existing `engine/text_compaction.rs::compact_text_at_with_work` takes a
legacy-only metadata mutation guard. Merely selecting a native guard is unsafe:
`lifecycle_migration::validate_policy_history` rejects unrecognized operations
after schema publication, including the existing TextGenerationSelected outcome.
The same engine also discards the successful selection receipt and can return
bare I/O errors at Selected/Published after a durable canonical commit.

Physical layout is not stable across logical import or production restore.
`metadata/restoration.rs` replaces the text catalog with a freshly prepared
selected descriptor. Thus a historical compaction descriptor must never be
compared blindly to the current catalog or used to select/clean up a file.
Foreign proof digests also cannot simply be accepted as self-consistent: after
fork, validating only equal before/after digests would miss a changed historical
proof. The plan explicitly addresses both edges below.

## Proposed receipt and transaction proof

Keep the existing legacy TextGenerationSelected outcome and compactText request
encoding unchanged, including omission of any native extension. Do not rewrite
old receipts or add explicit null/default fields. For new native selections use
one required typed outcome, NativeTextGenerationSelected, carrying a version1
NativeTextRelocation proof. This is safer than an optional native proof whose
absence could silently select a legacy interpretation. Unknown versions and
missing required proof fields fail closed in old/new readers.

Proposed fixed-size proof fields (no per-record payload or growing witness list):

- proof_version=1 and source GenerationRef at the precommit G;
- exact before and after TextGeneration descriptors (generation UUID, owner,
  safe relative filename, format2, sealed prefix length and SHA-256);
- separate before/after reference-map digests over every retained
  `(memory UUID, file offset, byte length)` tuple in canonical UUID order;
- total canonical record count and retained text-reference count;
- one canonical record-state digest shared by before and after;
- one retained text-copy digest shared by before and after.

The source ref must use the receipt's original storeID, and receiptG must equal
sourceG+1. The two text-generation UUIDs must differ; both descriptors are owned
by that original lineage and have their exact generated safe names. The after
UUID equals the outcome's selected UUID, and the transaction checks it is unused.
Both descriptor/receipt format and count fields must agree. No lifecycle version,
policy revision, record generation, timestamps, provenance, vector/label/edge
fields, or namespace fields change for physical relocation.

The record-state digest uses the existing native ProofRecord/NativeRecordState
representation, sorted by immutable namespace ID and memory UUID, including all
rows (terminal rows too). It excludes physical vector slots and text offsets.
Existing content/profile/label/history commitments and lifecycle tokens remain
included. Legacy-inferred text provenance remains unknown; calculating a copy
checksum does not bless old content as a native creation observation.

The text-copy digest covers the ordered retained `(memory UUID, byte length,
SHA-256 of copied bytes)` stream. Compare before/after bytes while both files
are available. This is a physical-copy measurement, not new logical provenance
or a promise to reconstruct erased text later. No text/vector bytes enter a
receipt. File checksums, known native creation payload commitments, record IDs,
and complete reference coverage are checked before selection.

Preserve the fixed internal compactText request domain with original internal
contract1, catalog scope and expected sourceG, independent of physical format
and the outer manual request's API contract. The required new outcome versions
the relocation proof, not the semantic request. Generated descriptors, UUIDs and
copy results are outcomes, not request inputs. The existing operation has no
client key or public API selector; do not infer an API contract from disk mode.
Validate this closed request hash using the receipt's original sourceG, never
current forkG. Also validate the known historical legacy compactText /
registerLegacyText hash domains against their original previousG. Hash checking
alone cannot distinguish old and new outcomes sharing compactText: the strict
current-origin schema/history walk must reject a legacy outcome after native
schema publication, and the foreign receipt anchor below must detect changing
or removing a native outcome. Genuine old bytes are a compatibility gate.

The engine retains its exclusive write gate through preparation, metadata
selection, handle installation and cleanup. The metadata helper chooses its
existing native/legacy guard from the immutable owned handle, checks that guard,
then independently checks actual control/schema/lineage inside the final TX.
It checks sourceG, the exact old selected descriptor, all old references and
native row identity/tokens before changing any offset. Compute and compare the
before/after reference-map digests against the actual TX rows and prepared-file
mapping; require complete UUID coverage and unchanged lengths. Historical maps
are proof commitments, never authority to read a later repacked file.
All relocated offsets,
new descriptor, active selection, G advancement and the full receipt commit in
one redb TX. Recompute normalized record-state equality in that transaction.
No descriptor/table allowlist substitutes for these checks. No cross-file atomic
claim: file+directory sync precede the one authoritative metadata selection.

## Strict history, import and fork validation

For the current origin, the existing reverse native-history walk checks the
record count/digest at the compaction's exact position. Relocation is an identity
transition on logical records; do not increment or rewind their tokens for it.
Current selected-file/header/sealed-prefix/extents validation remains independent
and authoritative. Imported physical layout may differ while logical records,
receipts and provenance remain identical. Historical descriptors never select
current payloads or justify deleting a file.

For foreign origins, propose a small explicit restore-lineage version2 extension
rather than accepting unverifiable equal digests or keeping erased payloads:

- After both restore inputs have passed their existing strict source validation,
  compute a domain-separated streaming digest/count over the exact retained
  foreign NativeTextGenerationSelected stored receipt rows in canonical key
  order. Include the scoped key and original stored receipt bytes. This uses
  only receipts actually retained by the existing restore merge, not all target
  receipts and not newly invented/rebound origin IDs.
- Put that fixed-size digest/count in a required versioned lineage anchor in the
  same private restore transaction. control.restore_lineage_version and the
  lineage row version agree at2. A version2 lineage cannot omit/null the anchor.
  Version1 remains unchanged for restores without native relocation history.
  The presence of any retained foreign native relocation receipt independently
  requires version2: changing both lineage markers to1 cannot grandfather that
  receipt. Pascal's bounded source review confirmed this required condition;
  it is not yet runtime validation or proof of a completed implementation.
- Strict readers recompute this root/count from the current retained foreign
  receipt rows and compare it to the anchor, in addition to original scope,
  request hash, sourceG+1, descriptor ownership and bounded ancestor checks.
  Current-origin compactions do not change this foreign root. A later fork first
  validates the old root, then anchors the newly retained set under the new
  lineage. Logical export/import preserves the anchor and all original receipts.
- Same-origin import uses the logical reverse proof after materializing its new
  layout. It does not manufacture a restore anchor or change lineage/G merely
  because offsets/file UUIDs changed.

This extension needs review at the actual restore/lineage seam before coding.
It is constant-size metadata rather than one witness per memory or a new raw
payload history table. Existing version1/None bytes remain unchanged. New
version2 markers make stripping the required anchor fail closed. Older readers
reject the new native outcome/required lineage version before writable open.
An equal-pair forged digest in a foreign proof must fail its anchored receipt
root; rebinding to forkG/current storeID must fail original hash/origin checks.
The anchor does not claim authenticity against arbitrary rewriting of every
canonical trust anchor, nor forensic erasure. It preserves the validated source
proof through the actual restore boundary without retaining deleted payloads.
The omission test removes foreign receipt rows while retaining the authoritative
v2 anchor: its count/root must fail. Marker downgrade with a native outcome still
present and removal of a required v2 anchor also fail. Removing every new native
outcome, both version markers and the anchor is total trust-anchor removal,
outside this guarantee. Do not add a restore-G1 receipt or per-origin coverage
framework for that attack. Flattened ancestors do not prove native ordering or
baseline1; no such inference is made. This boundary was clarified by the
coordinating agent before implementation.

No unknown table/variant is accepted merely to compose histories. Every logical
export/import, backup, strict read/current reader and production restore path
must understand this exact proof/version or reject immutably before publication.
The version2 reader must not weaken version1's existing serving/historical,
ledger ancestry, bounded source generations, or schema proof checks.

## Receipt, allocation and readiness behavior

Capture the exact WriteReceipt returned by the canonical selection TX. Return
it with CompactionResult and wrap every subsequent Selected/Published/publication
I/O failure as CommittedMutation using that in-hand value. Do not look it up or
reconstruct it after a poisoned handle. An uncertain TX acknowledgment remains
CommitOutcomeUnknown with its allocated operation ID, never a guessed receipt.
Before selection, failures remain uncommitted. Cleanup failures retain success
and receipt with explicit cleanup-pending reporting; reopen retries safe orphan
cleanup after validating the actual selected generation.

New attempt: one owned TextGeneration UUID before preparation and one owned
Operation UUID at receipt staging. Abandoned reservations are not rewound.
No clock or provider is sampled. Empty and zero-removal selections still commit
exactly one canonical G and receipt. No record/policy generation churn occurs.

Canonical commit invalidates the actual four-component publication registry.
The engine result proves commit, not derived readiness. Thread the exact receipt
through the actual compaction caller and its DerivedCompletion publication,
including the sweep result/error's retained receipt. The present sweep discards
the successful CompactionResult; merely adding a backend field is insufficient.
Real existing completion must publish matching actualG before reads/barriers
report ready. Injected
publication failure keeps the original compaction receipt and stale/unavailable
readiness. Worker-held ownership and accepted-work tracking must span the whole
operation; cancellation cannot finish a worker still selecting/publishing.

The existing engine compaction entry is the real consumer, not a new unused
helper. Native automatic sweep currently deliberately skips legacy compaction;
its actual135 consumer join must wire this validated path and truthful
compaction_triggered reporting before ordinary startup is admitted. That join
is an explicit remaining consumer, not permission to open ordinary mode now.

## Deterministic BEFORE and acceptance tests

1. On actual138 plus tests only, reproduce native guard rejection; a narrowly
   controlled fixture using the existing native guard and old selection helper
   demonstrates strict post-schema history rejection. Preserve source/executable
   hashes and distinguish that deliberate fixture bypass from supported runtime.
2. On that same baseline, Selected and Published failure probes demand exact
   in-hand committed receipt and fail on the current bare error. Unknown TX
   failure remains a separate no-guessed-receipt assertion.
3. Native fixture includes migrated legacy rows with unknown provenance, native
   rows with exact structured labels/profile/vector commitments, every retained
   text phase, terminal rows, unreachable garbage, and an empty-store case.
   Assert logical record/export items, f32 bits, every lifecycle/policy token and
   original receipt bytes unchanged; only selected layout/new receipt/G change.
4. Check exact owned ID draws/no clock/provider, closed/stale/purging/unknown
   rejection before preparation, repeated zero-removal compaction, and all
   existing durable file/dir/metadata/handle/cleanup interruption boundaries.
   Reopen twice, retain old before selection/new after selection, reject corrupt
   selected payload immutably, and never select an orphan or revive stripped text.
5. Real export/import and backup/production fork preserve logical payload,
   bindings/provenance, receipts/ledger and new anchors despite repacked layout.
   Exercise later lifecycle loss, purge and another compaction/fork. Corrupt
   original context/hash/G, descriptor owner/UUID/format, record/token root,
   foreign equal-pair proof digest, anchor root/count/version/absence and current
   file extents; all relevant strict readers reject with source/target unchanged.
6. Actual four-view readiness is stale after commit, remains false on injected
   derived publication failure, then repairs at the same canonical G with the
   original receipt. Do not equate canonical selection with successful cleanup
   or readiness. Exercise the actual caller's success and publication-error
   projection, not only the storage helper. Include a paused/cancelled worker
   ownership check.
7. Preserve genuine old legacy selection/receipt/lineage-v1 fixtures before
   production edits. Their bytes/reopen/import/fork continue to work. Missing
   native proof or stripped foreign anchor is not grandfathered as old legacy.

Use the single protected PR138 target and retained shared registry. On changing
source, record exact input/executable provenance and package-clean only that
owned target; do not clone caches or recreate released paths. Focused regressions
precede an appropriate full suite/fmt/Clippy. After publication and recorded
results, release disposable raw/scratch through the cleanup owner, without new
archive requirements. No personal daemon/data, network provider or deployment.
