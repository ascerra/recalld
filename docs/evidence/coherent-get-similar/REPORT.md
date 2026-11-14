# Coherent get and similarity validation

Runtime `8a0d260e2bbf4f1129cee0b2ddd9ce6f7d56b293` on branch
`impl/coherent-get-similar`, direct PR96 parent
`387c7118864040c68d7d28c44fb2600ade520a45`. No producer, unrelated native or
broad integration ancestry. The final evidence seal changes documentation only.

## Results

| Run | Actual result |
| --- | --- |
| Initial full, c67795e | 997 passed, 9 failed, 434.19s; format passed; Clippy not reached |
| Corrected focused | Seven new by-ID tests passed, 5.58s; every exact prior failed case passed individually (nine additional passes) |
| Final all-features | 1007 library tests passed, 422.74s; 2 CLI integration tests passed, 2.65s and 1.54s; 4 doctests ignored |
| Final format | Passed |
| Final Clippy | Exit0 with warnings, 20.36s |

The original nine failures are listed in `failed-cases.txt`; their exact output
is in `before-failures.log`. The full raw log remains immutable in the artifact
copy. This was an actual overrestriction introduced by c67795e, not obsolete
fixtures: canonical-only MCP point get incorrectly demanded all four derived
view stamps. The correction retains actual control G and generation-guarded
canonical row/text copy, plus pending-effects admission under that guard. Ranked
and projected queries still require the pinned views. No failed assertion was
removed or weakened. True get awaits exposure completion and retains its full
receipt on failure.

Tests exercise actual REST handlers, MCP dispatch and framed daemon calls with
synthetic tempstores: scope assertion, final returned-only exposure, false-mode
state equality, one sampled clock, close/drain descendants, concurrent forget and
selected-text compaction, markers/missing/empty results, copied text after handle
replacement, postcommit error receipts and exact receipt equality after reopen.
No process-exit result is described as power-loss evidence.

## Reproduction and provenance

`final-check.sh` runs `cargo fmt --all -- --check`,
`cargo test --offline --all-features`, and
`cargo clippy --offline --all-features --all-targets`, sequentially. There is no
offline_eval example in this minimal ancestry; none was attempted.

```sh
docker run --rm --name recalld-byid-final --network none --cpus 2 --memory 4g \
  -e RUSTUP_TOOLCHAIN=1.94.1 -e CARGO_BUILD_JOBS=2 -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target \
  -v /tmp/recalld-implementation/coherent-get-similar:/work:ro \
  -v /tmp/recalld-implementation/coherent-get-similar/.validation/cargo:/cargo \
  -v /tmp/recalld-implementation/coherent-get-similar/.validation/target:/target \
  -v /tmp/recalld-implementation/coherent-get-similar/.validation/final:/evidence \
  -w /work recalld-ci:1.94 bash /work/.validation/final-check.sh
```

Image ID: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Rust/cargo 1.94.1. No HOME, credential or socket mounts. New private cargo/target;
own-package clean removed1991files/6.8GiB before the first build. The target never
switched source roots. Source remained frozen during each full run. All224
source/fixture files match pre/post manifests. Complete executed binary hashes
are in `binaries.json`.

- BEFORE library SHA256: `37df7c7e05dc83f527957124fc7471dc637fcaa96e47bd87d63181d46c488f2a`.
- AFTER library SHA256: `543a337205ca35e53c90c5ee51e006fdf77edd36bf37f8a90c8773a38895cd04`.

The first compile failed on generic error inference and a WorkError conversion;
no tests executed. A subsequent formatter read an incomplete source file through
the Docker mount while the host file was complete; its failure is retained. The
retry verified the container-visible file hash before formatting/building. That
mount observation is not a reproduced storage runtime failure. Earlier focused
runs passed3, then5new+6existing payload tests, then6new before the full run.

## Preservation and remaining scope

40 artifacts were copied and SHA-verified at
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-coherent-get-similar-8a0d260`.
Full COPY-MANIFEST SHA256:
`bf31e1f5d211b83311575261c35bff99bd9232617fa2965c0ee9418042d1de06`.
`ARTIFACT-MANIFEST.json` records every copied size/hash. The actual HOME experiment
note received an appended entry. Full raw logs and before/after source archives
remain outside Git in that verified directory; no build caches are committed.

Prepared query responses carry actual readGeneration/evaluated_at/original access
intent and owned payload. Legacy transport response shapes remain unchanged;
MCP point fetch's public readGeneration envelope is still an A4 join. Similarity
is explicitly `RetrievalSemantics::V1`, preserving legacy caps/ties. This is not
A2-v2 ranking, native exposure activation, public whole-store CAS, or whole-core
freeze/reset proof. Pascal owns the later configured native-exposure observer;
Huygens owns wire activation; Hume owns A2 selection. The accepted namespace-keyed
single producer is a separate next child, never a one-item catalog batch.
