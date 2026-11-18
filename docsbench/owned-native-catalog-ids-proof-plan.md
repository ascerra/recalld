# Native catalog/policy allocation consumer: prepared proof

Base `a5865da02969631a5031f65c6e43269701d7a254` (PR124), separate worktree
`/tmp/recalld-implementation/owned-native-catalog-ids`. Feynman's final common
original-context parent is still pending; do not merge WIP or edit his shared
catalog/policy/purge files. Existing guard-requires2 statements describe inspected
baseline only; standalone API1 removals are underway, not frozen final behavior.

The source draft [native_catalog_owned_ids.rs](probes/native_catalog_owned_ids.rs)
is intentionally **not registered, compiled, executed or shipping runtime**.
After the final parent arrives, reuse its existing native_catalog_tests fixture,
provider, Clock and catalog fault injection as a child module; adjust only for
actual frozen interfaces. A migrated fixture prefix is not independent native
genesis. Do not add a production session beside an existing Recalld owner.

## Actual RED/GREEN design

First join the actual final parent with124's MetadataStore IdSource constructor,
receipt wrapper, batch reservations, purge continuations and text-generation
allocations preserved. Register the prepared tests without changing the new
native catalog/policy free-receipt calls. The expected failure is the actual
receipt operation ID/cursor mismatch: free control::record_receipt uses SystemIds,
whereas the oracle consumes one Operation from the owned checkpoint. Record the
actual failure, not a predicted RED. If the final parent already fixed it, report
that fact and do not manufacture a failure.

Then redirect only those actual new consumer call sites to self.record_receipt.
Do not change request hashes/context/proof codecs, numeric policy semantics or
existing production SystemIds defaults. Test both original API1 and API2 only
when the final parent supports them, preserving cross-contract key conflict.

Four prepared test groups cover:

1. Actual native namespace create: one clock sample, row/receipt timestamp,
   generated Operation and checked namespace ID, same-key replay before current
   provider/stale CAS, changed-input/cross-API conflicts with no clock or cursor
   movement. Close the actual owner, restore its captured allocation source into
   the same native constructor, replay without allocation, then create the next
   namespace and verify its exact next operation ID.
2. Native numeric policy update and keyed no-change: one allocation per new
   receipt, no clock/provider calls, actual outcome types, replay/conflict with
   unchanged G/cursor. Unkeyed equal-policy returns None with no allocation/G.
3. Real before_handle_publish known-commit and commit_unknown failures: retained
   operation identity/full known receipt, unresolved accepted work and rejected
   new admission; close/reopen real native recovery, replay without provider/time/
   ID consumption, zero pending accepted work. Retain fault receipts and cursor
   logs. Never infer a rollback or safely retryable error from a failed response.
4. Actual before_stage precommit failure: the inspected producer stages a receipt
   before invoking this filesystem-preparation boundary. Verify no canonical row,
   receipt or generation change, but one reserved ID remains consumed. Same-key
   retry allocates the next ID rather than rewinding the live source. Validate
   this ordering against the final parent before running.

Add focused checks using final common-context purge/batch regressions so allocator
join does not discard original API/cursor/reserved-child proof. Preserve known
precommit allocation consumption rather than rewinding live ID state. The prepared precommit case must be checked against the final actual boundary;
no guessed consumption count. Keep actual default SystemIds and schema2 old-receipt tests.

## Validation and ownership

No heavy validation during the announced PERFORMANCE DRAIN/quiet interval.
Source editing/review only until explicit END; no Docker/format/archive/copy job.
After release and final source handoff, use separate source-specific targets (or
own cargo package-clean) for pre-fix RED and corrected GREEN; retain source and
executed binary hashes, logs and cgroup events. Rust1.94.1 pinned offline Docker,
network none,2CPU/4GiB/private caches/no personal mounts. Focus first, then full
lib/CLI/examples as present, fmt/Clippy and the existing actual public/private
regressions on the exact corrected source. No new performance matrix.

This narrow consumer does not activate ordinary native startup, native controls,
schema2 checkpoint hashing, deterministic genesis or public-v2 serving. Those
remain tracked in native-owned-controls-plan.md on the separate planning branch.
The actual constructor/capture/restore consumer follows the validated ordinary
startup and public adapters, with one Clock/IdSource/registry and no second planner.
