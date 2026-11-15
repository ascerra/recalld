# Accepted producer evidence

Runtime: `209ce38ab6949c1231d54c099a98f48c9239c4b5`. Actual parents: PR78
`bec3bb5aa4e9525efed3170320e29c5413575d56` and PR70
`af20d7331410feef8ea1d2a8803e6a0772bb5d99`.

## Final results

| Check | Result |
| --- | --- |
| `cargo test --offline --all-features` | 906 library + 2 CLI passed; 4 doctests ignored |
| Added regression coverage | 14 tests included in the full suite |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --offline --all-features --all-targets` | Exit 0; 554 library / 614 test warnings (541 duplicates), one binary warning |
| Source hashes | All 198 runtime files identical before/after and verified against runtime commit |
| Initial script exit | 101 after successful tests: absent `offline_eval` example |
| Remaining-checks script | Exit 0 on unchanged source, without rerunning the suite |

Rust 1.94.1; `recalld-ci:1.94` image ID
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`. Docker network none, two CPUs, 4 GiB,
two build jobs. Source mounted read-only, private cargo and target directories.
The copied package outputs were cleaned before first build. No source-root
switching reused a test executable. No home, credential or socket mounts.

Library test executable SHA-256:
`4eba6453f4e7dd3f7ea7d35116e6ee568cb4a207ff9ef16c8f3a84c1fa534d7a`.
All six executable hashes are in [manifest.json](manifest.json).

## Artifacts and limits

- [Manifest](manifest.json): runtime, parents, source/executable/artifact hashes and exact environment.
- [Full test log](final.log.gz) and [remaining format/Clippy log](remaining-checks.log.gz).
- [Original validation script](validate-final.sh) and [remaining checks](remaining-checks.sh).
- [Before](source-before.json) / [after](source-after.json) source manifests and [generator](source-manifest.py).
- [27 source-adjustment files](source-adjustment-files.txt) and [remerge/source-adjustment diff](source-adjustments.patch.gz). The diff includes necessary parent conflict reconciliation, not just producer additions.
- [Development notes](development-notes.md): surviving failed-compile log and limits on intermediate log provenance. The wrong temporal peer-count assertion is not presented as a production red/green experiment.

`offline_eval` is not in the actual 70+78 ancestry. No unrelated PR75 or PR80
parent was added for validation. The absent-target error is retained verbatim;
it does not invalidate the successful full suite or imply an example passed.

The experiments use synthetic tempstores and deterministic pause points at real
file/handle, queue, provider and graph boundaries. Cancellation/unknown work stays
unresolved. Known publication failures preserve their receipt and durable pending
state, and the actual completion path repairs them without reopening. Existing
process-exit/reopen checks are not power-loss evidence.

HTTP provider-before-root, true/prepared query/exposure/feedback consumer coverage
and publication/read-view coherence remain Halley-owned joins. Native writer and
batch proof extensions remain Feynman/Pascal/Huygens work. This is not whole-core
readiness, a public CAS/v2 activation, or full A1/A6 completion.
