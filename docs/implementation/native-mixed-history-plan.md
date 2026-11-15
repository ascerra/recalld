<!-- Planning provenance: 2026-11-14; original working document /tmp/recalld-native-mixed-history-plan.md; SHA256 079949eeb7f9689afe08700dad7f521bb2419c39b4dfe059b9f23c7f377bb962. -->

Frozen pre-implementation plan. Actual parents are PR105/064686c, corrected PR101/51da3ca, and published PR108/3e6e596. The two reproduced failures were corrected in 1e04734; the final test-corrected source under validation is ac4be00. See native-mixed-history.md and the evidence manifest for final results.

# Minimal native mixed-history join

Proposed shipping child of PR105/064686cc5aa7192b45f6e9f75dc6cfd1e7f1f958.
Additional actual parents: corrected PR101/51da3cade35979a3200f976643bdb965dcb1498b
(runtime4ad7965), and Hume's direct97 keyed-no-change child after validated freeze.
No broad checkpoint, producer routing, batch or ordinary serving activation.
Feynman owns history/strict offline glue; Hume owns graph/store proof shapes.

Keep old and new codec/hash bytes unchanged. Reconcile only real dispatcher and
history interfaces, using the existing types and receipt/effects validators.

- lifecycle_migration::validate_policy_history: construct canonical config map
  plus validated progress2 retired beforeNamespace before running all-origin
  NativeGraphChanged, NativeGraphUnchanged and NativeMemoryInsertedWithLinks
  immutable namespace/profile/dimension validation. Existing keyed reservation,
  per-peer policy closure and same-origin before_purge checks remain mandatory.
  Include graph no-change in native_history; foreign ancestry does not exempt
  header/profile/receipt checks or compare unrelated origin counters.
- native_graph::validate_edge_history: retain graph/no-change/linked-insertion
  active suffix trigger. Reverse NativeMemoryHardDeleted and native purge chunk
  removed_edges with exact identity and duplicate rejection before older graph
  puts/no-change witnesses and linked insertion removals. Apply validated pending
  effects to the logical committed edge map first; do not permit arbitrary
  missing endpoints. Foreign physical edges are not replayed.
- Keep active record reverse history for linked insertion, changed graph and
  no-change assertions alongside policy, maintenance and purge. Restore deleted
  witness rows only into validation state before older assertions. Never restore
  text, labels or vector bytes from a content-free witness.
- Reconcile actual creation_proofs/primary accessor, effects arms and canonical
  validators from the real parents. No generic allowlist for unknown outcomes.

Tests must use real operations in the same synthetic store:

1. Native catalog/policy, linked insert and changed graph, keyed graph no-op,
   subsequent policy/graph change and original-key replay, maintenance phase
   loss/hard delete, then native purge. Include retained vector-owning Ghost and
   surviving peer, and prove exact counts/ledger/receipt identities.
2. Validate while namespace exists, after canonical purge and after cleanup;
   reopen twice; logical export/import and backup/production fork. Preserve
   immutable retired namespace dimension/profile, lineage and all receipts;
   compare logical retained payload/labels/provenance/f32 fields and hashes.
3. Actual RED on unintegrated composition, then exact green: live-only namespace
   lookup cannot reject valid purged history, and active physical edge rewind
   cannot reject the removed edge referenced by an earlier graph/no-op receipt.
4. Mutate retained foreign receipt namespace dimension/profile, peer revision,
   keyed no-op reservation or edge witness; strict source stays byte-identical
   on rejection. Do not weaken existing original proofs or skip foreign checks.
5. Exact pending native deletion/outbox crash boundary remains recoverable, while
   unfinished offline work rejects immutably. No all-writer/ReadView claim here.

Source-specific private target, offline Rust1.94.1,2CPU/4GiB,jobs2. Freeze parent
set before final full test/fmt/clippy, preserve source/binary/log hashes. Use
fresh target or clean only our package outputs before any source-root switch.
