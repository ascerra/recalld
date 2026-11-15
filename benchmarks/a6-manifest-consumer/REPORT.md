# Actual manifest consumer: retained validation

The actual PR75 internal runner consumes a validated manifest, separates actor replay inputs from evaluator labels, and errors on a real QueryEngine replay-cache miss. This is synthetic contract-diagnostic runtime evidence, not public/full-A6, semantic-quality, coherent ReadView or deterministic-reset proof.

Frozen parent: `8ecdd5a4b20c1f1555b98c84bd0771625e51c67d` (PR75). Validated runtime: `6ce05ab290437b39dce57b3e6cac92b505ef9cee`. No additional runtime dependency. The subsequent seal changes only documentation/evidence.

| Check | Observed result |
| --- | --- |
| Rust library / CLI / examples | 851 / 2 / 5 passed; offline_eval four tests also repeated focused |
| Python supervisor | 10 passed |
| Format / Clippy / release build | Passed; existing Clippy warnings retained |
| Actual executable manifest rejections | 41 rejected before episode output/store creation |
| Fresh real runner processes | 3 passed; each 100 accepted insertions, 300 source events, 117 actor actions, 10 cached vectors, canonical generation 104 |
| Real QueryEngine missing query | Expected error in every process; no generation/fallback |
| Evaluator-label perturbation | 117 actor actions and 10 vector associations unchanged; six evaluator Recall@5 results changed 1 to 0 |

The perturbation changes the evaluator artifact hash and therefore the full manifest/environment hash and replay keys. The checker compares vector associations under their recomputed keys and removes only the explicit replayCacheKey field when comparing actor actions. Actor input bytes, fixture, record states, replay results and source mappings are byte-identical. Original PR75 receipt comparison retains its existing store/operation-ID exclusions; full raw receipts remain archived. This does not establish deterministic UUID/reset behavior.

Exact executable SHA-256: `17260a6cb9eabd8080c7f869354586f257ed4535450c5cf69eb1198c7b3e7de4`.
Input manifest SHA-256: `e752677358bcd2376e5ee58293d7c4bcdb06cc4c08bb948a55e15dbd8074b568`.
Perturbed manifest SHA-256: `3b50fff8343bb70570f7a1f9c711e895905405db155e0ace2ac7dc8c2b700548`.
Pinned image: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.

The successful supervisor invocation used `--track manifest-internal-diagnostic-v1 --records 100 --dimensions 16 --k 5 --warmup 1 --samples 3 --runs 3 --seed 7`, registry cache `/tmp/recalld-canonical-harness-test-a1hpl2g2/cargo`, and build cache from the first attempt's private target. Both were copied into a fresh private run. Docker had network disabled, 2 CPUs/4 GiB, an isolated home, and no credential/socket mounts. The successful root and exact generated build script are indexed in artifacts.json.

The perturbation used the same frozen executable/source/image, mounted read-only, and `label-canary-check.py --binary /work/target/release/examples/offline_eval --manifest /work/output/manifest.json --baseline /work/output/run-0 --out /validation/output` in a separate private output mount. Its report records exact checker, executable and output hashes.

The initial `227f82d` attempt failed to compile because the installed sha2 digest does not implement LowerHex. It produced no episode. Nine Python checks and formatting passed before that failure; its logs and input hashes are retained, not counted as successful runtime validation. The corrected source was frozen at 6ce05ab and validated after the A5 sampling hold's explicit END.

Artifacts: failed-227f82d.tgz, passed-6ce05ab.tgz, label-perturbation-6ce05ab.tgz. They include all output/log files and generated build/check scripts, excluding bulky dependency caches, binaries and stores. artifacts.json lists every archived file's SHA-256, size and original scratch root; archive members were re-read and verified. runtime.patch retains the exact examples/scripts delta from PR75 to the validated runtime. SHA256SUMS covers this complete evidence folder without self-reference.

Source inspection establishes unsupported capability gates and ownership boundaries. The above executable checks establish only this actual legacy-schema-none internal diagnostic: native/public-v2/reset/coherent readiness remain unavailable. Deterministic scorer/ledger improvements are a separate child, not silently included in this frozen result.
