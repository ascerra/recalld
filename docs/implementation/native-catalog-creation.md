# Native namespace creation on the existing catalog publisher

This slice adds receipt-backed native namespace creation to the existing owning engine. Its minimal shipping parent is PR82 with Pascal's phase-clock correction `a69865b`; it has no broad checkpoint, batch or new backend dependency. New namespaces retain `legacy_access_v1` policy revision zero. Ordinary schema-v2 serving remains gated.

## Actual API and boundaries

`NativeLifecycleSession::create_namespace(&MutationContext, &NativeNamespaceInput, &dyn EmbeddingProvider)` requires contract2 and catalog scope. The input contains name, optional dimension/profile ID selectors, and optional initial stability, desired retention and decay multiplier. The supplied provider must report a valid profile descriptor with matching dimension. This is configuration identity, not proof of remote model weights. Callers cannot provide a descriptor as an attestation of the active provider.

Historical replay and lineage/CAS checks occur before sampling provider identity or the clock. Fixed numeric defaults are expanded exactly once in the request hash. Omission and explicit default initial stability/desired retention are equivalent. Original profile/dimension selectors remain distinct; inherited decay multiplier omission is distinct from an explicit value. Frozen inherited policy is resolved before ID reservation, canonical writes or filesystem staging. A retained receipt replays even if current provider/policy inputs have changed.

Creation holds the actual native publication/work guard across filesystem preparation, the canonical transaction and vector-handle installation. Namespace configuration, highwater allocation, generation and `NativeNamespaceCreated` proof/receipt commit in one metadata transaction. Vector file publication uses the existing staged sync/no-replace directory publisher. It is not atomic with that transaction. Failure after payload publication poisons the owner; reopen reconciles orphan files or committed receipts. Existing commit-unknown and committed-effects-pending error contracts remain distinct from success or whole-store derived readiness.

The typed creation proof retains the resolved descriptor, numeric policy, namespace ID/name/time, previous allocation highwater and previous generation. Strict history validation binds it to its receipt scope/origin, current namespace configuration and reverse commit order. A native binding without a creation proof fails closed. Legacy layouts, profiles and provenance are not reinterpreted.

## Review corrections and tests

The source-review regression at `498f0ed` reproduced two real failures: omitted versus explicit fixed defaults produced `IdempotencyConflict`; invalid inherited policy allowed creation. `689c57a` fixes both. The same six focused tests then passed, with an asserting provider and counting clock proving replay avoids fresh inputs. Invalid inherited multiplier/global settings leave the complete synthetic store bytes and control state unchanged and create no namespace directory. An explicit valid namespace multiplier overrides an invalid inherited multiplier; historical replay still succeeds after later policy invalidation.

The final test extends actual native namespace creation and record insertion through strict reopen, logical export/import and production fork restore. It verifies immutable source bytes, preserved namespace descriptor/configuration, exact historical creation receipt, forked store ID and rejection of old-lineage replay. Additional tests cover six interrupted publication boundaries, invalid selectors, changed-payload conflicts, and forged configuration rejection without source changes. The release run also includes Pascal's genuine old-receipt and malformed phase-rule coverage through the actual parent.

Exact final counts, runtime/tree and source/binary/log hashes are in the evidence README and manifest. Earlier compiler failures remain recorded, including the test-only attempt to access private metadata internals. The corrected test uses the owning offline snapshot and adds no database escape.

## Still required

Numeric namespace policy updates are next: same-transaction complete canonical row enumeration, policy/revision and record-token proof while retaining timestamps, anchors, payloads, phases, terminal identities and immutable binding. Native maintenance/phase loss and purge remain separate required writers with ledger/effects completion. Curie owns native batch codec, child binding and the ordered runner; Feynman reviews strict offline/history integration. Their validated bookkeeping references enter the shared proof dispatcher, not an outcome allowlist. Halley owns shared publication/read-view/barrier work; Huygens owns the real transport adapter. This creation slice does not claim full native writer participation or ordinary serving readiness.
