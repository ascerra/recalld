# Explicit coherent retrieval semantics 2

Base: PR119 `8ab79b45cf53f277f96c587808615273b0055882`. This plan and
implementation do not modify the frozen incremental-publication study.
CONTRACTS and A2 sections 4, 6, 7, and 9 govern the selector. No transport
activation, numeric API-origin inference, or ranking-profile change is included.

Reuse the admitted pipeline's validated query, namespace, projected eligible
records, and pinned strength context for payload assembly at the same canonical
generation. Existing callers retain explicitly named V1 selection. An explicit
internal semantics argument selects V2; requested access remains unchanged
while recording is deferred until final hydration.

Supplemental metadata describes actual adjacency only. Current mode omits a
proven historical raw neighbor before its ten-neighbor cap, counting this
separately from unresolved chains. It never substitutes a terminal into an edge
or connected-to relationship. History admits independently eligible actual
neighbors with their version status. This does not alter ranked graph-candidate
terminal resolution, inherited scores, or contributing hop provenance.

Auxiliary work shares the pipeline's remaining 400-state, 12,800-edge and
12,800-supersession-examination budgets. Stored adjacency lengths permit an
O(1) whole-neighborhood preflight; an over-budget neighborhood is skipped and
reported, never partially admitted according to insertion order. Rejected,
self and parallel entries count when examined. Depth zero adds no IDs; a
selected result at maximum contributing depth cannot introduce another hop.
Relations between already selected eligible IDs do not introduce another ID.

Hard eligibility and current/history checks precede neighbor admission. Use
finite descending weight, UUID, edge kind, connected-to UUID and directed-edge
identity to settle ties. The first five admitted ordered neighbors receive
full text. Diagnostics expose aggregate omissions, limits and returned IDs
only. Existing exposure consumers must derive their set from final hydrated
rows and neighbors, with direct-result access winning duplicates.

The eventual A4 coverage mapping must combine the final shared graph/chain
budgets and supplemental omission/limit diagnostics. Pipeline channel coverage
was collected before supplemental assembly and is not, by itself, overall
payload coverage. Ranked `totalMatches` retains its existing definition;
supplemental eligible/admitted pools are reported separately.

The existing MCP/API coherent search consumers use one prepared-response
exposure selector over final hydrated IDs. V1 preserves its activation-field
classification. V2 consults own/inherited channel provenance, so an injected
terminal supported only by graph evidence receives associative access despite
having no own activation field. Any direct-channel evidence wins, and UUIDs
are deduplicated with direct access winning a duplicate presentation. This
helper performs no writes; existing callers retain their requested-access
guard, pinned clock, actual recorder, receipt propagation and publication.
Get/similar paths retain their existing classification. Pascal owns numeric
API-origin forwarding on a separate minimal119 child; the later mechanical
join must carry that original number through the private selection wrapper,
never derive it from retrieval semantics.

The exposure helper also takes the actual serialized surface: MCP includes
neighbors; HTTP returns ranked hits only and fails closed if its producer
unexpectedly supplies supplemental neighbors. An actual HTTP fixture with
canonical graph neighbors must prove only serialized hits receive exposure.

Required regressions: terminal with no actual adjacency remains ranked through
inheritance without a fabricated supplemental relation; current/history and
filtered terminal differences; no depth-zero or maximum-depth bypass; stable
equal-weight selection and full-text allowance after rejected neighbors;
shared budget exhaustion and partial-neighborhood coverage; false-mode byte
identity and final hydrated exposure IDs. Preserve the existing V1 behavior.

Validation completed after the coordinated measurement hold ended: 9 focused
checks, 1,025 library tests and both CLI tests passed; formatting passed and
Clippy exited 0 with warnings. See coherent-v2-selection-report.md for the exact
frozen source and evidence. No performance, public-v2 or serving-readiness
completion is claimed.
