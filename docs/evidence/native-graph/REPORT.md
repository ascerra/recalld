# Native standalone graph proof — validation evidence

Runtime: `0ff05b0cb53593973fef640fb959d815594ea3cc`. Actual corrected PR82 parent:
`fb804a0a72ccf7dde0a6ee7eaea2e91a6b7ba2db` (published #82 evidence tail is
`bb1a0863b2ab0eba940e767941ca4e85fe9b8e77`). No PR83, batch/catalog or broad
integration ancestry is required. [Implementation and remaining scope](../../implementation/native-graph.md).

## Final result

- **14 focused native graph regressions passed.**
- **940 library tests and 2 CLI integration tests passed**, with default test
  parallelism under the declared container limits. Four doctests were ignored.
- `cargo fmt --all -- --check` passed. `cargo clippy --offline --all-targets`
  exited 0 **with warnings**: 607 library warnings; 646 test warnings, including
  606 duplicates. This is not a warning-free lint claim.
- The final 212-file allowlisted source manifest is
  `db040806f862d13722f1e83aeed7b6b508e6032691d431d2e1da79e5d4758856`.
  Mounted source and working source hashes matched before/after validation.
- Final library test binary SHA-256:
  `750fbfe941c43046a19fa554f3cd05c4578513d91263cf2f0f879a4f3621ccf1`.
  CLI and other Cargo-reported executable hashes are in [MANIFEST.json](MANIFEST.json).

The actual migrated/native fixtures exercise multi-edge semantic/entity/temporal
puts, attribute updates, reciprocal directions, one token increment per changed
source, strict scope/CAS/stale-peer checks, Ghost/Tombstone eligibility and retained
structural exceptions, supersedes cycles/branches, input limits, replay and no-ops.
They cover actual process exits before/after metadata commit, after edge commit and
after canonical acknowledgement; committed versus uncertain failures; malformed
proof/outbox/physical-edge rejection without changing originals; and logical
export/import plus deletion-aware restore with foreign-lineage rejection.

## Reproductions and preserved incomplete runs

| Run | Result |
| --- | --- |
| 1 | Initial 3 focused regressions passed. |
| 2 | Expanded 9 focused regressions passed. Full run deliberately stopped for two review corrections; **not full-suite evidence**. |
| 3 (RED) | Both actual regressions failed: `+0.0` → `-0.0` was incorrectly omitted by shared numeric edge equality; a malformed profile commitment in a restored historical graph receipt was accepted. |
| 4 | Both fixes passed within 12 focused regressions. Full run deliberately stopped for the newly identified keyed-no-op contract issue; **not full-suite evidence**. |
| 5 (RED) | A → fresh-key successful no-op → writer B at G8 → retry A incorrectly committed G9. |
| 6 | Preserved `962aeac`: 13 focused, full 939 library + 2 CLI, fmt and Clippy completed successfully; predates the semantic binding correction. |
| 7 (RED) | Actual restore accepted a foreign-origin receipt with a different, well-formed profile binding in both states; 0 passed / 1 failed. |
| 8 | Binding rejection passed; the added UnknownLegacy fixture incorrectly used contract 1 and was correctly rejected (13 passed / 1 failed). Full suite not reached. |
| 9 | Corrected contract-2 fixture: 14 focused, full 940 library + 2 CLI, fmt and Clippy completed successfully. |

The run-3 test binary was
`99120297be8d13f1ca20e6693af8df39e21c11e1c1c0c0240c29f986ee105da5`;
the run-5 test binary was
`d70968d66cafb975beef5eb71e53d06e5366db549cde38c1a0701e51b54c2fbd`.
The run-7 regression `native_graph_review_rejects_wrong_foreign_profile_binding`
uses an actual backup/restore, updates both proof states and any retained outbox
receipt consistently, and attempts native startup/export while checking original
bytes. Source manifest `03c0b6ccf24a4b7a35a31e186e4db223f5b232d0fd96863f5c73a6be7c364c9d`;
test binary `221589464e48d3e8171daeecb620dce925936e96784131051b06324a329410be`.
Shape validation alone is not semantic binding proof. The correction checks both
states against the immutable namespace configuration for all receipt origins.
Actual UnknownLegacy same-scope graph mutation and reopen succeed with None.

Both canceled full runs ended through an intentional stop of this task's container
(exit 137), not a reported test assertion failure. Their logs remain separate from
completed validation. No unrecorded retry or global cache cleanup was used. A separate scratch formatter
attempt failed on read-only file modes; those scratch modes were corrected before
the successful formatting and final source freeze.

## Isolation and evidence retention

Image: `recalld-ci:1.94`, actual image ID
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Rust 1.94.1, `CARGO_BUILD_JOBS=2`, Docker `--network none --cpus 2 --memory 4g`.
The only mounts were the immutable `/work:ro` source, read-only validation script,
and this task's private cargo/target paths. No home, credential or socket mounts.
Caches were copied by filesystem clone, not writable hard links. Every source-root
switch used `cargo clean -p recalld` in this task's target before rebuilding;
per-source executable hashes were preserved before later cleans. No other agent's
target was cleaned. Test runtime and resource limits are not performance evidence.

Raw logs, all nine source freezes, source manifests, scripts and verification records
are retained outside Git at the exact actual-home path:

`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-graph-0ff05b0`

[MANIFEST.json](MANIFEST.json) binds the source, executables, logs, actual runtime
configuration and each run's disposition. [COPY-VERIFICATION.json](COPY-VERIFICATION.json)
binds each copied file by relative path, size and SHA-256. Binaries/caches are not
copied or published; their hashes are retained. Raw evidence is **not in Git**.
The task-local originals remain under
`/tmp/recalld-implementation/native-graph/.validation`.

## Limits and next work

Fresh keyed graph no-ops now return explicit `InvalidMutation`: no success and no
reserved key. Unkeyed no-op `None` remains; matching committed receipts replay
first. This fail-closed restriction is **not final A4 keyed-no-op completion**:
a typed receipted no-change outcome (or an explicitly supported specification
error) remains tracked alongside policy no-change semantics.

Next priority is the Curie-coordinated frozen same-insertion link/advisory plan and
new additive proof. A work ticket alone is not durable intent; no automatic linking
or serving gate is enabled here. The old insertion proof/hash semantics remain
unchanged. Full graph/history scan costs remain, with no bounded-total-allocation,
throughput, performance-promotion or public native readiness claim.
