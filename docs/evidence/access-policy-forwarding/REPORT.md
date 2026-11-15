# Query-owned policy forwarding evidence

Runtime `8dd1cc79ad6772b55b6286c64c0fa264cc07680e` is a direct child of actual
#96 `387c7118864040c68d7d28c44fb2600ade520a45`. It contains only the additive
Rust recorder result/default and its actual QueryEngine consumer, with one
regression; no incremental publication, native writer, graph or producer ancestry.

The QueryEngine forwards its configured global decay policy and multiplier, the
original scope and accepted-work ticket, and its optional already sampled clock.
`AccessObservation::from_receipts` retains the existing full completion path.
Legacy implementations delegate through the new default to their existing sampled
or non-sampled methods. A future native adapter can consume the actual policy
without inventing a default or depending on unfinished optimization code.

The actual QueryEngine test sets nondefault stability 17.25 and multiplier 2.75,
checks forwarded values and accepted work, and delegates through the production
legacy StorageAccessRecorder. Real canonical writes retain the supplied timestamp
without a second clock sample; the old non-pinned path samples its recorder once.
Both return distinct receipts and finish full derived publication. Empty access
input remains byte-immutable and does not call the recorder; owned work drains.
Existing full-suite explicit-policy rejection and read-only tests remain in scope.
This is not native exposure activation or completion of the future native adapter.

**1 focused regression, 1001 library tests and 2 CLI tests passed**. The library
suite ran with normal parallel execution in 420.14 seconds. Formatting passed;
clippy exited 0 with warnings, not a warnings-clean claim. Doctests were not run.
Source manifest SHA256:
`d03c675df6462b45c038f50e12c0514d2865186786951b161bdfef35bbde9083`.
Validation used immutable allowlisted source, private reflinked cargo/target,
`cargo clean -p recalld` in the own target, Rust 1.94.1, `recalld-ci:1.94`,
two build jobs, `--cpus 2 --memory 4g --network none`, no home/credentials/socket
mounts. No source-root switching reused an uncleared package executable.

The final MANIFEST records exact source, test/CLI binaries, logs, resources and
validation counts. Raw source/logs belong only in the verified HOME evidence
copy; Git contains source, documentation and text manifests. No benchmark,
performance promotion, public API-v2 or native serving-readiness claim.

Verified HOME retention destination: `/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-access-policy-forwarding-8dd1cc7`.
`COPY-VERIFICATION.json` records every copied member and hash; source/logs are
not stored in Git. The immutable snapshot and working source were both verified
after completion; per-source library/CLI binary hashes are in MANIFEST.
