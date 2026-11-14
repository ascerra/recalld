# Canonical metadata list paging (private contract2 consumer)

Parents: PR88/3961f66 plus the actual PR82 startup recovery correction fb804a0. The diagnostic 2ea143b run used PR93, but catalog-create PR93/87 is not a runtime prerequisite and is excluded from this shipping composition. No moving query, native-policy or batch implementation is imported. This slice supplies actual inner MCP `list_memories` and REST `GET /memories` consumers. Ordinary contract2 remains rejected before work; this is not completion or activation of A4.

The existing owned engine admits a work root before scheduling. A single redb transaction under metadata publication supplies the actual store-wide GenerationRef, canonical namespace/config bytes, schema, records and filtered total. No vectors, embedding profile, provider, behavioral cache, derived readiness or full-text hydration is required. This is metadata-only consistency, not a whole-query ReadView. Listing is non-observing and has no receipt or deferred mutation.

Namespace resolution reuses exact-first/case-insensitive canonical lookup from vector snapshots. Omitted/null MCP namespace resolves to the frozen bridge default; HTTP resolves to the configured server default. Explicit unknown names fail. V1 omission, fields, offset sorting and payloads remain unchanged. UUID-byte order is ascending for contract2. Ghost rows remain visible; tombstones do not. Phase, tags, structured labels and inclusive creation timestamps are applied before page selection and counting. Labels preserve exact stored values/provenance; canonical empty labels never fall back to compatibility tags. Queries compile whole-label Unicode lowercase once, without trimming, Unicode normalization or aliases. Legacy filters retain legacy encodable-tag restrictions.

`limit` is 1–200 (default 50); `cursor` is at most 4096 bytes. MCP arrays contain strings. REST query arrays use JSON-array syntax, percent-encoded in the URL, so a comma in a canonical label is not split; integer parameters use JSON integer syntax. Repeated/unknown parameters, explicit null except MCP namespace, invalid types and reversed/unrepresentable times fail. V2 rejects v1 offset/sort/order and `recordAccess`: lists are always non-observing.

A continuation binds cursor/contract/eligibility versions, actual lineage/generation, immutable namespace ID and hash of its canonical configuration, filter hash and last eligible UUID. It is not authorization. Limit may change between pages; changed filters fail and changed generation/policy/lineage is stale. An anchor must still be an eligible canonical row. Each page scans all metadata for an exact filtered decimal-string `total`; it does not pretend a bounded candidate sample is exhaustive. `complete` describes exhaustion after this page and `nextCursor` is absent only at exhaustion. There is no server-side cursor cache.

Memory is bounded by one decoded row plus the retained page, not the store size. Raw rows over 1 MiB fail before decode. Retained encoded metadata may occupy at most `(8 MiB - 64 KiB)/32`; the conservative multiplier covers duplicated canonical/legacy arrays, escaping, MCP text plus structured content and transport framing. Crossing that bound fails the whole page with RESPONSE_TOO_LARGE: no matches disappear and the original cursor can be retried with a smaller limit. The scan itself has no hidden record cap. This O(N)-per-page implementation is not an A5 performance claim.

## Remaining action matrix

| Operation family | Actual consumer in this stack | Public / RPC gap and next owner |
| --- | --- | --- |
| Strict v1 validation, scope/read-only, context | Existing v1 MCP/REST and dedicated context RPC | Preserve compatibility; no public contract2 advertisement |
| Exact duplicate scan | Actual snapshot comparator, inner MCP/REST, generation/pair cursor | Native serving activation; dedicated RPC; representative performance measurements |
| Native namespace create | PR93 inner MCP/REST, replay/CAS/root/full receipt | Public activation and RPC; no global readiness claim |
| Canonical metadata list | This slice: owning transaction, filters, UUID continuation, inner MCP/REST | Public activation and RPC; bounded tag/namespace catalog continuation still outstanding |
| Numeric namespace policy | Feynman PR94 native core validated | A4 wire next uses actual PR93+94+96 (no list dependency), same engine/root and G-aware coordinator; test changed/no-op/replay before readiness claims |
| Native store / keyed batches | Curie/Hume native producer and proof owners | Frozen actual API, wire schemas, partial outcomes, replay and RPC conformance |
| Native feedback / forget / maintenance | Pascal native core | Actual versioned wire adaptation and receipt/error parity |
| Native namespace purge | Feynman extends typed start/progress and retained namespace witness | A4 owns later transport consumer after frozen core; no legacy fallback or invented retired name |
| Query / get / similar payloads | Halley coherent core, leases/root/exposure/registry | A4 strict wrappers after frozen APIs; metadata stamps are not payload coherence |
| Discovery / schemas / activation | Shared gated helpers and truthful v1 context | Complete per-operation input/output schemas, profile selectors and MCP/REST/RPC public conformance before enabling contract2 |

Activation is a remaining task, not a permanent excuse to stop at helpers. Native writers, coherent payload consumers and public transport contracts must be tested together before supportedContractVersions advertises 2.
