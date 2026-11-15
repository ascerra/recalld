# Private native forget with original application contract

Actual parents: PR129 f504db1a0e98c985bfd5db8b8e760a2f7705e48e (private numeric
origin/parser/executor) and PR131974421582f56641222f78ba18f8c652e93d5b352
(existing native record writer accepts1|2). No exposure/manual/compound/batch join.
Both parents are validated, published and frozen under Halley's integration ownership.
This child changes neither parent branch.

Huygens confirmed exclusive ownership here for focused private lifecycle consumer
proof and any concrete minimal private plumbing gap. Public selectors/routing,
API1 outward response compatibility and shared failure observation remain his.
No public activation is enabled or claimed.

New tests drive actual RemoteStorageAdapter/framing/private daemon for original1/2
forget, CAS and namespace rejection, terminal by-ID scope, cross-origin key
conflict, one clock, replay before changed policy/clock/CAS, strict reopen,
known publication failure/full receipt and repair, actual postcommit unknown
operation identity/reopen/replay, and terminal ledger scope after real namespace
purge. The original publication/migration provenance is never rewritten.
Retired scope is accepted without a new explicitID only when terminal identity
unambiguously resolves it; incorrect asserted name retains the known receipt.
No name-reuse test or public API1 transport-parity claim is implied by this slice.

The only runtime change is consuming PR131's existing five-line guard removal
alongside PR129's already-validated transport. Final validation passed1150 library
+2CLI tests (4doctests ignored), plus filters forget_origin5 (four new tests and
one existing), original_api5, request_context5 and native_forget7. These filters
overlap; they are not22 distinct new tests. Formatting passed; Clippy exit0 with
890library/954test warnings (865duplicates). All277inputs sealed unchanged.

Two failed construction runs are preserved. First failed compilation because an
existing test helper only accepts FixedClock; the correction opens the actual
NativeLifecycleSession API with NeverClock. Second passed4/failed1: forgetting
the only live vector did not exercise the intended dimension error. A surviving
record is now created through the real native insert transaction before the
publication fault. No runtime change was needed; all four new cases then passed.

## Evidence and cleanup handoff

Frozen runtime/testsource `0c82d18cd7adcd051f9c44aaeed206ed2e729302`; evidence
`48ad2c51f35054d5bb5315537cfc7d93181f1477`. Report:
`work-notes/native-forget-origin/REPORT.md`. Actual HOME copy:
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-forget-origin-48ad2c5`.
COPY.json SHA256 `f16a04fe3032a04994dcc2196a946bb8814ff05ec951f83a2ab5a2b6d958e039`.
All46 files/6,194,341 bytes verified individually; existing research notebook
appended. No executable or cache copied. All own validation containers exited.

After publication, release only .test-scratch/cargo and .test-scratch/target to
the central cleanup owner. No next Rust task requires retaining a private cache.
Keep the worktree, .git, all source, failed-run logs/archives and sealed evidence.
No tests rerun for this documentation-only handoff.
