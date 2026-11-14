# Native numeric policy replacement

Implementation in progress. Minimal parent PR87 at 94a352c, including corrected
PR82 startup recovery. No graph, batch, maintenance or broad-checkpoint ancestry.

Reuse the complete `NamespacePolicy` replacement input. Identity, dimensions,
immutable profile and lifecycle version are not editable. The original normalized
numeric replacement identifies retry intent; captured policy identity and actual
GenerationRef are preconditions. Replay precedes fresh policy resolution and CAS.
There is no provider or clock sample.

`capture_native_policy(scope, replacement, policy, parent)` returns an intent
using the existing CapturedLifecycle policy identity with no selected record
subset. `apply_native_policy(ctx, intent, policy, parent)` returns an optional
receipt; None means an unkeyed internal exact no-op. A supplied key binds a
NativeNamespacePolicyUnchanged receipt and advances canonical generation once,
without changing policy revision or any record token. Replay after a later
policy change returns that original no-change outcome/generation; changed
payload under the same key conflicts. This canonical generation also invalidates
derived publication stamps even though no effects epoch changes. Final execution holds the actual native guard,
validates the captured policy, and enumerates every canonical META row including
Ghost and Tombstone. One transaction replaces numeric namespace policy, increments
policy revision and each row token, and persists the typed receipt. All payloads,
anchors, lifecycle timestamps, phase, history and provenance remain unchanged.
No physical file operation or payload-loss ledger entry is needed. A canonical
receipt does not certify derived publication or activate ordinary serving.

NativeNamespacePolicyUpdated retains namespace/lifecycle identity, predecessor
and next revisions, capture/predecessor generations, before/after numeric policy,
record count and the sorted post-token digest. Strict history admits same-version
policy revisions alongside existing explicit lifecycle-version migrations. Numeric
history is rewound before matching native namespace creation; foreign-origin
history remains ancestry-bounded. Active reverse validation also rewinds numeric
configuration before older phase/write proofs. Existing proof bytes are unchanged.

Required checks: exact no-op; changed-input conflict; replay before stale policy,
CAS and inherited-input failure; complete terminal-row token participation with
byte-preserved payload/lifecycle state; stale captured policy; actual metadata
commit interruption; creation and lifecycle migration interleaving; strict reopen,
export/import/fork with original receipts; malformed token/policy proof rejection
without source changes. Offline bounded Docker, source-specific private target.

Feynman owns this slice and its numeric-policy dispatcher arms. Pascal owns phase
and hard-delete arms, Hume graph, Curie batch references. Purge remains a later
consumer of Pascal's validated deletion witness. This document records the plan,
not completed implementation or validation.
