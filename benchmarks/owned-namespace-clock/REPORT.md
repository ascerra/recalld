# Shared owned clock for legacy namespace creation

Measured source `535605d3c1ecc38f915dbe8daccbae9d5a0f0d0b`, branch `impl/owned-namespace-clock`, direct actual
PR104 parent `a99f9eeead595fe352272719a5098f535e7581dc`. No PR118 native allocation
join, snapshot/reset, native public activation or broad integration ancestry.

## Concrete problem and change

The actual helper3ff independent initialization comparison failed despite equal
public suffix outputs. Typed diagnostic source `2d69e53`, evidence seal
`7a2a404`, reproduced all four stored prefix/suffix hashes using the real logical
walker/encoder. Only foreign Namespace.createdAt and its namespaceCreated
receipt timestamp differed. Legacy namespace adapters sampled wall time instead
of the isolated core clock. No hash/config/label normalization fixed this.

Both MCP and REST namespace adapters now accept the existing Clock Arc and
sample it exactly once after accepted root/child registration, before the
blocking canonical worker. The same timestamp reaches the response, canonical
namespace and actual creation receipt. MCP bridge, daemon and REST construction
forward their existing core clock. The default adapter constructor still uses
SystemClock, preserving ordinary production behavior. No alternate clock,
process-global override, receipt serializer or logical-hash change is introduced.

## Exact validation

PASS1009 library+2CLI, 7 Python runner tests, 9 public-v1 transport episodes,
fmt and Clippy exit0 with warnings. A focused actual MCP/REST regression uses a
counting clock to prove one sample per request and compares each response,
canonical row and persisted receipt. Three real public MCP stdio processes
advance the private shared clock by 60000ms and each create a namespace with
`createdAt=2026-01-01 00:01:00 UTC`. Their transcripts/stores and seven inherited
launcher/framing negatives are retained. Inherited freeze3, ID4, native1 and
terminal1 focused checks pass; counts overlap full tests.

Offline Docker pinned image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Rust1.94.1, 2CPU/4GiB, no network, personal home/credentials/socket mounts.
Private source/cargo/target, package clean before this source build; full checks
reuse exactly that verified source and target without a source switch. Binary
hashes and commands are in the frozen manifests and validation-proof.json.
Initial20edea7 failed only new regression trait imports before any test ran;
raw failed source provenance/logs are retained separately.

## Scope and follow-on gate

This fixes the actual uncontrolled namespace-time source. The independent
canonical-hash gate must still be rerun by the real PR120 supervisor consumer
with this exact fix plus its existing PR115 and PR117 parents. It must retain
raw scores/order, receipts and three per-lineage suffix restores; no synthetic
fixture replacement or timestamp/hash normalization is permitted. That future
joined proof is not claimed by these direct104 checks. Original115/3ff evidence
is unchanged. The old legacy parent does not publish all derived views during
namespace creation alone; the public regression's subsequent real store performs
completion before inspection, retaining that unrelated availability limitation.

Broad controlledClock/IDs/RNG, native/v2 serving, arbitrary scheduling/tie/cache
coverage and fullA6 remain unclaimed. Explicit sweep controls and native
readiness/schema consumer joins remain separate required work.
