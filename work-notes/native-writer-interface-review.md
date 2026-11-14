# A3 handoff to the shared canonical writer owner

Light source/spec review during the 2026-11-14 A5 sampling hold. No new build,
test, format, archive or copy jobs. Feynman owns additive catalog/config,
phase-loss/hard-delete, purge and batch proof implementation. This agent retains
reader/observation composition. Existing PR79 serialized insert/tombstone
variants and original schema publication evidence remain immutable.

The current `SweepEvaluation::project_locked` is a legacy adapter: it passes
`expected.last_accessed_at` to the pure projection. Native phase work must pass
`schema.lifecycle.decay_anchor_at` instead. Reuse `resolve_policy` and `project`,
freeze namespace policy/version/revision plus global inputs, and carry the
original single clock sample through final validation. Retrieved/compatibility
timestamps do not become v2 decay anchors.

Replay matches original normalized intent and saved expansion before current
policy, record tokens or clock checks. Captured generation/policy hashes are
verified preconditions, not caller identity. The final transaction rechecks the
actual namespace state, record tokens, retained phase and terminal eligibility.
`check_native_capture(..., false)` alone does not enforce a phase-transition
eligibility rule.

Keep graph-before-storage/publication lock order. Recompute effective projection
and threshold under the final guard using the same sample/policy, including
strength-only plans. Record tokens alone do not detect a graph change. Native
donors must be canonical eligible same-scope projections, not stale materialized
graph strengths; `connection_bonus_with_strengths` already accepts that map.

Normative A3 phase rules remain: at most one destructive step per sweep, inclusive
thresholds, nonempty retained summary before Full-to-Summary, and a diagnostic
skip if that summary is absent. Summary-to-Ghost strips summary/content tags and
structured labels, without automatically discarding retained embedding ownership.
Ghost hard deletion needs the exact ledger/vector/edge transaction and proof
witness. Permastore and zero multiplier skip automatic transitions; forget still
overrides exemptions. Clamp changed lifecycle/phase timestamps; do not invent
retrieval, reinforcement or anchor updates, or restore lost content.

A numeric namespace policy transition must update the namespace and every
canonical record's policy revision/token together, including Ghost/Tombstone
rows omitted from the live namespace index. Preserve record anchors, timestamps,
history, stability, content, retained phase and permastore. Bind exact affected
tokens/digest and typed before/after policy; do not hide profile, identity, schema
or lifecycle-version changes inside a generic config update. Reuse the existing
explicit lifecycle-version migration. Invalid resolver settings must fail before
effects; policy preview remains read-only.

For hard deletion, the proposed `NativeDeletionWitness` contains guarded actual
`NativeRecordState` plus bounded numeric `AccessEvent` history, checked against
its history hash. A hash cannot reconstruct absent history. The typed deletion
outcome must bind namespace/UUID, terminal kind, removal masks and same-origin
suffix order. Only a validated witness may seed proof-only reverse state;
historical receipts cannot recreate payloads or authorize foreign-counter
comparisons. Detailed codec/proof review remains with the shared owner before
implementation.
