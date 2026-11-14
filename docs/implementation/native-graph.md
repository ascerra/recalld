# Native standalone graph transactions

This slice adds explicit native edge puts/updates to the already-owned engine. It
uses the corrected PR82 startup/recovery parent `fb804a0a72ccf7dde0a6ee7eaea2e91a6b7ba2db`.
The minimal shared content-profile validator extraction is commit
`1f4eb18c24a3d3f512030bbee0f9c4adcecfe656`; it preserves existing record-proof
validation and can be consumed without graph implementation ancestry. No PR83,
batch/catalog, producer, or broad integration parent is required.

## Interface and transaction boundary

`NativeGraphInput` contains ordered caller edge specifications: source, target,
type, weight and auto-created flag. It excludes server-generated timestamps.
Inputs have unique directed `(source, type, target)` keys and an explicit maximum
of 1,024 edges / 2,048 distinct endpoints. Oversized/invalid inputs are rejected
before cloning the capture plan. These are input/proof bounds, **not** limits on
examining or allocating the existing graph or lifetime history.

The owned metadata API captures canonical endpoint tokens and namespace policy.
The engine applies the captured intent under native publication admission with an
optional accepted-work parent. Matching keyed receipt lookup precedes fresh
capture/policy/endpoint validation and clock sampling. The final transaction
rechecks endpoint tokens, policy, scope, existence and edge constraints.

- Ordinary associative/entity/temporal/causal/contradictory edges require both
  endpoints to be Full or Summary in the same namespace.
- ParentChild preserves the retained-source exception and requires a live target;
  Supersedes preserves the retained-target exception and requires a live source.
  Existing cycle/branch checks remain atomic. Structural edges never restore
  payloads or authorize associative activation through Ghost/Tombstone records.
- A changed source gets one record-token increment, including an attribute-only
  overwrite. Its outgoing count is absolute. Unchanged targets, lifecycle clocks,
  phase, anchors, strength, stability, labels, payloads and provenance are preserved.
- One injected clock sample timestamps changed edges. Float bits, including signed
  zero, are preserved for native edge identity. Legacy float comparison is unchanged.

**Fresh keyed no-ops are unsupported and return `InvalidMutation` before success,
clock sampling or canonical mutation. No key is reserved and no successful outcome is
promised.** Unkeyed internal no-ops may return `None`. An existing matching keyed
receipt always replays first, even when the current graph or endpoint state has
changed; a different payload under an already committed key conflicts. The error
alternative avoids introducing an unproven no-change receipt merely to bind a key.

A new `NativeGraphChanged` outcome records exact edge before/after values and
payload-free endpoint state commitments. It does not widen historical
`NativeRecordWrite` creation proofs (at most two explicit parent/supersedes edges).
Peer patches, generation, receipt and durable edge outbox commit in one metadata
transaction. The edge database is applied idempotently afterward; this is not a
cross-file atomicity claim. Committed application failure carries the full receipt;
uncertain metadata commit retains its operation ID and poisons the owner until
reconciliation. Success here certifies canonical application, **not derived ready
publication**. The producer/coordinator must await its actual publication path.

## Validation and history

Strict receipt validation checks policy/generation boundaries, endpoint identity
and eligibility, exact count/token deltas and unchanged content commitments.
Profile commitment shape and exact immutable namespace binding are validated even
for foreign-origin historical receipts whose records need not survive restore.
UnknownLegacy requires and preserves None; a different well-formed profile is
rejected. The binding helper is separately reusable as commit `f0a7e15`. Outbox actions must match the typed receipt;
preflight compares endpoint records before touching original files.

Inventory validation checks the active-origin edge history against physical edges,
including the exact before/after alternatives of pending canonical work. It rewinds
native graph puts and creation edges. Historical foreign receipts remain
ancestry/policy validated and are never replayed into a restored store. A later
hard-delete/maintenance join **must** add its exact edge-removal rewind before
older graph proofs; ignoring a new removal outcome is not valid composition.

The implementation still loads the existing edge store and examines retained
history. No throughput, RSS, bounded-total-work or performance-promotion claim is
made. Legacy-only inventory does not collect the native edge-history vector.

## Scope remaining

The final A4 keyed graph contract still needs a strict typed receipted no-op (or
an explicitly supported specification error). Track that additive follow-up with
policy no-change semantics: bind a successful key, advance G once, preserve edge
and peer tokens, validate no-change history, and replay the original result after
intervening writes. The current explicit error is containment, not completion of
that contract. The next implementation priority remains same-insertion links.

Automatic semantic/entity/temporal store linking remains unsupported until its
producer and core are joined with a frozen plan in the **same insertion outbox**
and a new additive `NativeMemoryInsertedWithLinks` proof. An accepted work ticket
alone is not durable intent. Required follow-up proves process exit after insertion
commit cannot lose required links and completed keyed store replay cannot discover
new links. Candidate capture must use exact generation equality to catch newly
inserted candidates; temporal selection anchors to the final record's created_at (caller-supplied or
the core's single sampled clock); edge timestamps use that same clock sample. The planned
new store input/hash domain also binds requested advisory behavior without rewriting
old insertion or batch identities.

Curie owns producer selection/root/provider integration; this slice owns the native
storage proof. Feynman retains shared history review. Pascal owns native phase and
hard-delete proofs. Public native serving, transport-v2 activation, incremental
publication optimization, performance promotion and the paused publication-controls
audit are not completed by this change.

Validation results and exact source/binary/log hashes are recorded in
[the evidence report](../evidence/native-graph/REPORT.md).
