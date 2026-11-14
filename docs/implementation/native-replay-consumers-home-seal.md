# Native replay consumer home evidence seal

Runtime `75bc809d048c4762ef56ae36d08d2395b482870b`, tree
`cd5fabb4f54754ca5fe8d730ea9504eea96afd6d` passed 1093 library tests,
two CLI tests, five additional focused checks, formatting, and all-target Clippy
(exit 0, 860 library / 919 test warnings, including 831 duplicates).
Four doctests remain ignored. Full library run: 529.04 seconds.

Repository evidence commit: `723a2a866b3435773e6dc3729a54b152f600732c`.
The 18 evidence files plus consumer implementation document were copied to:

`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-replay-consumers-75bc809`

Every copied file was SHA256 verified. The dated entry was appended to the actual
`Experiments/Recalld correctness and retrieval research.md` notebook.

- COPY-MANIFEST SHA256: `43b8fb4b4fc491f5760ecf1b51337d2376d2697800fc478eed38de0dbb3df567`
- Final evidence manifest SHA256: `06e281d7e53f8b6aebb68270e92ceff5fe65338c46ffc945f6c4b4024204948c`
- Final raw log SHA256: `898bcddaeb72fcc0745fab079735e42f1a0ef9091163a9151daebc41f877f601`

Earlier failures retain their original source, executable, and log hashes.
This seal changes no Rust sources and requires no repeated Rust test run.
Ordinary startup and public API activation remain a separate consumer integration
obligation, tracked in the startup admission plan.
