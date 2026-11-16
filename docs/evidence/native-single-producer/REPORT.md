# Native single producer validation

Runtime: `4d9bc7f0d415b4ef75a2317f7020f5f21405cf79`.
Parents: actual PR106 `fe99020926eb81a6994c7e9214ad4623180eeed1` plus actual
PR96 `387c7118864040c68d7d28c44fb2600ade520a45`, local join
`3f730cd124ba16b57644a73bd2337b6b9a8e84e7`. Bounded fixture correction
`c3006f55c4becdf85c238b59311782213c3559d6`.

## Isolation and commands

All execution used synthetic temporary stores inside Docker, `--network none
--cpus 2 --memory 4g`, `RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_BUILD_JOBS=2`, private
`.validation/cargo` and `.validation/target`, and source mounted at `/work`.
No home, credential or socket mounts. Image `recalld-ci:1.94`, immutable ID
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
The copied private target was cleaned with `cargo clean -p recalld` before
first compilation. This target never switched source roots. No source was
edited during a validation run. Tests establish process-exit recovery, not
power-loss durability.

Final command: `cargo fmt --all --check && cargo test --offline --all-features
&& cargo clippy --offline --all-features --all-targets`.

## Retained failures and focused results

- Join source `3f730cd`: `cargo test --offline --all-features batch -- --nocapture`
  finished **82 passed, 1 failed**, 76.03 s. The existing native batch fixture
  corrupted the derived index before reservation publication and incorrectly
  expected child `Ok` instead of `NotAttempted`. Library executable SHA-256:
  `6e4b73e7fc64414f6c7da32b1d650bfcca67125895480f2f34c6603cb0d995e7`.
  Full failure log and source manifest are retained here.
- The first attempted rerun could not access Docker from the sandbox. It ran no
  tests. The authorized escalated retry used the same isolated settings.
- Only the matching supplied fixture patch was applied: exact parent receipt,
  zero provider calls, no child mutation before reservation readiness; then a
  real provider latch isolates a post-child publication failure with exact
  child receipt, one provider invocation and exact replay. The corrected test
  passed **1/1**, 3.70 s; coherent payload checks passed **6/6**, 3.16 s.
- First single-producer compile failed on two test assertions referencing a
  nonexistent receipt `scope` field. No tests ran. Assertions now look up the
  exact receipt in the namespace-scoped receipt table.
- Seven initial single tests passed, 12.81 s. Final focused command
  `cargo test --offline --all-features native_single_ -- --nocapture` passed
  **9/9**, 12.45 s, on runtime source `4d9bc7f`. One test is the subprocess entry;
  its parent exercises four actual process exits: before metadata commit,
  after metadata commit, after edge commit, and after canonical acknowledgment.
- The focused binary SHA-256 is
  `c0520c4af6cee7b5819053e40e862cc6472110a5af684bdbf2ad98eae45233ce`.

The nine checks also cover provider-free historical replay, request conflicts
for scaled/signed-zero vectors, requested-empty versus disabled advisory,
exact committed receipt on failed publication and reopen, close while provider
and graph work are pending, stale generation rejection after provider work,
injected I/O ambiguity after a real commit preserving operation identity/unresolved work, and
namespace/catalog/foreign-lineage controls without provider calls.

## Final frozen-source result

Final command exited **0**: format check passed; **1,075 library tests passed**
in 604.16 s; **two CLI integration tests passed** (2.55 s and 1.40 s); four
doctests were ignored. Clippy completed successfully with warnings in 22.78 s.
All **242** tracked runtime/test/Cargo source hashes matched after execution.
The final library executable SHA-256 matches the focused executable:
`c0520c4af6cee7b5819053e40e862cc6472110a5af684bdbf2ad98eae45233ce`.

This seal adds evidence only after the tested runtime commit. Public native
activation, wire profile-ID resolution, native exposure, owned-ID allocation,
and the separate FTS content-correctness follow-up remain independent work.


## Actual work-notes copy

Fourteen files, including the runtime source archive, were copied and verified at
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-single-producer-4d9bc7f`.
`COPY.sha256` SHA-256:
`75198f1330d9113408e433cbbf6bd971f09e35d27a733760b600409518ef3f5b`.
The actual HOME experiment note was appended with counts, source/binary identity,
failures, and remaining scope. The copied report precedes this copy-confirmation
paragraph; its own bytes are covered by that manifest.
