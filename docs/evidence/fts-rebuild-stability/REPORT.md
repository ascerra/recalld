# Same-generation FTS rebuild stability

This is a minimal correctness prerequisite on actual #96
`387c7118864040c68d7d28c44fb2600ade520a45`. It does not contain the incremental
publication candidate or change canonical receipts, ranking weights, tokenizer,
eligibility, score/UUID ordering, or readiness gates. Runtime source is
`7311775604539a386bdeaa9e1e881cf8eeeff9de`.

## Independent baseline reproduction

Production source remained unchanged for both RED freezes; only tests and a
`cfg(test)` statistics reader were added. Tests-only commit `495b8fb` preserves
the second freeze. Both used bundled SQLite **3.49.1**, not Tantivy.

| Run | Source manifest SHA256 | Test binary SHA256 | Result |
| --- | --- | --- | --- |
| red-1 | `8075219ffcc356f8eac3a76b8b57f7e9e549e16ad72f24f2901a7fde8b55f7d0` | `a53b7b3e1ca6c510239619d53dfc5531f7765ee9c79ff7d534d5869ba33c11b1` | 3 existing tests passed, 2 exact-score regressions failed |
| red-2 | `504f92d457af05dfe9ac19c6fc1080e6a432fec2f36f739d41d16bc477bd3e39` | `ca35508fa7dd7ee3d14afa13422775fc59e4986208633545d80b6f3571aa371e` | 1 exact-order regression failed |

The actual `DerivedCompletion::rebuild_all` fixture holds canonical generation 4
constant through four rebuilds. Every rebuild reports all views ready at that
same G. Live FTS rows, ID-map rows, and document-size rows remain 2. Vocabulary
statistics remain `alpha: 2 documents/3 occurrences`, `beta: 1/1`, `gamma: 1/1`.
The averages record (`fts_content_data` id=1) instead changes:

| Rebuild | Averages record hex | Aggregate document count | Leading score |
| --- | --- | ---: | ---: |
| 0 | `02050000` | 2 | 0.000003657354 |
| 1 | `040A0000` | 4 | 1.4608604 |
| 2 | `060F0000` | 6 | 3.3764617 |
| 3 | `08140000` | 8 | 4.622103 |

Those small averages-record varints encode document count and per-column token
totals. We record aggregate/live counts and vocabulary, not an independently
decoded tombstone count. The two-document fixture preserves order despite score
drift. A separate three-document fixture proves an order change: query
`alpha beta` returns IDs `[1,2,3]` with scores
`[0.316568,0.0000021103117,0.0000020323325]`; one clear/rehydrate returns
`[2,3,1]` with scores `[1.2404131,1.1945779,0.80518943]`. Content and term
frequencies are identical. Exact assertions are retained, without normalization
or tolerance. This invalidates general rebuild score/rank identity on this parent;
it does not invalidate unrelated source-specific tests or establish a reset bug.

## Mechanism and correction

The table is `content='', contentless_delete=1`. In the actual bundled
`libsqlite3-sys-0.33.0/sqlite3/sqlite3.c`,
`sqlite3Fts5StorageDelete` calls `fts5StorageContentlessDelete`; that path records
a tombstone and removes the document-size row but does not decrement aggregate
row/token totals. `fts5Bm25GetData` consumes those totals. Ordinary row deletion
is therefore not a complete corpus reset for this rebuild path. This is not an
external-content deletion-order or missing-original-text attribution.

The correction uses `INSERT INTO fts_content(fts_content) VALUES('delete-all')`
in the existing transaction before clearing `id_map`. SQLite's
[delete-all documentation](https://sqlite.org/fts5.html#the_delete_all_command)
explicitly supports contentless-delete tables. The bundled
`sqlite3Fts5StorageDeleteAll` invalidates cached totals, clears index/document-size
state and reinitializes the averages record. The source SHA256 is
`ff80c36ef1bb44eb357c7ff1d15be77540d41c28fb671088215a6cd12785c5d3`.
See also SQLite's [contentless-delete tombstones](https://sqlite.org/fts5.html#the_deletemerge_configuration_option)
and [averages record](https://sqlite.org/fts5.html#averages_record_format).

The bounded fix covers full clear/rehydrate. Ordinary per-document deletion,
upsert and namespace-removal statistics are not repaired by this patch and need
separate assessment if incremental callers consume those paths. No canonical
migration, dependency upgrade, or alternate scorer is introduced.

## Validation and evidence handling

Each freeze is an allowlisted immutable source snapshot, with pre/post hashes,
private cargo/target, package clean before source switching, Rust 1.94.1,
`recalld-ci:1.94`, two build jobs, `--cpus 2 --memory 4g --network none`.
No home, credentials or Docker socket is mounted. Both failed runs are retained.
The corrected source manifest is
`9d485b27d34784e97008839f9439182d944318741f125913a6dce479ae5ea214`.
**6 focused tests, 1003 library tests and 2 CLI tests passed**. The full library
suite ran normal parallel execution in 437.68 seconds. Formatting passed; clippy
exited 0 with warnings (summaries in MANIFEST), not a warnings-clean claim. The
focused count includes 3 new regressions and 3 existing tests. Doctests were not
run. Source snapshots and final working source were verified after execution;
per-source binary and log hashes are in MANIFEST. No performance measurements
or promotion claims.

Raw evidence is retained outside Git at
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-fts-rebuild-stability-7311775`.
`COPY-VERIFICATION.json` records all copied source/log/report members and hashes.
Both RED runs remain separate from the final green proof. Only source, report
and text manifests are committed; container inspect records remain private.
