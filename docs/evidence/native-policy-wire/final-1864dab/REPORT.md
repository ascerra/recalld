# Native numeric policy wire and catalog publication: final evidence

Validated source: `1864dab1ceed911ed09a6c4e3b2a3a2001f0a717` on
`impl/native-policy-wire`. Public contract2 is still closed pending the remaining
actual native serving and wire consumers. This is not complete A4 or a
whole-native-ready claim.

## Change and actual dependencies

Actual parents are PR93/91cc0f9 (catalog wire), PR94/8deb56c (numeric policy
receipt/replay core), and PR96/387c711 (owning publication coordinator).
PR100/list paging and the broad integration checkpoint are not ancestors.
The merge preserves the parents' receipt variants and accepted-work guards.
The final fixture correction reuses only the checkpoint's catalog test hunk;
it adds no runtime or batch code.

Catalog create/replay and complete numeric-policy replacement now carry the
same accepted root through the canonical blocking worker and the actual
same-owner `complete_with_work` coordinator. Every receipt includes keyed
no-ops and historical replay; completion targets actual current G, not the
receipt's historical generation. No generic stamp advancement is introduced.
The blocking worker owns its publication read lease through cancellation and
drops it before exclusive completion. Unknown workers leave their root
unresolved. Known completion failures carry the original full caller receipt.

Strict private MCP and REST policy consumers require immutable namespace IDs,
a complete numeric replacement, and shared lineage/key/decimal-CAS validation.
Replay precedes fresh configuration/CAS; policy and replay sample neither
provider nor clock. The operation cannot change lifecycle version or profile
identity. V1 methods, defaults and successful payload shapes remain unchanged.

## Results on the exact shipping source

- Eight focused policy/parser/public-gate tests passed in 6.43s.
- Exact inherited catalog publication regression passed in 0.43s.
- Normal-parallel all-feature suite: 1063 library tests passed in 499.73s.
- Two CLI tests passed in 2.72s and 1.47s. Four existing doctests ignored.
- `cargo fmt --all -- --check` exited 0.
- `cargo clippy --offline --all-features --all-targets` exited 0 in 26.39s,
  with 761 library warnings and 829 lib-test warnings (741 duplicates), plus
  one binary warning. This is a pass with warnings, not a warning-free run.

Commands and logs are retained. Each execution used `recalld-ci:1.94`, installed
Rust 1.94.1, CARGO_BUILD_JOBS=2, Docker network none, 2 CPUs and 4 GiB. Only a
separate read-only git archive, private cargo/target, and output directory were
mounted. No home, credentials, production data, or Docker socket was mounted.
The 770 archived files were hashed before and after and remained identical;
six executed test binaries are SHA-256 sealed. Before each source-root switch,
the previous executed binary hashes were preserved and `cargo clean --offline
-p recalld` ran in this agent's own target. No stale source-root executable is
used as proof. No cache or build output is committed.

## What the tests establish

Synthetic stores use real legacy writes, explicit offline schema migration,
and the actual native owner. They are not raw schema-row injections.
Catalog creation, keyed policy no-op, changed policy, historical replay and
reopen reach a real all-four-view readiness barrier at actual current G, with
zero pending/unresolved work. No-op preserves record bytes; changed policy
updates canonical rows. Replay retains its original outcome and historical
receipt while current policy/publication stays newer. Provider/clock calls on
policy and replay are forbidden.

The completion-failure matrix replaces the actual shared vector index with an
incompatible dimension, causing the real coordinator to fail after each of
catalog, no-op and changed policy commits. Each returns committed state and the
exact retained receipt. Replay while still broken also fails and preserves that
receipt with only its replay flag changed. Repairing the shared index permits
matching replay and actual all-four-view publication, without another commit.

Closed-admission tests run the actual adapter worker and completion under a
root accepted before close. Cancellation tests hold a real blocking worker
inside its clock, abort its async caller, verify the child and unresolved root
remain tracked and the worker retains its publication lease, then permit the
worker to commit. A deliberately panicking worker separately proves unknown
root/child retention. These intentional panic logs are expected passing tests.

Actual inner MCP/REST serialization agrees for policy replay, including MCP
text/structured content. Bad fields/types/nulls/bounds, scope IDs, lineage,
CAS and changed-payload retries preserve store bytes. Reopen/replay rebuilds the
real newly constructed views. Public MCP/REST contract2 rejects before work;
no new operation is advertised in public tools. Existing catalog fault/reopen
coverage remains in the full suite and passed separately on the predecessor.

## Preserved failures, not runtime workarounds

Initial immutable d5518b9: five focused tests passed, one failed because a
supposed-valid parser fixture used a nil storeId. The shared validator correctly
rejected it. The fixture now uses a non-nil ID; no production validation changed.

Immutable b97e097: eight focused and six catalog tests passed, then the full
suite passed 1062 tests and failed one in 484.89s. The exact panic was the
inherited test expecting `CommittedEffectsPending` after a known catalog commit.
The joined publisher correctly returns the full `CommittedMutation` receipt.
Clippy and CLI tests were not reached in that failed full run.

The reviewed checkpoint correction changes only this expectation and adds
exact receipt equality after reopen. Original unresolved-work, recovered-vector
handle, and zero-new-owner-pending assertions remain. No production error,
receipt, or lock behavior was weakened. Both failed sources, logs, immutable
manifests, and their executed binary hashes remain distinct from shipping proof.
Earlier catalog f32 fixture history remains unchanged; this feature makes no
production float-parity fix or receipt/request-hash normalization change.

## Remaining activation work

Actual native store/batch, feedback/forget/purge/sweep wire; coherent get/similar
selection and payload wrappers; native ordinary startup admission; remote native
RPC; namespace/tag continuation; complete input/output schemas and discovery;
and actual public conformance remain. Failure readiness needs an owner snapshot
that truthfully represents never-published/unknown stamps, not fabricated zero
or receipt-G values. Catalog/policy currently retain full receipts without
inventing that object. The gate is an interim safeguard, not the final A4 goal.
