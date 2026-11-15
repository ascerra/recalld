# Prepared strict query consumer

Actual parents are published PR142 `3953afec0e07e1eafe5e5d418746d193a6bccdc0`
(includes140/133/132) and PR88 `3961f6653d6c3076bdf9b5b8c27fa6b01a8f0b2e`.
Private merge `38f0644` retains the newer owner/work/receipt implementation and
adds the shared wire/session/snapshot interfaces. No141,144, catalog, list or
startup parent is needed merely for query preparation. PR132 already supplies
original-version exposure; tests must establish that actual path.

Implement strict recall/search request parsing and prepared MCP/REST adapters
which invoke coherent semantics2/current or explicit history. Reject unknowns,
invalid types/nulls/ranges and contradictory modes before accepted work/provider
calls. Scope uses the supplied frozen default, with canonical name/ID in output.
Supplied vectors require an explicit canonical profile binding; metadata-only
queries remain provider-free. Never route the new result through legacy ranking.

Coverage is built after supplemental hydration from final diagnostics, preserving
nullable unmeasured channel values, graph/chain limits, omission reasons and
returned-only provenance. Explanation output stays absent until Hume's actual
validated score-evidence consumer can satisfy the requested explanation. Do not
advertise a partial score decomposition as full §9 compliance.

The public version selector and capabilities remain contract1 only. Private
prepared consumers and real owned fixtures are useful before ordinary native
startup. Later public activation consumes actual startup/writer closure; startup
does not depend on public2 activation. Similarity scope/mode schema and its
prepared transport consumer follow the shared recall contract within this work;
the existing exact-scan parser/backend remains separate and unchanged.

Tests: actual owned fixture selection/hydration, current/history, scope, final
coverage, false byte invariance, original2 true exposure receipts, MCP/REST
parity, strict schema validation and public gate negatives. Retain all failures;
validate immutable source in isolated offline Docker with the existing private
cache/target. No moving WIP parents and no performance-promotion claims.

Remaining A4 error inventory outside this child: manual/sweep lost-worker
RecoveryRequired currently becomes publication_error text and HTTP generic500.
Known receipt/known uncertain operation identity take priority. This is not an
invented notCommitted outcome, but structured unknown commitState projection is
still required. No public compaction tool or speculative core change is added.
