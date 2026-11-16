# Startup ACK batch candidate

Status: focused and amended full validation passed on the unchanged runtime
freeze below. Sealed for publication review; not published or READY for
integration, and no performance promotion claim.

## Source and dependencies

Runtime/test freeze: `72b9a51ff0f0e35c83fb77bb1e3a3dc20715368e` on
`impl/startup-ack-batch`, direct main
`fe1dacbb53b0dca6dfdb1b765297c834d4addb3e`. Plan `ac87c345` was reviewed before
implementation. Halley released the ACK/caller hunks; Feynman reviewed both the
plan and actual runtime diff with no A1 source blocker. No PR176, vector-owner,
catalog, query or native-activation prerequisite is added.

The production change is in `src/storage/metadata/effects.rs` and its actual
`DerivedCompletion` caller. Other Rust changes are cfg(test) counters, faults and
regressions. The old per-ID ACK implementation is unchanged outside cfg(test)
counters. Old unchecked bulk conveniences are now cfg(test)-only, used by explicit
low-level fixtures; there is no unchecked production bulk entrypoint.

## Behavior

The old rebuild path revalidated the entire remaining outbox before and after
every individual deletion, with one durable commit per deletion. The new bulk
entry requires the actual rebuilt GenerationRef and all four existing view stamps
to match actual canonical G under the same native/legacy MutationGuard. It runs
the unchanged effects/receipt validator once and requires every effect to have
completed canonical application. It compares exact snapshot row bytes in the
write transaction, removes only those validated IDs, rejects any leftover row,
and commits the retirement once. Empty input validates G/views but does not commit.

The guard remains held through validation, commit and dirty-state update. It
excludes effects-only writers as well as G-advancing writers; G equality alone
is not the safety argument. No canonical generation, record, history, receipt,
profile, schema, vector, ranking formula or durability mode is changed. The ACK
does not publish view stamps. Unknown commit poisons the owner and retains
unresolved work. Cancelling a waiter does not cancel its already-owned worker
or establish a successful drain. See the [writer audit](startup-ack-writer-audit.md).

## Actual work-count BEFORE and focused result

The test-only BEFORE source was `31947ec00ff8292e430245fa6f3f92b72102746a`,
executable SHA256
`68fcdc589af6dddc3715e53612e93df46ff087c0177c1b2b8eb590070fb3fab8`.
The real publisher completed its rebuild and preserved G, then failed the
predeclared linear validation-row bound. This is an actual work-bound RED, not a
storage/ranking failure or the A6 pilot timeout.

| Actual batch | Effects validation calls | Decoded effect rows visited | Successful ACK commits |
| --- | ---: | ---: | ---: |
| BEFORE, 32 effects | 65 | 1,056 | 32 |
| Candidate, 0 effects | 1 | 0 | 0 |
| Candidate, 1 effect | 1 | 1 | 1 |
| Candidate, 32 effects | 1 | 32 | 1 |
| Candidate, 256 effects | 1 | 256 | 1 |
| Candidate, actual native owner, 2 effects | 1 | 2 | 1 |

The store-local cfg(test) probe begins at actual batch entry, before its guard,
and includes nested calls to the full effects validator. On this exact parent
the native guard validates control/serving lineage, not the lifetime outbox;
the native test confirms one effects-validator pass across batch entry. Counts
exclude setup and the publisher's earlier scan. They are not total startup work.

The candidate also performs two keyed raw-byte lookups and one removal per
nonempty effect. These source-supported operations are additional to measured
decoded-row counts. Existing sorting remains, and redb lookup/write/commit costs
are not measured by these counters. No claim of total linear CPU, physical fsync
count, latency, throughput, RSS improvement or optimization-gate completion.
The optional 1,024 case was not run; 256 is the measured large regression case.

Focused tests cover actual publication and reopen, exact retained receipt/record
bytes and G, empty/idempotent completion, actual native typed effects, malformed
effect/receipt/scope and unapplied rejection, stale/foreign G and invalidated view,
same-G changed row/extra row atomic rejection, real competing ACK guard exclusion,
transaction abort/lost result, process exit before/after commit, cancelled caller
with surviving worker, and accepted completion after root admission closes.
Fault-injection tests deliberately modify private tables; their view stamps come
from a preceding actual publisher, never manufactured readiness.

## Validation record

- BEFORE: 1 expected failing test; compile succeeded and pre/post source manifests
  matched. Compact output is in `evidence/startup-ack-batch/before`.
- First candidate focus `e3a39af5`: compile failed on two test-only issues (redb
  error conversion and scoped-thread capture). No runtime result was claimed.
