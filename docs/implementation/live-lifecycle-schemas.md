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
history. The ongoing schema/consumer fixtures need their own frozen validation.
No full-schema, CI, integration or full-A4 completion claim yet.

Remaining tools after this slice: get waits Curie's validated API1 shape;
store/batches need complete actual write/error unions; create awaits N1; delete
requires complete purge-proof shapes. list_namespaces stays an array in text.
V0 transport/no-queued-work evidence, public2 startup/writer closure, coherent
read contracts, outer-frame limits and A5 measurements remain separate gates.
