# Canonical namespace and tag continuation

Implementation parent: PR100 `ff577809f8f4b9575ad465ae677ab4bab2b13195` only.
PR103 policy/catalog publication is independent and is not a prerequisite.
Runtime: `fa71caf9f6255b79017c7b129190eff77ddeab35`. Validation evidence is
recorded separately; this document describes behavior and remaining scope.

## Canonical pages and continuation

Both consumers read canonical metadata and actual GenerationRef in one redb
transaction under the existing owning engine, publication guard and accepted
work. No vectors, derived vocabulary index, second registry, provider embedding,
or clock is needed. Live records include Ghosts and exclude Tombstones.

Namespace order is immutable ID ascending. Namespace entries contain exact live
record counts; catalog `total` counts namespace entries, including empty ones.
Tag vocabulary retains four buckets (`tags`, `entities`, `topics`, `emotions`),
ordered by numeric count descending then exact label UTF-8 order. Counts
deduplicate each exact label per live record. `memoriesCounted` counts live
records in the resolved scope. Count/total values are canonical u63 decimal
strings. IDs, limits and timestamps retain their typed numeric representation.

One opaque cursor carries all four bucket anchors. Each bucket reports exact
`total`, `items` and `truncated` (more entries after this page). The response
reports `complete` and a nullable `nextCursor`. An exhausted bucket returns no
more items while other buckets continue. Namespace pages use the same outer
continuation fields. A cap is never presented as the complete vocabulary.

Cursors bind operation/version, actual lineage/G, canonical namespace ID and
catalog configuration hash. Fixed-size label digests bound cursor size even for
maximum-length escaped labels; each anchor must exist uniquely in the actual
vocabulary. Digests are continuation identifiers, not stored-label encoding or
authorization. Changing `limit` is allowed; changed scope/G/rules reject.
Unchanged stores permit continuation after reopen. No server cursor cache exists.

Canonical structured labels, including authoritative empty labels, override
legacy structured tags. Original Unicode display is preserved. Each item has
`sources` counters for `rawTags`, `canonicalLegacyRetainedOnly` and
`canonicalNativeV2`; aggregation never upgrades retained legacy provenance.
This cannot recover labels already lost by older writers.

## Inputs, limits and errors

Private contract2 MCP consumers are `list_namespaces` and `list_tags`. HTTP
consumers are `GET /namespaces` and `GET /tags`. Public contract2 currently
rejects before storage work. Existing v1 namespace responses and MCP tag arrays,
defaults and fields are unchanged; `/tags` is a new HTTP route.

Namespace pages default to 50 entries, maximum 200, and always use catalog
scope. Tag pages default to 50 per bucket, maximum 500. Omitted/null MCP
`namespace` uses the actual session default unless `allNamespaces:true` is
explicit. An explicit namespace and `allNamespaces:true` cannot be combined.
HTTP query syntax uses ordinary strings and JSON integers/booleans, e.g.
`/tags?namespace=default&limit=50` or `/tags?allNamespaces=true&limit=50`.
Duplicate query keys, unknown fields, null limits/cursors, invalid types and
out-of-range values fail strictly. Cursor tokens are limited to 4096 bytes.
Input schemas and parsers share limit constants; these schemas are private,
not a declaration of publicly supported v2 output contracts.

Each page scans canonical rows to compute exact totals and vocabulary. Bounds:

- One decoded raw row/config: 1 MiB.
- Retained namespace catalog: at most 100,000 entries and 32 MiB raw config bytes.
- Retained vocabulary: at most 100,000 distinct bucket/label pairs and 8 MiB label UTF-8 bytes.
- Sum of encoded retained entries: `(8 MiB - 64 KiB) / 32`, sharing PR100's
  conservative allowance for the complete MCP text plus structured result,
  JSON-RPC/daemon framing, cursor and envelope. Oversized pages fail as a whole.

These are row, aggregation and serialized-output limits, not peak RSS bounds.
Examined canonical rows and scan latency are not capped. Every continuation
rescans and sorts its vocabulary; no A5 performance acceptance is claimed.
A future bounded indexed implementation must preserve these exact semantics.

