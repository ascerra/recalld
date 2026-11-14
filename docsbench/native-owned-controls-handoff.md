# Native owned controls: frozen handoff after the V1 probes

Source-only integration notes, 2026-11-14. This does not enable native startup,
controls, public API2 or independent native genesis. No runtime was changed or
executed for this note. The separate PR119 probe seal is
`f5704e8a8c60ac97f9ab8072daa4a47016c7ad03`; it is not a dependency here.

## Ownership and exact inputs

- Feynman supplies native catalog/policy owned allocation on actual PR124
  `a5865da02969631a5031f65c6e43269701d7a254` plus published PR134
  `b62d1e2f4b3f516b5889946605946e274f82286c`. His allocator child is not yet
  validated/frozen; do not consume its working branch or duplicate its fix.
- This worker owns the transferred same-core native evaluator constructor,
  controls and checkpoint consumer. Feynman retains ordinary native admission/API1
  startup. His handoff explicitly permits a private existing-native-fixture ingress
  sharing Recalld construction before ordinary startup; coordinate the common
  constructor seam before implementation. The original
  plan is frozen at `cb0ba4f06ea4316888d1a34a6aed9a275c30ab3e`.
- The actual legacy supervisor/control/reset composition is frozen PR130
  `e571104a2fc551a29314f6361b9b67503acd1fdb`, including actual115/117/125/128.
  Its branch is under Halley's integrator ownership and must remain unchanged.
  Use its consumer when those concrete controls are needed; recompute ancestry
  against the future startup parent rather than importing an unrelated checkpoint.
- Native disk admission, original API1 support, public API2 activation and A2
  selection remain separate gates. Public API2 is not a prerequisite for a
  validated API1-native evaluator. No new native-sweep parent is presumed ready.

## Verified implementation seams

| Frozen source | Existing path | Required consumer change / gate |
| --- | --- | --- |
| PR134 `src/system.rs:267–288`; `src/storage/engine.rs:424–462` | `with_clock` opens the ordinary legacy engine; private `open_native` chooses native preflight and metadata mode. | PR134 proves original request context, not ordinary native serving. For bounded fixture work use a private existing-native-prefix entry, not an ordinary automatic selector. Reuse the single engine, supplied Clock and MetadataStore IdSource before recovery; do not open a second native session. |
| PR134 `src/storage/metadata/native_catalog.rs:84`; `native_policy.rs:62,143` | Three free receipt calls remain in this frozen parent. | Feynman's owned-allocation child supplies the fix/proof, including keyed numeric-policy no-change. No local duplicate fix. |
| PR130 `src/system.rs:274–298`; `src/bench/frozen.rs:21–41` | The evaluator injects its clock and seeded IDs into the one core; the current storage mode is explicitly legacy. | Share core initialization with a private native-fixture entry using actual `open_with_ids(path,true,ids)` from124. Require existing schema2/default namespace/profile and no legacy default bootstrap. A fixture flag must not rewrite API1 to API2 or imply ordinary serving. |
| PR130 `src/storage/engine/checkpoint.rs:54–132` | Closed capture checks actual readiness, copies selected canonical files, strictly opens the disposable stage, checks G, hashes and rechecks readiness. | Retain the same freeze and staging pipeline. Select the native hash only after strict schema2 admission. No readiness stamp repair or second snapshot implementation. |
| PR134 `src/storage/logical_export.rs:192–215,218–319,322–355,360–403` | The native typed walker and `LogicalHash` already define native logical fidelity. Export adds a random envelope ID. | Expose a non-writing, deadline-checked consumer of the existing walker/hash for the checkpoint path. Do not invoke export merely to obtain a hash, invent another walker, or remove differing logical fields. Physical text/vector offsets and text catalog handling already belong to this declared logical-hash contract. |
| PR130 `src/storage/logical_export.rs:402–417` | Legacy hash explicitly rejects schema other than1 and has a private legacy domain. | Preserve its frozen behavior. Native manifests must explicitly identify their native logical-hash contract; never silently run native state through the legacy domain. |
| PR130 `src/bench/checkpoint.rs:207–229,255–370` | Restore validates track/build/config/cursor, retains source/ancestor locks, validates source and clone, installs a fresh destination, restores sources, opens the real core and verifies G/hash/cursor before exposure. | Keep these checks and old-worker exclusion. Add explicit native track/schema eligibility and invoke that same native-fixture constructor; do not widen only the manifest string. |
| PR134 `src/storage/schema_migration.rs:346–359` | Migration ID and migration receipt still use ordinary allocation in this inspected source. | Initially use a genuinely frozen native prefix if no validated owned-genesis consumer is supplied. Same-prefix restore equality does not prove independent native initialization. Preserve the independent-genesis gate as unavailable, not normalized away. |

These are documented gates and integration requirements, not newly reproduced
runtime failures. The current allocator and startup parents are not assumed to
contain every reader, payload, sweep or public adapter needed by the later track.

## Minimal actual consumer sequence

1. Receive the exact validated allocator child on124+134. Inspect its final
   interfaces and ancestry; record necessary joins and mechanical resolutions
   before runtime edits. Keep the validated legacy controller behavior. Ordinary
   startup is a later independent Feynman parent, not a fixture prerequisite.
2. Share Recalld initialization with one private native-fixture entry opening
   actual `open_with_ids(path,true,ids)` before recovery. Require an existing
   schema2 prefix with the default namespace and matching profile so the legacy
   default-creation branch cannot execute. Preserve the legacy constructor.
   Start with a pinned native prefix if genesis is not owned.
   Check actual G/all four stamps/pending0 before actor admission. Public requests
   retain original API1; actor discovery/input never exposes evaluator controls.
3. Extend existing capture/restore with the native typed hash and an independently
   named native-prefix checkpoint track. The same executable must consume that
   track and reject wrong schema, unknown versions/config/build/cursor, pending
   work, live old owner, modified source and pre-existing destination. No unused
   hash/schema framework is a publishable slice.
4. Execute an actual public-action suffix and three fresh-owner restores. Retain
   source mappings, receipts, raw scores/order, provider calls and typed hash
   inputs on any difference. Add explicit native sweep only after its actual
   origin-aware owner API is validated; no phase simulation or second sweep.

Use focused constructor/hash/checkpoint and real serialized consumer checks,
then required full/fmt/Clippy against the exact joined runtime. Retain all failures
and immutable source/binary hashes. Existing legacy evidence is inherited only
for unchanged source; it does not prove native behavior. PR119 V1 representation
sensitivity remains a future actual-publisher parity requirement, not permission
to sort/change legacy behavior in this constructor work.
