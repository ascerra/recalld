# Public sweep clock boundary: compatibility plan and HTTP RED

Status: actual HTTP RED retained; runtime correction approved and implemented,
validation pending. The existing endpoint behavior is intentional old API1
historical-seeding support, conflicting with the agreed A3 §3 clock boundary.
This is not an exploit or evidence of an unintended historical implementation bug.

Sole actual parent: PR147 `50da71abee66cea159988a203d286f570f091718`, which already
contains135's manual origin and145's native physical compaction. Worktree
`/tmp/recalld-implementation/public-sweep-clock`, branch `impl/public-sweep-clock`.
No153,156,owned-core,adoption or general integration parent is needed for the HTTP
probe. None of those frozen branches or published studies is modified.

## Actual caller map and compatibility impact

- `api/routes.rs` exposes POST `/decay/sweep` on the ordinary HTTP router.
  `TriggerSweepRequest` is camelCase serde, so the accepted wire key is `asOf`.
  `handlers::trigger_decay_sweep` forwards `req.as_of` directly.
- `FsrsEngineAdapter::trigger_sweep` selects original1 and delegates to
  `trigger_sweep_with_origin`; its caller-supplied time replaces the configured
  clock sample. The existing runner accepts one sampled time, performs actual
  phase writes and, on147, native selected text compaction. Missing/null asOf
  currently selects the clock; snake_case as_of is currently an ignored unknown
  field, not an implemented timestamp alias. Wrong typed asOf values currently
  fail serde extraction before work.
- No production MCP or daemon sweep-time RPC caller was found in the bounded147
  Rust source. The HTTP handler comment explicitly advertises use by a replay
  driver for historical seeding. No executable caller of this HTTP override was
  found in147's tracked scripts or the read-only research scripts searched here;
  that is not a claim no external API1 client uses the advertised field.
- Existing Rust diagnostic/test callers pass a sampled time into the runner or
  private manual adapter. Keep these internal APIs distinct from the public HTTP
  request. Autonomous startup/timer remains explicit original2; ordinary manual
  HTTP remains original1. Never infer either from physical schema.
- A6 owner confirmed published154/d516 does not use this HTTP override. Its
  private supervisor advances the existing owned clock, then invokes sweep with
  deadlineMs. `bench/frozen/sweep.rs` samples that same clock once under its
  controller permit/accepted work and calls the same existing runner. No new
  clock/control/harness dependency or change to frozen study inputs is needed.
- Huygens and Feynman confirm no active edits/ownership hold on this HTTP/time
  seam. Huygens retains future public2 selection/schema and separate unknown-error
  envelope work. Feynman retains inherited-policy adoption and compaction proof.

## Proposed correction for review, after actual RED

Reject supplied public time overrides before calling any decay adapter, sampling
clock, registering accepted work, reserving IDs or creating transactions. Reuse
existing `AppError::BadRequest`: HTTP400, code BAD_REQUEST, field asOf, concise
message `Public sweeps use the configured clock; remove asOf/as_of`. Isolated
control and historical-evaluation instructions stay in documentation. Do not silently ignore a supplied override.

Proposed presence semantics: reject both `asOf` and `as_of` whenever present,
including null and nonnumeric values. This intentionally also changes old null
and ignored snake_case behavior; make that compatibility impact explicit. Preserve
omission `{}` and unrelated API1 unknown-field compatibility. A small request-local
raw field map/presence check can preserve that behavior without a new shared
control type. Main approved these presence semantics and request-local parsing; no new shared
control type is introduced.
Do not make permission depend on schema, lifecycle policy, API number, localhost,
or a guessed client identity.

After prevalidation, ordinary HTTP calls `trigger_sweep(None)` and captures the
same configured clock exactly once. The internal sampled-time runner remains
available to the isolated owned-control consumer. Do not add a parallel clock,
new public time endpoint, mutation retry, synthetic receipt, durable skipped noop
or a second scheduler. No change to original request hash domains is proposed.

This is an intentional compatibility restriction for API1 clients using explicit
asOf; they must use ordinary now-based sweeps, or the existing isolated evaluation
control for historical work. Published studies keep their frozen code/inputs.
No claim that the old endpoint violated its own prior advertised contract.

## Probe and later validation scope

The test-only RED uses the actual full HTTP router, real legacy insertion and
real native schema migration/attestation, configured clock and publication owner.
A Full record is fresh at the owner's time1000. POST with500days later asOf must
be rejected under A3; record status/phase/text/generation and clock calls before
asserting. An omitted-time comparator must succeed using exactly one clock sample
and retain Full/text on both physical formats. No raw native rows or fake backend.

Freeze probe source, package-clean only our own target when switching mount from
156 source, rebuild the actual test executable, record source/executable hashes,
and preserve RED logs. Offline Rust1.94.1 Docker only,2CPU/4GiB/jobs2/networknone,
no home/credentials/socket mounts. Reuse one private Cargo cache and the protected
`/tmp/recalld-a3-matrix-target`; do not clone caches or recreate released paths.

After runtime design approval, add rejection/no-work tests for both spellings,
null/types/representable extremes, actual failure-shaped body, unchanged records/
receipts/ledger/generation/tracker and no clock call; retain omitted-time comparator
and existing manual/autonomous origin/compaction/reopen tests. Relevant full checks
are warranted for a real runtime change. The current task stops at reviewed plan
and actual RED; it does not claim the correction or compatibility decision landed.


## Actual retained RED

Probe source `ebb8990de645da88186e84efe817a7b91830f7da` on147 produced1PASS/2FAIL
in1.41s. Both explicit-asOf HTTP requests returned200, sampled the configured
clock0 times, moved Full→Summary and stripped text20→0. Legacy G3→5; native
G4→6. Omitted-time controls returned200, sampled once and retained Full/text/G.
This confirms the intentional old endpoint behavior against the new agreed
boundary; it does not relabel that behavior as an exploit or an unintended bug.
Exact source and all emitted executable hashes were verified; proof is retained
under `work-notes/public-sweep-clock/before/`. Only this package's cached outputs
were cleaned on the owned target before switching source mounts; dependencies
were retained and no other agent target was touched.
