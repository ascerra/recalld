# Preserve unverified Remote single-write receipt evidence

Runtime: `541de23d429b2b4fc26db7932cf34ce3baa22aeb`.
Exact released queue base: `b694d4f2f6fa067eaeef88b360db527060b05f9e`.
This is an isolated patch for the existing integration composition, not a new
helper PR, public2 activation, or full A4 completion.

## Actual defect and correction

The capable API1 Remote decoder extracts a typed receipt from a malformed owned
reply. Previously it wrapped that receipt in `CommittedMutation` and delegated
through the single reply's shared batch error mapper. For `BatchReserved`, that
mapper correctly relocates the receipt into the batch reservation field, but the
single reply copies only the ordinary receipt. The result lost the full evidence
and incorrectly attributed commitment and the unrelated parent's operation ID to
the current single write.

The correction is at the malformed API1 decoder boundary. A separate
`Reply::Uncertain(WriteReceipt)` retains the complete typed evidence without
authenticating its association with the current request. MCP and REST project
`receivedEvidence: { receipt: ... }`, `COMMIT_OUTCOME_UNKNOWN`, `commitState:
unknown`, and `ready: false`. Current-write `receipt`, `operationId`, `readiness`
and success data are absent. Warnings explicitly identify unverified received
evidence and unknown readiness. Own-variant decoding/serialization preserves the
receipt across another hop. Untyped malformed receipt data invents no evidence.

This does not widen `native_single::Reply::failure`, change `BatchFailure`'s
reservation mapping, validate untrusted evidence as a current receipt, perform
readback, retry a mutation, add a receipt reference/registry, or activate public2.
Ordinary API1 responses omit the new optional field; legacy successful shapes and
valid known-commit paths retain their existing projection. The HTTP consumer
handles the explicit uncertain variant rather than discarding its evidence.

## Executed RED and fixture provenance

On `2019500b` the new same-connection regression failed after its durable and
transport controls passed. It creates a genuine legacy batch reservation with
one invalid item, verifies generation advances from 2 to 3, closes the owner,
reopens through production validation, and checks the exact retained parent
receipt. After closing again it inventories every file recursively. The Remote
test uses an actual Unix connection and negotiated capabilities, sends one
`store_api1_v1` request, then receives a deliberately malformed wrapper containing
that unrelated genuine parent receipt. The next request must be ping, never a
legacy mutation fallback. All store-file bytes remain identical.

The original projected response contained `commitState: committed`,
`INDEX_NOT_READY`, and the unrelated parent's operation ID, but neither a full
receipt nor received evidence. The full-evidence assertion failed: **0 passed,
1 failed**. Source, executable hash, raw projected response and failed assertion
are retained. This is genuine persisted parent evidence carried by a malformed
test server; it is not a claim that a valid local single-write producer returns
`BatchReserved`, or that this test server committed the current single request.

## GREEN and limits

The same regression passes on frozen runtime `541de23d`, including exact typed
receipt equality, current-write unknown attribution, no fallback, unchanged store
bytes, own-variant redecoding, MCP text/structured equality and REST typed-failure
serialization parity. The focused sample's complete serialized sizes were MCP
1,587 bytes, REST 728 bytes and daemon JSON 560 bytes. Hash/UUID contents vary per
run; these small-fixture measurements are not proof all possible receipts fit.

The malformed Remote case exercises the actual connection and MCP projection.
Its REST check uses the same typed failure serializer as the HTTP consumer, not
an actual malformed Remote HTTP request. Existing actual HTTP success, profile
constraints and known-receipt failure regressions are validated separately.

**66 unique affected tests passed, 0 failed.** Filter invocations included 12
API1-store tests, 23 native-single tests, 7 batch postcommit tests, 2 malformed
batch tests, 20 protocol tests and 7 same-connection capability tests, plus the
focused regression. Overlapping names are counted once. The native-single filter
also reported one existing ignored test requiring immutable BEFORE123 artifacts
through `RECALLD_SINGLE_BEFORE_ARTIFACTS`; that historical dense-fixture test was
not executed here. No historical universal frame-size claim is transferred.

Fmt and all-feature/all-target clippy exited 0. Clippy reported warnings (1,239
library; 1,342 library-test, 1,180 duplicates), not a warning-free result.
Per-test names, raw-log hashes and actual cgroup counters are in `RESULTS.json`.
Final composed PR181 validation and exact-head CI remain the queue owner's gates;
no child full-suite claim is made.

Both 438-file snapshots were verified after execution; all GREEN files also
match the frozen runtime worktree. Executable SHA-256 values:

- RED, 150,410,296 bytes:
  `dc25a1a76cee1080caf42282e4dbf2f096d3e6671bde2f7b1ea062eff9ae044c`.
- GREEN, 150,411,928 bytes:
  `fdc6eec105b795e20746d29fdc096e9f0ff109a51d747af80a665e95c8c1baa1`.

## Ownership, resources and cleanup

Halley explicitly released these isolated hunks to Huygens; the active queue
worktree and its immutable validation were untouched. Feynman confirmed his
source ACK against exact runtime `541de23d` and found no source blocker. This is
bounded source review, not an
independent executed test. The coordinator approved the work under the user's
standing authorization.

Both runs use read-only 438-file source snapshots, Rust 1.94.1 in
`recalld-ci:1.94`, locked/offline Cargo, network none, 2 CPUs, 4 GiB and two build
jobs. No personal data, credentials, home or daemon socket are mounted. The RED
executable was sealed before cleaning only recalld in the same private target
and rebuilding GREEN. No dependency cache clone or second target was created.
Both runs reached the 4 GiB cgroup limit with reclaim events (RED 3,861; GREEN
4,218), but `oom`, `oom_kill` and `oom_group_kill` were all zero. This is observed
reclaim pressure, not an OOM. All three owned format/RED/GREEN containers were
automatically removed; a post-run Docker name query returned no containers.

Protected raw roots are
`/tmp/recalld-huygens-active-validation/single-remote-001-red` and
`/tmp/recalld-huygens-active-validation/single-remote-002-green`.
The central cleanup owner has the parent protection record. Failed evidence
remains protected pending coordinator clearance; passing raw may be released
centrally after integration/results clearance. Worktree/source/reports remain.
One owned cargo cache at
`/tmp/recalld-implementation/compaction-review-7281918/cargo` and target at
`/tmp/recalld-huygens-active-validation/target` are retained for active work.
No raw HOME archive or raw deletion was performed; 0 bytes reclaimed here.
