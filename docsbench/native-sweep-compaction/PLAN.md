# Actual native sweep compaction consumer plan

Approved implementation plan. Published #149
`25dd555db4d23253714a656dcd804210f937cb02` is the direct base; the only additional
parent is published #147 `50da71abee66cea159988a203d286f570f091718`, which already
contains actual #145 `22760a2` and #135 `24da9206`. Do not stack those parents
again. #149 remains READY/FROZEN under Halley's ownership. Actual join `b3be1b9` had one mechanical exact-scan test-module conflict, retained in `join-conflicts.patch`; both test groups remain.

## Actual seam and ownership

#147 `src/decay/sweep/native.rs` invokes the existing
`DerivedCompletion::compact_text_with_work` after a successful pass with Full→
Summary or deletion commits. Summary→Ghost alone does not compact. It exposes
`compaction_triggered`, `compaction_receipt`, `compaction_cleanup_pending` in
`SweepResult`; failures retain existing known/unknown identity fields. Its
physical selection uses internal contract1, independently of manual lifecycle1
or autonomous lifecycle2. `src/storage/engine/text_compaction.rs:73` allocates
the physical generation through the owner's actual ID source. No second sweep,
compaction operation, clock, phase formula, hash walker or restore implementation.

Feynman retains shared initializer/config-adoption audit ownership. This consumer
does not edit `system.rs`, ordinary startup or global-config admission. Changes
should be limited to the existing private control/checkpoint profile, actual
runner/fixture and tests. Any required owner seam change is sent back first.

## Explicit compatibility boundary

| Surface | New actual consumer | Prior recorded profile |
| --- | --- | --- |
| Diagnostic track | Advance the existing sweep consumer to `serialized-native-sweep-synthetic-diagnostic-v2` | Existing v1 manifests remain pinned to #149; new consumer rejects v1 before output/provider/store side effects |
| Checkpoint track | `isolated-native-sweep-checkpoint-v2` | A v1 sweep checkpoint rejects before clone/destination/provider admission; no reinterpretation or upgrade |
| Capabilities | `nativePhysicalCompaction=true`; actual receipt and cleanup status retained | Recorded false does not become true; unsupported old requests fail closed |
| Other version domains | manifest/sourceMapping/trace/checkpoint/control versions stay1; public contract1, canonical schema2, manual origin1 | No public API2 or full-A6 activation implied by diagnostic v2 |
| Existing non-sweep tracks | Native-prefix false-only and legacy sweep keep their declared contracts and receive same-source regressions | Their historical evidence stays on frozen source/binary/manifest pins |

This is a versioned extension of the existing real runner over validated #147,
not an unused schema or speculative client. The new binary will not offer an
emulated no-compaction sweep to preserve v1: old evidence remains executable on
its frozen binary. Runtime capabilities, manifest preflight and checkpoint
restore must agree on the admitted profile. Test both a real old manifest and
an otherwise valid same-binary checkpoint with its track changed to old v1, so
rejection is not attributed only to an executable hash mismatch.

## Nonempty physical proof and exact replay

Keep four real acknowledged migrated synthetic records. Extend the prefix
builder's explicitly versioned fixture input to create the foreign namespace
with its real numeric decay multiplier override0 before migration. The three
default-namespace records use the existing multiplier1 schedule and phase
transitions; the foreign full-text record survives each pass. This gives actual
nonempty text relocation and namespace/payload checks rather than only empty
generation selections. Record the namespace policy in fixture/provenance and
manifest capabilities; no direct phase mutation or simulated decay formula.
The old builder fixture version1 remains unchanged for existing-track runs.

After each actual sweep, retain the complete physical receipt/cleanup fields
before assertions. For the three changing default records, expected increments
are3 phase commits plus1 physical selection at Summary/deletion; Ghost has3
phase commits and no physical selection. Verify the receipt's actual generation,
source generation, native relocation outcome and selected text identity, using
its typed fields rather than a private dump exposed to the actor. Confirm all
four derived generations equal canonicalG and pending work0. Foreign false get/
search must retain the actual payload/namespace across relocation and restart.

Use the same owned constructor, clock and canonical ID state, same typed native
hash, same false-only actor, and three fresh-owner suffix restores. Compare exact
phase/physical receipts, canonical hashes, outputs/order/exposed scores, provider
requests/vectors and runtime state. Retain label/future canaries, offline misses,
old worker/source/destination guards, omitted/true access rejection and sparse
scorer labels. No receipt/hash/ID/float normalization; preserve any real mismatch
and coordinate its owning seam. Independent native genesis remains a separate
gate. Physical compaction does not imply secure erasure of retained receipts or
checkpoints, full-store authenticity, power-loss coverage or performance.

## Validation and resource plan

Freeze the actual #149+#147 composition, record all conflicts, then focused
origin/compaction/control tests → real v2 consumer and compatibility negatives →
appropriate same-source full checks and existing-track regressions. Reuse the
one private `/tmp/recalld-owned-native-controls-validation/{cargo,target}` cache;
new uniquely named immutable source/output roots only. Offline pinned Docker,
2CPU/4GiB/-j1, source and executed-binary hashes, no personal stores/config/sockets,
datasets or paid calls. Completed PR149 raw roots have been centrally deleted;
never recreate them. Preserve code/Git/reports and clean new disposable runs
after publication/results. Broader A6 scorer/grouping/token/trace and public
native-writer implementation remain owned work outside this bounded PR.
