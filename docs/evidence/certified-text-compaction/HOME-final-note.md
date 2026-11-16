

### 2026-11-15: certified text compaction full result and test-only boundary closure

Frozen runtime f908357369781950ffe369fb66622f74e14f0f2a, exact175+147 merge c19ee17e. Candidate006 completed:1430libraryPASS/1FAIL/2ignored864.78s,5CLI+10examplePASS,39focusedPASS52.18s,fmt0/all-feature all-target Clippy0withwarnings22.78s. Sole failure was the original175 assertion at vector_certification/tests.rs1377 that native physical compaction must fail; actual composed writer now succeeds. The full failed source/log/executable evidence is retained. No other full-suite failure.

Coordinating agent approved a narrow test-only correction after this actual failure. Follow-up13ef229e5dd9cd438e91591447f39d1e4a6654db verifies exacttypedreceipt lookup, canonicalG+1, unchanged certification and real4view publication agreement after ack. Candidate007 correctedboundary1PASS2.83s,39compactionPASS51.70s,fmt0/all-feature all-target Clippy0warnings22.77s. No production file changed after006; all413sourceinputs verified against007. No repeated full-suite or all-feature runtime claim. Exactpublished-head fullCI remains required.

006source-manifest9e77951e2ae0d84514af09814a78f3ea1ad2d989da341af6456782b6f3718dc5; executable-manifest96233252f867274fafa445b71bcf49144463c1e12d5622e203291713a17de029.007source-manifest75f5886d54de3312e8c111e12638c3a3cdb16399a7c49016de679fb5558cd9a6; executable-manifest3149a85e48f79954dfe5192a2d1e6ad7e088a896588117536d99bb796342f545. Both4GiBpeak, oom/oom_kill0;006memory.max1921,007956. This is reclaim pressure, not OOM.2CPU/jobs2 offlineRust1.94.1 and sole retained target unchanged.

Report: /tmp/recalld-implementation/certified-text-compaction/docs/evidence/certified-text-compaction/REPORT.md; full-manifest.json and tail-manifest.json separate the source-specific claims. Sealing for main review, not yet pushed/published. Raw seven run roots stay protected until resulting PR/results then central release; no raw HOME archive. Nextactual149consumer plan b573/d515/a68 contingent on this final validatedfreeze plusHuygens queryconsumerproof, with globalprivateprofileloops unchanged. No ordinary native/public2 activation.
