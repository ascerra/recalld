# Native batch validation evidence

Frozen runtime: `febe91310153f2caab9209ca4cfae0d9fbbc5844`.
Actual parents: PR82 `6f56624`, phase rule `a69865b`, minimal startup recovery
`fb804a0` (on82 evidence `360c453`), PR80 `ca205aa`, and PR83 `a6087ca`.
No catalog or broad checkpoint parent. Local composition changes and native
implementation are retained in `source-adjustments.patch.gz`.

## Final acceptance run

- `cargo test --offline --all-features`: **1000 library tests + 2 CLI tests pass**;
  4 doctests ignored. Library duration446.14s.
- `cargo fmt --all -- --check`: pass.
- `cargo clippy --offline --all-features --all-targets`: exit0, with694 library
  warnings and761 library-test warnings (680 duplicates). This is not warning-free.
- Script exit0; exact219-file source manifests before/after match and were checked
  against the frozen Git runtime. Actual executable SHA256s are in `manifest.json`.

Command: `sh .validation/validate-final.sh` (archived here). Rust1.94.1,
`recalld-ci:1.94`, image ID in manifest, `--network none --cpus 2 --memory 4g`,
`CARGO_BUILD_JOBS=2`, private cargo/target, read-only source during tests. No home,
credentials, socket mounts or personal stores. The copied package outputs were
cleaned before first compilation. No source-root switching reused a prior binary.
No `offline_eval` example exists in these necessary parents; none was invented or
pulled from an unrelated parent.

## Preserved development and failure evidence

| Run | Observed result | Meaning |
| --- | --- | --- |
| native-focused-2 | 5 pass | Early bounded cases |
| native-focused-3 | 7 pass,1 fail | Test expected86 instead of hook's actual91; test-only correction |
| native-focused-4 | 9 pass,2 fail | Actual native startup RepairAborted after process exit; runner used public provider helper that intentionally rejects schema2 |
| native-focused-5 | 11 pass | Disposable native recovery and private generated-vector path corrected |
| native-focused-6 | 12 pass | Added real accepted-root/provider close race; runtime010d2ad |
| native-full-1 | 992 pass,2 fail | Parent join required two exact full-receipt assertions; failed log unchanged |
| native-review-before | 0 pass,2 fail | Reversed error generations admitted; UnknownLegacy batch forget rejected although single forget succeeds |
| native-review-after | 14 pass;1+1 pass | Both review fixes, mixed Error/Ok/Error corruption, and each exact receipt assertion |
| native-guard-focused | 15 pass | Configured auxiliary work remains pending; completed replay takes precedence |
| native-full-final | 1000 library+2CLI pass | Frozen merged source, then fmt/Clippy pass |

Both reproduced BEFORE failures have full source archives, per-file source hashes,
patches and executable hashes (`native-before-fixes-*`, `native-review-before-*`).
The12-case/full992+2 source is pinned by `source-before-full.json` and
`source-after-full.json`; its executable is `native-after-fixes-binary.json`.
The guard executable was captured before formatting, while guard-source.json was
captured afterward: these are not claimed as an exact source/binary pair. Final
source/binary pairing is exact and independently supersedes that intermediate
provenance limitation. Earlier compile and incomplete-input formatter errors are
retained as development logs, not runtime defects. All raw logs are gzip-preserved
and hashed. Initial archive creation required Git temporary-object permission for
remerge diffs; the sealed archive was regenerated with that permission.

The15 native tests exercise actual MCP and framed daemon batches, stable child
identities, matching replay before provider/current policy, original vector bits,
fixed-default expansion, generated invalid output, ordered strict history,
UnknownLegacy forget offline roundtrip, same-transaction outcomes, current
publication-error receipts, unknown reconciliation, real process exits before
and after child commit, completed offline preservation, unfinished offline
rejection and roots draining descendants during close. Pascal's three inherited
recovery tests add nonbatch process exits and five malformed repaired-copy
negative scenarios with original bytes and ownership locks verified. Process
exit tests do not prove power-loss behavior.

## Scope and independent review

Feynman source-reviewed exact febe913/cb3bd31 and reported no blocker in the fixed
order/profile validators or replay-first gate. He read the actual regressions but
did not independently execute them; all execution numbers here are owner runs.

Configured native semantic/entity/temporal autolinks and duplicate advisory are
still unsupported: pending UNAVAILABLE before provider/child mutation, with
parent reservation allowed. Positive fixtures explicitly disable them. This
branch does not establish public contract2 serving, whole-store CAS, complete
native-store semantics, or whole-core coherent-view/barrier participation.
A ticket is not durable autolink intent; the next producer join needs a frozen
same-insert plan/proof (or durable workflow) that survives process exit and does
not rediscover links on replay. Historical NativeRecordWrite<=2 meaning remains.

The external work-notes copy is hash-verified separately with COPY-MANIFEST.json;
the handoff records its exact destination. No publication or deployment was
performed by this branch owner.
