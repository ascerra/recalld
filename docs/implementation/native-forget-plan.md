# Accepted native forget coordinator

Actual parent: PR96 `387c7118864040c68d7d28c44fb2600ade520a45`. It already
contains the corrected PR82 native tombstone primitive, accepted tracker and
current-generation registry. No sweep/maintenance, purge, catalog or graph parent
is needed. Huygens owns public wire/schema/envelope integration; Pascal owns this
accepted coordinator and the subsequent separate native feedback coordinator.

`DerivedCompletion::forget_native_with_work(ctx, id, asserted_namespace,
policy, clock, parent) -> WriteReceipt` consumes the existing configured policy
and clock from its caller. It creates no runtime settings, store/session, clock,
scheduler or planner. Ordinary schema-2 serving stays gated. This private entry
must not be routed to the legacy bool forget wrapper for contract2.

The existing native tombstone hash is unchanged. A small metadata lookup accepts
stable caller ID/context before current capture or clock sampling. Lineage and
matching replay precede CAS and fresh policy/record checks. Fresh operations
assert an optional name against the canonical by-ID namespace and require the
context's immutable namespace ID to match. No session default resolves by-ID scope.

Matching replay preserves the original full receipt. An optional name remains
an assertion against the same stable namespace ID; it never rebinds a reused name.
A missing/retired name without a validated retained binding fails closed with the
known receipt. This is an explicit future purge-witness integration condition,
not a permanent v2 feature omission. No purge history is invented here.

Accepted parent/root admission precedes awaits. The blocking canonical worker
owns its publication lease and graph read guard in graph -> storage order, even
if its waiter is cancelled. It delegates to the existing native capture/final
transaction, including monotonic phaseTimeRule, content erasure, ledger/outbox,
record tokens and typed proof. A fresh key on an existing tombstone follows that
native receipted transaction, rather than becoming a legacy unbound false result.
The worker drops locks before actual completion, which publishes the same owner's
views and retains the original full receipt on any known post-commit failure.

Focused validation uses actual legacy/schema/lifecycle migration and native
insertion, real publication failure, concurrent identical keys through closed
root admission and a fresh-generation barrier, one sampled clock, scope/CAS
rejection, terminal receipt replay, cancellation and strict reopen. No native
rows, receipts or view stamps are authored as positive fixtures. The initial
marker assertion expected3000 instead of max(oldmarker,effective5000); its failed
source/binary/log are retained and the assertion now follows the existing rule.
There is no performance, deterministic-ID, cross-lineage generation comparison or public activation
claim. The shared publication implementation remains owned by Halley; its separate
outer-lease cancellation hypothesis was reported without a falseReady claim.
