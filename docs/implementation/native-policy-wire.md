# Native numeric policy wire and catalog publication

This consumer joins PR93 (catalog wire), PR94 (numeric policy receipts), and
PR96 (the actual owning derived-publication coordinator). PR100/list paging,
batches, purge, and unrelated integration branches are not parents.

The private contract2 MCP operation `update_namespace_policy` and REST
`PUT /namespaces/{namespaceId}/policy` accept a complete `policy` object:

```json
{
  "policy": {
    "initialStability": 1.0,
    "defaultDifficulty": 5.0,
    "phaseThresholds": {
      "fullToSummary": 0.8,
      "summaryToGhost": 0.3,
      "ghostToDelete": 0.1
    },
    "permastoreThreshold": 100.0,
    "desiredRetention": 0.9,
    "decayRateMultiplier": null
  }
}
```

MCP also requires `contractVersion: 2` and an integer `namespaceId`. REST takes
the immutable nonzero u32 ID from the path; a body cannot shadow it. Neither
transport accepts a mutable namespace name/default as the policy target. All
six fields are required. Unknown fields, missing fields, numeric nulls,
nonfinite/overflow/underflow values, and invalid ordered thresholds reject.
`decayRateMultiplier: null` explicitly selects the owner's inherited multiplier.
Retention remains bounded inclusively in [0,1]; this does not switch the
namespace's lifecycle version or weaken explicit lifecycle policy validation.
Profile binding, dimension, name, lifecycle version, and creation time cannot
be edited through this operation.

Shared `storeId`, `idempotencyKey`, and decimal-string `expectedGeneration`
validation is reused. An HTTP key header must agree with a body key. A1 hashes
the original resolved immutable ID and complete policy; this consumer does not
introduce a second hash or include a sampled clock/provider. Retained replay is
looked up before reading fresh namespace configuration or checking CAS. A
matching replay returns its original numeric policy/outcome and receipt even
when the current policy differs. A keyed exact no-op has the retained no-change
receipt and advances canonical G once. An unkeyed exact no-op has no receipt
and does not advance G. IDs never rebind to a reused name.

Catalog creation and policy mutation now use the same `DerivedCompletion`
instance handles as the engine's real cache, graph, vectors, FTS, and entities.
The accepted root spans the blocking canonical worker and actual publication.
A worker-owned shared publication lease survives cancellation and is released
before the coordinator takes it exclusively. Every receipt, including keyed
no-op and historical replay, goes through `complete_with_work(Some(root))`,
which compares actual current G and all four derived stamps. The consumer never
copies a stamp or substitutes a receipt's historical G for current publication.
The caller's original full receipt survives a known postcommit completion error,
even if the coordinator error contains some other pending operation's receipt.
Unknown worker/commit outcomes stay unknown. Cancellation cannot finish its
root by assumption; a detached executing child stays tracked until it returns.
Accepted descendants can finish after admission closes.

A retired namespace without a canonical name is unavailable with its original
receipt retained when known. Historical-name publication awaits the actual
validated purge witness; no invented name or altered receipt is substituted.

Public application contract2 remains rejected before work. No operation is
advertised in public tool discovery, and older remote registries return
unavailable rather than fall back to v1. Existing v1 methods and result shapes
are unchanged. Inner MCP/REST tests exercise the real native owner, not public2
activation. Full query payload coherence, native store/batch/feedback/forget/
purge wire, remote native RPC, complete input/output schemas, and whole-application
conformance remain prerequisites for activation. This change makes no
whole-native-ready claim.

Final source validation: [1864dab evidence](../evidence/native-policy-wire/final-1864dab/REPORT.md). All-feature1063lib+2CLI, focused8+exactcatalog1, fmt and Clippy passed; preserved fixture failures are reported separately.
