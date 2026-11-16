# Explicit owned legacy sweep: frozen implementation plan

Base: exact PR125 `0b50168b64ad353e4f7a53c6459c270fe8500070`.
Branch/worktree: `impl/owned-legacy-sweep`, `/tmp/recalld-implementation/owned-legacy-sweep`.
No PR124/118 native producer or PR115 checkpoint ancestor is required. This
adds an actual control consumer, not another planner, clock or registry.

## Private interface and scope

Keep controlVersion 1 and publicContract legacy-v1. Add owned-only
`sweep {deadlineMs: 1..30000}` and readonly `sweepStatus {}`; reject unknown
fields/types before admission. Capabilities advertise `explicitLegacySweep`
and `sweepControlVersion: 1` only for the owned legacy constructor. No actor tool
is added. Native sweep, general controlledClock/RNG, snapshot/reset and full-A6
claims remain unchanged/false on this parent. Autonomous sweeps stay disabled.

Reuse `DecaySweepRunner::execute_sweep_with_work` with the production default
SweepConfig/DecayConfig/ActivationConfig, actual configured global multiplier,
a single sampled core clock, registered parent WorkTicket and existing
DerivedCompletion owner. Preserve its phase ordering, policy/record rechecks,
compaction descendants and publication behavior. Reject native owners explicitly.
No copied transition formulas or independent index rebuild/publication logic.

## Admission, cancellation and results

Use the existing FrozenControls busy permit. Accept/register a root before
sampling time/queuing the worker, then pass that root as parent. A sweep is a
mutation, so it must not hold a freeze/drain lease while waiting on its own root.
The coordinator retains the permit and core handles until its worker joins;
handler timeout/cancellation cannot release them. Clock advance, capture and
another sweep cannot race a surviving sweep worker. Finish the parent only after
its descendants/completion; unknown work remains unresolved under existing rules.

After successful sweep completion and root finish, use the actual readiness
barrier at the actual canonical GenerationRef, under the same monotonic deadline.
Use its join-retaining `with_frozen_core(Some(target), ..., ready)` implementation
inside the retained coordinator; do not add an inner handler timeout that could
return while a blocking readiness capture still owns leases.
Report actual sampled time, sweep counts/errors, available full known committed
receipt and uncertain operation identity, and barrier readiness. Any sweep error,
publication failure or unavailable barrier is a failed control, never a ready
success or a safely retryable fresh mutation. Preserve receipts across later
readiness failures. No invented aggregate receipt or rollback claim.

A single in-memory diagnostic slot retains this control request ID and its
running/final report, exposed by sweepStatus even when capture is unavailable.
It is not canonical receipt storage or a readiness registry. This prevents a
late worker result/known receipt disappearing after handler timeout. A timed-out
or failed sweep invalidates the episode and blocks another sweep, advance or
capture on that owner (including a later checkpoint consumer);
status remains inspectable. A fresh owner is required for another episode.
The deadline bounds cooperative response waiting, not blocking OS I/O or worker
termination. No late success is published to the expired caller.

## Required proof and handoff

Isolated Rust regressions: actual one-phase-per-sweep legacy transitions,
zero-multiplier NOOP, sampled timestamp, real completion/barrier; graph/worker
latch then timeout/caller cancellation, advance/capture/retry blocked until
worker join; known committed receipt retained through actual effect/publication
failure; invalid/unsupported requests leave canonical state unchanged.

Actual private-control/public-MCP consumer: synthetic loopback embedding fixture,
public stores/false fetches only, explicit advance and repeated sweeps, phase
payload loss, three fresh serialized runs, raw requests/responses/receipts and
failed attempts retained. No private ingestion. Helper owns the larger supervisor
and its later reset/chronological join. Its existing PR117 dependency remains
necessary for raw FTS score/reset parity; this minimal parent does not claim it.

Run focused checks first, then appropriate full/fmt/clippy/public checks in
network-none Rust1.94.1 Docker, 2CPU/4GiB/private source-specific cargo/target.
Stop scheduling heavy work when the announced performance hold begins, let
existing jobs finish, and ACK drain. No current measurement hold is active.