Input failures return 400 with a field; unknown namespace 404; stale cursor
409; storage/admission failure 503. Size failures return
`RESPONSE_TOO_LARGE`: page failures identify `/limit` and permit smaller-page
retry; catalog aggregation failures cannot be fixed by a smaller page limit.
No partial vocabulary is silently emitted. MCP execution failures set
`isError:true`, using the same typed envelope and code as HTTP. Remote adapters
without the new capability fail unavailable rather than falling back to a
legacy capped list. Actual dedicated remote RPC activation remains follow-up.

## Profile context

Namespace entries expose canonical profile ID/provenance and explicit
compatibility reasons. `configuredEmbedding` is optional configuration identity
captured from the actual provider when constructing the local adapter. It is
not remote-weight verification, provider availability, a hot-reload guarantee,
or part of canonical `readGeneration`. Invalid/missing configuration remains
unknown. Metadata requests never call the provider, even for profile metadata.
Exact profile identity plus dimension is required for `compatible`; equal
width alone never supplies identity. Unknown namespace profiles remain unknown.
Metadata listing works without a known provider/profile.

## Actual activation inventory

| Operation/boundary | Owner and required actual consumer |
| --- | --- |
| Catalog create and numeric policy | PR103 tested1864dab, actual same-owner completion; remote/native activation still separate |
| Metadata list | PR100; actual canonical same-transaction page consumer |
| Namespace/tag continuation | This independent PR100 child; private consumers, public contract2 still closed |
| Native store and keyed batches | Curie PR106; A4 parsing/envelopes/partial-outcome wire next |
| Native logical forget | Pascal validated51501b6 accepted coordinator; existing96 forget wrapper is legacy, not native proof |
| Native usefulness feedback | Pascal accepted native feedback after forget using82 core; existing96 reinforce callers remain legacy |
| Native access recording | Pascal accepted native exposure after feedback; Curie get/similar and Hume A2 consume it. Existing96 observation always uses contract1 and is not native true-mode proof |
| Native purge | Feynman PR105; A4 wire consumes real completion/progress/validated retired-name witness |
| Native sweep | Pascal PR107; A4 public Fsrs adapter and typed partial/unknown envelopes |
| A2 versioned query selection | Hume/Curie: actual semantics2 current/history, budgets, filter-before-caps and stable ties; legacy neighbor HashMap ties/cap10/first5 text are not a deterministic v2 oracle |
| Coherent recall payload | Actual96; coherent payload alone does not select A2 semantics2; remaining public-v2 serialization/conformance needed |
| Coherent get/similar | Curie wrapper child coordinated with Halley; still explicitly V1 selection until the actual A2 consumer |
| Failure readiness | Halley actual observation API; unknown applied stamps cannot become fabricated zero/receipt G |
| API1 on native-schema stores | Feynman/A4 explicit per-operation owner routing and compatibility contract; omitted API version stays 1, and existing payload/default semantics cannot accidentally route to legacy-only writers |
| Ordinary native startup | Feynman existing owner/Recalld/daemon admission after the complete writer/coherence matrix |
| Public schema, discovery, RPC, capability activation | A4; must match actual supported callers and retain v1 defaults |

Native exposure must reuse the query's sampled clock, deduplicate returned UUIDs,
group namespace-scoped writes, preserve complete per-group receipts/partial and
unknown outcomes, and publish through the accepted owner. The pre-exposure
`readGeneration` remains distinct from committed generations. Explicit false
means no write. A passing native read-only path does not establish true-mode
activation. No native-to-legacy observation fallback or public keyed-query claim.

API, retrieval, lifecycle and disk versions are independent. Canonical schema2
does not implicitly select API2 or explicit-feedback lifecycle behavior. Before
ordinary native startup opens, API1 callers need tested participating owner
paths (or an explicitly agreed unsupported operation error), never silent
legacy-writer fallback. Native read-only success cannot stand in for that proof.

Public2 remains closed during implementation. Closing this matrix and running
real public native transport conformance is the intended result; a permanent
unsupported gate is not A4 completion. No moving owner WIP is a prerequisite.
