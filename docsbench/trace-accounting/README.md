# Synthetic public MCP trace/accounting diagnostic

This track extends the existing #90 actor/runner on its direct published source.
It records each actual RPC attempt and outcome/error as an append-only trace,
with exact serialized client buffers and observed request durations. The supervisor
flushes an attempt before consuming its outcome. It does not claim that flush
preceded remote mutation, or that a flushed file survived power loss. Interrupted
operations remain unknown and are never retried or relabeled rolled back.

Inside the isolated offline container with matching source/executable pins:

```sh
python3 scripts/public-mcp-evaluation.py prepare \
  --track serialized-mcp-accounting-synthetic-diagnostic-v1 \
  --binary /target/debug/recalld --scorer /target/debug/examples/score_mcp_episode \
  --build /output/build.json --out /output/inputs
python3 scripts/public-mcp-evaluation.py run \
  --binary /target/debug/recalld --scorer /target/debug/examples/score_mcp_episode \
  --source-root /work/source --manifest /output/inputs/manifest.json \
  --out /output/episode
```

The manifest requires the exact independent accountingVersion1 policy. The
existing manifest/sourceMapping/trace versions remain1. Old track invocation and
fixture semantics remain unchanged; frozen studies continue using their original
source and executable pins. Unsupported tokenizer, answer-context-cap, cost,
sampling or control claims reject during preflight.

`trace.jsonl` contains attempt/outcome/error/incomplete events with decimal-string
counts, observed public response order/scores and source references. The actor sees
only current-step inputs and public outcomes. The supervisor owns trace run/group
context; evaluator labels/future questions are excluded from actor code, inputs,
environment and descriptors. Unavailable receipt/ReadView/readiness/reset facts
remain unavailable; no private-store inspection fills them in.

`evaluator/accounting.json` reconciles every observed external episode RPC and
provider record. Client byte counts come from actual serialized buffers and write/
read results. Failed writes with unknown progress stay unknown. Provider counts
refer to observed requests and HTTP bodies, not framing or backend operations.
Trace provider references use the delivery window, which can include startup and
shared work, rather than inventing exclusive per-RPC attribution. Sum provider
observations once in the episode summary. Do not sum nested durations or treat
client observations as server/provider timings or performance evidence.

Synthetic provider usage=0 is protocol fixture data, not measured tokens.
Tokenizer/context budget/production cost/backend durability/internal retries and
HTTP framing bytes are explicit null/unavailable. Zero paid calls is scoped to
this offline synthetic replay invocation; it does not estimate production cost.
The real tokenizer/answer-context/cost consumers and other A6 gates remain work.

Validation entrypoints: `scripts/test_trace_accounting.py` for focused unit and
preflight checks; `scripts/check_trace_accounting.py` for actual production MCP
baseline/label-perturbed/old-profile episodes and eight fail-closed manifest
negatives. Injected pipe/process failures are labeled separately from real runtime
observations. Label comparisons exclude expected environment/cache identity and
observed time/independent UUID differences; actual requests and provider vectors
must still match. No production Rust source, paid model, dataset, activation,
full-A6 or native readiness claim is part of this slice.
