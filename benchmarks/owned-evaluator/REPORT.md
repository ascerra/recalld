# Same-core private evaluator pipes

Measured runtime `6ba411ecc0d0333177127f81fc83d76cfe816d4a`, branch `impl/owned-evaluator`, actual prerequisite
[coherent payload PR96](https://github.com/calebevans/recalld/pull/96) seal387c711.
No public actor/scorer, catalog/batch/policy integration or broad ancestry was
added for convenience. The shared production direct-MCP factory was moved into
the library and is consumed by both ordinary CLI and this bench-gated example.
One existing Recalld owner supplies public actor MCP and private evaluator pipes.

## Exact validation

Frozen source passed1000 library+2CLI+7runner tests,6focused payload tests,
fmt and clippy(exit0 with warnings),9ordinary public-v1 episodes, and the new
actual endpoint process probe. The public episodes retain54returned-only
exposure cases,144fixture provider requests and9expected offline misses.
The private probe separately passed3fresh processes and7launcher/framing
negatives: duplicate descriptor, actor stdin descriptor, wrong direction,
non-FIFO, unchanged existing root, oversized frame and unterminated JSON EOF.

Each fresh private run records38request/response frames, including15explicit
control errors and8successful barriers. Public initialization/discovery never
exposes private methods. Realstore/provider latch, false search and physical-byte
checks, true search, feedback, foreign/future target rejection/reclose, strict
GenerationRef/deadline/unknown-field rejection, offline miss and control EOF are
exercised. EOF exit durations are in controls-summary.json (about1.13–1.17seconds).
The pending-store probe holds actual provider work while sending a barrier at the
priorG, checks no early barrier/store response, then verifies publication at the
newG after release. It does not introduce a second owner or a test-only rebuild.

Command/source/input/output/binary provenance is in manifest.json and
source-binaries.json. Rust1.94.1 pinned recalld-ci image, network-none Docker,
2CPU/4GiB/private source/cargo/target, own package clean, serial crate builds;
no personal HOME, credentials or socket mounts. Exact private example binary
SHA256 `44ec62dc1d64c27b2499accb5fd5089df4f420ec402563d74d0718bf8f326c57`.
The first source73933a1 compiled and passed all7negatives, then the positive store
failed because the synthetic OpenAI response omitted required usage fields. That
failure, transcript and original binary provenance remain in failed-fixture-output
and its index. It was a fixture-schema failure, not demonstrated barrier failure.

## Interface and scope

See docsbench/owned-evaluator-WIP.md for the exact constructor and pipe schema.
The supervisor transfers distinct >=3 read/write FIFO descriptors, supplies an
explicit loopback fixture config and absent private data root, and alone retains
the evaluator pipe ends. This is a trusted isolated-process diagnostic, not a
security sandbox against arbitrary same-user code. Frames are strict newline-
terminated JSONL, maximum16KiB. Control EOF ends the child; interrupted action
without an ACK cannot be scored as a no-op or recovered by generic retry.

Capabilities expose only capabilities/inspect/barrier. Inspect gives actual
committed GenerationRef and logicalHash:null; it is not readGeneration. Barrier
returns the existing owned-core readiness observation with cooperative monotonic
deadline, not a retained freeze. Unsupported snapshot/reset/advance/sweep reject.
Snapshot, reset, controlled clock/IDs/RNG and logical-hash flags remain false.
No native/public-v2 promotion, deterministic canonical replay, semantic quality,
fullA6 completion or release/performance-gate claim. Three fresh runs are not
reset replays. The current source's nonempty-neighbor exact-count fixture and
stronger get/similar payload view remain separate follow-ons. No paid models,
external dataset downloads or training.

All source/raw output is archived; forensic stores are not validated snapshots.
No cargo cache, target or executable is committed. Next work is the helper's
actual serialized supervisor consumer, owned canonical checkpoint/freeze/reset,
shared clock and separately scoped ID/RNG source plus deterministic ordering.
