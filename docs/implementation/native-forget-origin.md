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
