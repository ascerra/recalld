# R2 prepared Remote canonical pages

Owner: Huygens. Halley explicitly released sole isolated implementation after
source review under the user's standing authorization. Queue publication and
the R1 branch remain frozen and queue-owned. This is a plan, not validation.

Exact composition `3991e45b47df1a02fec559499a2c5246f4222ac1` is a conflict-free
normal merge of canonical pages #112
`bcbd4fe63409945a0a297d52c165ac3060fe3de7` and R1
`52c9bdf982270a576576c0a67164dbd64e322482`. Their merge base is
`5d18075eea193849056ae88cb07f27843d1c7019`. No #65, N1, or E1 convenience
parent. Feynman reviews snapshot/cursor authority before runtime correction.
Builds wait for the queue's bounded resource slot; source and test drafts may
proceed. No edits to any frozen parent or queue branch.

## Actual missing consumer

`RemoteStorageAdapter` inherits unavailable defaults for
`list_canonical_page` and `list_catalog_page`. The owned adapter already calls
the canonical, same-redb-transaction page implementations. Complete these
Remote trait methods through private daemon aliases `canonical_list_page_v1`
and `canonical_catalog_page_v1`; do not fall back to legacy list methods.

A separate `canonicalPagesV1` handshake capability represents actual storage
support. It must not require prepared query support, a known embedding profile,
or an active provider. Reuse R1's one-connection lock, negotiated frame limit,
bounded raw preflight, and attributed exchange without a second handshake or
post-send reconnect/retry. Select required capability by the actual method.
Unsupported/old peers and insufficient negotiated frames refuse before the
page request. Public version2 selection stays closed; v1 behavior is unchanged.

## Authority and representations

Requests reuse the existing strict list/catalog parsers. Resolve session
default namespace before sending; explicit namespace is preserved. No
recordAccess option exists for these metadata-only operations. Namespace
catalogs always select all namespaces; tag catalogs preserve explicit scoped
or all-namespace selection. Caller filters and cursor tokens remain unchanged.

Private list responses carry actual `SnapshotBinding` (GenerationRef,
namespace ID, namespace configuration hash), resolved namespace name, and
typed data. Catalog responses carry actual GenerationRef, scope, and typed
data. Do not invent a binding hash from the public envelope. The backend owns
canonical row selection, anchor existence/uniqueness, configuration hashes,
lineage checks, exact totals, and stale-generation rejection. Client validation
checks consistency, not remote authenticity.

Strict bounded decoding checks fields and variants, canonical decimal counters,
namespace/UUID identities, requested limit, ordering and uniqueness, page
cardinality, complete/nextCursor/truncation relationships, and cursor bindings.
Validate the incoming and returned cursor against the unchanged request and
actual returned binding. A changed page limit is allowed; scope/filter/policy/G
changes are not. Preserve exact Unicode display, label provenance, and
authoritative empty structured labels. Never recompute missing canonical labels
from legacy tags when canonical labels are present.

Namespace catalog compatibility uses configuration captured by the actual
daemon adapter. No provider method is called during paging. The current outer
MCP `with_active_embedding(storage.catalog_profile_context())` must not erase
received daemon context with a Remote `None`, or substitute client-side
configuration. Use a narrow context-finalization seam, reviewed before changing
the local and HTTP callers. Unknown context remains explicitly unknown.

Use the actual emitted representation when comparing local and Remote results;
do not repeat direct typed-to-Value assumptions from R1's retained numeric
failures. No scalar tolerance or global serde changes. Any numeric semantic
drift needs its own concrete evidence before changing the protocol.

## Bounds and errors

Keep the existing canonical row, catalog/vocabulary aggregation and retained
page limits. These are serialized/working-set bounds, not scan-latency or RSS
claims. Count complete private daemon replies before transmission and actual
MCP text plus structured and REST projections in fixtures. Oversize is a whole
typed page error, never skipped rows, partial totals, dropped buckets, or a
silently shortened page. Existing conservative MAX_PAGE_BYTES is not a proof
for an arbitrarily large outer MCP request ID. Do not claim that broader outer
transport gate solved here.

Map backend input/not-found/stale/size/unavailable errors without conflating
them. Malformed or unattributable peer replies fail closed with no fallback;
these calls perform no observation or writes, so they do not manufacture
receipt, readiness, operation identity, or committed status. Capability and
server dispatch checks precede page execution on unsupported transports.

## Files and validation sequence

Expected runtime scope: new `src/daemon/prepared_pages.rs`; existing daemon
client/server/bridge adapters/module wiring; narrow MCP storage capability and
catalog context seams; private typed DTO support in canonical list/catalog
modules; request conversions in `serialization/list.rs` and `catalog_page.rs`.
Tests and documentation accompany the actual consumer, not a helper-only PR.

1. Commit real BEFORE transport regression(s) against this parent composition:
   owned canonical page succeeds, same request through Remote fails. Run only
   after a resource slot is released; preserve immutable source, executable,
   exact assertions and logs before the runtime fix.
2. Actual owned/Remote/prepared MCP and HTTP parity on legacy and valid native
   owned fixtures. Exercise 201+ UUID rows, filters before pages, wrong defaults,
   case aliases, Unicode and authoritative empty/provenance labels. Native
   codec-only authored fixtures are labeled separately from serving proof.
3. Traverse namespace/tag pages, numeric count descending plus exact-label ties,
   uneven four-bucket exhaustion, empty namespaces, exact totals, allowed limit
   changes, rejected filter/scope changes, mutation staleness, and continuation
   after reopen. Preserve backend bounds and generation authority.
4. Provider/clock/embedding calls remain zero; canonical records, receipts,
   catalog, generation and exposure state remain unchanged. Expected SQLite SHM
   reader coordination is distinct from canonical or FTS logical mutation.
5. Same-connection malformed-peer negatives: missing capability, limited frame,
   bad correlation, duplicate/unknown fields, invalid counters/identity/cursors,
   reordered or duplicate entries, false completion/truncation and oversized
   frames. No legacy fallback and no observation on rejected input.
6. Actual whole-frame boundary and output-schema tests, then one frozen full
   run and relevant formatting/Clippy. Use offline isolated Docker, bounded
   resources, one private cache/target; clear only the owned package artifacts
   when switching source roots and record source/executable provenance.

Final source/CI/integration claims remain separate. Ordinary native startup,
public2 activation, A5 performance acceptance, and all-A4 completion are not
provided by this slice. Completed scratch is released centrally only under
the recorded retention policy; active evidence and future fixtures stay protected.
