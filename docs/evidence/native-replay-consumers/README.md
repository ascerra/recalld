# Actual retained-binding replay consumer and native purge barrier evidence

Final validated source `75bc809d048c4762ef56ae36d08d2395b482870b`, tree `cd5fabb4f54754ca5fe8d730ea9504eea96afd6d`. Primary actual PR114,
additional actual PR110 and PR111 plus its shared binding child. No PR103 wire
or broad checkpoint parent. Historical hashes/receipts and ordinary serving gates
are unchanged. The final full suite was rerun because the current reader changed.

## Final results

- warning: `recalld` (lib test) generated 7 warnings
- test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1092 filtered out; finished in 1.58s
- warning: `recalld` (lib test) generated 7 warnings
- test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1092 filtered out; finished in 1.66s
- warning: `recalld` (lib test) generated 7 warnings
- test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1092 filtered out; finished in 9.29s
- warning: `recalld` (lib test) generated 7 warnings
- test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1091 filtered out; finished in 2.74s
- warning: `recalld` (lib test) generated 7 warnings
- warning: `recalld` (lib) generated 35 warnings (4 duplicates) (run `cargo fix --lib -p recalld` to apply 2 suggestions)
- test result: ok. 1093 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 529.04s
- test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
- test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
- test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
- test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.59s
- test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.16s
- test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
- warning: `recalld` (lib) generated 860 warnings (run `cargo clippy --fix --lib -p recalld` to apply 93 suggestions)
- warning: `recalld` (lib test) generated 919 warnings (831 duplicates) (run `cargo clippy --fix --lib -p recalld --tests` to apply 6 suggestions)

Five focused checks are additional executions of tests also in the full suite.
Formatting and all-target Clippy completed with exit0; warnings are retained.
Exact command, environment, source-file hashes and executable hashes are in
final-manifest.json. No Rust repeat for documentation-only sealing.

## Earlier sources, kept separately

-77c2d5d:0/2fail before replay, exact native purge chunk missing from joined guard.
-21f1136:0/2actual binding RED after completed purge, feedback hash unavailable and
forget original-name assertion rejected despite its matching receipt.
-611083c:2/2GREEN through purge, recreated name/newID, and reopen.
-b6011a2:16focusedPASS, then1090PASS/2FAIL full547.46s. Inherited catalog/purge
fixtures expected older publication error evidence; both strengthened to exact
canonical receipt equality, not runtime downgrades. User supplied only first
catalog hunk from checkpoint2; patch and provenance are retained.
-b563b35:1092library+2CLI/4focused/format/ClippyPASS with warnings.
-17c9d52:actual positive four-view barrier RED after successful purge repair,
InvalidControl invalid current purge identity. Native version2 was unsupported
in the owned current reader.75bc809 fixes this closed shape/schema boundary with
strict original-lineage receipt point checks, no history scan or gate activation.

The real DerivedCompletion consumers and actual native purge/catalog/reopen/
export/import/fork are exercised. Matching replay uses deliberately invalid fresh
policy inputs, no new clock samples, unchanged current G/bytes, and original
receipt/hash/stale-CAS identity. Wrong name retains known receipt; changed payload
conflicts; foreign lineage rejects. Malformed evidence injection is synthetic
and rejection leaves those files unchanged. Missing profile provenance is never
manufactured. Restored completed progress uses original store-ID receipt keys.

All four registry views reject while publication is broken and report ready only
after repair at unchanged canonical G; this does not claim FTS scoring correctness
or ordinary native startup closure. Unknown commit handling remains unchanged.

Every run retains its own source/log/binary hashes. Own target package-cleaned
before first build, no source-root switching or personal/network/provider inputs.
Full raw log SHA256 `898bcddaeb72fcc0745fab079735e42f1a0ef9091163a9151daebc41f877f601`; final manifest SHA256
`06e281d7e53f8b6aebb68270e92ceff5fe65338c46ffc945f6c4b4024204948c`. Main/Halley75bc809 reviews are source-only evidence.
