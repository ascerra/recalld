# Native writer participation: proposed implementation boundary

2026-11-14. Planning only; no implementation or validation claim. Prepared
during the A5 scheduling hold. The combined checkpoint remains an isolated
validation branch, not a shipping parent.

## Owners and parents

- Feynman owns canonical catalog/configuration, maintenance/phase-loss, purge,
  strict offline/history review of native batches, and the shared
  history-validation/rewind dispatcher.
- Pascal owns native readers and observation, and reviews A3 transition/clock
  semantics. Existing native insertion/tombstone/lifecycle serialization stays
  unchanged. `NativeRecordWrite::validate(..., false)` remains tombstone-only.
- Curie owns producer admission, accepted/transitive work scheduling, native
  batch codec/child binding, and the ordered transport batch runner. Native
  bookkeeping reaches the shared dispatcher only through validated manifest
  references. There will be no second runner, tracker, or native codec owner.
- Huygens owns A4 parsing/profile evidence and transport activation; Halley owns
  the publication registry, barrier and whole-query ReadView.

Start the native catalog child from Pascal's **validated frozen** reader join
of 69/eefc987 and 79/6a70174. Its current work-in-progress is not a passed parent.
Actual profile62/ed242ec is already an ancestor; no extra profile merge is needed.
Add actual 80/ca205aa for the maintenance/purge/batch child, which supplies the
70+77 runtime/offline composition. Reuse the frozen Curie producer join only
where its typed-guard/accepted-root changes are required. Do not import the
broad published-leaves checkpoint into a shipping branch. Check ancestry
before adding any redundant parent.

## Proposed internal APIs

Names below are proposals, to be reconciled with the actual frozen parent.
All run on the existing owning `RedbStorageEngine` and `MetadataStore`.

```rust,ignore
create_native_namespace(ctx, input, active_profile, clock, parent)
    -> Result<WriteReceipt, StorageError>
capture_native_policy(namespace_id, replacement, policy_inputs)
    -> Result<NativePolicyIntent, StorageError>
apply_native_policy(ctx, intent, clock, parent)
    -> Result<WriteReceipt, StorageError>
capture_native_decay(namespace_id, memory_id, policy_inputs, sampled_at)
    -> Result<NativeDecayIntent, StorageError>
apply_native_decay(ctx, intent, current_graph_evidence, parent)
    -> Result<DecayMutationResult, StorageError>
```

`parent` is `Option<&WorkTicket>`. Metadata transactions receive the actual
`&MutationGuard` and verify its owning store. Native insertion/tombstone gain
the existing optional `&BatchChild` same-transaction hook where required;
preserve the existing argument order rather than creating a second batch API.
Graph evidence is computed and rechecked under graph then storage locks, never
accepted as arbitrary caller authority. No synchronous guard crosses an await.

## Reviewable slices

1. **Catalog and policy.** Reuse catalog directory preparation/sync and delayed
   handle publication; do not claim atomic cross-file writes. New namespaces
   use schema2 with `legacy_access_v1` as the default and an actual immutable
   active profile. A4 resolves provider `profile()` and dimensions: unknown or
   passthrough providers cannot bless a native namespace. Validate explicit
   dimension/profile selectors before directory creation or ID allocation.
   Matching replay precedes current-provider checks. Hash original caller
   selectors/options, excluding generated time and the sampled active profile;
   persist the actual binding separately. A descriptor proves configuration
   identity, not verified model weights or an invented model revision.
   Keep `runtime::check_namespace` ordinary-serving gate intact. Numeric policy
   updates cannot change identity, profile or dimensions. A typed before/after
   policy proof binds the affected record-token digest; update record revision
   evidence atomically without changing anchors, payload or history. Reuse the
   existing preview and `LifecyclePolicyMigrated` path for a version switch.
   Enumerate canonical metadata rows, including Ghost and Tombstone, rather
   than the live namespace index. Preserve all lifecycle timestamps, stability,
   phase and permastore. The replacement accepts only declared numeric policy
   fields; verified prior policy identity is a precondition, not caller input
   added to the retry hash. Preview current-time phase counts without removing
   payload; reject invalid inherited/default settings before writable effects.

2. **Maintenance and hard deletion.** Reuse the A3 planner and original sampled
   clock. Final guarded checks cover record token, policy/configuration and
   graph changes. One destructive phase per sweep; distinguish unchanged,
   exempt and stale skips without receipts. Add explicit typed phase-loss and
   hard-delete outcomes, not an overload of the tombstone proof. Removal masks
   follow the actual payload/owner changes. Reclamation uses the live canonical
   owner/epoch guard; publication completes after locks are released. Preserve
   a bounded deletion witness `{before: NativeRecordState, history}` before
   removing the row. Validate its numeric history/hash, UUID, namespace,
   terminal kind and ledger entry. It reconstructs only proof-validation state,
   never deleted text, labels or vector bytes.

   Pascal's source audit: `SweepEvaluation::project_locked` is a legacy adapter
   using `last_accessed_at`; native projection must instead pass the persisted
   `decay_anchor_at` to the shared lifecycle projection. Recheck graph influence
   even for strength-only/no-transition plans using canonical eligible
   same-namespace donor projections via `connection_bonus_with_strengths`.
   Full-to-Summary needs a nonempty retained summary. Summary-to-Ghost strips
   summary and structured/content labels, not automatically a retained vector.
   Clamp phase/update times; never invent retrieval, feedback or anchor changes.
   Permastore and zero multiplier skip automatic loss; terminal rows stay terminal.

3. **Purge.** Extend the existing deleting/chunk/cleanup/complete state machine,
   reserved operation IDs and same-transaction ledger/effects tail. Typed chunk
   evidence uses the validated deletion witness and exact namespace/config
   transition. No whole-purge success before final cleanup publication. Retain
   immutable origins across restore; offline validators must understand complete
   native progress and witnesses. Active/cleanup work stays blocked from export
   and restore unless an explicit recovery flow completes it first.

4. **Native batch.** Reuse BatchProgress and stable child IDs, in input order.
   Current `record_child` requires contract1 and legacy outcomes; native support
   therefore needs a versioned canonical request/default/child binding, not an
   outcome allowlist. Preserve all existing hashes, IDs and finalized errors.
   New native child results finalize atomically with their mutation and receipt.
   Pending/unknown commits reconcile before retry; bookkeeping generations do
   not imply atomic batches. Feynman updates strict table/export/restore proof;
   Curie routes actual producers through the typed seam.

Every slice preserves replay-before-CAS and checks stale lineage before replay.
History validation recognizes exact typed transitions and verified ancestry,
with per-origin ordering; it never compares unrelated lineage counters or
rewrites historical receipts. New required formats fail closed in older readers.

## Validation and activation

After A5 END, first finish and validate the isolated frozen-leaf checkpoint.
Preserve its failed source/binary evidence; use a source-specific private target.
Shipping native slices then get focused replay/conflict/clock/race tests and
strict reopen/export/import/retained-state restore tests with actual tables.
Include policy change versus sweep, feedback versus stale sweep, hard deletion
after slot reuse, interrupted purge cleanup, namespace recreation, batch crash
between commit and acknowledgement, and malformed/missing proof rejection with
unchanged source bytes. Compile the real consumers and require ready publication
or an explicit committed/unavailable receipt.

The native serving gate remains until the participating writers, actual
consumer paths and Halley's generation-consistent publication/read boundary
are joined and validated. The exact-vector snapshot is not a whole-query
ReadView. Remaining gates have the owners above; this plan does not claim they
are complete or authorize a competing backend.
