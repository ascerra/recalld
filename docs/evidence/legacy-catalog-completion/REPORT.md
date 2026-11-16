# Legacy catalog completion

Runtime: `c722318a2c3d4091887d7c938d0bbc21434a7e25`.
Direct parent: main `b0d19d1d40c5dcecb47cd4fa070ac13ba47b722c`.
No PR170, native-policy, or other convenience parent is included. The measured
PR170 source `fb6a4aa36fbf9cc9637a9cb313b1578fbec67781` has identical Rust source
and Cargo manifests to that parent.

## Behavior

Actual MCP and HTTP legacy namespace creation now uses the existing accepted
`DerivedCompletion` owner. A successful create publishes its actual catalog
generation through the existing four derived publishers before returning.
The worker retains its accepted child and publication read lease across caller
cancellation, then releases that lease before exclusive completion. There is no
warmup query, direct stamp acknowledgement, new registry, or public API2 activation.

Known publication failure preserves the complete retained `NamespaceCreated`
receipt through HTTP503 and MCP JSON text/structured content, including the actual
framed daemon transport. Lost commit acknowledgement preserves its actual operation
ID with unknown outcome and no invented receipt. Worker loss has unknown identity.
The storage-only HTTP adapter constructor remains valid for reads; creation now
requires `NamespaceRegistryAdapter::from_system` and fails before work if unbound.

The final correction restores HTTP's original pure `resolve_policy` call and
concrete `LifecycleError::InvalidInput` before accepted work or clock sampling.
Namespace creation does not call an embedding provider. The native/legacy physical
format gates and original policy-validation semantics are unchanged.

## Validation history

| Run | Source | Result and interpretation |
| --- | --- | --- |
| BEFORE fb6 | measured integration source plus isolated probe | Intended-success assertion fails: retained create advances G2 to G3, all four views remain G2, and the next empty `recordAccess:false` MCP recall fails coherence. |
| Focus001 | first implementation | 5 pass, 1 fail. Real public create/read succeeds, but an overbroad whole-directory byte assertion detects SQLite SHM reader coordination. All ten files and the precise differing byte are retained. |
| Focus002 | test-only invariant correction | 10 focused passes, including actual MCP and HTTP empty create/read, full receipt failure through framed daemon, lost acknowledgement, cancellation, invalid/duplicate/unbound, clock and closed-admission controls. |
| Full003 | `c25f65f03ee5f92b9fb599aac3de1b4577bc2b31` | 1271 library pass, 1 fail. A real typed-error compatibility regression: the existing HTTP adapter assertion expects concrete `LifecycleError::InvalidInput`. Fmt and clippy exit0 with warnings. Cargo stops before CLI tests. |
| Final004 | `c722318a2c3d4091887d7c938d0bbc21434a7e25` | 1272 library + 2 CLI tests pass; 4 doctests remain ignored. All 11 focused checks pass. Fmt and clippy exit0 with warnings. Full library suite: 626.92s. |

The original compatibility assertion is unchanged. Full003 to Final004 changes
only seven production lines in `src/api/adapters.rs`, restoring the original pure
check. No test assertion was relaxed for that correction. Full003's actual failed
library executable hash was captured before cleaning the owned package target.

All runs use isolated Docker `recalld-ci:1.94`, Rust1.94.1, network none, two CPUs,
4GiB memory and two Cargo build jobs. Source is immutable and mounted read-only;
there are no home, credentials, socket, or personal data mounts. One owned Cargo
cache and target are reused. Package cleaning precedes a source switch, and source
and executed-binary identities are retained per run.

Final004 verifies all 316 source files before/after and against the shipping
worktree. Six executed test binaries are individually hashed, including both CLI
integration executables; additional build products are listed separately. The
actual library binary SHA256 is
`b55110b9dae70f5d8b1492e2c1c1fb1a2652ec156421b740807c3188f7bf8a7f`.
All 29 copied final evidence files match their source hashes in
[`final-004/COPY-VERIFIED.json`](final-004/COPY-VERIFIED.json).
Peak cgroup memory was 4294967296 bytes, with `max=1665`, `oom=0`, and
`oom_kill=0`: reclaim pressure, not an OOM. Clippy reports 1078 library-test
warnings including 969 duplicates; its exit status is zero. The completed owned
container was inspected and removed. No further runtime change or full rerun
follows this evidence seal.

## SQLite read-only invariant

Focus001 changed only `fts.db-shm` offset104 from14 to19. Pinned bundled SQLite3.49.1
defines that offset as reader mark1, which a reader may raise to `mxFrame`. Its
source identity, exact ten-file inventory, compact failure log and raw-log hash
are in `focused-001`. The corrected assertion allows only bytes104..108 of SHM to
change and checks that reader mark against `mxFrame`; every other SHM byte remains
identical. Canonical tables, catalog, payload/vector files, receipts, FTS database
and WAL bytes remain checked, together with logical FTS counts/terms. This is not
a claim that an SQLite read leaves the whole directory byte-identical.

## Ambiguous work and receipt ownership

The existing metadata catalog commit-unknown path marks the owner unavailable
before returning. `MetadataStore::finish_work` finishes a ticket only while that
owner is available; dropping an unfinished ticket retains unresolved work. Thus
both child and root cleanup preserve ambiguity. The actual lost-ack regression
asserts `tracker.status().unresolved > 0` for MCP and HTTP independently of the
operation ID response. The worker-panic and queued-cancellation controls also
assert unresolved accepted work.

Any error from `complete_with_work`, including its mapped worker join failure,
is wrapped with the already-held full receipt. The tests execute actual completion
failure and precommit worker panic. Preservation specifically for a postcommit
completion JoinError is source-backed, not a separately injected test claim.

## Limits and integration

This closes legacy namespace-create publication and transport evidence only.
It does not activate native startup/API2, complete A4, certify vectors, or establish
an A5 performance result. PR170 must consume the validated fix and rerun its actual
empty-context path without store warmup. Exact publication-head CI remains required.
Raw run roots remain protected until publication and recorded results; cleanup is
handed to the central owner. The single active cache/target remains protected.
