# Actual MCP/REST contract consumer, in progress

Base: frozen prep ccdddee (runtime 7dea9d2), whose real prerequisites are PR69
and PR13. Feynman owns the exact frozen backend API in
[canonical-vector-snapshot.md](canonical-vector-snapshot.md), with minimal
parent af20d73 (actual raw-writer/publication guards). That backend is not yet
implemented/tested in this branch. It will merge here as a real prerequisite;
no generic helper PR or offline CLI is being created.

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

The exact comparator uses canonical UUID i<j order and the existing normalized
vector dot-product kernel, without nearest-neighbor caps. Capture and final
validation bind generation, namespace/profile/policy/lineage; a concurrent write
discards the computed page. Ghost with a valid compatible vector is eligible;
tombstone is not. Missing/invalid vectors are explicit exclusions and preclude
complete coverage. Pair budgets bound comparisons, including nonmatching pairs.
Cursor restart/binding/position and checked counts are mandatory.

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