- Corrected focus `c14938ea`: 11 tests passed, including the crash-child helper.
- Final freeze `72b9a51f`: 12 focused tests passed, including that helper and the
  final accepted-child drain regression. The original 900-second full invocation
  timed out with 1,190 unique completed test lines, no observed failure and no
  suite conclusion. Driver exit was 1; cargo exit was not recorded by that
  timeout handler. Post-source verification matched all 7,003 files.
- The explicitly authorized full-only follow-up retains the same source and
  built binaries, with a 30-minute full limit and a separate combined 10-minute
  fmt/clippy limit. No focus/1,024-row rerun or source change. Full all-targets
  exited 0 in **1,035.154 seconds**: **1,378 library tests passed, 1 ignored,
  3 CLI integration tests passed, 15 example tests passed**. The ignored test is
  the existing `legacy_api2_generate_genuine_before_one_fixture` generator,
  reserved for its recorded pre-correction source. Fmt exited 0 (0.795s), clippy
  all-targets exited 0 (21.394s). See the
  [budget amendment](startup-ack-validation-budget-amendment.md) and
  [structured summary](evidence/startup-ack-batch/validation/full-002/SUMMARY.json).

Warnings are retained, not treated as errors or claimed baseline-only. Cargo
reported 95 library warnings and 60 library-test warnings (22 duplicates);
clippy reported 1,088 library warnings and 1,160 library-test warnings
(1,050 duplicates), plus binary/example warnings. The compact evidence includes
warning lines and the full raw clippy log hash. No all-features check was run.

Final focused executable SHA256:
`f96a48baf315f12863ad26751d34ceabd5f9290ae668d5ba74326bf51bfca396`.
The follow-up uses that same artifact and verifies all package executable
hashes before and after. All **7,003 source files and 14 executables** matched
inside the container before full execution, after full execution, and after
fmt/clippy. Cargo reused the existing test artifacts (0.42s setup, no package
compilation). Each source revision has an immutable committed-source
export, inside-container pre/post manifests and a package-only clean of the
single owned target. The timeout-only follow-up reuses the identical physical
export and binaries without cleaning or switching source roots. No mutable mounted source or source-switch stale-binary evidence.

Docker image inspected during final execution:
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`;
Rust 1.94.1, 2 CPUs, 4 GiB, jobs=2, network none. Only isolated source/output and
the agent's cargo/target were mounted; no host home, credentials or socket.
Local intended scope is **all targets, not all features**. Shared-host execution
is correctness/work-count evidence, not a quiet performance comparison.

Final follow-up cgroup counters include verification, full validation and checks:
CPU usage 1,645,813,335 microseconds; memory peak 4,294,967,296 bytes; memory
`max` events 11,496, OOM events 0 and OOM kills 0. These include charged page
cache and are not process RSS or ACK-only attribution. The original partial
run's separate late sample peaked at the same cap with no OOM kill; it was not
a final snapshot. Another owner announced concurrent bounded validation on
the shared host. No speedup, memory improvement or scan/commit dominance is
inferred. The driver exited 0 and its own container was removed.

## A6 boundary and cleanup

A6 sealed diagnostic evidence was imported from `e5db5f2d` via exact Git objects;
[HANDOFF.json](evidence/startup-ack-batch/a6-handoff/HANDOFF.json) records hashes.
That diagnostic observed at least 2,816 of 5,183 ACKs at ACK-local 55.916391s before
a 60-second wall timeout; final count is unknown and no query ran. It localized
the ACK stage but did not measure validation-versus-commit dominance.

The old `096195d9` candidate, failed pilot, seed, executable and quality result
remain unchanged. This leaf cannot replace that study arm silently; any real
5,183-row A6 startup use requires an explicit new candidate amendment. No pilot
retry, predrain, benchmark window, public activation or default promotion occurred.

Retained completed run roots under this worktree: `.validation/before-001`,
`focus-001`, `focus-002`, `final-001`, `full-002`. The sole retained cargo/target are listed in
the plan and central cleanup ledger. Completed raw release waits for PR/results
and any integration input handoff. No raw HOME archive is required. Cleanup by
this leaf so far: no deletion, zero reclaimed bytes. Worktree/Git/compact evidence
and the original A6 inputs remain protected.

Compact report/plan/audit/validation evidence is also copied and hash-verified
at `/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-16-startup-ack-batch-72b9a51`.
The repository `evidence/startup-ack-batch/HOME-COPY.json` records the copy
manifest hash after sealing. No database, binary, cache or raw archive is copied
to HOME. No runtime work remains in this validation scope; publication review,
exact-head CI and integration remain separate gates.
