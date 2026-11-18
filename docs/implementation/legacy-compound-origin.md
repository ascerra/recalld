# Legacy insertion and compound original API identity

This slice joins PR148 (`508d98c65b0f824bbcc0290812c63e0dea426edd`)
and PR144 (`31a0463b2d7e0e6920c2daf6025759fb2e1d73ca`). It consumes the
existing insertion producer, original-context native codecs, legacy lifecycle
writer, accepted-work registry, and real derived completion. It does not add
another registry, public API version, or general whole-store CAS promise.

## Actual consumers and durable identity

The existing `DerivedCompletion::insert` and canonical legacy insertion
transaction now accept validated API1 or API2 context. The original contract
version remains in the existing request hash. Matching receipts precede current
CAS, generated vector/clock validation and allocation; changing API version under
the same key conflicts. Legacy physical-format fences and final namespace/policy,
UUID, endpoint, Supersedes cycle/branch, vector and payload checks remain.

The actual typed batch runner and daemon bridge consume internal mutation/manifest
format4 for legacy API2. This number is **not public contract4**. Public parser
routing remains a separate A4 consumer. Existing format1 means legacy API1;
native formats2/3 retain their existing origin codecs, hashes and physical rules.
No historical format is rewritten.

Format4 requires the closed `legacyOrigin` object:

```json
{"proofVersion":1,"contractVersion":2,"expectedGeneration":null}
```

`expectedGeneration` is required: null means absent CAS, while a supplied value
is a canonical decimal string. Missing/null origin, missing expectedGeneration,
unknown/extra/duplicate fields, unsupported versions and invalid generations
reject. Supplied CAS must equal the original parent generation minus one.
The child capability carries the same origin; children use original API2 with
no second parent CAS. Parent and child operation IDs remain distinct.

New `batch_*_v4` domains bind original API2, ordered caller intent, frozen
resolution and the reservation CAS. Per-item caller hashes retain original
supplied float bits, ordering and namespace/default-context intent. Fixed
`checkDuplicates` absence expands to true. Mutable namespace defaults freeze
in the reservation; matching replay does not resolve them again.

Startup and the final child write transaction both verify the immutable parent
receipt's full value, caller/resolution commitments and request hash. Editing
both manifest and ephemeral child copies cannot authorize a different CAS or
origin. This anchor exists before the first child and for all-invalid batches.
Successful child receipts and indexed outcomes still commit together.

For genuine format1 history, parent hash validation tries only the two meanings
the original writer could commit: absent CAS, or original parent G minus one.
The latter follows from that writer's fresh-CAS check and exactly-one-generation
reservation transaction. It uses the retained original receipt, not current or
fork generation, and is validation rather than a retry-time guess. The bundled
old-writer fixture includes both meanings, each successful and all-invalid.

## Selection and commit-state boundary

Fresh legacy API2 automatic/configured linking and duplicate advisory remain
unsupported until their original intent and selected outcome have a durable
same-insert consumer. The fresh batch check runs after matching parent replay,
before reservation/child allocation/provider/payload; all syntactically invalid
items retain normal indexed parent-only bookkeeping. API1 behavior is unchanged.

The reserved-child path checks the same restriction after matching child replay.
If a parent was already reserved, a blocked child remains `NotAttempted` with
`UNAVAILABLE`, its reserved operation ID and exact parent receipt. It is not a
claim that the parent never committed. Completed matching replay precedes current
selection settings, provider and clock. Publication failures retain the exact
current child receipt; unknown outcomes retain the current child operation ID.

Explicit parent and Supersedes inputs remain supported because canonical
endpoint/scope/cycle/branch checks and deterministic edge effects share the
actual insertion transaction. These are supplied links, not proof of durable
automatic selection. The existing graph-before-storage publication order stays.

## Recovery and offline consumers

The shared strict batch validator is consumed by startup and offline validation.
Proofs cover real process exits around reservation, payload synchronization,
metadata commit, edge commit and acknowledgement, followed by reopen and exact
receipt replay. These are process-exit tests, not power-loss durability tests.

The offline integration test uses actual backup/retained-state restore, schema
migration, logical export/import and historical restore. It compares retained
original receipt/progress bytes, checks source backup immutability, rejects old
store IDs after fork before fresh settings, and keeps historical destinations
outside ordinary serving. Genuine old1 and new4 histories coexist in that test.

The join also replaces PR144's manual readiness field projection with its
already-existing `report.into()` conversion, preserving PR148's optional
watermarks and observed-work fields. No extra lifecycle parent is required.

## Remaining boundaries

This is not public API2 activation, complete A1 acceptance, native producer
recipe certification, or a backend/default decision. The next owned code slice
is durable legacy API2 automatic/advisory selection with frozen intent and
same-insert completion. Public routing remains coordinated with A4. Ordinary
native vector/query activation additionally needs the explicit handling of older
Bound-but-uncertified generated vectors owned by the shared A1/A4 admission
proposal; the PR148 recipe fix did not certify historical vectors.

Exact experimental sources, failures, results and executable hashes are recorded
in [the evidence report](../evidence/legacy-compound-origin/REPORT.md).
