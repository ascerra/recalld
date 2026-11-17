# Prepared Remote canonical pages

The private daemon consumer now connects the existing canonical memory,
namespace and tag page owners to `RemoteStorageAdapter`. The minimal parents
are #112 `bcbd4fe63409945a0a297d52c165ac3060fe3de7` and R1
`52c9bdf982270a576576c0a67164dbd64e322482`; no namespace-creation or ordinary
native-startup parent is added. Public contract2 admission remains closed.

`canonicalPagesV1` is negotiated on the same connection used for the request.
It is independent of prepared-query support and provider availability. The
private methods `canonical_list_page_v1` and `canonical_catalog_page_v1` require
the capable owner and negotiated full-frame transport before execution. Missing
capability, malformed replies and insufficient frames fail closed without a
legacy fallback, reconnect retry, observation, receipt or readiness invention.

Requests reuse the existing strict parsers and resolve the default namespace
before transmission. The backend retains sole authority over the same-transaction
GenerationRef, scope, canonical selection, catalog/configuration hashes and
cursor anchors. List pages preserve UUID order; tag buckets preserve descending
numeric count and exact-label ties. Filters apply before page selection. Changing
the limit is allowed on continuation; changing scope/filter or the actual
generation is not. Empty exhausted tag buckets retain their previous anchor.

The decoder checks complete typed shape and exact JSON roundtrip, actual request
scope, identities, order, continuation and count consistency. These DTOs contain
no floating-point scalars. A continued page proves at least one prior entry,
not its ordinal: checked `returned + hasPrior + hasMore <= total` rejects
impossible counts without inventing a remaining-row total. These are consistency
checks, not authentication of a malicious remote owner's catalog or history.

Canonical Unicode display and label provenance are retained exactly. Canonical
empty structured labels never fall back to compatibility tags. The catalog
uses provider configuration captured when the actual adapter was constructed;
paging does not call a provider or clock. A carried configuration, including
explicit Unknown, takes precedence over an outer caller's configuration. The
decoder validates compatibility before any outer recomputation. Unknown legacy
namespace widths remain inventory data, while Bound/configured profile shape
validation and new-namespace width restrictions remain unchanged.

Existing backend row, namespace, vocabulary and aggregation caps remain. These
are serialized/working-set limits, not bounded scan latency or an RSS theorem.
The private consumer caps serialized output before allocating its JSON tree and
checks the actual full daemon envelope plus the actual modern MCP inner result
(text and structured content). A response that does not fit returns a whole
typed size error; it never drops rows or buckets, or alters totals to fit. Retrying
the unchanged cursor with a smaller limit is supported. Arbitrary outer MCP
string request IDs require a separate admission bound before public activation.

`serialization/page_schema.rs` describes these prepared output shapes. Its
decimal counter expression is the existing strict u63 schema; error categories
come from the actual typed enum rather than duplicated spellings. Formal tests
validate actual emitted successes and errors for all three page kinds. Rust
and backend tests retain authority over cross-field counts, byte limits,
provenance, scope and cursor hashes; JSON Schema does not replace those checks.

The actual native transport fixtures use the existing native writer/migration
and reopen path. The separate width8193 fixture deliberately authors an
already-open legacy metadata row: it proves inventory handling only, not that
a historical writer accepted that width or that native startup permits it.
Ordinary native startup, public2 activation, arbitrary outer MCP IDs and A5
latency acceptance remain separate gates.
