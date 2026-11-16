# RIF adjacency work and temporary collection bounds

This is a correctness child of PR #76 (`78f8e6e8a6ca55c4b4d51850858ba41b306a101e`), with no ingestion, coherent-registry or other new parent. The original opt-in RIF experiment remains default Off; V1, public transports, lifecycle writers, graph writers and existing v2 graph expansion are unchanged. The prior report's 100-target/1000-pair limits were admission limits only. This change separately bounds actual adjacency examination and the temporary adjacency/pair/similarity collections.

## Work policy

One query gets **12,800 adjacency entries** for the operator-only RIF pass. This is separate from graph expansion's work allowance and from the unchanged limits of 100 admitted unique targets per anchor and 1000 admitted pairs per query. The new allowance is a provisional correctness work limit, not a measured performance threshold.

The actual graph stores incoming and outgoing edge keys in two Vecs. `GraphNode::degree()` reads their stored lengths in O(1); it does not count an iterator or collect neighbors. For each frozen Off-ranked anchor, the graph adapter checks namespace and degree before allocating an edge buffer. If its entire neighborhood cannot fit in the remaining allowance, it returns a typed work-limited result with zero edges, zero edge-buffer capacity and the unexamined degree. The query stops there. It does not inspect or admit an insertion-order-dependent prefix, and it does not skip ahead to a cheaper later anchor.

A completed neighborhood counts every inspected adjacency entry before eligibility/deduplication: self entries, rejected/foreign/filtered targets, parallel edge types and reciprocal directions all consume examination allowance. A self edge occurs once in each adjacency vector and therefore consumes two entries. No lookahead entry is read: the stored degree provides the exact refusal count. All accepted neighborhoods retain the original true canonical S/R and available cosine, pair deduplication, strongest activation/type tie rule, activation/UUID ordering, 100/1000 admission caps and shared cumulative suppression context.

The graph API is now `rif_edges(anchor, namespace, allowance) -> Result<RifAdjacency>`. Production adapter and callers are updated. Unsupported/corrupt examined reads remain errors; malformed partial adapter coverage is rejected as Unavailable, never interpreted as an empty complete source. A refused unexamined neighborhood cannot certify whether its edges are corrupt. Its incompleteness is explicit.

## Diagnostics and allocation scope

`examined_edge_limit` reports 12,800. `adjacency_complete=false`, `blocked_adjacency_edges` and `unprocessed_anchors` identify a budget-limited read; `completed_anchors` distinguishes the successfully consumed prefix of whole anchors from all frozen eligible anchors. Existing eligible/duplicate/similarity/admission counts describe **completed anchors only** when incomplete. They are not totals for unexamined adjacency. Experimental mode can apply suppression from completed anchors; Shadow remains bit-identical to Off. If the first neighborhood cannot fit, no pairs are computed and experimental ranks are also identical to Off. This is explicit incomplete coverage, not a successful exhaustive empty channel.

Each temporary adjacency buffer contains at most 12,800 entries; its allocation is requested only after degree preflight. The pair tree has at most 12,800 entries across the query because each pair requires an examined entry. Each per-anchor similarity tree is also bounded by examined entries; a BTreeMap avoids capacity growth tied to a hash table. Sorting retains at most the same number of pairs before admission. `peak_edge_buffer_entries`, `peak_pair_entries` and `peak_similarity_entries` report retained entry peaks. They are not allocator-byte counters or RSS measurements. No new full-degree collection or graph writer/index structure is introduced.

These are bounds on the RIF **adjacency-derived temporary collections**, not a whole-query allocation/RSS guarantee. Canonical eligibility enumeration, the parent implementation's original score-contribution snapshots/candidates, vector storage/dimensions and graph storage have their existing separate costs. Dense-query latency/RSS and the A6 matched-stratum quality gate remain unmet; Off remains the default.

## Regression coverage

- Actual graph preflight with self/parallel/reciprocal entries verifies exact stored degree, zero refused buffer capacity, and a deliberately corrupt first edge that is untouched by refusal but errors when the whole neighborhood is admitted.
- An actual QueryEngine fixture has 12,798 entries in the first anchor and two in the second, exactly consuming 12,800. Its returned scores/multipliers match an independently selected exhaustive top-activation/UUID oracle. It verifies duplicate counts, 101 admitted pairs, collection peaks and reverse record/edge insertion order.
- Adding one entry leaves only one slot for the next two-entry neighborhood: exactly one anchor completes, the second is explicitly unprocessed, and the prefix's suppression is deterministic.
- Adding three entries makes the first neighborhood 12,801 entries: zero examination/pair/similarity allocation, explicit incomplete diagnostics and Off-identical experimental results. An oversized caller allowance cannot bypass the production adapter's 12,800 limit.
- Existing graph-error, rejected-ID, parallel-edge, signed-zero, all-channel, inheritance/graph-rediscovery, shared 25% cap, Off/Shadow/V1 and real canonical no-write/single-clock regressions remain in the suite.

Validation and immutable source evidence are recorded in the handoff manifest. No public v2 readiness, quality promotion, performance promotion, native serving activation or persistent RIF is claimed.

## Validation result

Immutable source manifest `0b7006a296088ebcdfd221070ba6ba1d82913f7153427eb6123653224b169c38` covers 171 files. Pre/post checks matched the read-only mounted snapshot and working source. On Rust 1.94.1 in offline Docker with 2 CPUs/4 GiB, private cloned Cargo/target and an explicit own-package clean: **18 focused RIF tests passed; 792 library + 2 CLI tests passed; fmt passed; clippy completed with warnings**. Four doc examples were ignored. This run had no failed tests or retries. Cargo-reported test executable and actual CLI hashes are retained; unrelated cached executables are not execution evidence. No concurrent source edit occurred during validation.

Source patch, complete log, manifests and executable hashes are copied to actual HOME work-notes, with location/hash evidence in `docs/evidence/rif-work-bounds`. No raw cache, executable or archive is committed. This is correctness evidence only; no benchmark, RSS bound or quality promotion was inferred from the test run.
