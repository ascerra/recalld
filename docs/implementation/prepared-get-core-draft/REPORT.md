# Prepared get: source draft, not ready for publication

**Current update:** [COORDINATOR.md](COORDINATOR.md) records the actual coordinator,
direct consumers and first focused validation plan. The helper-only description
below is historical at7939d6d; no execution claim is superseded by source work.

## Historical literal helper review

**Uncompiled, untested, not a shipping helper-only PR.** Exact base:
`19bd383a75cdb15283b4c15a865dfcb2a4b6e0a1`. Worktree
`/tmp/recalld-implementation/prepared-get-core`, branch `impl/prepared-get-core`.
Coordinator released only a source draft; no build slot or RPC/capability edits.
Plan: docs branch `e3c07c91f832e28a8a074ae5405e71303ad319ca`,
`docs/implementation/prepared-get-plan-2026-11-16.md`. SOURCE.json pins base/draft
bytes. No performance, compiler, runtime correctness or activation claim.

## Literal A1 diff

`metadata/publication.rs` adds private-field borrowed PointRead and
with_point_read<T>(for<'a> FnOnce(PointRead<'a>) -> Result<T, StorageError>).
The existing publication.read guard and open_canonical_read own the actual TX;
ensure_available rejects poison/canonical_pending/purge. No effects_stamp/all4
readiness widening of old methods. require_graph compares existing Graph stamp
with the same view.at, only called when projection uses graph. The coordinator
must retain the actual graph lease first. The facade's capture_exposure uses its
own MetadataStore and actual view.read; no caller-selected owner/rawTX escape.

`engine/read_payload.rs` extracts the unchanged row/namespace/text-copy body into
hydrate_point_from_view. Existing hydrate_get_at retains its effects_stamp guard.
New with_point_get invokes that body under the same borrowed PointRead, then its
synchronous closure can copy donors/projection/witness. Its caller must retain
engine.read (selected-file ownership) and any needed graph lease; no stdlock
crosses await. Actual coordinator and donor/StrengthContext wiring are NOT added
in this draft; tests must verify they obey these contracts before shipping.

## Literal A3 diff

AccessRecorder adds opt-in record_point_for_publication, whose default is an
explicit error. StorageAccessRecorder dispatches by actual owner format. Legacy
uses the same scoped writer/direct classification/original API/sampled time and
retains its current clamp/live-identity checks, not native captured-policy parity.
Native requires the actual ExposureIntent for the same ID/name/source and the
existing completion owner. No optional native witness fallback.

DerivedCompletion::record_native_point_exposure_with_work checks one Direct UUID
and source, registers accepted work, then passes the original intent into the
same native_exposures_accepted blocking worker. That worker retains publication
-> graph -> storage leases, final native record_native_exposure_with_work,
ObservationError/full receipts/unknown and publication fallback. Captured mode
cannot switch to fresh capture; existing callers pass None and retain their
old fresh-capture semantics. Intent fields are not rebuilt from current policy.
The final native writer still validates capture lineage/policy/record tokens and
allows unrelated generation advances. No graph CAS, public CAS or new codec.

## Required next work, not claimed here

A1/A3 literal hunk ACK precedes execution. Then real focused tests on isolated
stores: point canonical_applied/derived_dirty vs canonical_pending; unrelated
stale indexes; explicit graph current/stale; owner-bound capture; unchanged old
getter; final policy-only change with unchanged target token; target/terminal
change; unrelatedG allowed; one clock; false/missing/terminal skip; original1/2;
known/unknown receipt and accepted cancellation behavior. Preserve genuine
BEFORE by running probes against the immutable old source, with a source-specific
binary/target strategy. No original public2 rejection or legacy shape is a bug.

Only then wire actual prepared coordinator/typed GetReply and direct MCP/REST;
remote completion needs Huygens's literal shared dispatch/capability release and
validated R1 one-connection exchange. No current transport/hook/handler calls
these new seams. This is explicitly incomplete, not suitable to merge by itself.

Validation here: read-only source inspection and git diff --check only. No Cargo,
formatter, tests, containers, downloads, private cache or raw experiment data.
Cleanup:0bytes reclaimed; preserve worktree/Git/docs and current source draft.
