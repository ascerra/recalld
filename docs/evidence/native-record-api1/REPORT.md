# Native plain insert / forget original-API evidence

Parent: [published PR121](https://github.com/calebevans/recalld/pull/121),
`3fa91d4e542ec8ed5519fe4c3b87e9e665043a40`. Runtime source:
`b3a21e6b40805309886385bb92b019880b2e4798`; final test-only follow-up:
`f19655b5e886e364a1f0d471412ff82202bec554`. No wire, new compound-context,
purge/catalog correction, or broad checkpoint parent was added.

## Reproduction before the change

Test-only commit71721fa runs on unchanged PR121 runtime. The genuine fixture
writer passed1/1 in1.96s. It uses native `capture_insert`/`insert_prepared`, then
`capture_tombstone`/`tombstone`, under original API2 contexts with real expected
canonical generations. It exports the actual resulting store and records both
full receipts and raw receipt-table rows. The fixture is not serialized by a
new writer pretending to be an old one.

The three API1 probes then failed in3.22s:

- Actual generated insertion rejects with `InvalidMutationContext("native records require contract version 2")` before provider work.
- Actual forget rejects with the same API2-only restriction.
- The scope test expects the canonical scope-mismatch error, but API1 is rejected by the earlier API2 restriction. This third failure is diagnostic precedence, not a reproduced write or ownership violation.

Full failure output remains in `before.log`. BEFORE library SHA256:
`4aa631432c8837a25638f866b72ea444f5df7e0144d40261a56d11f4218475e4`.
The BEFORE source archive/manifest and two fixture files are hash-bound in the
artifact manifest. No process-exit or power-loss claim is made by these probes.

## Minimal correction and actual consumers

The only runtime change removes the redundant five-line `contract_version !=2`
check from the native record context function. Existing `ctx.validate()` already
rejects versions outside1|2; exact namespace and native-owner checks remain.
Request hashing still consumes the original API, scope, expected generation and
original caller input through the existing encoder. Nothing rewrites old hashes,
receipts or native record proofs. No new guard or context type was added.

The tests exercise real `NativeLifecycleSession` and engine entry points:

- API1 and API2 generated insertion call the provider once (three interface calls), advance one generation, and retain their own request identity. Exact replay with stale original CAS calls no provider or clock and leaves every store file and receipt row unchanged. Swapping API1/API2, changing the summary or CAS conflicts. Unsupported0/3 rejects; foreign lineage rejects. Reopen preserves replay. An unkeyed API1 supplied-vector insert also uses the native writer and generates a different UUID.
- API1 and API2 forget each advance one generation, leave a tombstone, and replay the full original receipt before fresh clock/current-state checks. Swapped API or CAS conflicts. Actual logical export/import preserves all receipt rows and both replay/conflict behavior, with unchanged generation/files/clock on retries.
- Wrong namespace/catalog scope and an ordinary legacy owner reject without file, generation or clock mutation. This is actual owner admission, not a mocked guard.
- The genuine BEFORE API2 export is imported and opened by the AFTER binary. Both old receipts replay through actual `insert_prepared` and `tombstone`. Reusing those keys under API1 conflicts. Every historical receipt row remains byte-identical; generation, files and clock remain unchanged.

The first corrected run passed5/5 in7.80s, including the historical receipt
lookup. The final source strengthened that historical insertion check to call
the actual engine consumer; final focused checks passed5/5 in8.56s. The five
reported tests include the fixture-export entry, which returns immediately when
its write environment variable is absent. The historical read fixture was
enabled in both AFTER focused runs and in the final full run. Those initial runs used the external fixture. The final test-only follow-up
bundles that exact fixture, so normal CI also exercises the genuine BEFORE
receipt compatibility check.

## Isolation and exact commands

Docker `recalld-ci:1.94`, image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`;
`RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_BUILD_JOBS=2`, `--network none --cpus 2 --memory 4g`.
Only the assigned worktree and its private cargo/target directories are mounted.
No HOME, credentials, socket, personal daemon or personal data is mounted.
The private copied target was package-cleaned before the BEFORE build. Source
root never switched; normal source edits separated builds, with each executable
and source snapshot independently hashed. Source stayed frozen during each run.

BEFORE, with `RECALLD_WRITE_RECORD_CONTEXT_BEFORE=/work/.validation/before-fixture`:
`cargo clean -p recalld && cargo fmt --all && cargo test --offline --all-features
native_record_context_before_fixture -- --nocapture && cargo test --offline
--all-features native_record_api1_ -- --nocapture` (expected final exit101).

AFTER, with `RECALLD_READ_RECORD_CONTEXT_BEFORE=/work/.validation/before-fixture`:
`cargo fmt --all --check && cargo test --offline --all-features native_record_
-- --nocapture && cargo test --offline --all-features && cargo clippy --offline
--all-features --all-targets`.

First full-run (b3a21e6) executable SHA256:
`cbc9c0126ce47e6aa8fe67a8e0df562591ca0b3594b2efae44ef458f81d83dd0`.
`provenance.json` also binds the initial AFTER executable. Final full-suite results and verified HOME evidence are recorded below and in
`HOME.md`.

## First full-suite failure and test-only correction

The first full run at b3a21e6 finished with **1,097 passed, one failed**,644.29s.
It stopped before CLI/doc tests and clippy. The sole failure was inherited
`native_forget_scope_contract_and_stale_cas_reject_before_clock_or_bytes`: its
negative case expected contract1 to reject, while this approved change allows
the actual API1 forget consumer. No runtime correction was needed.

With coordinating-agent approval and Pascal's explicit test-hunk release, that
negative case now uses unsupported contract3 and requires typed
`InvalidMutationContext`; its scope/assertion/CAS and byte/clock checks remain.
The failed full log and exact source/executable are retained without alteration.

Pascal independently source-reviewed the five-line runtime change and tests,
finding no runtime blocker. He also identified the normal-CI fixture gap. The
genuine two-file BEFORE fixture is now bundled in a15,878-byte ZIP under
`native_records/fixtures/request-context-api2.zip`, using the existing ZIP
dependency. Its members were compared byte-for-byte with the captured fixture.
ZIP SHA256:`d9f61e87b45de34ae8622dbb3145fd577fe45333943d7777294bf5aea5a4af78`.
Normal CI will run the historical proof without an external environment; the
explicit read override remains available for reproduction. This is test-only.

The performance drain began after the first full suite was already running. It
finished normally; the container exited and the owned-container check was empty.
The two test-only corrections remained unvalidated during the hold. No new
heavy job ran until explicit END at22:14UTC. The final shipping run onf19655b
uses no fixture READ/WRITE environment: the corrected negative test passed1/1
in2.43s and the five focused checks passed5/5 in11.18s, including the bundled
normal-CI historical receipt path. Full validation completed successfully; results follow.


## Final shipping validation

Exact tested source: `f19655b5e886e364a1f0d471412ff82202bec554`. This is a
**test-only follow-up** to runtimeb3a21e6. No runtime changes followed the original
failed full run. Its corrected unsupported3 fixture and bundled genuine old2
fixture ran without external READ/WRITE fixture variables.

- Corrected negative: **1/1 passed**,2.43s.
- Native record focused set: **5/5 passed**,11.18s; the exporter entry is an
  intentional no-op without WRITE, while the bundled historical replay runs.
- All features: **1,098 library tests passed**,881.06s, plus **2 CLI tests**
  (3.58s and1.69s); **4 doctests ignored**.
- `cargo fmt --all --check`: passed.
- `cargo clippy --offline --all-features --all-targets`: passed,36.23s,
  with warnings (940lib-test warnings,853duplicates); not warning-free.
- Docker container exit: **0**, captured by `docker wait` on the actual run.
- All **257** source-manifest entries and archived bytes match after validation.

Final library SHA256: `278d6e484d7f6d30937721d20969730d09a968797d039736c7b141928d82a321`.
Raw `before.log`, `focused.log`, failed `final.log`, and successful `shipping.log`
remain separate, as do their source archives/manifests and binary hashes. The
original failed log was not replaced. `artifact-sha256.json` binds the retained
raw package. These are synthetic tempstore checks; they do not establish public
native startup activation, all-writer CAS, or power-loss durability.
