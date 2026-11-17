# N1 default all-target run006: TIMEOUT, not PASS

Exact runtime/test source `22bc5f0c598aa0c78bb95926cbd418769b8dd7b6`, same immutable source used by focused005. Queue approved one final default all-target command with a 30-minute test cap, followed only on success by separate all-feature/all-target Clippy. The cap was not extended.

Actual offline Rust1.94.1/aarch64/default stack, 2CPU/4GiB/jobs1/test-threads1, same protected target and retained registry. No rebuild of a different source or cache clone. Formatting passed. Default all-target compilation passed in 54.01s, with warnings (library-test compilation reports72). The test command and container exited **124** at the 30-minute deadline. No FAILED assertion line was reported; this is incomplete validation, not a full pass. Clippy did not run.

A provisional regular-expression tally found1,414 `ok` lines and3 ignored lines. That is NOT unique completed-test proof: nested crash-child output splits some parent status lines and repeats helper names. A read-only `--list` inventory of the exact preserved executable after container cleanup establishes1,686 library cases. Conservative classification:

- 1,406 cases have exactly one unambiguous `ok` record.
- 3 cases have exactly one ignored record; their original historical-fixture reasons remain unchanged.
- 9 cases remain ambiguous or unfinished: four interrupted parent/child pairs plus the final started `native_document_recipe_actual_single_provider_input`.
- 268 library cases were not started.
- Therefore277 exact library names must still run, including every ambiguous/unfinished case. None is excluded just because alphabetical progress passed its name.
- All13 other compiled test harnesses remain unexecuted, containing10 CLI integration tests and15 example tests (plus zero-test bin/example harnesses).

The exact executable inventory and continuation names are recorded in `test-inventory.json` and `continuation-plan.json`; raw provisional status is retained in `observed-results.json` with this corrected classification. No source/assertion changed. The literal harness help supports multiple exact filters; any continuation must verify its selected list equals the277 requested names before execution and reconcile results by artifact+full test name.

The14 compiled test executables were hashed before execution and verified unchanged afterward. Four non-test subprocess binaries used by CLI tests were additionally hashed while the library was still running, before CLI tests, and verified after timeout. The library executable (`ecd1db3f588751cce45170b39e824e0fe1a06fdf6928eb42ee483d615ed5059e`) is hard-linked into the protected raw root. Container auto-removal was confirmed by an empty exact-name `docker ps -a`; host driver completed124, so no test workers or container remain.

Peak memory2,966,142,976B; max/OOM/kill/group-kill all0. This was a wall-clock timeout, not OOM. Source inputs verified unchanged. Finished2026-11-17T03:07:55Z. Raw006 and its read-only source dependency005 remain protected; no raw HOME archive, cleanup, push, READY or activation. Broader coverage and lint remain pending review of the bounded same-executable continuation.
