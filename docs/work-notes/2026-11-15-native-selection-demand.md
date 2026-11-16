---
type: experiment
status: completed
project: recalld
started: 2026-11-15
---

## Question

Can actual native single/batch consumers avoid unrequested candidate-vector work
without changing insertion validation, selected outcomes or durable proofs?

## Setup

Direct PR158, branch `impl/native-selection-demand`, plan19428d78. Feynman ACKed
the capture and two caller hunks; no shared certification/codec changes. Synthetic
temporary stores, actual owned consumers and completion, network-none Docker
Rust1.94.1, 2CPU/4GiB/jobs2, one private cache/target. See the
[report](../evidence/native-selection-demand/REPORT.md) for exact source and logs.

## Results and attribution

- Initial677: one real single-consumer excess-read failure; batch fixture rejected
  unsupported `createdAt` before selection. The initial progress report incorrectly
  attributed both to excess reads; corrected and original failure retained.
- Corrected BEFOREf342: both actual consumers pass edge/reopen/replay assertions
  then fail only excess-read counts. Off/entity/temporal each scan1/read2.
- Final runtime36bd with test tail6b7f: focused9 pass. Actual off scan0/read0;
  entity/temporal scan1/read0; semantic/advisory scan1/read2 with unchanged outcomes.
  Corruption, uncomputed-score, numeric/profile and reserved-child controls pass.
- Full frozen6b7f: 1,336 library pass/0 fail/2 ignored, 2 CLI pass, 4 doctests
  ignored; fmt/clippy exit0 with warnings. Final cgroup peak3,982,888,960 bytes,
  no OOM. Source/binary hashes sealed in the report; no extra full run for docs.

## Decision and limits

Use frozen intent to compute only requested auxiliary channels. Insertion itself
retains profile/numeric validation. This is a work-count result, not a latency,
throughput, certification, native activation or full-spec acceptance result.
Future certification integration still requires the owner's validated interface.

## Resources

No raw archive created. Active raw remains in this worktree's `.validation` until
publication/results permit central cleanup. Corrected-BEFORE registered worktree
and tracked source/reports remain protected. One private cache/target is reused;
owned package clean and per-source executable hashes prevent BEFORE/AFTER reuse.
