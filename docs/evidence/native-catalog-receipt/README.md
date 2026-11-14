# Catalog publication receipt correction

Validated runtime `e2bd16b7f6e3ca832393598830b0b42ed13dbe00`, tree `ec0062a8c9007f37803b9f200fefd1486360c112`, child of published PR87 `b52801d`. No new source dependency or batch parent. Original creation evidence remains unchanged in ../native-catalog-creation.

Actual BEFORE `4aa419d` failed the exact-receipt test at `after_metadata_commit`: the error carried only an operation ID, while the poisoned owner blocked readback. Source, executable and log hashes are retained. The runtime fix changes four lines across two existing publisher failure sites, returning `CommittedMutation` with a clone of the receipt already held. This also supplies the fuller error on shared legacy catalog paths. Unknown commit acknowledgment remains `CommitOutcomeUnknown`; pre-commit failures carry no fabricated receipt.

The seven-point native publication matrix includes three definite post-commit failures and an ambiguous acknowledgment. It asserts the poisoned owner cannot look up the receipt, then compares the carried receipt exactly to stored lookup and keyed replay after reopen. Historical replay uses an asserting provider to ensure no fresh provider call. Metadata generation and namespace allocation are not repeated.

Exact final command: `cargo fmt --all -- --check && cargo test --offline --lib catalog && cargo test --offline && cargo clippy --offline --all-targets`. **21 catalog-focused +929 library +2 CLI tests passed**,4doctests ignored. Formatting passed; all-target Clippy exited0 WITH warnings, recorded in final-manifest.json. Full validation was rerun because this changes the shared runtime error path, not for an evidence-only tail.

Offline Rust1.94.1/recalld-ci:1.94, network none,2CPUs/4GiB,jobs2,testthreads2, read-only registry and dedicated target; no source-root switching, home credentials, provider network or socket mounts. Final source/build-input/executable hashes are in final-manifest.json. Raw final log SHA256 `9fd1c3786eaddf5bbb59a0c33387872dcee0b5c9399a482853424ffa73e0ffc8`, final manifest SHA256 `dd647dd5286562b7a02af3c90cea77435570258e1d80ad203ba85d3fc365f0de`.

Ordinary native serving remains gated. Huygens owns the actual shared-entry/wire consumer and accepted-root propagation. Pascal separately owns the PR82-only unclean-redb disposable-recovery correction; it is not silently claimed by this in-process failure proof. Numeric policy, maintenance/purge and remaining writer/read-view integration still require their own slices.
