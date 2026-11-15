# Measured vector/entity channel coverage

Actual minimal parent: published PR142 `3953afec0e07e1eafe5e5d418746d193a6bccdc0`. It already contains coherent pinned adapters, shared hard eligibility and the V2 total entity-channel allowance. PR146 explanation collection and PR102 operator RIF are not needed to count source work; their composition remains independently frozen in PR150. No broad checkpoint or transport activation is required.

## Source-supported gap

`FlatVectorIndex::search_eligible` iterates the shared index entries, rejects canonical-ineligible or namespace/filter-mismatched records before dot products, and maintains an exact top-k heap. It currently returns only the heap. The successful producer scan knows actual entries examined, distinct eligible scored records and whether qualifying records exceeded k; the pipeline cannot infer these from heap length or canonical namespace size.

`EntityIndex::find_by_entities_filtered` visits only the target namespace postings for distinct normalized query keys, applies exclusion and canonical eligibility, then sorts all eligible unique UUIDs by shared-count/UUID. `SharedEntityIndexReader` subsequently applies k. The producer/adapter together can measure posting work, eligible distinct UUIDs and actual candidate limiting. A full returned cap alone does not prove truncation.

`ChannelDiagnostics` intentionally leaves these measurements unknown today. Existing tests prove that boundary for adapters that provide only vectors of results; retain that compatibility rather than converting unknowns to zero/false.

## Bounded interface and semantics

Add an optional measured source outcome alongside results at the existing registry seams. Default trait implementations delegate to the existing method and return no measurement; production pinned/live adapters override using one read handle and the actual producer loop. Existing callers that only need results retain their result-only methods. Do not run a second scan, sample a second clock, or infer results from index/canonical sizes. Do not require all mocks/custom adapters to pretend to be measured.

Producer measurements distinguish examined work, eligible unique candidates, actual source completion and candidate limiting. Proposed examined units are vector entries visited (including rejected/foreign entries in the shared physical scan), and entity posting entries visited (including exclusion/rejection and repeated UUIDs under different normalized keys). Repeated query aliases do not revisit the same key. Document those units explicitly with the A4 consumer; do not call posting visits distinct UUIDs. Eligible/offered/admitted remain distinct candidate IDs, so counts across these different units are not additive. This avoids an additional large deduplication allocation solely for counters.

For a completed nonzero-cap scan, exhaustion is true even when candidate limiting is true: all entries were examined but additional qualifying candidates were excluded by top-k. Candidate limiting is measured qualifying candidates greater than k, not heap length equal to k. Work exhaustion is false because these existing producers have no early work ceiling. An early zero-cap shortcut must not claim exhaustion of a nonempty unexamined source; preserve unknown measurement there unless the producer actually establishes the relevant fact. Empty requested sources may report measured zero/exhausted; unrequested channels retain NotRequested/unknown fields.

Keep downstream admitted/returned provenance separate. Direct minScore, union overlap, current/history resolution and final result truncation must not rewrite source examined/eligible facts. Preserve named V1 result selection and total-V2 entity2L; do not alter FTS compatibility, ranking, filters, graph/RIF or public selectors. No new scan cap or allocation/performance claim.

## Required evidence before publication

First add actual producer/adapter/QueryEngine regressions that fail because requested production vector/entity coverage is unknown. Exercise under-cap, exactly-cap and over-cap populations; canonical rejections and foreign namespace vectors/postings; entity aliases and a UUID shared across keys; vector/entity overlap; empty and unrequested sources; and a result-only adapter retaining unknowns. Compare result UUIDs/score bits to the existing result-only exact path to detect selection changes. Validate requested source errors remain errors and no ineligible IDs leak through provenance.

Use one retained private cargo/target, package-only clean on source switch, committed atomic source export verified inside container before compilation and after execution, offline Rust1.94.1/two CPUs/4GiB/jobs2. Preserve actual RED and final source/executable hashes. No new quiet interval. After completed PR/results, release raw roots centrally; keep source, report and small text manifests.

Public all-channel coverage, supplemental graph/supersession flags and similarity cosine explanations remain separate existing ownership boundaries. This slice supplies actual missing vector/entity source facts; it does not claim full A2 conformance, exhaustive fused retrieval, experimental RIF quality or serving readiness.

A4 unit handshake: Huygens confirmed actual entry/posting measurements are compatible when the producer includes explicit per-channel `examinedUnit` (`vectorEntries`/`entityPostings`); existing FTS distinct-row and graph/chain edge units stay distinct. Unknown adapters keep both count and unit absent. This additive unit must accompany the actual public consumer join, not be guessed by serialization.

Remaining A5 implementation after A2 coverage/cosine closure: actual online ingress/provider/flush/publication counters, in addition to unmet performance promotion gates. Main acceptance reconciliation is `evidence/a5-acceptance-2026-11-14/REPORT.md` in the read-only research tree. Default instrumentation remains off; no new benchmark quiet interval until a concrete runtime/spec plan is ready.
