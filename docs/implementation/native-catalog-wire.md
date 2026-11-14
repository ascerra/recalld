# Native namespace-create wire consumer: active child plan

Minimal parents are PR88 `3961f66` and PR87 `b52801d`, joined at `8afffdb`.
The merge preserves both legacy LifecycleMaterialized and native catalog proof
variants, plus maintenance and native-catalog engine modules. No batch or
coherent-query WIP is included.

The next consumer keeps the existing MCP create_namespace and HTTP namespace
creation paths. Strict v2 input validation will reject unknown fields, explicit
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

Before wiring committed failure envelopes, preserve the full receipt already
available at the publication boundary. The current catalog publisher can return
CommittedEffectsPending with only operationId after marking the owner unavailable;
looking it up afterward is not a reliable replacement for carrying that receipt.
Commit-unknown remains distinct, with reconciliation required. A canonical
namespace receipt alone is not proof of all derived views being ready. No
synthetic readiness object or fresh control-counter ReadView will be added.

Tests must exercise actual native creation through both inner transport handlers,
cross-transport matching replay with stale CAS, changed-payload conflict, foreign
lineage, original-selector identity, fixed-default equivalence, no provider call
on replay, invalid-input byte immutability, typed commit/publish failures and
strict reopen. Public contract2 remains unconditionally rejected. A new provider
configuration or caller descriptor must never silently rebind a namespace.

Final native promotion additionally requires Pascal's validated minimal PR82
startup preflight repair-copy-only correction, discovered by Curie's real
process-exit test. Its moving WIP is not consumed here. The correction does not
require batch ancestry.

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

Feynman now owns the two-site full receipt correction in PR87; this child has not
edited either publisher site. Await its frozen validation plus Pascal's startup
correction before final full validation. Catalog success currently omits shared
readiness explicitly and carries a warning; it must not be presented as whole
writer/serving readiness. Actual committed/uncertain transport fault checks and
strict operation schema completion remain outstanding in this child.

The subsequent focused catalog run passed27 tests in28.57s, including existing
real process-crash recovery cases; the unchanged v1 namespace-option parity
regression passed1 test in0.36s. These are bounded checks, not the final full
suite. [Initial evidence](../evidence/native-catalog-wire/initial/manifest.json)
retains28 artifacts with source/binary hashes and explicitly identified capture
timing. No before/after source-root switching occurred within these runs.
