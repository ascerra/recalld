# Actual native sweep invocation

Use the frozen source's `owned_evaluator`, `native_prefix_fixture` and
`score_mcp_episode` examples, compiled with feature `bench`. The scorer's compiled
source commit, source manifest hash and pinned image must match `build.json`.
The retained validation driver records the exact offline Docker invocation,
immutable source hashes and executed binaries. A previous track's manifest or
binary is not compatible provenance.

Inside the isolated environment, after compiling that source:

```sh
python3 scripts/public-mcp-evaluation.py prepare \
  --track serialized-native-sweep-synthetic-diagnostic-v1 \
  --binary /target/debug/examples/owned_evaluator \
  --scorer /target/debug/examples/score_mcp_episode \
  --build /output/build.json --out /output/inputs
python3 scripts/public-mcp-evaluation.py run \
  --binary /target/debug/examples/owned_evaluator \
  --scorer /target/debug/examples/score_mcp_episode \
  --source-root /work/source \
  --manifest /output/inputs/manifest.json --out /output/episode
```

The supervisor validates artifacts before creating output/provider/store state,
builds the real synthetic migrated prefix, launches `--native-prefix
--native-sweep`, and owns the inherited private control pipes. Restore launches
the same binary with the exact checkpoint manifest/digest. The actor gets only
stdio and its current public request; it has no control pipe, future schedule,
source factory or evaluator labels.

Results are in `episode/result.json`; each suffix retains `sweep-reports.json`,
`phase-hashes.json`, `step-receipts.json`, `outputs.json`, `suffix.json`, source
mappings/events, actual scorer output and trace. The trace currently records
post-step barriers, not a response ReadView; this slice does not close that A6
trace gate. Source events describe acknowledged migrated-prefix provenance,
not live public ingestion. Compaction remains unsupported in this track.

Old published native-prefix, legacy sweep and reset studies retain their frozen
sources/manifests. They must not silently run this new example binary. Same-source
regressions, when performed, receive newly generated and explicitly named inputs.
