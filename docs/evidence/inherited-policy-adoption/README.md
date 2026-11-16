# Inherited-policy adoption validation

Runtime3e5fd806 is validated below; publication review is pending. No branch has been pushed or handed off as READY. Sources are based on exact
143+135 merge0d50ee8. Each run records every copied source SHA256; the logged HEAD
is the base while the candidate remains uncommitted. The final seal must identify
its exact committed runtime and distinguish subsequent test/doc changes.

- `candidate1`: isolated compile failed (33 diagnostics from missing imports,
  MutationContext serde input support and private policy-field access). No test
  execution; not a runtime RED. Failed source and compiler JSON retained.
- `candidate2`: all-feature library test compile succeeded. Focused filter
  `inherited_policy`: **4PASS,0FAIL,3.90s** (three new adoption regressions and one
  existing catalog invalid-inherited-input regression). This is not full-suite
  or actual restart/fork proof.
- `candidate3`: actual143 loader/query restart regression and logical import/two
  production forks plus missing-binding negative added; compile failed on one
  missing test-only ReadableTable import. No tests ran.
- `candidate4`: **6focusedPASS,12.33s**, including actual143 restart and two real
  forks. Full library: **1097PASS,141FAIL,348.56s**. Shared dependent fixtures
  reached the new unknown-inheritance guard; one new history-validation unwrap
  panicked on an intentionally absent schema. That panic is corrected to a typed
  error, and fixtures now invoke actual explicit preview/adopt. Failure artifacts
  retained. memory.events oom=0/oom_kill=0, peak4294967296bytes (4GiB cap).
- `candidate5`: compile failed on one missing test import and an attempt to call
  a private feedback helper. Corrected to the actual accepted-work public crate
  wrapper. No tests ran; no test-only visibility widening.

Runs: `/tmp/recalld-inherited-policy-adoption-validation/`. Single reused private
Cargo target and read-only shared registry. Rust1.94.1, offline Docker, networknone,
2CPUs/4GiB, no personal home credentials/socket/data mounts. The runner cleans only
recalld in this agent's target before switching immutable copied sources and
captures the actual executable SHA256. Completed raw roots are released to central
cleanup only after PR/results are recorded; no redundant raw HOME archives.

Original actual restart BEFORE is retained separately under audit528cf28 and
its existing compact repository/HOME evidence. No current fixture was silently
blessed to erase that unknown-history boundary.

Complete candidate4 classification is in candidate4-failures.json (141 named
failures with exact source-manifest/executable/log hashes). Groups:121 explicit
unknown-admission errors,1 generic transport projection error,1 new malformed
schema panic,5 batch item rejections,6 producer outcome mismatches,1 early child
exit,4 producer wait timeouts,2 other assertions (missing committed child receipt
and a late-migrated sweep publication error). Masked outcomes/timeouts remain
source-based hypotheses until the original expectations pass the corrected run;
they are not retroactively called proven fixture-only failures.

Candidate6: all-feature library compile succeeded; focused8PASS/1FAIL12.50s.
The new actual manual-sweep negative expected an Ok error-report, but the existing
adapter returns Err containing the exact unknown-inherited rejection. Corrected
to assert that actual typed error; no runtime admission weakening. The full run
did not start because the focused test failed.

Candidate7: focused **9PASS,17.21s**; full library **1230PASS,11FAIL,502.63s**,
no CLI execution because Cargo stopped at library failures. Source, binary and
all eleven diagnostics are indexed in candidate7-failures.json. Peak4031348736bytes,
oom=0/oom_kill=0. Four lifecycle-migration fixtures assumed revision1 despite real
explicit adoption; one edge assertion assumed the pre-adoption token; two separate
late migrations still had unknown inheritance; four configured-policy tests assumed
fresh mismatching global inputs remained valid or returned the earlier conflict
classification. Corrections must use actual adoption/current canonical revisions,
retain same-key replay and immutability checks, and await rerun. Candidate4 masked
producer failures/timeouts now passed their original expectations in this run;
this does not reclassify the independently corrected malformed-schema panic or
nonprojecting-operation admission boundary as fixture-only.

