# Coherent V2 payload selection: correctness handoff

Runtime `d8afe45d276800bfb65b35695625d6a73ec2921c`, direct PR119 base
`8ab79b45cf53f277f96c587808615273b0055882`, branch
`impl/coherent-v2-selection`. This is an internal selection and exposure
correctness slice, with no public V2 activation or performance promotion.

## Behavior and actual consumers

The admitted pipeline now retains its validated query, namespace policy,
projected hard-eligible records and single pinned strength context for coherent
payload assembly. Explicit `prepare_coherent_payload_with_semantics` selects
V1 or V2 independently of the original numeric API contract. The existing
wrapper explicitly chooses V1. Requested access intent survives deferral.

V2 supplemental neighbors and relations use actual adjacency, hard eligibility,
current/history status, deterministic weight/UUID/kind/connected-to/directed-edge
ties, and the remaining shared state, edge and supersession budgets. The
ten-neighbor cap and five-full-text allowance follow eligibility. Stored degree
provides an O(1) whole-neighborhood budget preflight; skipped neighborhoods are
reported without collecting their adjacency. Examined self, parallel and
rejected entries consume work. Depth zero and maximum contributing depth cannot
introduce another neighbor. Relations among already selected IDs add no hop.

Current-mode historical adjacency is omitted rather than rewritten into an
invented terminal relation. It is counted separately from an unresolved chain.
Ranked terminal resolution, inherited scores and graph hop provenance remain
in the pipeline; a ranked terminal need not have the predecessor's actual edge.
History can return an independently eligible adjacent historical or unresolved
record, with the corresponding status. Missing graph state fails closed.

Existing MCP and HTTP search consumers use the prepared response's
`exposures(returns_neighbors)` helper over final hydrated IDs. V1 retains its
activation-field classification. V2 uses own/inherited channel provenance,
including associative access for a graph-only injected terminal with no own
activation field; direct evidence wins duplicate presentations. MCP returns
neighbors; HTTP returns ranked hits only and rejects unexpected supplemental
neighbors before recording. Existing recorders, pinned time, requested-access
guard and receipts remain in use. Get/similar classification is unchanged.

## Validation and immutable evidence

One frozen run passed: **8 focused V2 tests + 1 actual HTTP test, 1,025 library
tests, 2 CLI tests**, formatting and Clippy (exit 0 with warnings). No source
correction, retry or repeated full run was needed after the freeze.

The offline container used Rust 1.94.1, image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
network none, 2 CPUs, 4 GiB, and two Cargo jobs. The allowlisted source was
mounted read-only; Cargo and target were private copies, without writable
hardlinks. `cargo clean -p recalld --offline` cleared this run's debug package
before compiling. No source-root switch occurred after that clean. No home,
credentials or host socket was mounted. Container exit was 0, with no OOM, and
the owned container was removed after evidence capture.

All 229 allowlisted files matched both the immutable mounted snapshot and the
final working tree. Input manifest SHA-256:
`1cb32c43023e28157dc3c31339d9b08fc8091f69b77de9966d80de019d70400d`.
Validation log SHA-256:
`7b33a70fcacfff750be47477c55e7c589dc08329cea64a51d23e702be6d6ccc3`.
Library test executable SHA-256:
`17017e255602b871ba6d316970d98589720e351a298efc080d1395f1a125d981`.
The [input manifest](evidence/coherent-v2-selection/source.json),
[verification record](evidence/coherent-v2-selection/verification.json), and
[exact commands](evidence/coherent-v2-selection/validate.sh) are retained in Git.
Raw logs, container evidence, source snapshot and exact binaries remain outside
Git in the HOME evidence location recorded below.

The focused checks cover: inherited ranked terminal without fabricated
adjacency; current/history and filtered-terminal differences; eligibility before
neighbor/text caps; depth-zero and maximum-depth restrictions; already-selected
relations without another hop; missing graph-node failure; shared state, edge
and chain budget exhaustion; adjacency-order tie invariance; actual recorder
classification and direct-wins deduplication; false-mode byte identity; pinned
exposure time; and HTTP exclusion of unreturned neighbors. Budget tests inject
boundary counters into a real retained query context and graph: they do not
claim the tiny fixture itself examined 12,799 edges. The adjacency-order test
reverses valid in-memory adjacency at the same canonical generation.

## A4 integration contract and remaining scope

`CoherentSearchResponse.response.diagnostics` contains the final shared
`graph` work and `supersession_edges_examined` after supplemental selection.
`payload.supplemental` is `Some(SupplementalDiagnostics)` for V2 and `None` for
named legacy selection. Its fields are:

- Request/work: `neighbors_requested`, `neighborhoods_examined`,
  `edges_examined`, `supersession_edges_examined`.
- Limits: `supersession_work_limited`, `neighborhoods_skipped_edge_budget`,
  `neighborhoods_unexamined_state_budget`, `depth_limited_sources`,
  `neighbor_limit_applied`.
- Omissions/pools: `ineligible_neighbors`, `historical_neighbors_omitted`,
  `unresolved_neighbors`, `eligible_unique_neighbors`.
- Returned supplemental IDs only: `versions` and `depths`.

A4 must combine these final values with channel coverage when producing overall
coverage and reasons. Earlier pipeline channel coverage alone is insufficient
because supplemental work happens afterward. Known-historical omission is not
unresolved; a cap or skipped neighborhood must remain visible. Ranked
`totalMatches` keeps its ranked definition and does not include the supplemental
pool. Numeric API-origin forwarding remains Pascal's independent child: carry
the original value through the private selection wrapper; never infer it from
semantics. HTTP and MCP must retain their different serialized exposure sets.

V1 intentionally retains legacy cap-before-filter payload selection,
weight-only ties and raw-position text allowance. This change neither fixes nor
claims deterministic V1 ranking. Separate PR119 component permutation probes
are not an end-to-end query or incremental-publisher regression proof; their
initial coherent fixture failed its non-vacuity guard. No such proof is claimed
here. No native activation, all-writer readiness completion, public wire
contract, RIF quality promotion, or whole-roadmap A2 completion is included.

## Private evidence seal

HOME path:
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-coherent-v2-selection-d8afe45`.
The text-only `evidence/coherent-v2-selection/COPY.json` identifies every copied
member, byte count and SHA-256. Raw evidence is not in Git. The final handoff
also reports this manifest's hash; the manifest excludes itself and this report
to avoid circular hashing. The report is sealed separately in `DOCUMENTS.json`.
