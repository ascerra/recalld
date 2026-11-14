<!-- Planning provenance: 2026-11-14; original working document /tmp/recalld-native-replay-binding-plan.md; SHA256 dc235d039e0bfbaad5b7c9fb0814de573ac82e8bb095b1ed9bdee32203f536c1. -->

Agreed follow-up API plan with Pascal and Huygens. Not implemented or validated by the mixed-history run. The actual consumer join must close the after-purge replay binding gap before native activation. This planning record travels with the corresponding consumer branch.

# Shared native namespace binding for replay

Follow-up on the validated mixed-history child, consumed by Pascal/Huygens actual
96 publication paths only when their minimal consumer joins need the dependency.
Feynman owns the shared binding authority. No hash/receipt rewrite, guessed name,
new session, provider resolution or parallel raw parser.

Proposed owning MetadataStore seam:

capture_namespace_binding(&MutationGuard, NamespaceId)
    -> Result<(GenerationRef, Option<NativeNamespaceProof>), StorageError>

- guard.check(self), then one read transaction; reuse native_control_at (make
  pub(super)), so owned native mode, availability, schema and lineage checks
  remain. Do not reacquire publication: caller may already hold MutationGuard
  and an unchanged redb write transaction during receipt lookup. Returning an
  owned proof releases redb borrows before hash preparation/publication awaits.
- Returned GenerationRef is current canonical state, not the receipt's historical
  G and not readiness. Caller compares it to read_control(existing_write_tx)
  before hashing; stale lineage is checked before replay.
- Live namespace lookup returns its typed ID/name/dimension/schema binding.
  Native writers cannot rename/reuse that ID. Existing native startup/history
  validation establishes the handle's invariant; uncertain owners stay blocked.
- If live binding is absent, read the existing validated native purge history
  through purge::validate_existing and borrow its NativeNamespaceProof. Reuse a
  single helper with lifecycle_migration's retired-config assembly; reject
  ambiguity/mismatch. Clone only the selected typed proof into the result.
- Use the exact stored canonical name and embedding binding, including unknown
  provenance; never turn matching dimension or current provider settings into
  an attestation. No new serialization or operation kind.
- The API must distinguish no evidence (None) from malformed/ambiguous evidence
  (Err). The consumer already holds the matching receipt and preserves it in a
  typed committed failure if identity cannot be proved. It must not retry as a
  new mutation or silently substitute a recreated name's new namespace ID.

Required consumers/tests:

1. Actual keyed feedback/forget/numeric policy, purge old namespace, recreate
   same name/new ID, replay original namespace-ID key/hash and original outcome
   without provider/clock/new revision or allocation. Changed input conflicts.
2. Calling under an already-held MutationGuard + unchanged write transaction
   cannot deadlock/reacquire publication. Foreign owner guard fails; compare
   returned current GenerationRef to transaction control.
3. Missing/invalid/conflicting retained witness fails closed with known receipt;
   no ordinary data-read bypass during pending/unknown work. Scope-name mismatch
   remains a typed conflict, not successful rebinding.
4. Restore ancestry validates the binding and preserves original names/profiles
   without allowing a stale pre-fork storeID to replay in the new lineage.
5. The shared borrowed helper is consumed by strict lifecycle history, and the
   actual replay adapters consume MetadataStore capture. Do not publish an
   unused standalone helper as completion of the replay gap.