Candidate8-ghost: focused **9PASS,1FAIL,16.71s**. This intentionally tests the
unfixed new preview against a genuine legacy Summary→Ghost history with a retained
vector and live linked neighbor, migrated through the real schema writer. Preview
returned InvalidControl("invalid native maintenance bonus"). This is a newly
introduced unpublished preview bug, not an existing published-store vulnerability.
Correction follows the actual maintenance writer's Ghost bonus0 rule; regression
also checks adoption does not restore stripped text/summary or remove its vector.

Candidate9: compile failed on one test-only Clock trait method call; no tests ran.
The fixture now uses its already explicit sampled time for real adoption.

Candidate10: compile succeeded44.12s; focused9PASS/1FAIL16.37s. The Ghost
production correction passed its former failure point, but the new regression
incorrectly expected deletion from the fixture's materialized0.01. The actual
planner projects canonical anchor/stability at30days and retains that Ghost.
Correct the expected count to0; retain the exact no-payload-resurrection assertion.
No runtime change from candidate9/10. Full did not start.

## Final candidate11 seal

Exact runtime **3e5fd80658e5d8ba2eec92a1c5e27bf8e45ae4a9**, tree
`d73a76b0d1214a3146c6971d13aa40676bd2cdb2`. All335 copied source files match the
committed runtime. The source manifest records dependency merge0d50ee8 because
snapshotting preceded the local commit; it is not being relabeled as that parent.

- All-feature library compile37.15s; focused11PASS16.52s.
- Each of the eleven candidate7 failing regressions now passes individually.
- Full all-feature library **1243PASS,0FAIL,497.72s**.
- Actual CLI integrations **3PASS**: inherited-policy1.41s, lifecycle1.48s,
  text-migration0.76s. The adoption CLI tests invalid requests/source immutability,
  sparse512MiB budget rejection, and empty per-child inspector scratch directories.
- Example tests **10PASS**: offline_eval6, performance_probe1, score_mcp_episode3.
  All binary/example targets compiled; zero-test targets are not counted as tests.
- `cargo fmt --all --check` and all-feature/all-target Clippy exit0. Clippy has
  warnings (lib-test summary1091 including982 duplicates); no warning-free claim.
- Source-after hash verification passed; peak4294967296bytes, memory.events max=704,
  oom=0/oom_kill=0. No6GiB retry or target/cache clone.

The runner/command, source manifest, executable hashes (including the actual CLI
binary), all run-log hashes and exact results are in manifest.json/full-results.txt.
The library executable SHA256 is
`4d4cb1ff49af47f1cce157747d2250a98bb98a0063adcfaf43c2b785a052f10b`;
full-log SHA256 is
`5cee44291aa1584438b1be50352f61b8364de9f93cbce8ddd984367466baa559`.
Original restart RED evidence and exact test-only patch are copied unchanged under
../inherited-global-startup; no extra runtime parent was imported for that evidence.

Candidate8's new Ghost regression is GREEN with the actual maintenance bonus0
rule; its candidate10 count correction is test-only and separately recorded.
Current unknown-history reopen/export/original-key replay remains tested without
adoption. Mixed adoption→maintenance→purge→export/import/productionfork validates
retired binding history. Fresh namespace creation asserts the binding refers to
its exact creation receipt; Some→None requires real subsequent adoption.

This seal closes the explicit inherited-policy slice on actual143+135. It does
not activate ordinary native/public API2 serving or certify historical embedding
provenance. The source-only vector-producer admission proposal remains separate.
After publication and recorded results, completed raw run roots can be released
through the central cleanup owner. Retain the one shared private target/registry
and genuine old-context input while still required by future validation.

Compact HOME evidence destination:
`/Users/caleb.evans/work-notes/work/Artifacts/recalld/2026-11-14-inherited-policy-adoption-3e5fd806/`.
COPY.json records the verified copied files and exact repository evidence head.
No raw executable/cache/source archive is copied HOME.
