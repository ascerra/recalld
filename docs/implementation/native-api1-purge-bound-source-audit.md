# Purge admission bound: writer and historical-entry source audit

Pinned source: `c6b5755d5837f7edfd31095a035754819dc94327` in Feynman's
`native-api1-purge-consumer`. Huygens read-only review, 2026-11-15. This supports
the approved option-1 implementation; it is not estimator validation or concurrent
execution proof. No runtime changes or new build/test jobs by this review.

## Exclusion while an admitted purge is active

`metadata/purge.rs::require_no_active_write` scans the durable progress table and
rejects **any** non-Complete operation. `namespace_state::require_namespace_active`
calls it before reading the requested namespace, so the restriction is global,
not only for the namespace being deleted. Fresh `begin_purge_with_work` itself
uses this helper. A second independent fresh purge cannot start during the first.
Matching retained keyed replay is checked earlier and is not fresh admission.

The in-process barrier is complementary: `native_guard_with_work` holds the
metadata publication lease and calls `ensure_available`, which checks both
handle/canonical-effect availability and the active-purge flag. The lease remains
owned through the guarded transaction. Do not claim every writer literally calls
the durable helper: some rely on that same-lease availability check.

| Writer family | Source authority on this pin |
| --- | --- |
| Native plain/linked insert and batch children | `engine/native_records.rs::insert_native_record_common` takes native guard; `metadata/native_records.rs` checks the final captured namespace through `check_native_capture`; `effects.rs::finish_record_effects` independently calls the global durable namespace guard for every non-purge mutation. A previously captured intent cannot bypass the final check. |
| Native graph, phase decay, hard delete, forget | Engine native graph/maintenance/record paths take native guard. Final capture validation and the non-purge effects guard reject active purge. |
| Exposure and feedback, including native lifecycle selection | `metadata/native_lifecycle.rs` mutation entrypoints take native guard and validate the final namespace/capture. `legacy_lifecycle.rs::lifecycle_guard_with_work` selects native guard for a native owner. |
| Native namespace creation / numeric policy | Creation takes engine availability and native guard through staging/commit; it does not independently call the durable helper in its create TX. Native policy lookup/capture/apply are guarded and check current namespace capture. These cannot enter concurrently across the held publication lease. |
| Batch reservation and rejection bookkeeping | `metadata/batch.rs::reserve_batch_version_with_work` and `finalize_batch_error_with_work` take the actual native/legacy guard. Child policy checks use the global namespace helper. A queued accepted ticket is not permission to bypass those checks. |
| Text relocation / compaction | `text_compaction_guard_with_work` selects native guard; `select_text_generation_proved_guarded` also calls `require_no_active_write` in its final TX. |
| Explicit inherited-policy adoption | Final `adopt_inherited_decay` TX calls `require_namespace_active`; offline entry also requires owned source access. |
| Legacy raw record/edge/catalog methods | `mutation_guard` and `commit` reject native ownership through `ensure_legacy_operation`. Engine edge methods additionally check availability. Raw test insertion/deletion/tombstone helpers are cfg(test); they are not production escape paths. |

Recovery exceptions are narrower than ordinary admission. Fresh purge begin calls
`ensure_handle_available`; this rejects canonical pending effects before a bound
can be accepted. `engine/effects.rs::apply_pending` validates and applies only
unapplied durable effects, then clears canonical_pending. A failed application
marks the handle unavailable. Derived rebuild/acknowledgement alone does not add
canonical record content or edges. Offline restore/import/migration cannot run as
another live writer against the same exclusively owned source; they are not a
concurrent ordinary mutation path.

## What permitted continuation can change

`engine/purge.rs::resume_pending_purges_observed`, the existing explicit resume
method and startup recovery use the same purge-step builder. They can choose
subsets up to CHUNK_LIMIT; a worker-local chunk preference is not an invariant.

At this pin, fresh admission starts with no other active purge. A competing owner
completion may continue the **same** operation. In `capture_chunk`, deletions are
selected current target rows; removed edges are incident to that subset; peers
are surviving sources of those removed edges. A target row outside the current
subset may be a peer now and a deletion later. Therefore the initial union must
budget target rows in both roles, plus external incoming sources and incident edges.

The `NativePurgeChunk` branch of `finish_record_effects` changes a surviving peer's
`edge_count` and increments `lifecycle.record_generation`. It preserves lifecycle
times, access history, summary/text/vector/profile commitments and other record
content. Edges are removed, not added. No ordinary writer can append history, change
profiles, insert a new incoming source or recreate the target during the active
operation. Bound the mutable counts and generations by their full typed widths,
not their current decimal spellings. Saturation/overflow remains a typed failure,
not permission to emit an oversized receipt.

Once Complete clears the purge flag, ordinary writes may proceed, but they cannot
change the operation's immutable retained child/start/final receipts. A later
foreign pending receipt must remain in the typed source chain and must never be
projected as this purge's child. A bounded diagnostic must not stringify nested
full foreign receipts. Matching own evidence still requires store/namespace/parent
identity, and its entire receipt is preserved.

This source argument supports a conservative initial union bound for any one
future receipt. It does not justify counting only one predicted partition or
current peer widths. Estimator code must still prove its serialization inequality,
checked arithmetic, exhaustive/capped examination and all outer-frame costs.

## Historical entry separation

The actual public request is name plus force, with no idempotency key or parent
operation selector. The native name producer resolves through
`get_namespace_by_name`, which calls `ensure_available`. Consequently an old active
purge blocks the fresh name route before begin/resume; it is not silently resumed
and projected through a newly smaller transport. Completed purge removed the
namespace: absent name returns NotFound. A deliberately recreated name resolves a
new stable namespace ID and starts a new explicit operation; never auto-retry it.

The private `PurgeSelector::Context` path is different. Its begin TX preserves
matching replay before fresh namespace/size checks. The new budget seam must not
coerce a public name request into that historical route or change immutable old
receipt bytes. Old private/offline recovery is not proof that a bounded public
transport can represent every old receipt. Exposing historical resume later would
require its own reviewed evidence behavior; it is not added here.

## Required execution still outstanding

- Pause a real admitted purge after begin and attempt record/graph/lifecycle,
  catalog/policy, batch bookkeeping and compaction mutations through actual owners,
  including an intent captured before begin. Verify rejection and unchanged
  content/edges/history; a second fresh purge must also reject.
- Let another completion advance the same operation in differing valid chunks,
  with target-to-target and foreign incoming edges. Check the actual maximum
  serialized receipt against the admitted bound, including peer-before/after widths.
- Preserve real oversized old history; prove name-only entry never resumes it,
  keyed replay remains unchanged, and a recreated name is a new identity. Neither
  an unknown acknowledgement nor a missing metrics observation may upgrade evidence.
- Re-run these checks on the final estimator/wire composition. This pinned audit
  does not validate future guard changes or authorize unrelated activation.
