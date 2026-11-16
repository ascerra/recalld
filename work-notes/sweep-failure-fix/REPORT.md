# E1: bounded HTTP sweep failure evidence

Validated bounded correction, frozen for queue review/consumption. Exact-head CI
and final main-composition validation remain gates. Production source last changed at
`b62fcbd8fc1c22fad76bdfc68005a072920225c1`; the final test candidate is
`45a1710e0598ed7baeb9249c27b794a9b74328ae`.
Sole runtime base: `33865ca0d19599a13ba999d31a1ec736c89db233`.
No PR184 or queue-composition parent. This is an implementation child of the
sealed [RED report](../sweep-failure-contract/REPORT.md); that historical report
and its source/executable remain unchanged.

## Change

The concrete public manual sweep adapter now retains actual complete receipts
in canonical-return order and returns a typed, bounded HTTP503 failure body.
It distinguishes known commits, actual unknown outcomes, budget refusals and
dependency refusals. Unknown keeps legitimately returned prior receipts and an
actual uncertain operation ID (or null for a lost worker), and suppresses the
singular known-failure alias. Failure messages are fixed; they do not duplicate
receipt JSON through an error display string. Success shape, original1, configured
one-sample clock, and public asOf/as_of rejection remain intact.

The existing runner/completion/storage implementation accepts an optional private
pass context. None retains the existing internal/autonomous path. Under the
existing publication/graph/storage guards, bounded completion refuses foreign
pending effects, unapplied effects or active purge before any rebuild, ACK,
purge resume or early no-op. It never resumes purge. Producers recheck this
authority under their final metadata guards. Empty-pending ACK also rechecks
same-generation additions in the final transaction before clearing dirty state.

Each actual complete candidate receipt must fit its remaining allowance inside
the existing metadata transaction, before certification preparation, dirty/epoch
publication and commit. Rejection aborts staged rows, indexes, ledger, outbox and
control changes. It does not promise zero consumed IDs or no private unselected
compaction file. Known receipts are retained before later fallible completion;
failure stops later records, compaction and unconditional generic completion.
Mutation/completion worker loss remains unknown and accepted work unresolved.
Read-only capture failure and derived phase-index checkpoint failure do not
invent an uncertain canonical operation.

The HTTP-body cap is the existing 8 MiB constant. Accounting uses the same typed
serializer as the response: `2048 + 2 * sum(encoded_receipt_length + 1)` with
checked arithmetic and a capped writer. It bounds response evidence, not total
proof/rebuild memory. Current-pending validation uses keyed receipt checks, not
lifetime history scans. Strict startup/offline proof remains unchanged.
Large public manual passes can now stop before a candidate with a typed budget
failure and complete earlier receipts. They also refuse foreign pending work
instead of completing it. Internal None-mode runners retain their prior behavior.

## Evidence and attribution

All checks use isolated synthetic stores in `recalld-ci:1.94`, Rust1.94.1,
`--network none --cpus 2 --memory 4g`, one Cargo build job and the same private
target/cache. No home, credentials or Docker socket mounts; no personal daemon,
data, models or study retrieval run. Source snapshots are read-only and contain
443 selected build inputs. Package clean precedes each source-root switch.

| Candidate | Exact outcome |
| --- | --- |
| `ca40fe3b` / green1 | Compile-only failure: old private compaction test helper call lacked newly added argument; restored original wrapper. No executable/test run. |
| `b62fcbd8` / green2 | fmt/compile PASS; 13/14 exact tests PASS. Peer-effect race passed; subsequent purge fixture wrongly targeted protected default namespace. |
| `3f555d3b` / green3 | fmt PASS; compile-only missing test type-module path. No executable/test run. |
| `090b7ded` / green4 | fmt/compile PASS; 20/21 exact tests PASS. Purge setup now valid, but its ordinary control-state inspection correctly rejected active purge before releasing the latch. |
| `bc4e20c8` / green5 | fmt/compile PASS; 22/22 exact focused tests PASS. Default-feature all-targets command stopped at library: **1647 passed, 4 failed, 3 ignored**, 1992.28s, exit101. Clippy all-targets exit0 with warnings. |
| `45a1710e` / green6 | Owner-reviewed four-file test-only correction. **27/27 focused, 10 CLI integration tests, 15 example tests PASS**; four bin targets ran zero tests. fmt/compile/clippy/provenance exit0. No second local full library run. |

