# E1 HTTP sweep failure: bounded RED-only handoff

Status: **actual defect reproduced; production correction not implemented**.
Sole base: `33865ca0d19599a13ba999d31a1ec736c89db233` (actual main).
Plan: `970e7310104d2d05d52d141af42cfe00b025e275`.
Executed source: `2e3cdc25a8b26014b3e3b320a0340912872def15`.
No PR184, frozen queue branch or additional implementation parent.

## Observed result

Four separate, nonzero exact-filter executions used one sealed test executable.
Both unknown cases reached the final HTTP assertion after all durable state and
strict reopen assertions passed. They returned HTTP503 with `SERVICE_UNAVAILABLE`
and a message containing `commitState=unknown operationId=...`, but no structured
`commitState`, `operationId`, `ready` or prior receipt array. The actual failed
assertion was missing `commitState` (`null` versus `"unknown"`). The retained raw
response separately demonstrates the other absent fields; their subsequent test
assertions were not reached and are not counted as independently executed checks.

| Actual handler case | Canonical generation | Result |
| --- | --- | --- |
| First record lost ACK | G6 → G7 | Expected HTTP evidence RED |
| Later record lost ACK | G6 → G8 | Expected HTTP evidence RED |
| Existing known derived publication failure | G6 → G7 | PASS: HTTP503, committed, ready=false, full exact receipt |
| Supplied `asOf:null` with hook armed | unchanged | PASS: HTTP400, zero clock calls, unchanged records/G, hook unconsumed |

The unknown fault is an instance-local cfg(test) counter in the existing
successful post-transaction closure. It synthesizes a lost ACK **after an actual
successful metadata transaction**, not a physical I/O error, power-loss test or
postcommit process exit. The known control uses an actual wrong-width derived
vector view and is not an unknown-commit simulation.

## Durable proof before the intended failing assertion

The fixture uses actual legacy insertion, schema migration with profile
attestation, explicit inherited-policy adoption, native owned open, and real
derived completion before arming the hook. These are native-format records with
the preserved `legacy_access_v1` lifecycle; the probe does not imply v2 policy
migration or ordinary public native activation.

Each POST `{}` samples the same configured clock once. Unknown cases establish
an unavailable owner, outstanding accepted work and failure of the actual
readiness barrier. A disposable metadata inspection copy then proves the exact
G increment, all original receipt bytes unchanged, ordered actual new scoped
receipts and original1 request hashes, one pending failed effect, and the exact
full-text removal ledger operation/generation/mask. There is no extra canonical
mutation or text compaction. The later case contains two actual durable receipts,
not a count-based inference.

Strict native reopen succeeds without another canonical generation or receipt.
Affected records are Summary, full text is stripped, and summary/tags/history,
stability, created/access timestamps and decay anchor remain unchanged; the phase
marker equals the sampled time. An unaffected record remains byte-identical.
Receipt snapshots use the existing disposable-copy inspection helper on this
quiescent fixture. This is not a general concurrent snapshot claim. The production
handler never reads back a receipt to guess the unknown result.

## Validation and immutable attribution

- `3fe971870b930cbe50cb4122a9cb99e45f0778cd`: initial source review draft.
  Compile-only exit101: two test accesses to private `MetadataStore.db`.
  No executable or HTTP execution. Formatting check exit1 on the new draft file.
- `2e3cdc25a8b26014b3e3b320a0340912872def15`: only the test receipt-snapshot helper
  correction and new-file rustfmt. Compile exit0 (68 warnings), fmt exit0;
  two intended RED and two positive-control PASS executions. No full suite or
  clippy run was requested or claimed for this RED-only stage.
- Executable SHA256:
  `901ca7c0a5aec929b0a83dd55dfc075d636f3a6a5cb10613cc9a5c91f9ffb31c`;
  570,874,384 bytes. Exact source/executable/log hashes and timestamps are in
  [EVIDENCE.json](EVIDENCE.json). [WIRE.json](WIRE.json) preserves all four exact
  response strings, decoded bodies, durable receipts and executed test summaries.
- Both immutable source snapshots contain 441 build-input files from the exact
  commits: Cargo manifests/lock, toolchain/clippy configuration, src/tests/examples.
  All source hashes were verified unchanged after execution. Documentation is not
  represented as compiled input. `cargo clean -p recalld` preceded each candidate
  to avoid stale package artifacts when changing source mounts.
- Rust1.94.1, local `recalld-ci:1.94` image
  `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
  offline Docker `--network none --cpus 2 --memory 4g`, one Cargo build job.
  No home/credentials/socket mounts, personal data, daemon access or retrieval run.

## Ownership, scope and next gate

Curie, Feynman and Huygens reviewed exact `3fe97187` source; Halley gave literal
RED-only build release. The private-field correction stayed within that released
test-only scope; Huygens also reviewed its exact source delta. Pascal alone edited
this child. All non-test production code is unchanged from the base.

The corrected behavior is not yet implemented or approved for runtime changes.
The plan's remaining blockers are concrete: exact HTTP serialization accounting,
and bounded completion admission under the final existing locks so foreign pending
effects or purge work cannot introduce unbudgeted nested evidence or new commits.
Unknown precedence must survive nested known wrappers; prior full receipts cannot
be fabricated, silently omitted or truncated. See
[the plan](../../docs/implementation/sweep-failure-contract-plan.md).

No standalone PR, merge, deployment, full-suite/CI claim or ordinary activation.
This source and report are an exact RED handoff for the coordinator and owners.

## Resource handoff

Both owned validation containers were removed. No heavy job remains.
Protect `/tmp/recalld-sweep-failure-validation` (compile-only failed attempt,
actual RED, successful controls, source/logs/executable) until the coordinator
authorizes release. Central cleanup has acknowledged that protection. Retain the
one reusable owned `/tmp/recalld-a3-matrix-target` and
`/tmp/recalld-api1-lifecycle-validation/cargo`; the shared registry was mounted
read-only. No cache clone or HOME raw archive was created. No bytes released in
this handoff, and no primary-study input/evidence was touched.
