# Recorded-answer synthetic consumer

New track `serialized-mcp-recorded-answer-synthetic-diagnostic-v1`, direct PR193. It executes the existing public MCP query, independently verifies the actual rendered/tokenized payload, acknowledges its same-core barrier, then invokes a current-only actor phase that consumes one pinned synthetic recorded answer. The role actor inherits neither public nor control descriptors. It receives no schedule, gold labels or complete cache.

This proves recorded quotation/citation provenance, not semantic answer quality or a model invocation. Answer quality, model token identity and production cost remain null. The fixture's modelRevision is explicitly null; no model or upstream golden is fabricated. Extraction/planning/judging/native writes/full A6 remain outside this profile.

Versions are separate: manifest1/sourceMapping1/trace2 plus closed roleTrace1. `trace.jsonl` retains RPC/context/barrier kinds; `evaluator/role-trace.jsonl` records role attempts and actual actor outcomes. Final proof binds both streams. Attempts flush before consuming an output, without a remote-mutation or power-loss claim. Interrupted outcome is unknown. Complete required-answer roster is mandatory; no successful intersection or miss-to-empty conversion.

`recorded_answer.key` hashes the domain plus length-prefixed canonical configuration/input JSON. Configuration pins runtime source identity, exact owned evaluator binary, embedding identity, clock/allocation/cache/RIF/graph settings, prompt/renderer/tokenizer and parser versions. Input is exact current question and rendered artifact. Full manifest/environment, cache artifact and E source identity are separately pinned: no self-referential hash. Evaluator labels/schedule/run identity are excluded from this declared role-config domain, never normalized out of actual actor inputs. Changed actual render/UUID/token causes a miss; there is no generation/rekey fallback.

Checked-in `scripts/fixtures/recorded-answer.json` freezes28 inputs from PR193 and synthetic quotation outputs before E outcomes. Output variants are named negative fixtures, not model attempts. Successful quote membership is not entailment. Old C fixtures and closed readers retain their semantics.

Historical frozen reproduction only: use executed source
`249ff40e8149743efcddcbe2dfcd100bb5171c4e` and the source, build and binary
pins in [REPORT.md](REPORT.md), including owned evaluator SHA256
`5f1f627715ed95cee714f992c4111ea5265e7fe8219d304aa230c0ed91e4e93a`.
The commands below are not a current-main smoke test. This frozen cache
intentionally rejects changed runtime sources or binaries; do not rekey it or
regenerate answers to make a later checkout pass.

The paths in the retained execution record describe that historical run.
Its disposable build/episode roots were removed after publication under the
retention policy. Reproduction requires restoring the exact pinned inputs and
binaries; their present availability or a byte-identical rebuild is not newly
proved here. With those prerequisites restored in an offline immutable
source/binary environment, tokenizer bundle and scorer build receipt:

```sh
python3 scripts/test_recorded_answer.py
python3 scripts/test_combined_mcp_evaluation.py
python3 scripts/test_recorded_answer_episode.py \
  --source /source --binary /target/debug/examples/owned_evaluator \
  --scorer /target/debug/examples/score_mcp_episode --build /output/build.json \
  --assets /bundle --out /output/episodes
```

Actual driver stops immediately if the first complete cohort differs from the exact prerecorded context. It never regenerates output after outcomes. The first cohort has7episodes/28answers; subsequent narrow variants cover miss/refusal/parse/citation and actor label/future isolation. Source/checker tests are not runtime proof. No full Rust suite is required for this Python-only consumer; compiled scorer/source provenance must still match. The historical validation results and failures are sealed in REPORT.md; this clarification adds no execution claim.
