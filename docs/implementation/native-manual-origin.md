# Request origin in manual and autonomous maintenance

Validated implementation on direct PR107 (`785dfec4`). Additional parent: none.

The existing startup/timer/internal notification runner explicitly passes2 to
execute_sweep_with_origin_work. Existing Rust/manual compatibility entry passes1.
This number is an application request-hash domain for user requests and the
explicit existing internal2 choice for autonomous work. It is not inferred from
physical record format or namespace lifecycle policy.

Both native and legacy producer arms carry the number to the actual canonical
transaction. Native maintenance removes its local2-only check. Legacy phase /
strength maintenance removes its local1-only check. Both retain common1|2
validation, namespace checks, existing physical writer admission and final
snapshot/policy/graph guards. Legacy hard-delete threads origin through the
existing engine effects transaction: new private
`delete_memory_with_edges_with_origin_work(..., u32)` and
`commit_graph_effects_with_origin(..., u32)`; existing compatibility wrappers
explicitly pass1. The shared finish_record_effects signature/child/proof is unchanged.
Unknown numbers fail before producer work and before transaction creation.

The native API adapter consumes exactly the native_policy field and
with_native_lifecycle_policy builder already used on126, added locally rather
than importing unrelated wire ancestry. Native manual work requires that same
configured policy before sampling its existing clock. Legacy manual defaults
retain current behavior. `trigger_sweep_with_origin(as_of, u32)` is the private
explicit entry; existing FsrsEngine::trigger_sweep selects1. Huygens owns future
public2 routing and shared failure envelope; neither is enabled here.

Internal no-change/exempt/stale/missing-summary results remain unkeyed diagnostics.
Legacy physical text compaction remains its existing separate internal operation
and format domain; this slice claims original context for per-record lifecycle
effects, not housekeeping receipts. Native sweeps never invoke legacy compaction.
No supplied key exists on this adapter and no skipped diagnostic becomes a
successful bound keyed operation. A future keyed maintenance manifest needs its
own durable noop semantics. There is no new scheduler, no native legacy
compaction, no performance or ordinary native serving claim.

Passing tests cover actual native and legacy manual1/autonomous2 across phase loss and
hard delete; raw request-hash discrimination, actual scheduler/root completion,
one clock per pass, retained timestamps/anchors, configured native multiplier,
unsupported3 before clock/writes, existing stale graph/policy/partial failure
proofs and strict reopen. The legacy regression retains raw API1 receipts authored
by the current compatibility path, not a historical binary; it compares every
retained byte after new API2 phase and deletion operations. Existing native old2
compatibility proofs remain unchanged. Full source-specific offline Docker passed
1027 library+2CLI tests with4doctests ignored. Four new origin tests and the
existing sweep/maintenance focused checks passed. Formatting passed; Clippy exit0
with737 library/804 test warnings (717duplicates). All231 source inputs were
sealed and verified unchanged. No failed validation stage occurred in this slice.
