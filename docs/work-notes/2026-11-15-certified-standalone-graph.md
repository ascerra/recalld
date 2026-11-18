---
type: experiment
status: done
project: "[[Recalld]]"
started: 2026-11-15
ended: 2026-11-15
---
## Question
Does the actual standalone native Semantic graph producer admit an uncertified
Bound namespace, and can existing exact-G certification admission close that
boundary without blocking structural Entity/Temporal/explicit graph work?

## Hypothesis
139's Bound-only check permits semantic reads and graph commits. Reusing175's
owned certification check at the selected canonical generation should reject
fresh Semantic before vectors/clock/mutation while preserving structural work.

## Setup
- Code: impl/certified-standalone-graph, runtime57e8e1190e17540aaf083b94d3544cf05466abc9.
- Actual parents:139098135ad5d8d6c4ccbe9f46640ebeb6ed34de375 and1757283a700c4eca5f8d1d8907b33c867b1d1e3a6bb.
- Shared helper: frozen3f76d77e extracted ascc282956; no query/169/177 parent.
- Data: isolated synthetic temporary stores; real legacy insert/migration for
  retained uncertified fixtures; real offline reembedding for positive fixtures.
- Rust1.94.1; networknone2CPU/4GiB/jobs2; one private cargo/target; no hostHOME,
  credentials, socket, personal data or daemon mounts.
- Metrics: actual vector reads/clock calls/canonical bytes+G, exact edge outcomes,
  full receipts, strict reopen, real metadata-writer interleaving, test results.
- Rerun: python3 docs/evidence/certified-standalone-graph/run.py before before001
  on the recorded baseline, then focused/full with fresh labels on corrected
  source. Clean only own package on a source-root switch; preserve executable hash.

## Variants
| Variant | Change | Attribution |
| --- | --- | --- |
| before0012508 | Actual trait probes | Compilation errors, no executed proof |
| before002a930 | Two test construction corrections | Genuine Semantic failure |
| after00357e8 | Three-line production admission and controls | Six boundary tests pass |
| focus00457e8 | Existing producer/core filters | 65 executions pass, overlapping filters |

## Results
| Variant | Metric | n | Notes |
| --- | --- | --- | --- |
| before002 | Semantic bypass | 4 API/cap scenarios | API1/2 x cap0/1;3reads each; cap1 commits/1clock |
| before002 | Structural control | 6 API/mode scenarios | Entity/Temporal/explicit pass, replay/reopen retained |
| after003 | Semantic rejection | Same4 scenarios | VectorCertificationRequired;0read/0clock/unchangedG+bytes |
| after003 | Boundary suite | 6 tests | Certified edges, corruption, exactG/canonicalns, structural, missingowner |
| focus004 | Focused filters | 6+20+39 executions | All pass; not65distinct new tests |

## Log
### 2026-11-15
- Preserved before001 compile errors for Tag construction and one Explicit wrapper.
- before002 binary7b37550124bdde9465807fd4e3c31a59ca3e1e99b82346b2fd6308e2734e0446.
- after003/focus004 binaryf43a97eb6499ad861e0b5dcb1714f2ebc00ee971629016d705bde6f43f44f8d7.
- Feynman source-ACKed six exact merge resolutions before broad validation. Both
  parent publication/cancel suites remain, with175 tests moved into a separate
  file under their original module identity. Source ACK is not runtime proof.
- Full005 on57e8:1436lib+5CLI+10examples pass,2ignored; allfeatures/alltargets,
  fmt0/clippy0warnings;1257.03slib;390sourcefiles unchanged. No doctests run.
- Parent175a54 one-line accepted-socket test correction joined as90d6 afterfull:
  2CLI/fmt/clippy pass; no src/Cargo/examples delta or redundant full rerun.
  Final-head CI remains required after publication.
- Full peak4,055,203,840bytes,0OOM; after003 reached4GiB/reclaim1558,0OOM.
- All owned containers exited. Repository evidence preserves failed/passing
  source/executable/command/log hashes; compact HOME copy only, no raw archive.

## Findings
- Actual Semantic cap0 was an unsupported success, not merely excess work.
- The guard rejects before vector reads and does not require an active provider.
- A metadata-only writer can commit while the engine write guard is held; the
  existing final selection G check rejects its stale candidate plan.
- Explicit graph intent stays structural and replays its original full receipt.

## Conclusion
- Decision: use the existing certification authority and exact canonical read
  transaction; do not add a second parser/cache/proof version or public API.
- No ordinary startup, query, full-store CAS, latency, retrieval-quality or
  power-loss claim. Counts measure reads only; process exits are process exits.
- Keep raw scratch protected until publication/results, then central cleanup
  owns deletion. Compact repo/HOME records suffice; no raw archive required.
