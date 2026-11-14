# Monotonic native phase markers

This narrow child of PR82 corrects an actual migrated-record clock regression.
Legacy insertion at time 1000 followed by schema migration at 2000 leaves
`lifecycle_updated_at=1000` and `phase_changed_at=2000`. Native forget with clock
500 previously committed `phase_changed_at=1000`; strict reopen accepted it.
The isolated regression exercised actual insertion, schema and lifecycle
migration, forget, replay and reopen before asserting that failure.

A3 section 8 intentionally uses migration time when historical phase time is
unknown. That migration state and its immutable publication proof remain
unchanged. Section 3 keeps `effective_now=max(sample,lifecycle_updated_at)`;
section 10 requires timestamps not to move backward under clock regression.
For a new actual phase change, use:

```text
lifecycle_updated_at = effective_now
phase_changed_at = max(effective_now, prior phase_changed_at)
```

The marker is a clamped logical timestamp, not reconstructed historical evidence.
`timestamp_origin` remains unchanged, including `legacy_inferred`. Forget does
not change the decay anchor, observation/retrieval/reinforcement timestamps,
compatibility access time or access history. The operation samples its injected
clock once; matching replay samples it zero times. An already-terminal record
remains unchanged. Existing historical outcomes are not repaired retroactively.

## Receipt compatibility

`NativeRecordWrite` adds optional `phaseTimeRule`, with the only accepted value
`"monotonic_marker_v1"`. Every new native forget emits it. Native insertion
neither emits nor accepts it. Unknown values, explicit null, numeric values and
invalid marked transitions fail closed.

An **absent** field retains the exact original PR79 tombstone computation and
encoding. The shared proof dispatcher selects the recorded rule; it never tries
an older interpretation after a marked proof fails. This preserves genuine old
receipts and their replay/export/restore semantics, including an old marker that
already regressed. Deleting the new discriminator cannot validate the corrected
regressed-clock outcome under the old computation.

Older readers have `deny_unknown_fields` on this proof, so a receipt with the new
field is intentionally unreadable to them. This is not full backward format
compatibility or permission to reopen the corrected store with an older writer.
No record schema, migration timestamp, original receipt, request hash, operation
identity or public serving gate is rewritten by this change.

## Evidence

`phase_clock_tests.rs` covers the actual new mutation/replay/reopen, a real old
receipt through strict reopen/export/import/retained-state fork restore, and six
malformed-proof cases that leave the corrupt input bytes unchanged on rejection.
The old fixture at `native_records/fixtures/phase-clock-legacy.jsonl` was emitted
through the production exporter by the failing BEFORE runtime, not synthesized
by editing canonical rows. Its SHA-256 is
`c411d6abec5814d7467865f33b815a35aef8ba7c35a12f41edeb3f3c54738aaa`.
The validation report records exact source, executable identity and raw logs.

Ordinary schema-2/public contract-v2 serving remains gated. Feynman owns future
phase-loss writers and their typed proof variants; those can use the same
monotonic marker rule without changing original tombstone receipt semantics.
