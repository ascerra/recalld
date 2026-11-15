# Legacy disk lifecycle operations with original API context

Work in progress; no activation or completed-validation claim.

Actual parents: PR136 `047ebe086b01747c0dec38228141a3696d885d29` plus PR132
`5864c09050e532454c97ba8e73e00e7ba1670fb4`, joined at `fd73618`.
PR132 supplies the actual legacy observation writer/recorder original1|2 path.
Manual135 and compound catalog/purge branches are not dependencies.

The original application contract remains explicit1|2. The already-owned engine's
physical entry mode selects its existing native or legacy transaction, never the
application version. Ordinary native/public2 serving remains gated.

Old `feedback_native_v1`/`forget_native_v1` private aliases retain native-only
admission, before any mutation. New `feedback_lifecycle_v1`/`forget_lifecycle_v1`
aliases explicitly support both formats, require the original selector, do not
retry automatically and never fall back to old mutating methods. The private
Request carries the entry capability separately from its numeric application
origin. Existing prepared public2 parsing opts into the new capability; public
routing and API1 compatibility remain Huygens's ownership.

Legacy feedback/exposure preserve legacy anchor/history behavior. Feedback uses
the same shared planner and configured policy; its actual transaction checks
matching replay and CAS before sampling the injected clock. Legacy forget has
no canonical timestamp field to update and does not invent a clock event.
Legacy tombstone accepts validated1|2 contexts while retaining all physical
schema1, scope, terminal, effects and admission checks. Its unasserted request
hash is unchanged; a new explicit name assertion is hashed and checked under
the final transaction. No historical receipt is rewritten.

Private `legacyData` is additive, omitted for native replies, and contains only
receipt-supported identifiers, quality/application time or tombstone status,
plus the historical flag. Native data remains unchanged. Successful replies
require exactly one matching data view. Legacy historical strength/stability or
policy revision are not reconstructed from today's record. Malformed new replies
retain a matching decoded full receipt as committed failure when possible;
otherwise they report an unknown outcome. The shared envelope/public gates are
unchanged. Schema version is not application version.

Before correction, a frozen real binary reproduced three failures: prepared
legacy2 feedback and forget rejected at the native-only guard; direct canonical
legacy2 tombstone rejected its API1-only guard. The same before binary wrote real
API1 exposure, feedback and forget receipts, backed up into the tracked fixture.
This is before-correction writer output, not hand-authored receipt rows or a
claim to have executed an old daemon binary. The old-handler socket tests emulate
the old unknown-method behavior with real current framing/client code.

The new worker retains publication/graph leases through canonical completion
when its caller is cancelled. Known commits survive later publication/binding
failures; failures do not invoke a self-draining readiness barrier. Purged scope
uses retained terminal identity and validated purge-name proof, never a reused
name or connection default. Compound historical-context codecs are unchanged.
