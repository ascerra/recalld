# Private owned evaluator constructor (validated bounded consumer)

The `owned_evaluator` example, gated by the existing `bench` feature, constructs
one ordinary Recalld instance from an explicit config and a fresh data root. It
uses the same extracted `direct_bridge` factory as the production direct MCP
command. Actor stdin/stdout run the existing MCP JSON-RPC transport. No daemon
fallback, alternate ingestion path, evaluator tools or second engine are added.

Invocation:

```
owned_evaluator --config CONFIG.toml --data-root FRESH_DESTINATION \
  --control-read-fd REQUEST_PIPE_READ_END --control-write-fd RESPONSE_PIPE_WRITE_END
```

The supervisor creates two distinct Unix pipes and transfers only the indicated
ends to this child. Both descriptor numbers must be at least 3, open pipes, and
have the expected read/write direction. Its actor subprocess receives only MCP
stdin/stdout handles, never these evaluator handles. The supervisor must allowlist
the environment, give the child a private HOME/TMPDIR and run in network-none
isolation with no personal paths/credentials/socket mounts.

The launcher accepts only an explicit HTTP loopback OpenAI-compatible fixture.
The only key name is `RECALLD_EVAL_OFFLINE_KEY`, with literal synthetic value
`offline-fixture`. It disables autonomous sweeps. The data directory must not
already exist, and is created with mode0700. No configuration discovery occurs.
An offline replay miss is a request failure, never live-provider fallback.

This is an isolated trusted-process diagnostic; pipe separation is not a security sandbox.

Control frames are newline-terminated JSONL, at most16KiB. Oversized frames and
valid JSON without its terminating newline at EOF reject without dispatch:


```
{"id":1,"method":"inspect","params":{}}
{"id":2,"method":"barrier","params":{"target":{"storeId":"UUID","generation":"1"},"deadlineMs":30000}}
```

Responses carry the same ID and exactly one `result` or `error`. `capabilities`
requires empty params. `inspect` returns actual `committed` GenerationRef and
`logicalHash:null`; this is not a coherent read-generation certificate. `barrier`
consumes the owned tracker/readiness implementation and validates lineage, target,
actual durable pending state and all required view stamps. Parameters and unknown
fields are strict. Deadlines are cooperative; OS I/O is not a hard real-time bound.
The supervisor's independent timeout must invalidate interrupted episodes.

`logicalHash`, snapshot, reset, controlled clock, IDs and RNG remain unavailable.
Unimplemented methods fail closed. Public contract remains named legacy-v1.
Control EOF/error ends the child; runtime shutdown waits at most one second for
blocking workers. An exit is never a snapshot certificate or permission to score
an action whose ACK was not received. Pending interrupted actions remain invalid.

`scripts/test_owned_evaluator.py` is a narrow actual pipe/constructor probe with
three fresh processes, real public store/search/feedback, returned generation
barriers, physical false-window byte checks, private-method isolation, malformed
control inputs, duplicate/stdin/reversed/non-FIFO descriptor rejection, unchanged
existing-root rejection, oversized/unterminated-frame rejection, explicit offline
misses and EOF shutdown. It is not the dataset
supervisor/scorer, owned by the separate A6 manifest/public-actor work. Its fresh
runs are not deterministic reset/replays. Exact source, source/binary provenance and results are in
[the frozen report](../benchmarks/owned-evaluator/REPORT.md).

Initial source73933a1 compiled and passed all seven launcher/framing negatives,
then its positive store failed because the synthetic OpenAI reply omitted required
usage fields. Raw failed output remains `/tmp/recalld-owned-evaluator-test-jxa9w_2e/output`.
Corrected source6ba411e passed1000lib+2CLI+7runner+6focused payload+9public episodes,
3actual private-pipe processes and7launcher/framing negatives; fmt/clippy exit0
with warnings. The failed attempt remains immutable.
