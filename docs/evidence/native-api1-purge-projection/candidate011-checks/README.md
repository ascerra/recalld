# Frozen checks011: formatting and all-feature Clippy PASS with warnings

Exact90938aa5 source snapshot. `cargo fmt --all -- --check` exit0, and
`cargo clippy --locked --offline --all-features --all-targets` exit0. Each command
had its own600s timeout. Container03:30:12Z to03:30:33Z2026-11-16; Clippy20.40s.
Warnings include the existing large-error/dead-code/type-complexity categories;
this is not warning-free lint or all-feature test execution. No unrelated cleanup
or production edits followed the full run. Source re-hash passed. Same offline
2CPU4GiB/jobs2/read-only-source setup; memory.max0/OOM0. See exact command, hashes
and resource files. Full default all-target execution is a separate evidence set.
