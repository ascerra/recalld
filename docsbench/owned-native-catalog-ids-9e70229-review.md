# Prepared source handoff: PR124 versus 9e70229

Reviewed 2026-11-14 during performance DRAIN. No builds, tests, formatting,
containers, archives or file copies. No edits to Feynman's source. This is an
unvalidated-source review, not acceptance of9e70229 as the final parent.

Inputs: PR124 `a5865da02969631a5031f65c6e43269701d7a254` and Feynman
`9e7022933c9e1223217509ec369bfd4a45c0b9a1` (known purge replay receipt correction).
`git merge-tree --write-tree --name-only <124> <9e>` created only a virtual
conflicted tree `4a197839293e35b94900d9e6c8f34830bc93aa17`; no branch/index or
source worktree was merged. Recompute against the actual final handoff.

## Exact conflict inventory

Nine paths conflict. Required resolution is semantic union, never a whole-file
replacement. The table describes the inspected conflict hunks, not a performed
or validated resolution.

| Path under src | Conflict and retained requirements |
| --- | --- |
| derived_completion.rs | Keep124 native_graph/native_store modules and9e native_decay/native_purge; retain common feedback/forget and one completion owner. |
| storage/control.rs | Union typed outcome match:124 linked insertion plus9e access/catalog/policy/purge/phase/hard-delete. Keep124 record_receipt_with_ids/default fallback semantics and every9e validation arm, not a blanket outcome accept. |
| storage/engine.rs | Union native_catalog/native_graph/native_maintenance/native_policy modules. Preserve124 open_with_ids plus9e strict native recovery/admission behavior. |
| storage/lifecycle_migration.rs | Two history-match conflicts: retain124 native batch bookkeeping recognition and9e native phase/hard-delete policy/revision rewind proofs. Do not weaken unknown-history rejection. |
| storage/metadata.rs | Retain pub(crate) native_records visibility needed by124 producers, plus9e native_maintenance/native_namespace_binding/native_policy modules. Keep one MetadataStore ids field/helper. |
| storage/metadata/effects.rs | Six hunks: native graph no-change variant, linked insertion plus phase/hard-delete eligibility/outcome handling, purge/phase effects, native phase record handling and phase-index update. Preserve124 batch child reservations and self.record_receipt; retain strict9e native proof validation. |
| storage/metadata/native_records.rs | finish_record_effects final argument is124 child versus9e None. Retain actual child capability; dropping it allocates/revalidates incorrectly for batch. Preserve original API changes from final parent. |
| storage/metadata/native_records/tests.rs | Union store_batches/store_producers (includes124 owned-ID regressions) with mixed_history/namespace_binding tests. |
| storage/metadata/purge.rs | Build9e native/legacy typed start outcome and retained original request context first, then self.record_receipt. Preserve124 owned next_operation_id reservations. Do not retain9e free start receipt call after resolving the surrounding conflict. |

Auto-merged files still need review: metadata/catalog/native_graph/native_lifecycle,
mod.rs, validation.rs and the allocation plumbing must not silently discard API
context, scope or strict proof changes. A clean textual merge is not correctness.

## Actual interfaces and remaining receipt calls

9e metadata/native_catalog.rs validates catalog scope after ctx.validate, without
the prior hardcoded contract2 guard. NativePolicy request_hash likewise checks
namespace scope without hardcoded2. The prepared tests' API1/2 loop is therefore
consistent with this inspected source; it still must pass on the final parent.
The original API stays in the request hash and cross-contract reuse must conflict.
Session create_namespace/update_policy signatures and the existing native catalog
fixture/Clock/Provider/fault APIs match the draft. No signature workaround or
second session beside an active engine is required.

New files absent from124 still use free control::record_receipt:

- metadata/native_catalog.rs:84, native namespace creation.
- metadata/native_policy.rs:62, keyed equal-policy receipt.
- metadata/native_policy.rs:143, numeric policy update receipt.

That free function is new_v4 on9e; after124 it defaults through SystemIds.
The intended consumer fix is these three self.record_receipt calls. In addition,
metadata/purge.rs:679 on9e (virtual merged line686) must resolve to
self.record_receipt while keeping the native/legacy outcome selection.124's
continuation source calls auto-merge into the virtual tree, but require review.
No other free receipt caller appears under metadata in this virtual result.
The virtual tree is conflicted and cannot be used as a test source.

## Expected allocation and clock counts, not measured results

Counts begin after opening the genuine existing native fixture with SeededIds;
fixture migration/genesis IDs are outside this narrow suffix proof. An oracle
restored from a copied IdState consumes predictions independently of the live
source. Namespace IDs are canonical checked integers, not additional UUID draws.

| Operation | Operation ID draws | Clock samples | Canonical effect |
| --- | ---: | ---: | --- |
| New native namespace create | 1 | 1 | One namespace/max_id/receipt/G commit; actual profile and dimensions consulted before fresh creation. |
| Matching create replay, changed-key-payload conflict, same-key cross-API conflict | 0 | 0 | No new G/row; replay returns original full receipt. Provider(false) must never be consulted. |
| Numeric policy change | 1 | 0 | One updated-policy receipt/G; namespace policy and record policy tokens use existing semantics. |
| Keyed equal-policy request | 1 | 0 | One no-change receipt/G; policy revision/value remains unchanged. |
| Policy replay/conflict or unkeyed equal-policy request | 0 | 0 | No new receipt/G; unkeyed equal policy returns None. |
| before_stage create failure | 1 | 1 | Receipt was staged before prepare boundary, but transaction abandons: no durable row/receipt/G. Cursor is not rewound. |
| Retry after that precommit failure | 1 additional | 1 additional | New operation ID, not replay of an uncommitted reservation. |
| before_handle_publish known commit / commit_unknown modeled durable commit | 1 | 1 | Full known receipt or uncertain operation identity retained; unresolved/unavailable owner cannot safely continue. |
| Closed-owner recovery and keyed replay with restored cursor | 0 | 0 | Same stored operation receipt; real recovery resolves handle/pending state. Test must verify rather than assume open consumes zero IDs. |

Useful cumulative assertions: create→policy update→keyed no-change consumes3
Operation IDs; any number of their replay/conflict attempts consumes0 more.
Create→closed reopen→same-key replay→next distinct create consumes2 total.
Precommit-fail→retry consumes2 despite only one committed create. Both API contexts
run separately from the same seeded suffix; no shared/global source.

Preserve existing wider124 counts: a batch reservation allocates one Operation
per reserved item then one parent Operation; each generated inserted memory uses
its sampled-time UUID, while child receipts use reserved IDs. Purge start consumes
one start Operation plus one next-child reservation; each nonfinal step commits
its reserved ID and reserves one next ID, and final completion reserves none.
These are source-derived invariants to check against final common-context changes,
not newly executed batch/purge evidence.

The prepared four-test draft remains unregistered/uncompiled. After finalhandoff
and explicit END: recheck these hunks/interfaces, register tests on the joined
source, preserve actual pre-fix failure if present, make the minimal wrapper
changes, then focused and full relevant isolated checks. Keep failure receipts,
source/binary hashes and resource evidence. No plan-only PR or native capability.
