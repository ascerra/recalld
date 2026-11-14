# Accepted native exposure consumer

Actual dependencies are published get/similar PR113 `149beb6` (runtime `8a0d260`)
and published policy-forwarding PR116 `ea43544` (runtime `8dd1cc7`), a minimal PR96 child. The latter adds only
`AccessObservation::from_receipts` and configured policy/time forwarding. This
branch does not consume his optional-delta optimization, feedback/forget,
maintenance, sweep, purge, or another integration checkpoint.

`StorageAccessRecorder::record_access_batch_for_publication` dispatches explicit
native entry to `DerivedCompletion::record_native_exposures_with_work`, using the
actual QueryEngine's `NativePolicyInputs` and sampled query time. Constructor-only
legacy callers retain the existing legacy observer. Native mode requires its
configured publication owner and policy; it never falls back to a contract1 write.
Native results use `AccessObservation::from_receipts` and full publication, with
no incremental hint. Hume owns the independent legacy optional-delta path.

The existing observer's canonical UUID deduplication/namespace grouping helper is
shared: one event per UUID, direct dominates associative duplicates, first
occurrence within each namespace, namespace order ascending. Scope source IDs are
validated but only returned IDs are exposed. Each namespace captures current
canonical policy/tokens and applies the unchanged native planner/transaction via
`MetadataStore::record_native_exposure_with_work`. Capture's generation is from
that actual post-selection transaction; it is NOT relabeled as the earlier query
readGeneration. This does not claim whole-query CAS or cross-namespace atomicity.
Current scope, terminal eligibility, record and policy tokens remain checked.

The query's sampled value is wrapped in the existing ManualClock only to call the
native core Clock interface. No second runtime clock/configuration source exists.
Native-capable direct search/get/similar fallback paths forward their existing
StrengthContext time rather than resampling the recorder. Legacy fallback paths
retain their previous recorder dispatch/clock, and explicit pre-sampled APIs
always retain their supplied time. V1 retention formulas
and defaults are unchanged; native v1-policy namespaces use the schema-aware
writer, while v2 exposure preserves the retention anchor, stability, feedback,
retained phase and provenance. Effective time still clamps against canonical
lifecycle_updated_at. IDs remain nondeterministic; this is not full A6 replay.

Accepted work is registered before awaits. The blocking canonical worker retains
publication and graph read leases through storage writes even if its waiter is
cancelled. Lock order is publication, graph, storage. On success, QueryEngine's
existing full completion runs once after payload/canonical leases are released.
On a later namespace failure, the native worker returns preceding receipts and
the coordinator awaits completion before returning that partial error. Unknown
commit operation IDs survive; a poisoned handle cannot claim ready and requires
reopen/reconciliation. These internal exposure requests are unkeyed: do not
blindly retry them, or promote explicit selected-ID hashes to public keyed query
identity. A4's future keyed queries must hash original caller intent, look up
replay before fresh selection, then hydrate current visible content read-only.

The read-only pinned recorder carries only the original configured native-writer
capability so deferred true selection can pass the policy gate. Every pinned
write method still rejects. Unsupported recorders retain the old fail-closed
policy guard. MCP get receives the same configured `Arc<QueryEngine>` and delegates
its sampled-time exposure hook; it does not construct another policy or publish
twice. Direct MCP and daemon constructors inject that existing engine. False mode
and empty outcomes never enter the writer. Native constructor-only MCP callers
without the observer reject true rather than choosing a default policy.

Ordinary schema2 opening/serving and public contract2 activation remain gated.
This slice does not claim native get projection parity, a new public receipt
shape, global namespace atomicity, keyed-query semantics, source generation
comparisons across lineages, or performance improvements. Huygens owns A4 wiring
and activation inventory; Feynman owns the independent retired namespace binding
consumer for frozen PR110/114. No binding edits are included here.

Validation uses real migration/native insertion and the existing production
query, HTTP and MCP adapters in isolated offline Docker. Initial 3PASS/2FAIL source
and executable identities are retained: missing pinned capability forwarding and
an incorrect fixture expectation for regressed-time clamping. A subsequent old
unit-struct test constructor failed compilation; no test ran. A formatter then
reported a truncated Docker mount read while the host file was complete; the
failure and host hash/source are retained, and the next run verified all mounted
runtime input hashes before formatting/building. A further metadata-only fixture omitted its required filter; its 7PASS/1FAIL source and binary are preserved without relaxing validation. Corrected focused validation passes 9 tests (8 new consumer tests plus the existing both-policy native core test). Full-suite/fmt/clippy validation is pending.

The initial full frozen3955 run finished989PASS/27FAIL: unconditional inferred-time forwarding broke legacy-only recorder dispatch and bypassed a legacy configured clock. Exact source, binary identity, full log and27 failed names are preserved. The bounded correction selects inferred time only for native-capable recorders; no legacy assertions or explicit pre-sampled semantics are relaxed. Corrected validation pending.
