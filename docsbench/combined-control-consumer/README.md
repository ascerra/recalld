# Run the combined controlled diagnostic

Use the frozen image, installed tokenizer bundle and full source/build pins fromREPORT; no runtime network. Build only the existing CLI plus `owned_evaluator` and `score_mcp_episode` examples with `--features bench`. Supply compiled `RECALLD_A6_SOURCE_COMMIT`, `RECALLD_A6_SOURCE_TREE_SHA256` and `RECALLD_A6_IMAGE_ID`; a Python-only source change still requires matching scorer provenance. Use the explicit snapshot allowlist, not an ambient workspace mount. The legacy `host` convenience command is not this bounded validation driver and clones caches; do not use it for this track.

Inside the isolated2CPU/4GiB container, with immutable source/executables/assets and writable private output:

```sh
python3 /source/scripts/public-mcp-evaluation.py prepare \
  --track serialized-mcp-combined-control-synthetic-diagnostic-v1 \
  --binary /target/debug/examples/owned_evaluator \
  --scorer /target/debug/examples/score_mcp_episode \
  --build /output/build.json --context-assets /bundle \
  --context-profile boundary --out /output/inputs
python3 /source/scripts/public-mcp-evaluation.py run \
  --binary /target/debug/examples/owned_evaluator \
  --scorer /target/debug/examples/score_mcp_episode \
  --source-root /source --context-assets /bundle \
  --manifest /output/inputs/manifest.json --out /output/cohort
```

`--context-profile default` selects the4096 evidence-token cap; `boundary` selects128. Both count the complete rendered BPE context, including fixed overhead, under their separate full-context caps. A named `--combined-variant middle-cache-miss` is an explicit negative fixture and must fail, preserving the complete roster without aggregates. `gold` and `future` are actor-boundary canaries, not alternate quality arms.

Full actual bounded proof uses `scripts/test_combined_mcp_episode.py` with `--source`, `--binary`, `--scorer`, `--build`, `--assets`, `--out`; [actual command receipt](evidence/actual-command.json) records the host/container boundary. Apply host and inner deadlines with owned-container retirement. No source mutation, retry loop, dataset/model download or personal store is needed.

The new reader admits traceSchemaVersion2 only for this named track. Old profiles remain version1. Query ReadView, general reset/RNG, model-token identity, answer quality and cost remain unavailable. Inspect/barrier evidence is private evaluator data; actor receives only current question and public outcomes.
