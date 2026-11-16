# Native catalog wire consumer: final validation

Validated runtime: `d4c8a2f3a9660bdbc6b21bf9d0092aa4cecea640`.
Branch: `impl/native-catalog-wire`.

## Feature and actual dependencies

The existing MCP create_namespace and REST namespace creation paths now have
gated native inner consumers. Strict v2 parsing preserves original provider
selectors, expands no dynamic defaults, rejects unknown fields/nulls/types/range
errors, and reuses shared lineage/key/decimal-CAS validation. Both concrete
namespace adapters use their existing owned engine and explicitly configured
provider, clock and frozen policy. No database is reopened and no second
registry is created. The shared engine orchestration is also used by the native
session; replay and CAS precede provider sampling and filesystem staging.

An accepted work root reaches both replay lookup and final publication, including
when new-root admission closes between acceptance and execution. Standard locks
are confined to the blocking worker. Caller profile conflicts and unavailable
provider configuration have distinct typed outcomes. Actual creation does not
invoke embedding or infer identity from a display model label.

Parent set: PR88 `3961f66` plus PR87 creation/full-receipt correction `27ef23d`,
and the minimal corrected PR82 startup runtime `fb804a0`. The published PR87
head `94a352c` combines those same core corrections and adds evidence; its
post-validation ancestry alignment changes no src/tests/Cargo/runtime bytes.
No batch, graph, whole-query or payload-consumer WIP is imported.

Public application contract2 remains unconditionally rejected. Existing v1
success fields, null compatibility, numeric bounds and serialization remain
unchanged. Remote native catalog RPC is unavailable with no legacy fallback.
This is a real gated consumer, not full A4 completion or ordinary native serving
activation. Canonical catalog success is not whole-store derived readiness;
the current envelope omits readiness and states that limit explicitly.

## Results

- Exact receipt fault regression:1 passed in9.25s.
- Full all-feature suite:987 library tests passed in417.69s, plus2 CLI tests
  passed in2.56s and1.38s;4 doctests ignored.
- Formatting passed; all-feature/all-target Clippy exited0 with630 library and
  697 lib-test warnings (622 duplicates).
- Earlier independent checks passed2 parser tests,5 parser/native-wire tests,
  27 catalog tests and1 unchanged v1 namespace-option parity test. These are
  preserved separately and do not replace the final full run.

All commands used Rust1.94.1 in recalld-ci:1.94, Docker network none,2 CPUs/4GiB,
CARGO_BUILD_JOBS=2 and private cargo/target directories. Final source was mounted
read-only. No home, credentials or Docker socket was mounted. The test command
was cargo test --offline --locked --all-features; fmt and Clippy used the same
image and offline locked dependencies. container.txt records actual image and
mount/resource settings.

The224 pre-run Rust/config hashes and230 supplementary committed source/input
hashes, including non-Rust fixtures, all matched afterward. The supplementary
manifest was captured during the run from the clean frozen tree and is labeled
accordingly. Six executed test binaries are hashed. This worktree's initial
source-root switch used cargo clean -p recalld in its own target after PR88
binary hashes had been sealed; no later source-root switch occurred. No cache or
build output is committed. manifest.json hashes the27 final/failure artifacts;
the earlier28 artifacts live in ../initial/manifest.json.

## Actual tests and limits

Synthetic stores are created through real legacy namespace creation and schema
migration, then opened by the native owner. The wire tests perform actual native
namespace creation, cross-transport matching replay with stale CAS, changed
payload and foreign-lineage conflicts, strict reopen and replay. Provider calls
are forbidden during matching replay. Invalid input and incompatible/unknown
profile cases preserve store bytes. The admission test proves accepted-root
lookup and commit continue under a closed admission window without reopening it.

Real inner MCP/REST fault tests cover after_metadata_commit,
before_handle_publish, after_handle_publish and uncertain commit acknowledgment.
Definite failures retain the full receipt, unknown outcomes do not fabricate one,
and lookup through the poisoned owner stays blocked. Reopen recovers the receipt;
matching replay does not allocate another namespace. The returned receipt equals
the retained receipt as parsed canonical JSON, as a typed WriteReceipt, and as
exact canonical encoded bytes. MCP text/structured output agrees, and the real
cross-transport replay compares full receipt/data values with only the declared
replayed flag differing.

These are real native inner handlers and serializers. The outer public2 gate is
tested separately and remains closed; no public2 end-to-end activation claim is
made. Shared derived readiness, versioned public schema advertisement, remote
native RPC, other native mutation/batch consumers and metadata pagination remain
separate work.

## Preserved fixture failures and corrected diagnosis

The initial receipt run passed5 tests and failed1. Its expectation used
serde_json::to_value(&retained_receipt), widening f32 values, whereas the actual
typed serializer used shorter f32 decimals. Both original MCP and REST paths
agreed. This was an incorrect test representation, not proof of a production
float-parity or canonical-receipt bug.

An initial source diagnosis mistakenly assumed MCP itself used to_value. A
REST-only conversion attempt was made, and the next exact run failed before the
full suite started. Inspection confirmed ToolCallResult::json serializes the
typed payload to text before parsing its structured copy. Main review directed
the minimal correction: retain the original serializers and derive expected JSON
from the actual canonical receipt encoding. The REST-only attempt was reverted;
an unexecuted shared conversion proposal was also removed. Both executed failed
sources, logs and executable hashes are retained in receipt/ and recovery-joined/.

The final source makes no production float normalization change. It does not
round canonical receipts, request hashes, generations or provenance. Assertions
were strengthened with typed and exact encoded-byte equality. No test was
dropped, no storage boundary weakened, and no extra full run was performed after
the final pass.
