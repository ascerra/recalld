# Native store producer validation

Frozen runtime: `4c151f8b51394babef938194bd8ecf7eee60f6c0`.
Actual parents: PR91 `b033c9534f26886e8ea31a871a38619b3e1afd14` and
corrected PR101 `51da3cade35979a3200f976643bdb965dcb1498b` (includes PR97).
The evidence seal changes documentation only. PR91 remains unchanged.

## Final results

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --offline --all-features` | 1045 library tests passed, 620.36s; 2 CLI integration tests passed; 4 doctests ignored |
| `cargo clippy --offline --all-features --all-targets` | Passed with warnings, 22.27s |

The minimal ancestry has no offline_eval example; none was attempted. Tests use
synthetic tempstores. Docker image `recalld-ci:1.94`, immutable image ID
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Rust 1.94.1, network none, 2 CPUs, 4 GiB, CARGO_BUILD_JOBS=2, private cargo/target,
read-only source mount, no HOME/credential/socket mounts. Root cargo credential
files were absent. Exact sequential cargo commands are in `final-check.sh`.

```sh
docker run --rm --name recalld-native-producer-final --network none --cpus 2 --memory 4g \
  -e RUSTUP_TOOLCHAIN=1.94.1 -e CARGO_BUILD_JOBS=2 -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target \
  -v /tmp/recalld-implementation/native-store-producers:/work:ro \
  -v /tmp/recalld-implementation/native-store-producers/.validation/cargo:/cargo \
  -v /tmp/recalld-implementation/native-store-producers/.validation/target:/target \
  -v /tmp/recalld-implementation/native-store-producers/.validation/final:/evidence \
  -w /work recalld-ci:1.94 bash /work/.validation/final-check.sh
```

## Provenance and retained failures

The original pre-run manifest covers 229 Rust/Cargo files. During the frozen run,
the manifest was expanded from immutable Git runtime bytes to include six embedded
fixtures. All 235 files matched the working checkout, and the post-run manifest
matches both runtime and the original pre-run subset. No source changed during
validation. `binaries.json` contains hashes of all six executed test binaries.
Library SHA256: `7a63b040450c98c13177c002c64dba5d0b6a5b61eb0d74c1ca165904d1821f94`.
The private target never switched source roots. Before its first build, own-package
clean removed inherited recalld build outputs while retaining dependencies.

Retained failures are not hidden or counted as passing proof:

- Initial coordinator fixture: 1 passed/1 failed. It asserted storage availability
  before the short admission lookup completed; the corrected fixture waits for
  the actual publication lease. Failed source archive and binary hash retained.
- Selection filter run 7: zero tests, a filter mismatch, not evidence of coverage.
- Producer run 12: compile failure accessing a private field; fixed through a
  narrow read-only accessor. No test executable claimed for this compile failure.
- Producer run 13: 3 passed/1 failed, keyed tombstone fixture omitted storeId.
  Failed source archive and executable hash retained. Corrected run 14: six
  producer tests passed in 7.84s, before this final full run.

Earlier checkpoints: 61 native-focused tests at 9263920; 82 batch-filtered tests
at 38bde43. Final shipping-tree proof is the full run above. Actual process exits
exercise before/after metadata and acknowledgement boundaries; they do not prove
power-loss durability. Provider latches exercise publication failure after
reservation, exact current child receipts, replay with zero fresh provider calls,
unknown-commit recovery, and accepted descendants during close.

## Artifact preservation

60 files were copied and SHA-verified in actual HOME work-notes:
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-store-producers-4c151f8`.
Full COPY-MANIFEST SHA256:
`67f48ef4c23760ece01496470bea4a4086b9d7051169f2cfa5cb8fc481318646`.
`ARTIFACT-MANIFEST.json` lists exact sizes and hashes. The actual work-notes
experiment received an appended dated entry. Large source archives and full logs
remain in that verified artifact directory, outside the repository.

## Scope

Private batch version3 maps to native contract2. Historical versions1/2 retain
hash/proof/child-ID meaning. Canonical links and numerical advisory are frozen
into same-insert proof; current advisory text is hydrated only from frozen IDs
after readiness. Public version3 admission remains gated for the separate A4
consumer/activation join. No public contract3, whole-store CAS, complete native
serving, reset or whole-core barrier claim. Get/similar payload work remains a
separate child of PR96. Future PR96 integration must preserve the distinction
between reservation publication failure and post-child publication failure.
