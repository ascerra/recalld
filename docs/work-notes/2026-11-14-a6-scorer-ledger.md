# A6 synthetic scorer and turn-ledger consumer — 2026-11-14

Isolated child of actual PR84 seal586f02e, itself directly on frozen PR75. Plan sent main/Halley before edits. No core/reset/public integration or runtime ancestry added. Manifest/sourceMapping/trace1 remain independent of the new explicit internal track v2. Reuse existing runner and append-only events; no standalone scoring framework.

Synthetic hand-calculated cases cover perfect/empty/partial/duplicate/unknown and cutoff behavior, with separate zero-label and missing-annotation denominators. Ledger goldens cover accepted/no-op/rejected/truncated/pending/unseen and duplicate terminal rejection. Real runner verification recomputes from actual actor result UUIDs and committed source mappings. Runtime failure logs and source/binary/input hashes will be sealed after isolated validation.

Host formatting invocation failed because cargo is unavailable (`zsh: command not found: cargo`); no build ran on host. Formatting uses the pinned offline Docker image instead. Initial Python checks passed 11/11; their log is retained at /tmp/recalld-a6-scorer-python-initial.log.
