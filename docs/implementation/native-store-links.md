# Native insertion with frozen links

This internal core is a direct child of PR97 `292fb29`; it adds no native batch,
producer, maintenance, catalog, or broad integration parent. Curie's frozen
canonical selection interface is consumed by an adapter in a separate producer
join. No ordinary native serving gate is opened here.

## Interface and identity

`storage::native_store::NativeStoreInput` contains the existing `NativeInsert`
and an expanded `check_duplicates` boolean. `NativeStoreSettings` contains only
consumed link settings: enabled, original f64 threshold bits, three link caps and
temporal window. `capture_native_store` pins these before provider work.
`FrozenInsertLinks` carries canonical capture G/profile, outgoing static
semantic/entity links, all temporal candidate IDs/creation times, and an optional
numeric advisory. None means disabled; Some(empty) means requested and checked.

`prepare_native_store_links` checks canonical scope, live candidate phases,
profile, uniqueness, finite values, caps and temporal times, and captures peer
tokens. The final transaction rechecks exact store lineage/G, tokens and policy
before payload allocation. This catches newly inserted candidates even when the
previously selected peers have not changed. The producer remains responsible for
its canonical vector/hash scan, normalized entity selection, and similarity
scores; the storage core does not introduce another search implementation.

`lookup_native_store` checks historical replay before current configuration,
provider output, capture, IDs or clock allocation. Its distinct
`memory_insert_with_links_v2` request hash binds original fields, ordered arrays,
checkDuplicates, and receipt-pinned profile/lifecycle/link default expansion.
Generated IDs/times, provider output and selected candidates are not caller retry
identity. Current settings changes replay the original result; changing the
requested checkDuplicates flag conflicts. The old insertion hash is unchanged.

The shared `NativeStoreInput::request_hash` is the exact child identity seam for
Curie's later batch-manifest-v3/native-contract-2 join. This base has no BatchChild
API; that join must bind reserved child outcomes in the same metadata transaction
and retain original v1/v2 hash/manifest decoding. This is not batch activation.

## Atomic outcome and recovery

`NativeMemoryInsertedWithLinks` selects a new strict `NativeStoreWrite` proof.
It records actual final states: new record token 1, absolute outgoing count,
changed parent token incremented once, unchanged target/advisory state, and exact
edge bits/timestamps. Explicit parent/supersedes links and selected automatic links
share one metadata transaction, generation, receipt and existing edge outbox.
There is no insert-then-link workflow or in-memory ticket substituting for durable
intent. After metadata commit, recovery applies every required edge from that
outbox; a completed keyed replay never generates new links.

The core samples one clock. Temporal selection anchors to final record created_at
(caller time if provided, otherwise that sample), orders by absolute time delta
then UUID, and preserves the existing minimum weight. Edge timestamps use the
same sample. No second clock or competing ID/RNG mechanism is introduced.

Common creation checks are extracted without weakening the old
`NativeRecordWrite` validator's two-edge limit. Retained creation consumers use a
small actual-primary `CreationCommitment` for both outcomes, not a fabricated old
insertion proof. New receipts validate policy, exact peer transitions, allowances,
advisory presence/order, and before/after immutable namespace profile bindings
across all origins. The proof dimension must equal the validated namespace
dimension as well as satisfy the supported range; a well-formed profile ID alone
does not establish this equality. Physical edge history, pending-effect validation, startup,
logical export/import and deletion-aware restore recognize the additive outcome.
Each peer policy version/revision equals the single-namespace operation header;
the header participates in policy-history validation before any foreign-origin
record rewind is skipped. This does not validate separate cross-namespace purge
or hard-delete peer proofs. Unknown canonical outcomes still fail closed.

Existing ownership, admission, sync, uncertain-commit poisoning and full committed
receipt errors remain. Canonical success is not a derived-readiness claim; the
actual producer must await its publication boundary. Advisory hydration must use
only recorded IDs at current visibility, with a separate read generation and
unavailable/omitted-content warnings. Historical receipts contain no text payload.

## Bounds and remaining joins

The existing native graph limit of 1,024 actual insertion edges is enforced with
an explicit error, never hidden truncation. This is not a candidate-scan, total
allocation or RSS bound. Canonical candidate capture, peer text commitments,
existing graph loading and retained creation-history checks can perform linear
work. No performance or default-promotion claim is made.

The separate producer join must prove actual disabled-provider replay without
calling embedding/capture/selection, preserve settings at batch reservation, and
publish all derived views before ready acknowledgement. It consumes the actual
new hash and proof, not a boolean bypass. Native phase/hard-delete/purge joins
must extend typed reverse history and restore removed-edge evidence before
rewinding these insertion edges. No such unrelated ancestry is included here.

Standalone keyed graph no-ops remain explicitly unsupported pending their own
strict typed no-change outcome; this insertion proof does not close that A4 gap.
Performance work remains queued after the critical graph/store joins, using actual
PR96 and typed contiguous deltas, per-component proof, and full-rebuild fallback.

See the [validation evidence](../evidence/native-store-links/REPORT.md).
