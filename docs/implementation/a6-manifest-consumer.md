# Manifest-consuming internal diagnostic

Parent: frozen PR75 `8ecdd5a`; no public-runner, tracker or coherent-core ancestry is imported. This is the independently named `manifest-internal-diagnostic-v1` track. It consumes the real canonical insertion/startup and QueryEngine paths already used by PR75. It is not the A6 public actor, a dataset adapter/scorer implementation, a semantic-quality result, or deterministic canonical reset.

`scripts/offline-evaluation.py` creates an immutable `manifest.json`, `actor-inputs.json` and evaluator-only `evaluator-labels.json` from its explicit synthetic arguments and actual source/image hashes. The mutable supervisor status/output hashes move to `diagnostic-run.json`: the input manifest never hashes itself. Synthetic revision means the SHA-256 of the actual actor artifact, not an invented upstream revision. External datasets, licenses, model revisions and tokenizers are not used or inferred. All units belong to one declared regression group, with no training/split claim.

The actual example requires `--manifest PATH --out NEW_DIRECTORY`. CLI corpus overrides are removed so the validated plan is authoritative. It validates all input fields and files before creating the output directory or store. Unknown/unsupported versions and fields, unsupported public-v2/barrier/reset/snapshot/clock-advance/deterministic-ID/RNG requirements, mismatched source or artifact hashes, injected actor labels and invalid source/evidence references fail closed. The supervisor validates numeric plan arguments before creating scratch. Supervisor preparation artifacts are distinct from executing an episode.

Run the real supervisor from this checkout:

```sh
python3 scripts/offline-evaluation.py \
  --cache /tmp/recalld-PRIOR-SCRATCH/cargo \
  --build-cache /tmp/recalld-PRIOR-SCRATCH/target \
  --records 100 --dimensions 16 --k 5 --warmup 1 --samples 3 --runs 3 --seed 7
```

It copies registry/dependency caches to a fresh private target, snapshots only allowlisted source, and uses the pinned existing image with network disabled, 2 CPUs, 4 GiB, an isolated home and no credential/socket mounts. The Rust example consumes the same manifest in all three fresh processes. Old studies retain their historical scripts/source hashes; an old direct example invocation fails rather than silently measuring this new track. Do not rewrite historical reports or compare their timing results as equivalent measurements.

The manifest pins independent harness versions at 1 and the actual **legacy internal** runtime selection: contract/retrieval/canonical control/record versions 1, receipt/request-hash 1, legacy lifecycle, baseline ranking, RIF off. Backup2/export1 are format pins, not exercised snapshot/export operations. Derived implementations are pinned by actual source hashes, not invented format numbers. Native/public-v2 and coherent controls remain unavailable. A fixed lifecycle ManualClock and seeded synthetic vector generator do not control random store/operation UUIDs. Real receipts and all semantic differences remain retained without new identity normalization.

`source-mappings.json` maps actual committed memory UUIDs to complete synthetic source identities, ordinal, role, source timestamp, content hash, transformation and artifact revision. The evaluator source ledger emits seen/attempted/accepted or rejection events; the original raw fixture, record/control snapshots, replay results and receipt ledger remain intact. Schema-bearing records are not synthesized. Query source metrics use the validated evaluator sidecar and the UUID mapping, not an implicit UUID-to-label scoring shortcut. These metrics describe synthetic evidence units only, never upstream dataset/scorer semantics or proposition preservation.

`trace.jsonl` contains version1 actor-side action/outcome records, including warmups, full insertion receipts and an expected failed query. Evaluator metrics and historical diagnostic traces live under `evaluator/`. `readGeneration` is null and readiness explicitly unavailable; no metadata generation is substituted for a coherent read view. Tokens, snapshots, models and reward are unavailable, not fabricated. `actor-replay-cache.json` contains only actor-query vectors. Its key binds the complete query text, namespace, role, profile/prompt/tokenizer state and frozen environment hash. A missing entry errors through the actual QueryEngine; no live/generative fallback exists.

The exact config/manifest/trace interface is confined to `examples/support/evaluation_manifest.rs`, consumed by `examples/offline_eval.rs`; it adds no production owner or schema framework. Halley owns future coherent-core/barrier/reset/clock/RNG capabilities. Enabling one requires actual integration and a revised supported manifest capability contract, not toggling a claim in the output.

Validation: pending. The runner executes actual-consumer preflight rejection goldens, the original source-boundary and replay checks, provenance/receipt accounting, and evaluator-canary checks on actor inputs, outputs, trace, mappings and cache. Raw failed attempts and source/executable hashes will be retained with the final evidence; no passing runtime result is claimed by this implementation note yet.
