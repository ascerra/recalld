# Native sweep physical-compaction consumer evidence

Validation complete; final publication review pending; no publication claim. Minimal actual parents are
PR145 `22760a2dd9522d35bd282a665286084366848fac` and PR135
`24da9206ba141493cdb9ca988188be6d5690a814`, joined at43c0822. No broader
checkpoint or private initializer parent. Pascal explicitly released consumer
ownership to Feynman; Pascal retains public API1 feedback/forget and A3 review.

The join preserves native versus legacy dispatch, explicit original contract,
receipt-aware legacy compaction, strict native outcome validation and both
parents' regression modules. The new runtime delta from the join is18 added
lines/two replaced comments in `src/decay/sweep/native.rs` at9cf9a44. After a
successful pass strips full text or deletes records, it calls the existing145
`DerivedCompletion::compact_text_with_work`. It reports the physical receipt and
cleanup status only on success; known/unknown failure remains in the existing
SweepResult failure fields. Any phase/publication failure skips fresh compaction.
No second scheduler, clock sample, codec, system initializer or ordinary gate.

| Source | Command selection | Actual result |
| --- | --- | --- |
| 441e746 | native_sweep_compaction | 0 passed/1 failed,0.90s: actual manual native phase work returned compaction_triggered=false |
| 9cf9a44 | native_sweep_compaction | 1 passed,2.38s: manual and autonomous passes, strict reopen and fixed physical contract1 hash |
| 9cf9a44 | native_sweep | 8 passed,7.63s, including original partial-publication receipt and stale graph/clock regressions |
| 9cf9a44 | manual_origin | 4 passed,2.62s, real native/legacy manual1 and autonomous2 producers |
| 9cf9a44 | compaction | 32 passed,25.18s, including145 current/fork/omission/cancellation/BEFORE compatibility |

Selections overlap; these counts are not summed as independent tests. The BEFORE
failure occurs on the first manual case; it is evidence of the skipped consumer,
not a corruption claim or a separate reproduced autonomous failure. GREEN tests
both producers across Full→Summary→Ghost→deletion. Phase mutation request hashes
retain their actual1/2 origin; each physical selection validates internal1 with
original sourceG. Summary→Ghost alone does not compact. Exactly one relocation
receipt is selected for each qualifying successful pass, and all four real
publication views agree with fresh canonicalG before success. The genuine145
historical fixture runs from its tracked ZIP with no local environment input.

Runner `run.py` freezes source and executable hashes, package-cleans only recalld
when switching source roots, and uses one private target. Offline Docker
Rust1.94.1 image sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef,
2CPU/4GiB/jobs2/testthreads2, no network/personal credentials/socket mounts.
The full command covers all-feature library/binary/integration/example tests,
then formatting and all-feature/all-target Clippy; warnings remain disclosed.

Ordinary startup/publicAPI2 stay gated. The helper's actual143+135 prefix evaluator
can later consume this validated physical-maintenance join. Global inherited
configuration participation remains a separately owned source audit, with its
real restart BEFORE pending. No full-store authenticity, power-loss or performance
claim. Once published with recorded results, central cleanup may delete the whole
raw run root under user retention override; keep only the one active target/shared
registry, source/Git, compact evidence and required tracked fixture inputs.

Pascal independently reviewed the native consumer hunks at9cf9a44 and reported
no blocker: original pass clock/origin preserved, accepted work reused, no fresh
compaction after partial/unknown phase failure, and existing receipt/error identity
retained. This is source-only A3 review, not an independent runtime validation.

## Final full result

Frozen runtime `9cf9a447d411f1937b645dfc4d5f7fb7b350b0c8`, tree
`f63f56dca3da44206d8189f1b03372e348e0f09a`:1200 all-feature library
tests passed in401.70s; two CLI tests passed in1.39s/0.74s. Binary/example
targets compiled; their zero-test targets are not additional test counts.
Formatting passed. All-feature/all-target Clippy exited0 with warnings, detailed
in full-validation.json. No doctest/performance claim. Source inputs matched the
immutable snapshot after completion; the actual target library executable matched
the pre-test copied executable after the full suite and Clippy. Zero OOM events.

No production changes followed this full run. The final seal is documentation
and compact evidence only. The full-run focused selections repeated the named
regressions with the same source; their exact results are separately recorded.
Pascal's source review remains distinct from this runtime validation.

All owned validation containers exited. The raw run root remains protected until
the resulting PR and recorded-results handoff; no raw HOME archive is needed.
Agent cleanup deleted0bytes. The prior PR145 root was deleted by the central
owner after publication; its observed concurrent free-space delta991076352bytes
is not exclusive attribution. One private target/shared registry remains active.
