# Actual owned MCP supervisor consumer

Final measured source **c2ddf67605a1d6f3b9956fb4a67a0f40db0168a1** passes the
named `serialized-owned-mcp-synthetic-diagnostic-v1` actual runner. Baseline and
changed-label episodes each complete **19 MCP actions, 10 actor deliveries, 10
loopback embedding requests (one expected replay miss), 5 source mappings, 15
source-ledger events, and 11 actual ready barriers**. All four required views
match the barrier's committed generation, pendingWork is zero, and the following
inspect agrees. Nineteen malformed/unsupported manifests fail before episode
output/provider/store creation. Thirteen Python checks and three scorer tests pass.

## Exact composition and inherited validation

Direct merge parents are published92 `0084ad30d768747b13c2815d838be8f0feaee284`
and published99 `534d9be9c4df54a95b5fca440f163b4a703a3238` (runtime
`6ba411ecc0d0333177127f81fc83d76cfe816d4a`). Merge `3e23717fe9ee49d6dd33d7498a4e174e11e8274e`
records 17 conflicts. PR102 is not an ancestor or required dependency. No newer
clock/ID/reset/control work is imported.

The initial consumer source **c8f126bd0d737106d0c4aedd65c521a460cc2125** independently
passed **1,040 library + 2 CLI + 10 example tests, 6 focused payload tests, 13
consumer Python checks, 7 public runner checks, fmt, clippy (warnings, exit0),
and 9 actual public-v1 episodes**. The public tail records 144 loopback provider
requests, 9 expected misses and 54 returned-only exposure cases. Final source
changes only Python fixture/consumer logic and invocation documentation. Every
Rust file and Cargo input is hash-identical; `inherited-runtime-evidence.json`
records exact prior logs and hashes. The broad Rust/public suite was not repeated.
The provenance-bound examples were recompiled for final source pins and the actual
consumer/canaries rerun in an independent source-specific target.

## Actual behavior and boundaries

The actor receives only its current step after the previous ACK. Its standalone
copied code imports no evaluator fixture factory or project module. Both actual
supervisor control descriptors are checked closed in every actor process; only
MCP stdio is inherited. Strict controlVersion1 capabilities, ACK identity, store
lineage, generation, four distinct required views, exact applied generations and
pending0 are validated. Unsupported reset/snapshot/advance/logicalHash calls fail.
Raw private requests/responses and readiness receipts remain evaluator-only.

Baseline actual committed G: initial2, ingestion7, linked store8, observing get9,
feedback10, invalid feedback10, observing graph query11, then false queries and
replay miss remain11. Every checkpoint is obtained from the real same-core owner;
no test-side rebuild, stamp manipulation or inferred readiness.

The nonempty graph fixture uses a same-namespace acknowledged parentId, an unlinked
decoy and a foreign decoy. `limit=1, depth=1, compact=false` returns exactly one
primary and one `graphContext.neighbors` record, with unique UUIDs and correct
connectedTo, excluding both decoys. The synthetic scorer ranks primary memories
followed by serialized neighbors. Two queries scored, one zero-label-unscored and
one unknown annotation remain distinct. Hand-calculated baseline Recall=.75,
nDCG=.8065735963827292, all-required=.5 are synthetic contract diagnostics only.

Changing evaluator labels preserves all normalized actor actions/projections,
actual provider vectors and returned source ranks; environment hashes/cache keys
change as designed. Raw UUIDs/timestamps are retained, with explicit per-run
source mapping for comparison. Future query canary is absent from actor code,
inputs, wire and provider requests until its current delivery. No deterministic
reset, byte-identical output or semantic-quality claim is made.

Exact per-record exposure event counts are not observable through public-v1 or
control-v1. This consumer proves unique returned UUIDs, observing G advancement
and actual readiness. Separate owner96 proof is inherited and passed here:
`src/api/vector_query_tests.rs:732` asserts returned-only access-history increments;
`src/daemon/read_only_tests.rs:1433` exercises 12 real REST/MCP/daemon
get/similar/search/feedback operations with completion. Dedup source audit at
`src/decay/observation.rs` records first-occurrence IDs and direct-over-associative
precedence before namespace batches. These are distinct from this episode's
per-step G observations, not a fabricated private event-count oracle.

## Preserved failures and merge review

The first Python check failed from a local helper-name collision; its exact
source/log is archived. The initial c8f126bd actual episode reached all 11 ready
barriers, then failed our neighbor assertion because default compact=true
intentionally omits graphContext (`src/mcp/args.rs` include_neighbors=!compact;
`src/mcp/tools.rs` full response shaping). c2ddf676 requests compact=false and
consumes the actual neighbor array. This was a fixture/checker error, not a runtime
failure or weakened assertion. Both failed runs, source and executable hashes
are retained. No failed log was overwritten.

Merge resolutions preserve99 root admission before enqueue/provider, child
ownership in actual blocking workers, unresolved outcomes/full receipts, pinned
ReadHandles and publication completion. Additive92 session context, structured
outputs and instrumentation wrap the same work. Halley reviewed these ownership
rules. All 17 original hunks and exact resolution patch are recorded. Inherited
evidence whitespace is retained; runtime diff checks and pinned formatting pass.
Host cargo was unavailable (exit127); formatting used the pinned offline image.

## Reproduction and provenance

The two source/output archives contain immutable allowlisted source, drivers,
commands, all logs, actor/control transcripts, scorer inputs/outputs, rejection
receipts and source/executable hashes. `archive-members.json` verifies every member;
SHA256SUMS covers files without self-reference. No cargo/target cache or executable
is committed. Source and executed consumer binaries match pre/post. Core library,
CLI and example test hashes match pre/post in the inherited run. Final focused
scorer binary has an additional direct execution receipt with explicit pre/post
hashes; no build or broad suite rerun was needed. Both runs have OOM/oom_kill0.

Image sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef,
Rust1.94.1, network none, 2CPU/4GiB, -j1, immutable RO source, independent private
targets. Dependency artifacts were cloned; initial join cleaned the recalld package
before compilation. Final correction inherits only verified identical runtime
source and recompiles examples for new provenance. No ambient HOME/config/socket
or real credential mounts, downloads, paid calls or personal recalld access.

Invocation: docs/implementation/owned-mcp-supervisor.md. Older studies remain
pinned. Manifest/sourceMapping/trace versions1 are independent from public-v1 and
private controlVersion1. Canonical schema is null/unattested. FullA6/nativev2,
whole-response ReadView, snapshot/reset/logicalHash, frozen clock/IDs/RNG and later
writers remain unavailable. The checkpoint's native catalog/policy no-outbox
negative and actual batch repair positive remain in its separate local report;
this minimal92+99 consumer does not promote those paths.
