# Exact151+166 chronology composition

Normal merge `d2c705d49154c8b9284d7564f4366d2f2e3ba23a` joins frozen151
`2b2560516e1e8e4548c9511f296a72862110caa9` into the source-plan child of166
`3f3ca05d160cad917b716758dd360412a652e9d9`. No other parent or cherry-pick.

Eleven files had conflicts, resolved per hunk:

- API search trait/adapter: add151 query/similar methods; retain166 lifecycle
  methods and native_lifecycle module. No public routing activation.
- Daemon storage/MCP bridge/adapter: preserve166 API1 capability negotiation,
  nonretrying lifecycle routes and typed errors beside151 inherited interfaces.
  Keep the existing one-connection lifecycle test helper.
- Prepared MCP dispatcher: preserve166 feedback/forget handling, then151
  recall/similar dispatch; neither shadows the other.
- Pipeline: retain166 performance::measure around151's response/selection tuple;
  Observed.finish still checks effects before finishing accepted work.
- Observation: retain166 worker/lock/transaction/publication timing around151's
  record_locked_with_scopes helper; scopes, original contract, retained partial
  receipts and child completion remain the existing actual implementation.
- Canonical control: preserve166 typed variants/matches; incoming conflicts only
  reorder existing variants. Final control.rs is byte-identical166/159.

The nonconflicting read-view union retains CanonicalReadView native profile
forwarding, pinned native_query_admission/shared clock/policy and RejectExposure.
query_wire.rs remains byte-identical151. No runtime changes follow the merge;
chronology implementation changes only test modules/helpers.

Huygens source-reviewed the committed merge and the named query/observation/API
seams: no blocker, no edits, no execution claim. Feynman also reviewed inherited/native/publication/system no-diff and the pinned
lock/admission/clock/policy union: no blocker, no runtime or full151-audit claim. Validation results and any real
composition failures will be recorded separately; source review is not a test.

Frozen0101 first validation: all four new chronology cases passed. Existing strict
query cases passed12/failed2; the unchanged-source similar_v2 diagnostic passed2/
failed7 with explicit inherited-policy-unknown errors. Remaining focused public,
adoption, legacy, decoder and historical replay groups passed; full was gated off.
Source354/emitted executables15 were hash-verified before the test-only correction.
Huygens reviewed the exact fixture helper: actually adopt default+other after real
migration, preserve mismatch/unknown complete namespace bytes and unknown binding,
and retain pre-adoption rejection assertions. No production guard is relaxed.
