# Live API1 feedback and forget output schemas

Huygens owns this isolated child of PR192 `6759c320d027bc3ebe4013ff132b55d6d63932d1`.
Halley released only schemas, registry entries, tests/formal fixtures and docs.
Pascal reviewed actual API1 producers and malformed-reply retention before edits.
No producer, storage, serializer, transport, default or public2 activation change.

The two modern output schemas describe existing ReinforceResult / {id,deleted}
successes and the actual raw native_lifecycle::Reply failure. API1 error scope is
a nullable two-element tuple; data/receipt/error/readiness use their existing
fields and null choices. This is not a contract2 envelope. Legacy-era omission
of outputSchema/structuredContent remains. Existing plain text validation errors
remain plain text. REST lifecycle errors are separate from MCP output schemas.

Full receipts have strict serialized codec shapes for legacyFeedbackRecorded,
lifecycleFeedbackApplied, memoryTombstoned and nativeMemoryTombstoned. A typed
matching malformed proof can survive the existing Remote invalid-response path.
Therefore these schemas do not enforce canonical storage-proof relationships:
finite out-of-domain lifecycle values, extra typed records/edges and optional
vectorExecution must remain representable when retained by that path. They do
not establish authenticity, namespace/request binding, historical validity or
readiness. The original receipt bytes and existing attribution are unchanged.
Unknown/null phaseTimeRule fails the typed codec; absent historical fields and
monotonic_marker_v1 remain distinct. Profile strings in retained codec proofs
are not advertised as verified active profile identities.

First executed boundary proof is source `887dc3c6be314c4814fe40a3f3b3f0219bbbe24f`:
one test passed against real owner receipts copied into explicitly authored
malformed peer specimens, using actual API1 decode and MCP projection. Record
bytes, generation and clock checks are scoped to that helper, not whole-store
immutability. Authored absence is codec compatibility, not genuine old writer
history. Final frozen f383d400 validation passed1770 library+10CLI,8focused Rust
and71formal output cases; fmt/Clippy0 with warnings. Exact-head CI and integration
are separate gates. See [the sealed report](../evidence/live-lifecycle-schemas/REPORT.md).
This is not full A4 completion.

Remaining tools after this slice: get waits Curie's validated API1 shape;
store/batches need complete actual write/error unions; create awaits N1; delete
requires complete purge-proof shapes. list_namespaces stays an array in text.
V0 transport/no-queued-work evidence, public2 startup/writer closure, coherent
read contracts, outer-frame limits and A5 measurements remain separate gates.

The separate extrema fixture source64667fc6 passed its Rust exact typed receipt
checks but failed formal validation: actual typed f32 output is +/-3.4028235e38,
slightly wider than +/-3.4028234663852886e38 from f32-to-f64 widening. Pascal and
Halley source-reviewed the schema-only correction: derive bounds from the actual
serde_json f32 endpoint spelling, covering both typed text and Value-first
emitters. No tolerance, receipt reserialization change or global serde change.
The original failure is retained in evidence/003-max-red; final004 proves the
schema correction with unchanged exact typed receipt assertions.
