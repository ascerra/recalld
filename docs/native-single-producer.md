# Accepted namespace-keyed native single producer

This slice adds an internal typed single-operation consumer, not a one-item batch
adapter or public API activation. Its actual parents are PR106
`fe99020926eb81a6994c7e9214ad4623180eeed1` (PR101 same-insert proof and frozen
selection/advisory helpers) and PR96
`387c7118864040c68d7d28c44fb2600ade520a45` (same-owner publication). The local join
is `3f730cd124ba16b57644a73bd2337b6b9a8e84e7`. PR113 and the broad validation
checkpoint are not ancestors.

`McpStorageAdapter::store_native_single(ctx, input, policy, parent)` accepts a
namespace-scoped contract-2 `MutationContext` and `NativeStoreInput`. It returns
one exact `WriteReceipt`, whether advisory was requested, and current hydration
of only the advisory IDs frozen in that receipt. Disabled and requested-empty
remain distinct even when `NearDuplicateReport` is absent. `StorageError` remains
typed through this seam, including `CommittedMutation` and
`CommitOutcomeUnknown`. Transport activation must preserve these types.

The existing `DerivedCompletion` owns accepted work before provider execution,
loads keyed replay before current settings/provider/clock evaluation, captures
canonical selection, then calls the ordinary single `store_native_prepared`
with no `BatchChild`. Publication lease and graph guard move into the blocking
worker before taking storage write ownership. Cancellation cannot release those
leases while that worker commits. The original core validates generation,
policy, endpoints, profile and normalized vector bytes at final commit. All
views complete before success; advisory hydration retains its outer accepted
root. Unknown outcomes keep unresolved work; known publication failures carry
the exact committed receipt and can recover by retry/reopen.

Identity reuses PR101's canonical hash unchanged: fixed numeric defaults are
expanded once; historical policy defaults and link settings are used on keyed
replay; ordered caller arrays and original supplied vector bits remain intact.
Generated UUID/time/provider output and selection results are excluded. A new
namespace incarnation cannot reuse another immutable namespace's key. There is
no catalog reservation, parent receipt, or reserved child identity for a single
operation. Callers supply actual `NativePolicyInputs`; this seam does not create
a separate policy cache or silently use a global default.

The wire owner must still parse and bind namespace/scope/lineage, preserve
original caller intent, choose the native contract explicitly, and carry the
receipt/advisory envelope. Existing public dispatch is unchanged. This slice
makes no whole-store CAS, full writer closure, public v2 activation, A2 ranking,
or freeze guarantees. It does not incorporate later native exposure or the
independent FTS rebuild-content fix.

## Join fixture boundary

The PR106+96 join reproduced the known native batch fixture failure: **82 pass,
1 fail**, `NotAttempted` versus `Ok` at the old line 405. Only the matching
`batch_tests.rs` hunk from the supplied checkpoint patch was consumed in
`c3006f5`; no catalog/runtime hunk or checkpoint parent was inherited.
Reservation-publication failure asserts exact catalog parent receipt, zero
provider calls, no child receipt or record. A real paused provider then moves
the corruption after successful reservation publication, asserting exact child
receipt, one provider call, one record, and replay without another call.
The exact corrected test and six coherent-payload tests passed.

Validation and retained failure provenance are recorded in
[the evidence report](evidence/native-single-producer/REPORT.md).
