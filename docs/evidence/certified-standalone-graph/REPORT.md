# Standalone semantic graph certification — validated

Actual parents:175 `7283a700c4eca5f8d1d8907b33c867b1d1e3a6bb` and139
`098135ad5d8d6c4ccbe9f46640ebeb6ed34de375`. Local merge `e6cb9d8c` preserves
both publication/cancellation suites and the final policy/generation checks;
owner review is recorded separately. Exact frozen shared helper
`3f76d77e47715499b1ec9180514fec8af2578018` was extracted as `cc282956`.
No169/177/query/compaction convenience parent, no published-parent edits.

Runtime/test source: `57e8e1190e17540aaf083b94d3544cf05466abc9`.
The only new production consumer guard is the three-line Semantic-only
with_generation_read_at/check_native_vector_namespace callback before vectors.
Real Entity/Temporal/explicit structural work remains certification-free. No new
proof/hash/codec, provider inference, public contract or activation is included.

## Source-specific proof

- before001, source2508a541: test compilation failed on Tag construction and one
  missing Explicit argument wrapper. No tests executed; no binary attribution.
- before002, sourcea93043827988c9c2c068f93394f1df87b06d469d:1passed/1failed.
  The actual Semantic trait succeeds incorrectly for API1/2 with caps0/1. All
  four cases read3vectors. Cap0 returns None with0clock/no mutation; cap1 returns
  a graph receipt with1clock and changedG/bytes. The separate actual structural
  Entity/Temporal/explicit control passes (API1/2, keyed replay and reopen).
  Binary SHA2567b37550124bdde9465807fd4e3c31a59ca3e1e99b82346b2fd6308e2734e0446.
- after003, source57e8:6boundary tests pass,32.05s. Same four Semantic requests
  now return VectorCertificationRequired,0reads/0clock/unchangedG+bytes, including
  cap0. Certified semantic positives/reopen, source/candidate hash corruption,
  exact namespace mismatch, missing-owner RecoveryRequired and finalG rejection
  after an actual unrelated metadata writer are covered. That writer runs while
  the actual engine write guard is held, proving engine ownership alone cannot
  substitute for final canonical G.
- focus004, same57e8:6boundary+20producer+39core-graph=65executions pass.
  Filters overlap; this is not65distinct new tests. Durations31.42s/49.48s/121.59s.
  Both parents' actual receipt/publication/cancellation/process-exit cases pass.
  Binary matchesafter003:
  f43a97eb6499ad861e0b5dcb1714f2ebc00ee971629016d705bde6f43f44f8d7.
- full005, same57e8: **1436 library passed,0failed,2ignored**,1257.03s;
  **5 CLI and10 example tests passed**, allfeatures/alltargets. Fmt0, Clippy0
  with warnings,22.99s. Driver0; all390 source files unchanged. Thirteen Cargo
  test harnesses and three actual CLI applications used by integration tests
  are hashed. Library binary matchesafter003/focus004. No doctests were run.
  Memory peak4,055,203,840bytes; maxevents0/oom0/oomkill0.
- tail006, source90d6f494028edb207ea30a8813130601210f4327: after the frozen full
  run, merged actual parent175a54cdc7dc89221f66049147ee8543d84bbb49977.
  Its entire source delta is one line in tests/vector_certification_cli.rs:
  set accepted test socket blocking explicitly for portability. No src/Cargo/
  example change. Both actual CLI tests pass,2.05s; fmt0, Clippy0 with warnings,
  0.56s; driver0/source unchanged. Peak1,678,270,464bytes/max0/oom0.
  Full suite was not repeated for this test-only parent fix. Exact final-head CI
  remains required after publication; no final-head local full claim is made.

All executed runs use Rust1.94.1, pinned image
sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef,
networknone,2CPU/4GiB/jobs2, one private cargo/target, read-only source and isolated
temporary stores; no hostHOME/credentials/socket mounts. Own package cleaned
before source-root switch. No BEFORE-source switchback reused an executable.
Separate source/executable hashes are retained. Raw logs remain immutable; the
compact evidence omits repeated warning preambles, not failures. Compiler/Clippy
warnings are not claimed absent. Process exits are not power-loss experiments.

## Changes and evidence

Production change beyond approved parent composition: graph/native_producer.rs
three-line Semantic admission and frozen metadata/publication.rs helper extraction.
Counter instrumentation is cfg(test) only. Tests add graph_certification_tests.rs,
retain139 graph_producer_tests with explicit positive fixture setup, and preserve
175 publication tests in graph_publication_tests with typed Explicit wrappers.
Module entries and the six merge resolutions are recorded in OWNER-REVIEW.md.
Final parent portability change is tests/vector_certification_cli.rs only.
Report, manifests, rerun script, implementation/work note are the other changes.

before002/after003 binary hashes differ; source root was cleaned before first
compile. after003 reached the4GiB cap with1558 memory.max events but0OOM; this
is reclaim pressure, not a failure. Remaining run memory data is preserved in
COMPLETED-RUNS.json. Full original log hashes/bytecounts are inRAW-MANIFEST;
compact excerpts retain failures/test results and command/source/executable data.
All own containers exited and were removed before sealing. No broader backend
comparison/default-choice or A1 completion claim follows from these tests.

## Bounds and resources

This is the standalone semantic producer consumer, not ordinary native/public2
startup, query, insertion/batch autolink, full-store CAS or full A1 acceptance.
Read counts demonstrate work avoided, not latency or retrieval quality. Existing
uncertified vectors are not silently certified/reembedded. Standalone explicit
edges are caller intent, not proof of computed semantic provenance.

Protected active raw root: this worktree/.validation. Existing central cleanup
owner alone releases completed raw after PR/results; no raw HOME archive needed.
Sole private cargo/target is /tmp/recalld-curie-active-build. No cache clones or
recreation of previously released177/169/164 paths. This child is not published.
