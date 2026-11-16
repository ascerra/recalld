# Retained freeze, owned clock and canonical allocation consumer

Measured runtime `d09ca4988d99e2ba28a549bd6b0504347fc056e9` on `impl/owned-freeze`. Actual parent is
PR99/534d9be; additional scoped allocation parent1a21c60 is based on sealed82bb1.
No broad integration, later catalog/policy/native-batch or v2 activation ancestry.

The same core now retains publication, graph/vector/FTS/entity and engine leases
inside its blocking capture worker. A controller permit survives handler timeout
until that worker joins. Admission reopens only after outer leases release. The
controller registers no root that it would wait to drain. Private `advance` and
clock/cursor inspection use this same retained boundary. No snapshot manifest is
published by this source.

The isolated example injects one ManualClock (initial1767225600000ms), canonical
allocation seed7, and the existing shared MetadataStore source. Production defaults
remain SystemClock, UUIDv4 and MemoryId::new. Canonical store/receipt/memory IDs,
purge continuation, selected text identity and current legacy batch reservations
use that owner; keyed replay precedes allocation. UUIDv7 keeps all74 hash bits,
not allocation-order monotonicity. Exhausted cursor remains inspectable but is
rejected for writable restore. IDs consumed by failed attempts are not rewound.

## Exact validation and failed evidence

Rust1.94.1 in pinned image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`. Docker network none,
2CPU/4GiB/private source/cargo/target, serial crate compilation, no personal home,
credentials or socket mounts. Each source-specific target was cloned privately
and `cargo clean -p recalld` ran before building. No source switch occurred in the
public tail; all source and prior executable hashes were checked unchanged.

PASS1008library+2CLI;3freeze/batch tests;4ID tests;1native single-clock/replay
regression;1unchanged terminal-ID regression;3actual private processes+7launcher/
framing negatives;9ordinary public-v1 episodes+7runner tests;fmt and clippyexit0
WITH warnings. Public fixtures retain54returned-only exposure cases,
144synthetic provider requests and9expected misses.
Cgroup oom/oom_kill0; memory.max events are retained and are not labeled OOM.

The full driver's final shell tail had an unterminated quote AFTER the full suite
and clippy passed. Its original manifest remains statusfailed/exit2. The separately
recorded public-tail manifest exited0 and binds that original manifest plus
unchanged source/test/example binary hashes. No failed invocation is relabeled.

Earlier428e572 passed both race tests but its private probe wrongly expected an
ISO timestamp; the actual wire value was `2026-01-01 00:01:00 UTC`. Corrected only
the assertion. Earlier allocationfb4b2cd passed4focused+native1 but full suite was
930pass/1fail: supplied terminal UUID rejection sampled the clock early (3vs2).
Correction1a21c60 restored rejection before sampling; original assertion retained
and passes in the measured combined source. All failed source and raw logs remain
in the archives and original scratch roots.

The exact1025-seed Rust fixture reproduced383old-layout collisions (10seedbits,
same timestamp/counter), versus0full-hash collisions in that fixture. This is not
a proof of universal uniqueness. Actual canonical duplicate/terminal checks stay.

Private example binary SHA256 `481e734b03b5c8edbc113499f1cf3b9377f01a5eee7ddbd07905e39dae3e4ff1`.
Public binary SHA256 `3d57988d9befb73a50988ad367a11b640900cdc6374394acea6d67e51b84159d`.
All complete commands/source modes/inputs/binary hashes are in the manifests.

## Interface and remaining scope

`advance`: strict `{"deltaMs":0,"deadlineMs":30000}`; negative/overflow and bad
deadlines reject. No implicit sweep. `inspect` captures actual committedG,
`logicalHash:null`, and `runtimeState` (nowMillis plus versioned allocation state)
under retained freeze. Ordinary production constructors keep prior behavior.
Private capabilities distinguish lifecycleClockControlled and
canonicalAllocationControlled; broad controlledClock/Ids/Rng, snapshot/reset and
logicalHash remainfalse. Actor MCP discovery never includes evaluator controls.

The latch test actually times out a worker holding all leases, rejects advance
and capture retry, verifies unchanged clock and no published result, then releases
and joins the worker before advancing. Batch proof checks actual reserved child
and parent UUIDs, no cursor movement on keyed replay, actual completion and frozen
cursor inspection. Three private replays match declared IDs/generations/clock/
cursor projections. This is NOT deterministic canonical-state/reset/replay proof.

Canonical checkpoint staging and logical hashing are being implemented in a
separate child, not measured here. Still required: actual snapshot publication,
cache/config policy, fresh-owner reset and suffix replay, stable ties, full clock/
RNG coverage and native/public-v2 producer/reader joins. No fullA6, semantic-quality,
performance/releasegate or hard timeout over blocking OS I/O claim. No paid models,
external datasets, training or personal recalld access.

Frozen source and complete JSON/log traces are archived. Physical synthetic stores
remain at the recorded private scratch roots; they are forensic files, not A6
snapshots. Build targets/cargo caches/executables are excluded. Prior historical
reports and measured branches remain unchanged.
