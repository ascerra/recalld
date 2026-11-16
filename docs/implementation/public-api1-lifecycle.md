# Public API1 lifecycle consumers

This child uses PR144 only. It connects existing API1 MCP reinforcement/forget
and HTTP POST reinforcement/DELETE to the existing accepted canonical lifecycle
owners on legacy and explicitly owned native stores. It does not open ordinary
native startup or public2 admission, nor implement insertion, query observation,
profile admission or keyed public writes. Huygens owns those routing joins.

## Results and operation identity

API1 success objects retain their existing fields: reinforcement returns id,
strength, stability, phase and isPermastore; forget returns id and deleted. HTTP
retains its data/tookUs wrapper. MCP forget preserves the caller's UUID spelling.
Quality still defaults to3 only when omitted, and existing public unknown-field
compatibility remains. A syntactically valid missing/nil UUID keeps its API1
missing behavior; the stricter native-only private parser is unchanged.

Original application1 is explicit in the canonical context and private alias.
Physical schema and namespace lifecycle policy do not choose it. Native feedback
uses the configured policy and clock; its numerical projection is derived from
the typed receipt. Legacy feedback returns an ephemeral view captured inside the
same metadata transaction as the state/history/receipt. Legacy forget retains its existing timestamp behavior; no clock-derived history
is invented for its status-only receipt. No durable result fields,
old receipt encodings, hashes or migration proofs change. A second accepted write
can complete before the first response barrier returns without replacing that
first operation's numerical view.

API1 assertions are compared case-insensitively against the stable namespace
binding inside the same accepted scope-resolution guard. The owner receives the
canonical stored spelling. Stored names are not lowercased or renamed, and
PR144's keyed/private request normalization and hashes remain unchanged. Missing
or foreign assertions cannot become an unscoped successful delete.

The canonical owner supplies one accepted root, worker-held leases, publication
and a current barrier after root completion. The HTTP delete handler no longer
performs duplicate cache/index/entity/graph cleanup. Terminal UUID evidence from
the removal ledger is mapped to content-unavailable only at the feedback ledger
check; unrelated DuplicateId errors retain their existing meaning. Tombstoned
and hard-deleted feedback cannot sample a clock or commit a new mutation.

## Daemon compatibility

The existing ping advertises `api1LifecycleV1` only for an implementing owner.
The client stores this capability with the negotiated frame limit under the
same connection mutex. It resets both on reconnect. Route selection and sending
use that same locked connection, before mutation:

- An advertised capable peer receives `feedback_api1_v1` or `forget_api1_v1` with
  explicit original1 and the typed ephemeral projection/full lifecycle evidence.
- A peer without the capability receives the exact existing scoped/unscoped
  `reinforce_memory` or `delete_memory` alias. Namespace assertions are retained.
- An error, unknown method, malformed response or lost response on the selected
  new alias never triggers an old-alias fallback or automatic mutation retry.

The old private aliases remain unchanged. An old API1 alias on a new native
owner still rejects its unsupported legacy writer before mutation. An old peer's
successful scalar/object reply does not gain a fabricated receipt. Existing old
peer error/evidence limitations remain; new guarantees require the new capability.
Tests emulate the old handler shape using actual Remote/framing; they do not
execute a historical daemon binary.

New aliases require full8MiB frame negotiation before work on both client and
server. This is conservative capability admission, not a general bound on native
writes. Current feedback and forget proofs contain one primary record with fixed
numeric/hash fields, bounded profile identity and no retained payload. Forget
retains edges: its proof contains no peer array expansion. Actual canonical linked
insertions build32 retained edges in the regression; forget leaves those edges
unchanged and still emits one record/zero edge proofs. Diagnostic text is bounded
to1024 UTF-8 bytes without truncating the receipt. The full daemon error-frame
probe retains an actual receipt under a large control-character diagnostic.
These bounds do not cover insertion, hard deletion, purge or batch witnesses.

## Failure semantics and limits

Known committed publication/decode failures retain the full matching receipt.
HTTP uses its existing committed error503 shape; MCP returns structured and text
error evidence. The two native worker joins use typed unknown outcomes, including an unwind at
clock sampling before commit (not a postcommit-panic durability claim). Unknown
worker/commit outcomes retain an allocated operation ID
only when it is actually available, with no fabricated receipt, readiness stamp
or rollback. Failure does not self-drain the failed operation. API1 remains
unkeyed: this does not promise safe blind retries or public keyed query semantics.

Existing PR144 strict private identity/quality/scope/commit-state decoding remains
in force. The API1 decoder additionally checks its projected value; modifying
both the native data view and the API1 numerical projection cannot override the
canonical receipt's result. Malformed projection errors keep a matching known
receipt. Legacy receipts do not contain historical numerical views; only a fresh,
unkeyed legacy transaction may supply the new ephemeral values.

Source-specific validation passed:1209 library tests,2 CLI tests,18 focused public
tests and5 existing scope tests; formatting and clippy exit0 (with warnings). See
[the report](../../work-notes/public-api1-lifecycle/REPORT.md) for exact provenance,
BEFORE failures and the publication boundary. The
separate PR147 compaction producer is Feynman's frozen branch; Pascal's review
there is source-only. Inherited-global adoption/preview remains Feynman's active
A3/A1 work. Neither this leaf nor the final writer alone establishes all A3
acceptance or an empirical lifecycle-default promotion.
