# Actual readiness observation for committed failures

`Recalld::readiness_observation(deadline)` returns a `ReadinessObservation` from
the existing owned core. It does not create another owner, close admission, drain
or finish the caller's work root, rebuild a view, or change the canonical store.
The successful barrier `Readiness` type remains unchanged.

The result contains:

- `canonical`: the actual current `GenerationRef` from a redb read transaction.
- `derived`: each required component's name, `applied: Option<GenerationRef>`,
  and whether that real stamp matches the captured canonical generation.
- `acceptedWork`, `unresolvedWork`, `admissionOpen`: the actual tracker observation.
  Unresolved tickets are included in accepted/pending tickets; these are not
  independent operation counts.
- `pendingEffects`, `pendingPurges`, `pendingBatchItems`: actual durable state
  read from the same transaction as canonical generation. Categories may overlap
  with registered work. Do not sum them as a count of unique operations.

An unpublished component remains `None`; it is never assigned zero, the receipt
version, or a guessed current generation. There is intentionally no `ready=true`
field. Admission can change after this observation. Zero observed work is not a
barrier, and matching generation stamps do not prove component implementation
correctness. In particular, the independently confirmed unchanged-96 SQLite FTS5
rebuild statistics bug remains Hume's separate fix and validation gate.

The coordinator retains its publication read lease inside the blocking worker,
including after caller timeout. The engine read guard and metadata publication
read guard retain actual canonical/derived agreement while the transaction and
counts are read. Work is separately sampled from the existing tracker while
those guards remain held. Purge/batch row scans check the monotonic deadline
cooperatively. No hard timeout over blocking OS I/O or lock acquisition is claimed.

Unknown/unavailable canonical handles, invalid control state and expired deadlines
return an error rather than an invented readiness object. A transport caller must
keep its full committed receipt independently even when observation itself fails.
This does not replace an operation's original query read generation or its receipt
commit generation with the current observed generation.

The full startup/offline validation remains strict. Current healthy-owned-state
checks use existing table shapes and row invariants; completed purge ancestry is
preserved. The freeze check still rejects any real pending effects/purge work.
No dirty flag is treated as proof of zero durable work.

Actual tests exercise a committed canonical delete followed by a wrong-width
vector rebuild failure: the receipt remains available, FTS is unpublished,
other stamps remain old, one durable effect is observed, and the caller's own
registered root is still live. Real completion repairs all four stamps at the
same canonical generation; only explicit root completion removes that work.
Initial unpublished stamps and unavailable/deadline failures are covered too.

This branch is based directly on PR96 (`387c711`), without allocation/checkpoint,
new catalog/policy, or broad integration ancestry. Huygens owns the actual failure
envelope consumer and its explicit nullable/absent-stamp wire decision. Do not
activate public v2 or call this a complete transport contract based on this Rust
observation API alone. Shipping should accompany that actual consumer rather
than add an unused registry layer.
