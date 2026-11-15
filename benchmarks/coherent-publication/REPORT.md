# Coherent publication and accepted public work

Measured runtime: `ddd4cc20874015a0b77f10b5c1879cd0090ca097`, tree
`369c7037666e3465eeb2c6af2cc1891844ce3243`, branch `impl/publication-controls`.
Actual dependency composition: corrected native82 `a69865b` and producer83
`209ce38`, with their required core/tracker ancestry. No broad integration ancestry.
The file inventory relative to corrected82 includes producer83 dependency changes.

## Exact validation

The frozen source passed **991 library tests, 2 CLI tests, 7 Python runner tests,
formatting, clippy (exit 0 with warnings), and 9 public-v1 episodes**: three fresh
stdio MCP, three HTTP MCP, three REST. Fixture version 3 passed all 54 returned-only
exposure cases, nine expected fail-closed replay misses, false-read durable-file
and public-state invariance, strict quality/scope, similarity thresholds, restart
and deletion checks. The full library suite includes actual HTTP/MCP/daemon
postcommit receipt and hydration error regressions. These are synthetic contract
checks, not semantic quality or a release/performance gate.

Exact command and source/input/output hashes are in `manifest.json`; executable
hashes are in `source-binaries.json`. `passed-source-output.tar.gz` contains the
allowlisted source and complete raw output, including synthetic forensic stores.
Those stores are evidence, not validated A6 snapshots. No cargo cache, target or
executable is archived. Prior successful/failed attempts remain in their original
scratch paths and `prior-run-outputs.tar.gz`, indexed by `prior-run-index.json`.

Validation used Rust 1.94.1, image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Docker network none, 2 CPUs/4 GiB, private source/cargo/target, no personal home,
credentials or socket mounts. Each source had a new private target and package
clean; no BEFORE/AFTER executable reuse. Full target compilation used `-j 1`.
Final cgroup memory peak reached 4294967296 bytes; oom and oom_kill counters were
zero. Clippy warnings are retained in full; this is not warning-free validation.

An earlier two-target compile at `14d9af5` was killed by signal 9. That attempt
has no captured cgroup OOM evidence: report a resource kill, not a proven OOM.
Its serial retry compiled but found three real fixture/owner integration failures
(988 pass/3 fail). Final source admits MCP searches through the actual QueryEngine
owner, preserving immutable offline readers, and makes the RIF fixture explicitly
false-mode. Final focused regressions and the complete suite passed. A prior
creation-cache assertion was narrowed to require that any served cached entry
matches canonical links; correctly rejecting a stale cache entry is permitted.
Earlier zero-match daemon filters are not daemon evidence; the final full suite
executes the actual daemon tests.

## Implemented boundary

One owned registry compares all required derived stamps against actual canonical
GenerationRef read from redb. The coherent read-only Rust query pins one canonical
transaction plus graph/vector/FTS/entity leases, bypasses behavioral record cache,
and samples one query clock. True mode is rejected at this entry point. Existing
public query/creation/deletion/feedback paths now carry owned roots and descendants
through provider waits, mutation, hydration, publication and committed receipts.
The readiness barrier drains accepted transitive work, checks actual pending
canonical effects/purge/batch state and all component stamps. It fails closed on
missed generation notification, foreign lineage, future target and stale views.

The coherent Rust response currently contains metadata/scores, with full_text=None.
The passing legacy public-v1 episodes do NOT establish generation-coherent full
payload responses. The next child must capture payloads under the actual engine
read lock at the selected GenerationRef, reject whole-page mismatch before
exposure, and consume the owned result without second hydration. Concurrent
forget/text-compaction regressions are required before that promotion.

Readiness is an observation; admission reopens afterward. It is not a retained
whole-core freeze or snapshot/reset permission. Deadlines are cooperative, not a
hard bound over OS I/O. Provider awaits currently retain derived leases and can
block publishers. Completion rebuilds whole record views (O(N)); no incremental
publication or performance improvement is claimed. Current barrier coverage is
for this exact joined source, not later unjoined native catalog/batch producers.

Actual public-v2 activation, physical frozen reset, controlled canonical clock,
owned ID/RNG stream, exact cache/config restoration and evaluator control-channel
activation remain outstanding. UUIDs and public wall clock remain nondeterministic.
The private endpoint draft is not part of this measured source. Full A6 is not done.
No datasets, live/paid models, training or human-judge quality results were used.
