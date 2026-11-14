# Original API context consumer status

Actual base: PR121 `3fa91d4`. The reviewed plan and bounded four-candidate
compatibility refinement are in native-request-context-plan.md. This branch
changes compound retained context and actual purge/catalog/policy consumers only.
MutationContext::validate and native_records/native_lifecycle/native_maintenance
remain unchanged. Standalone owners proceed on their actual parents; compound
batch later consumes this codec. No ordinary startup or public API gate opens.

## Frozen evidence and followups

- Runtime d933e5f, topology-doc head cb6a452: eight focused PASS12.85s with genuine
  old2 force/CAS source/fork artifacts and old binary proof rejection.
- Deliberately unanchored new API1 candidate f79614e: one stripping test FAIL as
  intended1.37s. This is a proposed-candidate defect, not a published API1 exploit.
- Frozen cb6a452 full:1100PASS/1FAIL516.61s. Existing unknown-child test failed;
  CLI and Clippy did not run after cargo test exited101. Log SHA256
  764391cb73717f3d932325571f61e3ecce3ad672d25abb27f275c79a01c23e14,
  test executable SHA256
  a6a1f06bf4d6576875277ca39764e694856492fe8abdc47eecd1f9b609d2972b.
- Test-only574850d exposed the actual error: ordinary control_state rejected
  pending purge before retry, with RecoveryRequired("namespace purge pending;
  resume canonical deletion before admission"). Its exact assertion remains.
- Runtime a4acd5f replaces that ordinary read with a narrow immutable owner-mode
  check under the existing storage write guard. The shared purge transaction
  still verifies current control schema under its mutation guard; ordinary reads
  remain blocked. Existing unknown-child test PASS1.57s, exact reserved child
  CommitOutcomeUnknown classification and reopen preserved.
- The same a4acd5f binary then reproduced the separate known-receipt gap:
  API1 replay of damaged scope progress returned bare InvalidControl after its
  canonical receipt was found. New regression FAIL1.10s. The proposed correction
  wraps load/start-validation errors with that already-in-hand full replay
  receipt. It does not reconstruct or reread a receipt. Lookup failures before
  a receipt is known retain their original error class.

The coordinating agent reviewed d933e5f source and requested the narrow receipt
correction. The final regression covers both originating contracts, damaged
start scope, missing progress, and missing receipt before lookup, through the
actual accepted coordinator. It asserts exact receipt equality, retained raw
receipt bytes, unchanged generation/metadata, and no invented receipt for an
missing canonical receipt. A separate test preserves legacy-owner
rejection for both contracts.

At the performance DRAIN instruction, the already-running a4acd5f probe finished
normally and all own heavy jobs drained. Wrapping correction/test additions are
source-only and **not validated yet**. No subsequent formatter, build, test,
container or large artifact copy is authorized before explicit END. Preserve
receipt-red source pointer, target and log until its binary hash is recorded;
then run focused checks followed by one full corrected fmt/test/all-target
Clippy run. Do not advertise a final green or publish this unfinished slice.

## Remaining actual consumers

Huygens owns public catalog/policy/purge routing. Curie owns compound batch codec
consumption and independent standalone record propagation. Pascal owns independent
lifecycle/exposure/manual propagation. Ordinary startup remains Feynman's later
actual consumer integration; no second engine or forced API2 context is allowed.

Halley's frozen controls inventory35aa7a identifies an additional real seam:
124's owned ID allocator must replace free control::record_receipt calls in
native catalog/policy on the actual shared-context join, with cursor/create/
update/keyed-nochange/replay proofs. He owns that adaptation. It is not present
in this121 child. Published PR125 `0b50168b64ad353e4f7a53c6459c270fe8500070` and
PR128 `5deb840a21ad5f1f80006e0d05a6372919292acf` must enter the later recomputed startup
closure through actual consumers, not a blind ancestry merge.
