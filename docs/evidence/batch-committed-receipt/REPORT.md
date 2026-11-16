# Preserve batch receipts after certification publication failure

Runtime: `6cbb2e53d05239c2bc68f34940666243178afbf1`.
Assigned base: `9ed1cb8b5d6be5dfbdf2edb73ac3a9c10faddeaa`.
This is an isolated patch for the existing PR181 composition, not a new helper
PR, public activation, or a claim of full A4 completion.

## Defect and resulting behavior

The rejection finalizer commits the indexed error, batch progress, full
Catalog-scoped `BatchItemRejected` receipt and generation together. Certification
publication then can fail with `CommittedMutation`. The batch runner previously
swallowed that typed error into its generic unknown branch and omitted the new
bookkeeping receipt, even though strict reopen confirmed the rejection committed.

The runner now checks the in-hand receipt's store, header, parent, index and
original error, retains it in `bookkeepingReceipts`, preserves the reserved child
operation ID, reports the original indexed error as nonretryable, and marks the
response unready. It returns immediately without reading through the unavailable
owner or starting later children. Previous successful child receipts and previous
bookkeeping receipts remain intact. An unexpected typed receipt propagates as the
original typed failure; it is not relabeled as the current rejection.

A second actual failure occurs when reservation commits but certification
publication fails. The generic receipt field caused API1 Remote to downgrade the
reservation to unknown. `BatchFailure` now places a typed `BatchReserved` receipt
in the existing `reservationReceipt` field, with `READINESS_UNKNOWN` and no drain.
API1's existing caller-hash/store/header/parent binding remains required. The
public API1 error explicitly identifies `commitEvidenceScope: reservation`: this
does not prove any child succeeded or make the batch atomic. Other receipt
variants and actual commit-unknown/worker-unknown mappings remain unchanged.

`WriteReceipt` has no scope field. Catalog binding is established by the actual
typed outcome and, in the regression, canonical lookup under Catalog with a
negative namespace lookup. No new scope field, codec, hash domain, stored proof,
registry, exposure code, or receipt readback was introduced. The readiness
consumer's older comment mentions historical oversize reservation evidence; the
same no-drain branch now also handles reservation publication failure.

## Actual RED and GREEN

The original source finding became five executed REDs on
`09900e49588fdca40b9816a2b73cb862703efa38`: first rejection, later rejection,
rejection after a successful child with a later pending item through real Remote,
local reservation, and real Remote reservation. Every test first closed the
poisoned owner, reopened through production validation and proved the exact
durable progress, receipts and generation. Only then did receipt-retention
assertions fail. This is not a synthetic error-only or authored JSON proof.

The initial source-only fixture idea (`summary: false`) was corrected before
execution: those errors are finalized inside reservation and cannot reach this
boundary. Actual tests use valid input with a nonexistent parent UUID, which
reserves successfully and fails execution admission before provider calls.
Pascal reviewed the existing instance-local Nth post-TX publication hook; no new
fault seam was added. N1 is reservation, N2 first rejection, and N3 later rejection
or rejection after a successful child, confirmed by durable generation/receipts.

On exact runtime `6cbb2e53`, **7 tests passed, 0 failed**, including the original
five repaired cases plus actual lost-ACK controls and prepared API2 MCP/REST
responses. Tests retain original-contract caller identity, receipt order and
scope, original child IDs, preceding success and later pending slots. Reopen and
same-key replay preserve final outcomes; already finalized work adds no IDs,
generation, history/file bytes, provider/profile calls or sampled time. A genuinely
pending item may execute once. A second complete replay is byte-identical.
Mutations of actual reservation receipts and changed caller intent remain
unverified/unknown; no forged evidence upgrades commitment.

Prepared API2 tests exercise actual private MCP/REST entry points and the native
owner with original contract 2. They do not assert ordinary public API2 activation.
API1 Remote tests use the actual negotiated daemon connection and shut it down
before strict reopen. Existing whole-response framing tests remain the transport
budget proof; the new small fixtures are not a universal receipt-size bound.

An additional **16 affected tests passed**, for **23 unique passing tests** on
exact runtime `6cbb2e53`. They cover malformed evidence, actual RPC frame
admission, final-barrier failure/replay, projection bounds, historical oversized
completed/pending replay, CAS, genuine unknown-child recovery and MCP/daemon
parity. Fmt and all-feature/all-target clippy exited 0; clippy reported warnings
(1325 in the lib-test summary, 1168 duplicates), not a warning-free result.
Per-test results, raw-log hashes and cgroup counters are in `RESULTS.json`.

## Source and validation attribution

- Halley's released `5d550ff7c21488100190290e9681a796a0eb84ca` changes two inherited
  test vector constructors to `VectorIndexOwner`, preserving wrong/right widths.
  Local cherry-pick: `e20f6c34`.
- Released `92e8829869452aea9adde9f8600ccec6275b91dd` adds absent optional native
  fields to two inherited legacy test constructors. Local cherry-pick: `1411ee2b`.
- Neither compatibility tail changes production behavior. No unvalidated moving
  queue branch or unrelated parent was imported.
- Feynman and Curie independently gave bounded source ACKs for exact `6cbb2e53`;
  these are source reviews, not independent test execution.

Tests use immutable read-only source snapshots, `recalld-ci:1.94`, toolchain
1.94.1, locked/offline Cargo, network none, two CPUs, 4 GiB and two build jobs.
No home, credentials, daemon socket or personal-store mounts. One existing owned
cache/target is reused. Before switching source, the RED executable was hashed
and the package cleaned in that owned target; no source-switch stale executable
claim. Both 434-file snapshots are verified before/after execution.

RED executable SHA256:
`6e93244349c84c07dea73b6ca948165983f347bbfec26639706a35a0870b2554`.
GREEN executable SHA256:
`978d03f460a94d9768c66b318e16c50c272572fc74c47621028d33f827b29d6b`.
RED: 36.28 seconds compilation, 10.03 seconds tests. GREEN: 36.88 seconds
compilation, 16.99 seconds tests. Both cgroup peaks reached 4294967296 bytes;
memory.max events were 2379 and 1393 respectively, with oom=0 and oom_kill=0.
These are reclaim-pressure observations, not OOM failures.

## Integration gate and limits

**No child full-suite result is claimed.** Halley separately reproduced an
inherited default-stack overflow in the legacy API1 batch path. His initial
forwarder candidate `6ffdc860` also failed and was not imported here. The user
approved bounded validation and handoff of this independent receipt patch.
Queue PR181 must run its final actual full suite after composing this patch and
the separately validated stack correction. This patch alone is not PR181 READY
for publication or evidence of public API2 readiness.

## Retention and cleanup

No raw archive or cache clone. Containers use `--rm`; completion verifies they
are gone. No raw deletion or space-recovery claim in this slice. Preserve the
failed RED and GREEN roots until coordinator clearance:

- `/tmp/recalld-huygens-active-validation/batch-receipt-001-red`
- `/tmp/recalld-huygens-active-validation/batch-receipt-002-green`
- Sole owned target: `/tmp/recalld-huygens-active-validation/target`
- Sole owned Cargo cache: `/tmp/recalld-implementation/compaction-review-7281918/cargo`

Central cleanup has the protected paths. Code, worktree and compact source/result
manifests remain durable; raw release follows integration/results clearance.
