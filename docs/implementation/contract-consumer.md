# Actual MCP/REST contract consumer, in progress

Base: frozen prep ccdddee (runtime 7dea9d2), whose real prerequisites are PR69
and PR13. Feynman owns the exact frozen backend API in
[canonical-vector-snapshot.md](canonical-vector-snapshot.md), with minimal
parent af20d73 (actual raw-writer/publication guards). The frozen backend and
alias correction/evidence ccf8636 are merged here, as is PR19/2bebebc for the
existing session resource and actual legacy scan coverage. No generic helper PR
or offline CLI is being created.

Huygens owns exact duplicate comparison/cursor/schema/transport adaptation and
truthful discovery/tool guidance. Shared strict wire components can progress
while the storage owner implements capture/current validation. The actual
consumer must call the concrete snapshot methods, not a configurable fake
ReadView or latest-generation shim. Snapshot bounds fail before allocation;
corrupt slot references/hash/ownership and I/O are whole-operation failures.

The production paths are find_similar_memories(mode=scan) and
POST /namespaces/{name}/duplicates. Preserve their existing v1 success shapes
and semantics; exact pair edges/coverage belong to explicit v2. The generic
application-v2 gate remains closed. Native owner/schema serving and real
publication/writer joins must pass end-to-end before activation or advertising
v2, keyed writes, coherent list/search cursors or whole-query readiness.

The exact comparator uses canonical UUID i<j order and cosine computed from
the original f32 values using f64 dot/norm accumulation, without nearest-neighbor
caps. Scores and threshold comparison remain f64, clamped to [-1,1] only for
rounding at the mathematical endpoints; exact zero is serialized as positive
zero. There are no epsilon tie groups or cross-architecture bitwise claims.
Capture and final
validation bind generation, namespace/profile/policy/lineage; a concurrent write
discards the computed page. Ghost with a valid compatible vector is eligible;
tombstone is not. Missing/invalid vectors are explicit exclusions and preclude
complete coverage. Pair budgets bound comparisons, including nonmatching pairs.
Cursor restart/binding/position and checked counts are mandatory.

Snapshot `Available` retains raw f32 bits and does not itself promise exact unit
length. The actual backend separately requires every Bound vector, including
explicitly attested legacy vectors, to satisfy the canonical squared-norm
tolerance of 1e-5. Invalid Bound proof fails the whole capture; it is not a
recoverable exclusion. That proof check is unchanged. The comparator handles
finite, nonzero scaled inputs without changing bytes, but its scaled-vector
unit fixtures do not claim valid Bound storage. Even an accepted unit vector
can have enough f32 rounding error for raw dot and cosine to disagree at a
nearby threshold; a regression covers that case. A5 oracle agreement and
performance validation of this f64 implementation remain pending.

## Response bound

The comparison budget remains default 10000, maximum 100000; it limits examined
pairs, including nonmatches. Independently, the shared page must fit the existing
8 MiB daemon transport limit, including MCP's escaped pretty-text and structured
copies. Before appending a matching edge, a conservative wire bound charges
512 bytes per edge and reserves 64 KiB for the bounded cursor, namespace,
coverage, warnings, envelope and transport fields. The resulting maximum is
16256 returned edges per successful page. This is a declared transport safety
bound, not an A5 experimental response cap or silent truncation.

On overflow the entire request fails with `RESPONSE_TOO_LARGE` on `/pairBudget`;
no partial pairs, coverage or advanced cursor are returned. Retry the original
cursor with a smaller pairBudget. A lower budget does not enter cursor identity
and does not change the accumulated edge set. No result is dropped to make the
response fit. Default-budget pages cannot exceed the edge bound. An authored
serializer stress fixture checks the actual Axum REST response body, MCP
ToolCallResult plus JsonRpcResponse text/structured encoding, and a
DaemonResponse through the real bounded frame writer/reader, including
maximum-width fixed fields and long f64 representations. These are transport
serializer fixtures, not native handler/activation evidence.
Future payload additions must update this bound and its serializer regression.

After the A5 hold ended, all 17 exact-related and 9 wire regressions passed,
including these serializer checks. The interim full run was 905 passed and
1 failed: an old namespace-deletion fixture expected a foreign orphan posting
to survive the newer canonical rebuild. Its replacement retains a real foreign
canonical owner under the same UUID as the stale target posting, preserving
exact posting/count and canonical-row assertions. That focused regression passed.
Formatting passed and Clippy exited 0 with warnings. The failed run is retained;
final full validation waits for the frozen native reader dependency and actual
native scan acceptance tests. These interim results are not native activation
or performance-gate evidence.

Required tests: pure exhaustive oracle at201+records including cross-page pairs,
dense>10,ties/disconnected, budget1, no-match progress, empty/singleton,
Ghost/exclusion and arithmetic limits; malformed/change-bound cursor rejection;
actual storage mutation during comparison and restart; real MCP/REST legacy
shape/no-write tests. Authored serializer/oracle fixtures are not native serving
proof. Pascal owns the native writer/serving acceptance join needed for true
writer→publication→scan→reopen tests and malformed-proof rejection.

Additional A4 work remains: versioned public profile/keyed write adapters over
actual native/batch APIs, full per-operation schemas and envelopes, truthful
resolved session discovery, generation-bound filtered list/catalog continuations
and whole-query ReadView. No blanket A4-complete claim is made by this branch.

## Capture cost and measurement limits

Initial consumer caps are100000 namespace rows and256MiB of accounted encoded
record/copied vector bytes, with checked arithmetic and whole-page error on
overflow. All rows, including terminal/excluded ones, count against bounds.
The snapshot is recaptured for every pairBudget page; no cursor-owned vector
cache or implicit server-side job is introduced. This can dominate comparison
cost, particularly at20000 records with1536-dimensional vectors. The simple
implementation is correctness-first, not a performance-gate result.

Coordinate actual backend accounting with the storage owner and record per-page
rows, accounted metadata/vector bytes, capture wall time, pair comparisons and
comparison wall time in isolated validation artifacts. Accounting bytes are not
allocator-exact memory or physical disk I/O. Do not estimate invalid-vector read
bytes from only Available rows. Actual200/2000/20000 measurements remain required;
pure oracle fixtures and focused correctness tests do not satisfy that gate.
A bounded iterator or explicitly bounded pinned state can be considered later,
only with a concrete lifecycle/invalidation design and measurements.
