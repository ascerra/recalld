# Native selected-text compaction

Implementation child of PR138, exact parent
`05e0d55aa3a92483e294b2a5e79cf490ce270609`. The approved design is in
[native-text-compaction-plan.md](native-text-compaction-plan.md). The original
engine and generation-selected file protocol are retained; `system.rs` is
unchanged. Final validation status is recorded in the evidence report.

## Mutation and publication

The existing owning engine selects its native or legacy guard from the actual
owner, then validates the physical schema again inside the write transaction.
Native compaction records `NativeTextGenerationSelected` with required version1
relocation evidence: original GenerationRef, exact before/after descriptors,
reference-map commitments, total and retained row counts, logical record-state
commitment, and measured retained-text copy commitment. It covers terminal rows
and every retained extent. Physical offsets change; record/lifecycle generations,
policy revisions, timestamps, provenance, labels, vector commitments and graph
content do not. Legacy-inferred text provenance remains unknown.

The internal request remains contract1/catalog/`compactText`, with the original
precommit CAS generation. It is independent of the outer manual request's API
version. Generated file UUIDs, seals and copy results remain outcomes, not request
inputs. Existing legacy selection outcomes and request bytes are unchanged.
New native proof versions or missing required fields fail closed.

Payload and directory synchronization precede one metadata transaction containing
all relocated references, selection/catalog, canonical G and receipt. No
cross-file atomicity is claimed. Each attempt consumes one owned text-generation
ID and one owned operation ID, samples no clock/provider, and retains abandoned
reservations. Empty or zero-removal selection advances canonical G once. Cleanup
failure returns the committed receipt with `cleanup_pending=true`; safe orphan
cleanup is retried after validated reopen.

`CompactionResult` now carries the exact `WriteReceipt` and cleanup status.
Selected/Published postcommit errors retain that in-hand receipt without reading
back through a poisoned owner. Ambiguous TX acknowledgment remains the existing
known-operation-ID `CommitOutcomeUnknown`, with payload retained for reopen.
These added required fields affect Rust callers constructing CompactionResult
literals; they do not add a public protocol endpoint or alter request defaults.

The actual maintenance caller uses `DerivedCompletion`. Before preparing a new
selection it rejects an unavailable or active-purge owner and completes prior
accepted publication work. A failure there preserves the prior phase/deletion
receipt, G, IDs and selection. This is not a global drain/barrier. After the new
selection, successful return awaits real publication of all four views; failure
retains the compaction receipt. The sweep keeps the receipt and cleanup status
and reports every successful empty selection. The canonical worker owns its
publication lease and graph guard even if its async waiter is cancelled.

## History and restore

Current-origin history reverse-validation checks logical record identity at the
compaction's exact generation without rewinding lifecycle/record tokens. When
current physical selection has the same generation UUID as a native receipt,
strict selection validation checks its exact catalog descriptors and, at that
receipt's current G, complete reference-map identity. Logical import and restore
may legitimately repack to a different UUID: historical descriptors never select
current files or authorize orphan deletion.

After strict validation of the real restore inputs, the private restore
transaction first merges the original retained receipt rows. It then creates a
version2 lineage anchor containing a versioned count/SHA-256 over the exact
foreign native compaction receipt scoped keys and original stored bytes in key
order. No new restore-G1 receipt, origin rewrite or per-record witness is added.
Existing lineage1/None bytes omit the new field. Logical export/import preserves
it; later actual forks validate and anchor the newly retained merged set.

Strict readers validate receipt shape, original request hash, sourceG+1, scope,
descriptor ownership and permitted ancestry as well as the anchor. Row omission
with the authoritative anchor retained, changed proof bytes, absent/null/unknown
v2 anchors, and marker downgrade while native outcomes remain all reject.
Flattened ancestry is not used to infer schema/native ordering or baseline1.
Removing every new outcome and every associated version marker/anchor is total
trust-anchor replacement, outside the authenticity guarantee. This is neither
forensic erasure nor protection against arbitrary rewriting of the entire store.

## Required next consumers and limits

Pascal owns the actual135 native autonomous/manual compaction join after this
validated parent; that branch currently skips legacy compaction. Ordinary native
startup remains gated on all participating writer/read/adaptor consumers. The
helper owns the shared initializer and private native-prefix admission. Public
API2 activation and ordinary API1 native dispatch are separate consumer work.

A worker JoinError has no reliable receipt/operation ID and is reported internally
as unknown recovery failure. Source tracing of the actual manual HTTP path found
a generic500, with no false `notCommitted` classification, but no structured
unknown state either. A4 owns that later structured error projection. This slice
does not claim a runtime panic reproduction; its actual uncertain-TX test covers
the separate known-allocated-ID path. Process-exit tests establish application
restart behavior, not filesystem power-loss durability or performance gates.
