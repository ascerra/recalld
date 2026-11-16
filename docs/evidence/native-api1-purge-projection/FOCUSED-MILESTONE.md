# Native API1 purge: focused milestone, not publication evidence

Actual parent is validated PR176 `1f5d083d8142dd631f16f95617d23fddc85751ad`.
The later queue head is not included. Native startup/public API2 remain closed.
No full-suite or final-head CI result is claimed for this unpublished child.

The actual BEFORE test at 423b9f21 reached canonical Complete at G18, then returned
a plain PartiallyApplied string containing a final G18 receipt and nested G16 child
receipt. The test failed while parsing structured evidence. This was not an absent
receipt or unsupported native mutation; the evidence was embedded in diagnostic text.

Candidate002 67a60a50 passed the corrected real-adapter regression: exact matching
G16 child receipt, purgeStep scope, SERVICE_UNAVAILABLE and no readiness assertions,
while separately inspected canonical progress was Complete at G18. No error-path
readback replaced the in-hand child with that later receipt.

Candidate003 07d1c3c9 and diagnostic-only candidate004 b7caaeee each had four passes
and two failures. The actual competing-completion race, lost begin, cleanup counts
and matching-child projection passed. Foreign-first and unknown-child cases failed
structured start-receipt classification. Typed receipt decoding succeeded; the extra
Value reserialization equality was too strict for numeric text.

The exact dependency probe found initialStability 3.7145 serialized as
3.7144999504089355 and parsed as Value 3.714499950408935, while the typed f32 retained
bits 0x406dba5e. The correction keeps exact recursive field/key/array shape checks,
typed numeric decoding and native proof/identity/scope validation, and returns the
original parsed payload. No epsilon, coercion, dependency or retained-history change.
The probe source, result and executable hash are in `float-probe/`.

Candidate005 `c8961d23` passed all eleven focused tests in 5.81 seconds and formatting:

- Actual direct adapter returns matching child evidence; foreign-first publication
  failure retains its typed foreign cause internally and projects only the exact
  known purge start. The error path performs no upgrade readback or extra mutation.
- Real cleanup reports actual orphan FTS rows and actual directory removal, and
  retains original API1 compound request identity. Name-based deletion is not retried.
- Competing completion reaches Complete before the caller captures observations.
  The caller returns PURGE_METRICS_UNAVAILABLE with the exact final receipt, without
  inventing physical counts/readiness, changing canonical bytes/G or drawing IDs.
- Actual modern/legacy MCP, RemoteNamespaceAdapter and old framed daemon paths
  preserve complete evidence. Complete-error frame sizes were 1724, 1065, 1724 and
  759 bytes respectively; actual start/child daemon frames were 1318/10409 bytes.
  These are the exercised small fixtures, not a universal frame-size proof.
- Unknown begin has no invented ID/receipt; unknown child keeps its real reserved
  ID and known start. A lost completion keeps start evidence and unknown status,
  then actual reopen validates the later complete canonical state.
- Cancellation retains the completion worker's publication/graph leases and
  unresolved work. Structured remote validation rejects nested unknown fields,
  fractional/wrong-type integers, invalid ranges, nonempty namespace extensions,
  nil identity, wrong scope, overflow generation and oversized diagnostic prose.

All focused runs used isolated offline Rust1.94.1, network none, 2CPU/4GiB, the same
private target with package cleaning, no personal data/credentials/daemon sockets.
Exact source/log/executable hashes, memory counters and results are in
`FOCUSED-MILESTONE.json`; complete active raw roots remain protected until publication.
Intentional worker panics in lost-outcome tests are fault probes, not suite failures.

The next mandatory check is a real maximum-chunk receipt through the old frame
boundary. Its source-only test uses the actual CHUNK_LIMIT of 1000 and actual legacy
insertion/migration/native purge. It is not yet formatted, compiled or executed.
Do not truncate a reachable oversized receipt or invent a receipt reference: preserve
and report an actual failure for a bounded wire decision first. Empty observed
zero/false, recreation/replay and broad affected regressions/full checks also remain.

The coordinating agent requested a scheduling hold after current runs. All owned
heavy jobs are drained; no new build/test/formatter/cache-copy/container job starts
until release. Source and documentation may continue. PR181 is separately published
and frozen with Halley; central cleanup confirmed its released raw root removed
97,517,108 logical bytes, leaving this worktree/raw root, sole cache and historical
inputs untouched.
