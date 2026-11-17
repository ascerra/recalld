# Existing API1 delete output schema

This schema-only child uses PR198's actual purge receipt definitions and the
already present main purge producer. No N1/189, unfinished native Remote catalog
or other convenience parent is added. Native/legacy producer behavior, plain-text
compatibility errors, physical metrics, receipts, framing and public2 activation
are unchanged. Modern object schemas rise from11 to12 of14 tools.

Success describes DeleteNamespaceResult's actual name/id, deletion flag, four
u64 physical counters and vector-file flag. Structured errors follow the existing
native_purge_api1::structured_failure parser, which already validates the typed
start/advance receipt and its scope. This schema is not proof authentication.

- PURGE_METRICS_UNAVAILABLE: committed final receipt and purgeComplete scope;
  no operationId, readiness or invented physical metrics.
- SERVICE_UNAVAILABLE: committed known start/step/complete receipt and its actual
  scope, with no operationId or inferred readiness.
- COMMIT_OUTCOME_UNKNOWN: unknown current operation, required UUID-or-null
  operationId (the original Rust-parser UUID spelling is retained, including simple,
  URN and braced forms, so the schema does not assert canonical dashed format);
  optional earlier full receipt requires its matching scope. Earlier
  evidence does not prove that the unknown operation committed.

Message maxLength512 is a safe character upper bound on the existing runtime's
512 UTF8-byte limit, not a replacement byte validator. Known receipt headers are
version1, non-nil UUIDs and nonzero canonical decimal generation because this
specific parser checks them. Generic retained-error schemas from198 remain broader.
Root fields and scope/stage alternatives are strict; full typed receipt fields
are reused without truncation or receipt numeric/hash conversion.

Create remains deliberately without an outputSchema. Its existing handler accepts
any JSON object carried by PartiallyApplied. A catch-all object branch makes a
strict-looking success/error union ineffective because isError is outside the
structured payload. Closing this requires an owned compatibility decision for the
actual create error boundary across supported local and Remote bridges. Merely
adding a future native Remote producer does not constrain the existing legacy
lane. This child does not silently narrow that lane or call it completed. The
coordinated source plan is local1360492; no blocked research history is published.
list_namespaces remains an array in text, without an invented object wrapper.

Validation reuses actual native/legacy purge success, forced competing completion,
complete start/child error framing, unknown worker/child identities and malformed
peer checks. Adapter successes passed through the actual typed MCP emitter are
labeled as shared-emitter captures, not separate public invocations. Existing real
MCP/Remote failure cases retain their assertions. Formal checks cover emitted
values and a few meaningful missing/type/scope/receipt negatives. No new broad
matrix, full backend suite or retrieval/performance run is required for this
schema-only change. Exact-head CI and final composition remain queue gates.
