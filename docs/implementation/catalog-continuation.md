# Canonical namespace and tag continuation

Implementation parent: PR100 `ff577809f8f4b9575ad465ae677ab4bab2b13195` only.
PR103 policy/catalog publication is independent and is not a prerequisite.
This document records the bounded consumer plan; it is not validation evidence.

Both consumers will read canonical metadata and actual GenerationRef in one
transaction under the already owned engine and accepted work. No vectors,
derived vocabulary index, second registry, provider embedding, or clock is
needed. Namespace order is immutable ID ascending. Tag vocabulary preserves
four buckets (tags/entities/topics/emotions), count-descending then exact-label
UTF-8 order, and the existing per-bucket limit defaults. One opaque cursor holds
all four bucket anchors so a large entity vocabulary cannot hide plain tags.
Each bucket reports its exact total and whether more entries remain; the page
reports exhaustion and one continuation. Counts deduplicate labels per live
record. Canonical structured labels, including empty labels, override legacy
structured tags. Original Unicode display and provenance are not normalized.

Cursors bind operation/version, actual lineage/G, canonical scope/catalog
configuration and anchors. Fixed-size label digests bound cursor size even for
maximum-length escaped canonical labels; every anchor must exist uniquely in
the actual vocabulary. Digests are continuation identifiers, not stored-label
encoding or authorization. Limit changes may be accepted; changed scope/G/rules
reject. No server-side cursor cache is introduced.

Row decoding, vocabulary aggregation, page retention and whole transport output
will have explicit bounds. Exceeding a bound returns a typed whole error with
no silently omitted entries. A page cap is never presented as the complete
vocabulary. V1 methods, arrays, defaults and fields remain unchanged.

Namespace entries expose canonical profile binding/provenance. Compatibility
may use only actual supplied active-profile configuration, remains separate
from canonical readGeneration, and is unknown when evidence is unavailable.
Equal dimensions alone do not establish compatibility or semantic readiness.
Metadata listing itself must remain available without a known active profile.

## Actual activation inventory

| Operation/boundary | Owner and required actual consumer |
| --- | --- |
| Catalog create and numeric policy | PR103 tested1864dab, actual same-owner completion; remote/native activation still separate |
| Metadata list | PR100; actual canonical same-transaction page consumer |
| Namespace/tag continuation | This independent PR100 child, not yet implemented/validated |
| Native store and keyed batches | Curie/Hume actual frozen producer and runner; A4 parsing/envelopes/partial-outcome wire next |
| Native logical forget | Pascal accepted native coordinator after sweep; existing96 forget wrapper is legacy, not native proof |
| Native usefulness feedback | Pascal accepted native feedback after forget using82 core; existing96 reinforce callers remain legacy |
| Native purge | Feynman frozen candidate awaiting full seal; A4 wire consumes real completion/progress/retired-name witness |
| Native sweep | Pascal frozen core correction/full validation; A4 public Fsrs adapter and typed partial/unknown envelopes |
| A2 versioned query selection | Hume/Curie handoff: actual semantics2 current/history, budgets, filter-before-caps and stable ties; legacy neighbor HashMap ties/cap10/first5 text are not a deterministic v2 oracle |
| Coherent recall payload | Actual96; coherent payload alone does not select A2 semantics2; remaining public-v2 serialization/conformance needed |
| Coherent get/similar | Curie wrapper child coordinated with Halley; completion alone is not readGeneration |
| Failure readiness | Halley actual observation API; unknown applied stamps cannot become fabricated zero/receipt G |
| Ordinary native startup | Feynman existing owner/Recalld/daemon admission after the complete writer/coherence matrix |
| Public schema, discovery, RPC, capability activation | A4; must match actual supported callers and retain v1 defaults |

Public2 remains closed during implementation. Closing this matrix and running
real public native transport conformance is the intended result; a permanent
unsupported gate is not A4 completion. No moving owner WIP is a prerequisite.
