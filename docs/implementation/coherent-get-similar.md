# Coherent get and similarity payloads

Assigned child of actual PR96 `387c711` (`c16e214` runtime), without producer,
ID/reset, catalog or broader checkpoint ancestry. Halley explicitly transferred
read-view and payload helpers, narrow MCP/REST/daemon callers and race tests;
`api/state.rs` optional adapter methods and `search/pipeline.rs` routing are also
included. Huygens owns public-v2 parsing, envelopes, discovery and activation.

Source inventory, not yet a reproduced failure: REST get obtains a cached row
then independently reads full text. Similarity prepares rows against ordinary
adapters before later response hydration. MCP get holds one engine guard but
does not pin a canonical transaction/generation across row and namespace reads.
Completing derived work alone does not prove these payloads belong to one G.

## Bounded interface

Use PR96's existing accepted-work owner, publication leases, canonical read view,
cache bypass, exposure rejection, and `hydrate_at` payload-copy boundary. Add
owned `CoherentGetResponse` and `CoherentSimilarResponse` results carrying the
actual readGeneration, one evaluated_at, original requested access mode, selected
response data and the complete copied payload. Optional production adapter
methods return None only for explicitly immutable/mock adapters, preserving the
existing fallback contract. No second registry or clock is introduced.

Get needs a separate marker-aware capture: preserve the existing content-free
legacy tombstone response without weakening the Full/Summary eligibility of
search/similarity `hydrate_at`. Namespace assertions and canonical row selection
must share the pinned view. Empty/missing results must also verify the actual G.

Similarity reuses the existing prepared selector with original requested access
and deferred recording, source exclusion, minScore and legacy tie/cap behavior.
The temporary query engine pins matching vector/graph/FTS/entity readers and the
canonical transaction. Copy every final row/text/vector under engine read
ownership and metadata generation ownership before releasing the view. A
concurrent generation/handle change rejects the whole page, never a partial mix.

True-mode exposure happens once after successful whole-payload capture, using the
same evaluated_at and final distinct returned UUIDs. Source/scope assertions
remain in the final writer. Keep the accepted root through capture and awaited
publication; retain full committed receipts on later failure. False mode records
nothing. No handler reloads payload from the mutable cache/storage afterward.

## Validation to implement

Actual public get/similar races with forget and selected-text compaction, true
and false mode, namespace assertion and source exclusion, final returned-only
exposure, cancellation/close behavior, and original mode/time/receipt propagation.
A baseline experiment must establish any claimed failure before a correction is
reported. Existing PR96 search and generation/readiness regressions remain
required. Tests use a new isolated private cargo/target; never switch source
roots on the producer target. No whole-core snapshot/reset or native-serving
activation claim follows from this slice.

## Implemented boundary

The query engine now shares its existing pinned-reader construction across search,
get and similarity. Production adapter preparation returns owned payloads and the
original requested access mode; false-mode is never substituted to avoid policy
checks. Similarity reports `RetrievalSemantics::V1` explicitly. Existing source
exclusion, score filtering, caps and tie behavior remain named legacy semantics;
this is not A2-v2 ranking or deterministic-reset coverage.

REST and in-process query get use the coherent preparation. MCP/daemon direct
fetch uses the canonical point-read path under one engine guard and the metadata
generation read guard, preserving its legacy marker projection. MCP similarity
and REST similarity use the pinned selection and fully copied payload. Daemon
RPC dispatch already routes through these adapters, so no second RPC algorithm
or runtime dispatch change is necessary. Legacy response envelopes are unchanged;
A4 still owns public readGeneration/receipt envelopes and version activation.

The outer accepted root survives payload capture and awaited exposure publication.
Every blocking capture has a child ticket; graph ownership precedes storage
ownership, and payload/view leases are dropped before publishing exposure. Missing
and empty results are generation-checked. Scope errors retain their existing
classification; generation/readiness failures report unavailable. False mode
records nothing. True mode records final IDs once, with DirectRetrieval for get
and AssociativeRetrieval for similarity, using the same evaluated_at.

## Validation checkpoints

The first compile failed on two generic error-inference sites and one WorkError
conversion. Those compile errors were fixed; no tests ran in that attempt. A
subsequent formatter saw a truncated file through the Docker source mount although
the host file was complete; the log is retained. The retry checked the container's
file SHA against the host before formatting/building. This is a source-mount
observation, not a proven storage runtime failure or a passing test run.

Three initial actual-adapter tests passed. The expanded run passed five new by-ID
tests and six existing coherent-payload tests, including actual REST/MCP false-mode
state equality, scope rejection, returned-only exposure, metadata/text races with
forget and compaction, postcommit receipt retention, accepted descendants during
close, one clock sample, markers/missing/empty results, and owned text surviving
selected-handle replacement. Framed daemon payload and receipt-reopen proof is
being validated before the final frozen full run. This checkpoint does not claim
final validation or publication.

The implementation uses actual PR96 `387c711` only. PR106 producers, unrelated
native graph/catalog changes, public-v2 activation, ID/reset ownership and the
registry are not dependencies. Native policy/exposure consumers and A2-v2
similarity remain separate joins. This slice makes no whole-core freeze guarantee.
