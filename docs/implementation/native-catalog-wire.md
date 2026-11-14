# Gated native namespace-create wire consumer

Minimal parents are PR88 `3961f66` and PR87 `b52801d`, joined at `8afffdb`.
The merge preserves both legacy LifecycleMaterialized and native catalog proof
variants, plus maintenance and native-catalog engine modules. No batch or
coherent-query WIP is included.

The consumer keeps the existing MCP create_namespace and HTTP namespace
creation paths. Strict v2 input validation rejects unknown fields, explicit
null numeric/profile selectors, bad finite bounds and malformed profile IDs;
the existing v1 parser and success fields remain unchanged. Shared WriteOptions
validates explicit lineage, printable idempotency keys, body/header equality and
decimal expectedGeneration. A1 owns the original normalized request hash,
replay-before-CAS order, immutable IDs and actual catalog commit.

Feynman approved extracting the existing NativeLifecycleSession orchestration
into a crate-private engine method reused by that session and concrete adapters:
create_native_namespace_with_provider(ctx,input,provider,policy,clock). The
adapters must supply their actual provider, frozen policy and clock to the same
shared owning engine. They cannot reopen storage, instantiate another owner,
guess a default policy, sample provider identity before historical replay, or
hold a standard lock across await. Profile and dimension calls are synchronous;
namespace creation does not embed text.

Feynman's validated PR87 correction now carries the full receipt already
available at both definite publication-failure sites. The wire preserves it
without attempting a lookup through the poisoned owner. Commit-unknown remains
distinct, with reconciliation required and no fabricated receipt. A canonical
namespace receipt alone is not proof of all derived views being ready. This
consumer omits shared readiness and states that limitation explicitly; public
contract2 remains disabled. No synthetic readiness or ReadView is added.

Tests must exercise actual native creation through both inner transport handlers,
cross-transport matching replay with stale CAS, changed-payload conflict, foreign
lineage, original-selector identity, fixed-default equivalence, no provider call
on replay, invalid-input byte immutability, typed commit/publish failures and
strict reopen. Public contract2 remains unconditionally rejected. A new provider
configuration or caller descriptor must never silently rebind a namespace.

The final candidate consumes Pascal's validated minimal PR82 startup correction
fb804a0 and Feynman's validated full-receipt correction27ef23d. Disposable
preflight repair remains confined to the copy; ordinary offline abort-on-repair
semantics are unchanged. Neither dependency adds batch ancestry.

Separate remaining children consume Curie's frozen batch manifest/runner API
and an actual same-transaction paging/filter API. Halley's coherent query alone
does not supply catalog/list pagination or hydrated text at its generation;
legacy hydration cannot inherit that response's readGeneration. Full native
wire activation waits for the complete writer/readiness/serving gates.

## Current independent implementation checkpoint

The shared strict request parser and actual namespace adapters/inner MCP and
REST handlers are implemented. Direct MCP/HTTP/daemon construction supplies the
actual provider, clock and QueryEngine frozen policy inputs. The same owning
engine now implements the session and adapter orchestration. Accepted work
propagates through both replay lookup and final publication; standard locks are
held only in the blocking worker. Typed profile selector conflicts remain
distinct from unavailable provider configuration. V1 paths and public2 rejection
remain in place. Remote native catalog RPC is not implemented and cannot fall
back to legacy creation.

Five focused tests passed offline in Rust1.94.1 Docker (network none,2CPU/4GiB):
strict input/key/CAS parsing, actual cross-transport replay with stale CAS and
provider calls forbidden, reopen/replay, byte-immutable rejected inputs/profile
conflicts, and accepted-root lookup+commit during closed admission. Actual MCP
text/structured results agree. These are inner-handler tests, not public2
activation or full suite evidence. Source and six executable hashes are retained
under /tmp/recalld-native-catalog-wire-validation/consumer. This source-root switch
used cargo clean -p recalld in the private target after preserving PR88 hashes.

The validated parent corrections are joined. Actual MCP/REST fault checks cover
definite commit failures and uncertain acknowledgment, poisoned lookup refusal,
strict reopen, exact retained receipts and one-time replay. Catalog success
omits shared readiness and carries a warning; this is not whole-writer/serving
readiness. Public schema advertisement and contract2 activation remain disabled.

The subsequent focused catalog run passed27 tests in28.57s, including existing
real process-crash recovery cases; the unchanged v1 namespace-option parity
regression passed1 test in0.36s. These are bounded checks, not the final full
suite. [Initial evidence](../evidence/native-catalog-wire/initial/manifest.json)
retains28 artifacts with source/binary hashes and explicitly identified capture
timing. No before/after source-root switching occurred within these runs.

## Receipt fixture correction

The first fault fixture incorrectly compared serde_json::to_value of a typed
f32 receipt with JSON parsed from its actual serializer. The former widens f32
values, while both original MCP and REST typed serializers emit the same shorter
f32 decimal representation. This was a fixture mismatch, not a demonstrated
production float-parity bug. A REST-only response-conversion attempt did not
resolve the bad expectation and was reverted. A subsequent shared conversion
proposal was also removed before execution after main review.

The corrected expectation parses the retained receipt's actual canonical
encoding. Strict JSON equality, typed WriteReceipt equality and exact canonical
encoded-byte equality all remain. The real cross-transport replay test compares
full MCP/REST receipts (only the declared replayed flag differs), data and MCP
text/structured content. No production response normalization, rounding,
canonical receipt/request-hash change or v1 serialization change remains. Both
failed executed sources, binaries' hashes and logs are retained in the seal.

## Frozen final result

Runtime d4c8a2f passed the exact receipt fault regression and the full987 library
+2 CLI suite, with4 doctests ignored. Formatting passed; Clippy exited0 with
warnings. PR87's published94a352c evidence/ancestry alignment leaves all tested
runtime inputs unchanged. The [final report](../evidence/native-catalog-wire/final-d4c8a2f/REPORT.md)
records exact counts, commands, source/binary hashes, failures and remaining
activation limits. This is not an ordinary public contract2 readiness claim.
