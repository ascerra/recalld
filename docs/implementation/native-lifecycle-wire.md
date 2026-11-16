# Prepared native feedback and forget transport consumer

This child consumes published PR121 (`08914a4`, runtime `75bc809`), whose actual
parents are PR114 feedback, PR110 forget and PR111 retained namespace binding,
and PR88 (`3961f665`) for the strict shared contract and prepared transport
boundary. It does not consume PR119, native batch wire, or an activation
checkpoint. The prerequisite merge keeps the newer accepted-work, native proof,
and publication implementations while retaining PR88's exact-scan and session
ports. Both real Ghost creation and seeded migration fixtures remain available.

The existing REST StorageEngine/FsrsEngine adapters and MCP storage adapter
call the same accepted native feedback/forget coordinators. A caller must attach
its existing frozen NativePolicyInputs through `with_native_lifecycle_policy`
and use the same existing `with_clock` input. Missing configuration fails closed;
there is no second default policy, owner, session, planner, observer or scheduler.
The public activation owner must connect these inputs from the existing system
configuration when the remaining participation matrix is complete.

Shared `serialization/native_lifecycle` validates UUID, optional namespace name
assertion, positive u32 namespaceId, omitted-only quality default 3, integer
quality 1–4, unknown fields, explicit nulls, lineage, key encoding, and canonical
decimal-string expectedGeneration. REST gets the UUID from its path; a body UUID
is rejected. Body/header keys must agree. The existing v1 handlers retain their
parameters, response shapes and behavior through versioned wrappers. By-ID
resolution never consults the connection default or resolves a reused name.

An existing row, including a tombstone, establishes immutable namespace ID.
When no row remains, unambiguous retained terminal ledger identity can establish
that scope without a new explicit namespaceId. Otherwise a retry needs the
original explicit stable ID. The rare missing-row lookup currently scans the
payload-free ledger and its validation; this is not a constant-cost or A5 claim.
Verified original namespace name comes from PR121's live/retired binding helper.
Name capture follows the coordinator's receipt lookup so corrupt binding proof
cannot erase an already-known committed receipt. Original retry hashes and
receipts remain unchanged; replay precedes fresh CAS, policy resolution and clock
sampling. A wrong assertion with a known historical receipt retains that receipt
alongside the typed conflict.

MCP prepared dispatch and REST prepared bodies share parsing, typed error and
envelope mapping. Dedicated private RPC aliases `feedback_native_v1` and
`forget_native_v1` denote the internal RPC encoding version, not application or
lifecycle version. They carry explicit application contract 2 and never fall back
to legacy mutating methods. Old daemon unknown-method rejection fails closed.
Transport or decode ambiguity returns unknown, never a claim of rollback, and
is not automatically retried. Native responses retain the full receipt and
operation identity; no adapter parses diagnostic strings to recover commit state.

One accepted root covers canonical scope resolution, the coordinator worker,
publication and name proof. The worker samples its shared clock once for a fresh
operation and zero times for matching replay. After that root finishes, the
existing owned readiness barrier drains accepted work and checks the actual
current registry. Only its successful report supplies ready=true and real view
watermarks. A barrier/publication failure retains the committed receipt and
omits unavailable readiness; no unknown view stamp becomes zero or latest G.
Historical numerical outcomes are explicitly marked historical/replayed and
never rehydrate deleted content. No query readGeneration is invented from a
later control read. Cancellation/unknown owners retain existing unresolved work.

## Validation boundary

The focused fixtures use real legacy insertion, selected-layout/schema migration,
explicit policy migration, owned native entry, canonical transactions and derived
publication. They compare prepared MCP text/structured envelopes, REST body and
header parsing, and real RemoteStorageAdapter/DaemonClient/framed daemon dispatch.
They exercise feedback and forget replay, stale CAS, conflicting payload/key
reuse, terminal feedback, retained identity after purge/reopen, real publication
failure and recovery, and an actual post-commit unknown fault followed by reopen.
The old-daemon fixture uses its exact unknown-method rejection over real framing.
Malformed inputs preserve canonical record/history, generation and clock counts.

These successful prepared-handler/private-RPC checks are **not public contract-2
activation**. Actual public MCP and REST version selectors still reject 2 before
work. Discovery, schemas, `require_supported`, ordinary schema-2 opening and
unparticipating writers remain gated. Remaining A4 activation must wire the
shared configured inputs, publish complete operation schemas, validate negotiated
MCP eras and public endpoints across the complete participation matrix. There is
no public keyed-query, complete A3/A4, profile, performance or deterministic-ID
claim. Native phase/purge, exposure and batch/public activation have their own
actual dependency leaves and owners.

Exact source, commands, counts, logs and binary provenance are recorded in the
adjacent evidence report after final validation.

Known coordinator failure returns immediately after finishing known work; unknown
owners keep unresolved tickets. Neither path invokes a readiness barrier or
self-drains. The separate readiness-error observer `cc865c296247a55cadb09081e349c059c8724619`
(runtime `f8765b28`, direct PR96 child) is not an unused parent here. Huygens owns
the shared unknown/null applied-stamp representation and its actual consumer.
Until that join, failed lifecycle envelopes omit readiness while retaining every
known receipt. Observation failure must never replace that receipt or manufacture
pending-work totals.

V1 compatibility here means unchanged existing v1 handlers on their supported
legacy owner. It does not prove application-v1 writes against an ordinary native
store. Native cores currently require mutation contract 2. The separate original
API contract versus native disk-mode seam must preserve caller contract 1 in
request identity, rather than coercing it to 2. Feynman coordinates that core
proof/admission change; this private explicit-2 consumer does not bypass it.

Final frozen validation: 6 focused tests, 1136 library tests and 2 CLI tests passed;
4 doctests were ignored. Formatting passed. Clippy exited 0 with 890 library
warnings and 954 test warnings (865 duplicates), plus one binary warning; this
is not warning-free. All runs used offline Docker, Rust1.94.1, two build jobs,
2 CPUs/4GiB and this worktree's private caches/target. Full evidence, including
source and all executed binary hashes, is in `work-notes/native-lifecycle-wire`.
