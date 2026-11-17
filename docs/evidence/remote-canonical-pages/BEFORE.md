# R2 actual Remote page BEFORE

Source `c094ac1f804481d84b0a49ac8205ef64dfb4e471`, composed from exact #112
and R1 as recorded in the implementation plan. No production page-transport
correction is present. Two actual adapter/Unix-socket regressions ran offline.

Result: **0 passed, 2 failed**, exit101. The owned canonical list succeeded with
the expected row and total; Remote returned `Unavailable`. Both owned namespace
and tag catalogs succeeded; both Remote calls returned `Unavailable`. The tests
finished the socket task and asserted unchanged canonical records/control before
the intentional success assertions failed. This is executed evidence of the
missing Remote consumers, not a malformed-peer or public2 activation claim.

Command: `cargo test --locked --offline --all-features --lib prepared_pages_remote_ -- --nocapture`.
Compilation completed in97seconds; tests ran0.17seconds. Compilation emitted59
warnings. No full suite, formatting, Clippy, or schema result is claimed here.

Container `recalld-huygens-canonical-pages-001` used image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Rust1.94.1, networknone,2CPUs,4GiB,jobs1. Only owned source/cache/target/evidence
were mounted; no home, credentials, personal data, or daemon socket. A package
clean in the owned target preceded this source-root change and exited0.

All465 immutable source input hashes matched after execution. Library executable
SHA256: `5c4b2d24c56fa2e537a18d1178b64ea264aac2e6ff98a07474073cea8d8ca5a6`.
The623865952-byte executable was preserved before any subsequent target rebuild.
Cgroup memory peak4294967296bytes; max events24919,oom0,oom_kill0. This is reclaim
pressure, not OOM. The exited container was removed; no raw data was deleted.

Protected raw root:
`/tmp/recalld-huygens-active-validation/canonical-pages-001-before` contains
source manifest, immutable source, command, log, exit/resource facts, verified
executable and source recheck. The central cleanup owner holds this failure.
One existing private cargo cache/target was reused without cloning.

Feynman source-reviewed plan3dabb6d8 and the actual backend cursor invariants.
His context follow-up requires validating remote compatibility before any
recomputation, preserving carried Unknown over outer Configuration, and keeping
the backend as the sole authority for canonical hashes/anchor existence.