The completed `bc4e20c8` full run, rather than the earlier source hypothesis,
established four distinct failing assertions:

- `producer_work_tests` line412 saw zero pending effects: its wrong-width vector
  view caused initial completion to fail before the intended deletion.
- `maintenance_tests` line323 received a typed **notCommitted**, empty-evidence
  failure from the same unpublished-baseline setup, not a postcommit failure.
- `maintenance_tests` line601 received the correct typed committed full receipt
  but still downcast to the previous `StorageError` carrier.
- `system_inherited_policy_tests` line154 expected raw "inherited" diagnostic text
  instead of the deliberately fixed-message failure representation.

The corrected positives perform real completion while the view is healthy,
assert unchanged control, empty pending effects and current four-view publication,
then install the existing wrong-width fault. They require exactly one new G and
full receipt/alias equality, retaining same-owner repair, tracker, freeze and
reopen/free-slot checks. A separate actual HTTP initial-broken-view regression
retains the original refusal before canonical mutation with unchanged logical canonical state,
no receipts/ID, no phase change and no stranded work. The policy negative keeps
its original query/capture refusals, real adoption and unchanged-state checks;
only its error-carrier assertion changes. No production code changed.
Initial completion may attempt derived rebuilding before detecting a broken
view; the negative does not claim no derived I/O or byte-identical derived files.

All-targets CLI/bin/example execution was skipped when the library failed and
is explicitly included in the corrected tail. Its command names all five CLI
integration targets; it does not use `--tests` to accidentally rerun the library.
Final-head CI and the eventual main composition remain gates. There is **no
corrected local full-suite PASS claim**.

The three ignored tests are deliberate historical-only generators or require
external evidence: `legacy_api2_generate_genuine_before_one_fixture`,
`native_single_wire_genuine_before_dense_full_receipt_fits_all_transports`
(requires `RECALLD_SINGLE_BEFORE_ARTIFACTS`), and
`api1_batch_private_before_timestamp_rejection_export`. They were not executed.
The failed full's clippy run exited0 with **1267 library warnings and 1358
library-test warnings (1203 duplicates)**, plus target warnings; not warning-free.
The corrected tail has the same clippy warning counts. It uses default features
(no `--features` override), one Cargo build job, two CLI/example test threads and
one test thread for each separate exact library invocation. It executes all five
CLI integration targets explicitly; four example targets contain 0/6/1/8 tests.
These are synthetic unit/integration checks, not another study retrieval run.

Final library executable SHA256:
`618027fda27b14d72c3d2fdacb58a0d63e23dd74dc68fd08a5529f3b47604c52`.
The failed full used
`a39c9e8592cb77ef708dcb75d712884e775caf30dca978d2137bc80339989e73`.
[EVIDENCE.json](EVIDENCE.json) records all candidate source/log hashes and results,
including the final 18 executable identities (14 test executables plus the four
actual CLI binaries invoked by the integration tests). All 443 final build-input
hashes were verified unchanged. Subsequent report/plan edits are not compiled
source. [CHANGED-FILES.txt](CHANGED-FILES.txt) lists source changes versus the sole
runtime base. No build output/cache is staged or committed.

The two purge failures are test setup/observation errors, not production defects
or completed purge-race proofs. Their exact logs/source/executables are retained.
No production availability, purge, owner, schema or history guard was weakened.
The original first/later lost-ACK RED remains separately attributed to `2e3cdc25`.

