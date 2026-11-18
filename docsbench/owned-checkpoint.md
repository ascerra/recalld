# Isolated legacy checkpoints and fresh-owner replay

This child of PR104 implements an evaluator-only checkpoint and fresh-process
constructor for `isolated-legacy-checkpoint-v1`. It is not production restore,
public v2 activation, or complete A6. The actor still uses the actual MCP stdio
parser, validation, handlers and canonical writers. The evaluator has separate
inherited pipes; it never ingests actor memories privately.

## Interface and ownership

Use the `owned_evaluator` example with an explicit loopback synthetic embedding
fixture and fresh data root. Checkpoint mode additionally requires
`decay.disable_sweep=true`, `rif.enabled=false`, and
`graph.autolink_enabled=false`. `snapshot {"deadlineMs":30000}` on the private
channel returns `manifest`, `sha256`, `committed`, `logicalHash`, and `track`.
`advance` remains the owned shared-clock control. `inspect` reports the captured
clock/allocation state and generation; its `logicalHash` remains null. The full
logical hash is available at checkpoint boundaries, not an O(history) scan
silently added to every inspection.

After receiving a checkpoint ACK, terminate and join the old actor process.
Start the same executable with the same effective configuration and:

```
--data-root NEW_SIBLING_ROOT
--restore-checkpoint MANIFEST_PATH
--checkpoint-sha256 EXPECTED_SHA256
```

The expected hash comes from the supervisor's prior checkpoint ACK. The new root
must not exist. Its parent, checkpoint directory, source root, and retained
ancestor roots must share the same private fixture parent. Old source roots and
their existing lock files must remain present. This constructor opens no second
core in the old root and never overlays an existing destination. It acquires
existing A1 ownership locks for each ancestor, retains them in the new actual
engine until its last worker releases it, and publishes the clone with the
existing atomic no-replace directory helper. Up to 64 source ancestors are
supported; a deeper chain is rejected. No sibling registry is introduced.

Capabilities distinguish conditional `snapshot` and
`freshOwnerCheckpointRestore` from the unsupported in-process `reset` method.
Broad `controlledClock`, `controlledIds`, `controlledRng`, and public-v2 claims
remain false. Lifecycle time and canonical allocation are controlled in this
constructor; arbitrary random sources are not.

## Capture and validation

Capture closes actual work admission, drains registered descendants, and retains
the publication, graph/vector/FTS/entity, engine and metadata leases. The capture
worker owns those leases through cancellation. Its controller stays busy after
a caller timeout until the worker joins, so clock advance or another capture
cannot race a still-running copy. The controller does not register a root that
it then waits to drain.

Canonical input selection is explicit: metadata and edge databases, the text
gate and metadata-selected text generation, and namespace vector files including
their ownership bitmaps. An open redb header is recovered only in a disposable
copy, closed, then copied into the private stage. The original is never repaired.
Strict `OfflineSnapshot` validation checks the stage before publication. The
shared typed logical-export walker supplies the receipt-inclusive logical hash;
a private legacy/batch mode has a separate hash domain and does not relax the
production schema-2 export contract. Unknown canonical tables fail closed.
Physical database layout, vector slots and text offsets are excluded using the
existing logical representation. IDs and references are not normalized.

The strict manifest records generation, canonical file SHA-256 map, logical
hash, manual clock, owned allocation cursor, normalized effective configuration,
source/ancestor roots, executable/Cargo.lock hashes, and cache/index policies.
Only the isolated data-root configuration path is normalized. The source commit,
container digest and build resource limits belong to the frozen run manifest;
the runtime does not invent its source commit. Checkpoint publication validates the captured clock against the same writable
allocation contract used by restore. `advance` itself accepts nonnegative i64
time without overflow; crossing the UUIDv7 range makes a writable checkpoint
unavailable and does not publish a manifest. Manifest input reads at most
1 MiB plus one sentinel byte and rejects excess, unsupported fields/versions,
invalid or exhausted writable allocation state, and a clock preceding allocated
UUID time. Both pre-open and post-rebuild verification use actual generation and
logical state. Actions start only after all four derived views are rebuilt and
the retained freeze verifies the restored cursor and canonical hash.

Manifest publication occurs after successful capture join and deadline checks.
Late/failed capture drops its unpublished temporary stage. Deadlines are
cooperative between byte/row operations; blocking OS I/O and redb integrity
inspection do not have a hard wall-time guarantee. Full canonical capture and
current full derived rebuilds are O(state/history), not an optimized hot path.

## Cache and replay limits

The record cache is invalidated and its maintenance drained under the retained
freeze; RIF and automatic graph linking are disabled for this track. Production
`Recalld::with_runtime_sources` constructs its provider with
`embedding::EmbeddingConfig.cache_embeddings=false`. That field is not on
`config::RecalldConfig.embedding`. Therefore this constructor does not wrap a
`CachedProvider`. The warm-file/prefetch utilities are not wired into this
constructor. No provider cache export/restore is claimed. The actual probe warms
two identical queries before capture and checks that both suffix queries reach
the offline provider in every owner, comparing full provider request sequences.

Moka admission/telemetry history, HTTP connection pools, FTS/OS page caches and
latency are not replayed. With RIF off, cleared record entries and coherent
search's cache bypass, this fixture compares logical outcomes and provider
request costs, not equal latency or global cache internals. Provider retry jitter
and legacy equal-weight graph-neighbor ordering remain uncontrolled. The probe
uses distinct vectors and a single-result query, retaining full semantic
responses, IDs, receipts and provider logs. Three identical suffix results prove
this bounded fixture, not all action schedules or full-store determinism.

Native v2, explicit sweep/phase schedule joins, broader schema/fault matrices,
all behavior-affecting RNG/cache coverage, dataset consumers and calibrated human
judging remain separate prerequisites. The supervisor consumer is owned by the
A6 helper; it must consume the constructor rather than copy files or reproduce
clone/hash/cache logic.

An independent tests-only unchanged-96 probe (`8075219f`, runtime `387c711`)
confirmed SQLite FTS5 3.49.1 aggregate-statistics and score drift across clear/
rehydrate and repeated real DerivedCompletion at the same canonical generation.
Order stayed unchanged in that first probe. A subsequent independent tests-only
probe (source `504f92d4`, seal `495b8fb`) also changed a multi-term query order
from `[1,2,3]` to `[2,3,1]` after clear/rehydrate of unchanged content. Hume owns
the minimal baseline fix. This
checkpoint's compact single-result probe does not inspect FTS scores; the richer
supervisor consumer retains raw scores/order and must keep any mismatch as a
failed replay. Broader score-parity promotion requires that actual fix and replay,
not a new hash normalization or a generic readiness-stamp exception.
