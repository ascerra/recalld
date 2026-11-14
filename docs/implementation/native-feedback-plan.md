# Accepted native feedback coordinator

Actual parent: PR96 `387c7118864040c68d7d28c44fb2600ade520a45` only. Existing
native feedback planner, proof, transaction and current-generation publication
are sufficient; no forget/sweep/maintenance/purge/catalog/graph parent is imported.
Huygens owns public MCP/HTTP/RPC wire/context/envelopes. This private consumer must
not route native contract2 feedback through the legacy reinforcement wrapper.

`DerivedCompletion::feedback_native_with_work(ctx, id, quality,
asserted_namespace: Option<String>, policy, clock, parent) -> WriteReceipt`
uses the existing caller-configured policy and clock. It registers accepted work
before awaits; its blocking worker owns the publication lease and graph read guard
through the canonical operation even if its waiter is cancelled. New operations
check canonical by-ID namespace/name and live-content eligibility, then delegate
to `MetadataStore::apply_native_feedback_with_work` and the unchanged shared
prepare/commit logic. Existing direct metadata/session APIs delegate with no parent.

The native request hash is unchanged: scope (including namespace name and source),
original ID/access-kind list, quality and original context. Lookup reads the keyed
receipt before fresh capture/policy/time, checks its operation/ID/quality, then
reconstructs the immutable live ID-to-name binding to verify the exact original
hash. Catalog has no rename operation; numeric policy changes cannot alter the
binding. A retired missing binding fails closed with the known receipt, without
claiming verified replay. Feynman owns the subsequent shared validated purge-witness
accessor; no duplicate parser, guessed name, rewritten provenance or new hash is
introduced. Lineage validation precedes replay, which precedes fresh CAS/time.

Feedback uses the existing decay anchor and resolved policy from that same version,
never the latest retrieval timestamp. The existing transaction records lifecycle,
bounded history, typed receipt and generation atomically. It has no record-effect
outbox; actual completion observes canonical generation and rebuilds current views.
Known publication failure keeps the original full receipt. Actual uncertain commit
keeps its operation ID, poisons the owner and requires reopen; it is not relabeled
as an ordinary safely retryable error.

Seven focused tests use actual migration/native writes: anchor-versus-access with
nondefault caller policy; oneclock/replay before invalid policy/time/oldCAS; typed
quality/contract/scope/name/CAS rejection without byte changes; partial publication
without outbox plus real repair; old feedback replay after forget without resurrection;
accepted descendants through closed admission and cancellation lease retention;
actual post-commit I/O uncertainty/reopen; and real policy rollback followed by
backup/fork lineage rejection. Current-generation readiness never uses the old
receipt generation as a substitute. No native positive rows/receipts/stamps are
invented. Initial compilation used a u8 permastore field where the shared planner
requires bool; source/log preserved, no test executable ran for that attempt.

Ordinary schema2 serving remains gated. Native exposure's accepted adapter is a
separate next slice, coordinated with Curie/Hume/Huygens. No public keyed-query,
whole ReadView, deterministic-ID, performance or full-A3 completion claim.