The new matrix uses actual HTTP handlers, accepted workers and canonical writers:
first/later lost ACK, later known publication failure, physical-compaction receipt
followed by completion worker loss, body caps, first/later budget rejection,
legacy/native phase/deletion, legacy strength/bridge deletion, pending/purge
refusal and strict reopen. Backend Selected/Published compaction fault tests and
pure serialization/unknown-precedence tests are labeled separately from HTTP
execution. The synthetic lost ACK occurs after a real metadata commit; completion
panic is worker loss at its explicit boundary. Neither is a power-loss or actual
disk-I/O failure claim.
The HTTP native-record fixture uses real schema migration/attestation and policy
adoption while retaining `legacy_access_v1`; it does not silently migrate those
records to lifecycle v2. The separate purge namespace uses actual native catalog
creation. No policy-formula or study-quality claim follows from this error matrix.

Logical canonical comparisons cover records, namespaces, control, edges, text
selection and ordered raw receipt/effect/removal-ledger rows. They are not
whole-file byte-identical or general concurrent snapshot claims. Reopen validates
historical receipt bytes and no extra canonical generation. The private purge
inspection repairs only a disposable copy, never the source store.

The measured current-pending scaling check holds one current effect fixed:
1 versus 65 historical insertions both produced **1 receipt decode, 0 full
control validations, 0 certification scans** at the guarded admission helper.
This is a count regression, not timing or whole-sweep complexity evidence.

## Compatibility and limits

Direct typed f32 response serialization intentionally changes the old failure
Value spelling `0.020643016323447227` to `0.020643016`. The decoded full receipt
remains equal; stored receipts, hashes and historical bytes are unchanged.
Old raw wire evidence is preserved. No rounded/truncated receipt is substituted.

An internal admission-invariant error owns any unadmitted complete receipts and
refuses ordinary failure projection. It is not an approved generic HTTP fallback:
the actual closed producer paths must prevent such evidence from reaching the
adapter. No claim covers arbitrary fabricated wrappers or future unparticipating
producers. No keyed sweep, successful keyed no-op binding, public2 activation,
ordinary native startup activation, complete A3 coverage, restore guarantee or
quality/RL promotion follows from this slice.

## Ownership and resources

Pascal owns this branch and implementation. Halley retains queue integration;
Curie reviewed shared effects/ACK, Feynman storage proof, Huygens typed HTTP and
failure representation, Hume worker/lock participation. Their source ACKs are not
independent executed-test claims. The plan and explicit releases are recorded in
[the implementation plan](../../docs/implementation/sweep-failure-implementation-plan.md).
Curie/Hume reviewed `b62fcbd8`; Feynman also reviewed the `090b7ded`/`bc4e20c8`
test tails. Huygens also reviewed the `bc4e20c8` test tail; his ACK explicitly
excludes approval of a generic fallback for internal invalid evidence.
Halley approved the exact `bc4e20c8`
full gate after all 22 focused tests passed.
The actual four failures superseded the preliminary three-downcast hypothesis.
Curie, Feynman, Hume and Halley reviewed the expanded four-file patch
`9a8372993e3b4033927d3e7ea526282a0b882e6794c87654c8dea3f249e7702d`
and its exact base-file hashes before application. Its added independent initial
refusal test and stronger ready-baseline/one-new-G assertions are preserved in
the executed `45a1710e` source. No unconditional earlier ACK was reused for a
different failure. The completed failed full and corrected tail remain distinct.

Protected evidence: `/tmp/recalld-sweep-failure-validation` (including failed
runs). Reused target: `/tmp/recalld-a3-matrix-target`; private Cargo:
`/tmp/recalld-api1-lifecycle-validation/cargo`; registry is mounted read-only.
No cache clone or HOME raw archive. All owned run containers are removed. The
reusable target/cache and raw evidence remain protected until queue acceptance;
cleanup remains centrally coordinated. Primary study raw/seeds/evidence untouched.
