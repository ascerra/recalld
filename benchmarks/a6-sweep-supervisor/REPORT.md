# Actual owned sweep/reset supervisor diagnostic

Measured consumer **e16c10964175f4876109d4a053f74bfe82cfb4e2** passes the named
`serialized-owned-sweep-synthetic-diagnostic-v1` track. Both independent baseline
and changed-label runs complete the original phase-changing suffix plus **three
fresh-owner restored suffixes**, each with **seven runtime negatives**. All **five
independent canonical-hash pairs** match without normalization:

- Prefix: `4fc2216ac2a013ccb124817bc6c481df16f945c700f01ed3ba1baf09f413f7fe`.
- Original and all three restored suffixes:
  `598f61f37c1e1a1f26946d45284d7aef124e8cc6a04954be9031cb6c342716f7`.

The actual owner performs Full → Summary → Ghost → deleted at owned time
`4920825600000`, after a 36,500-day advance with multiplier 1. Actual sweep
readiness is G6/G7/G9, all four required derived views equal actual G, pending 0.
The extra namespace created by the existing actor explains the generation offset
from the owner's smaller G5/G6/G8 fixture; no expected G was fabricated. Full
sweep reports, public responses, mappings, state, readiness and provider vectors
match exactly across every restore and independent label run. Request IDs are
caller-selected correlation only; every suffix sends the same unused 1001+
range before comparison. No output field is rewritten or removed to pass.

**64 actual actor requests**, projections and provider input/vector results are
stable under changed evaluator labels, while manifest environment and replay-cache
identities change. Four measured queries produce **2 scored, 1 zero-label unscored
and 1 unknown annotation** (deleted query). Changed labels score all four; source
ranks remain equal. These are synthetic contract checks, not semantic quality or
upstream dataset/scorer goldens. Each suffix consumes 10 public trace rows, one
source mapping and three source events reconstructed from acknowledged checkpoint
prefix provenance (seen/attempted/accepted), not live re-ingestion. Private
advance/sweep/status attempts and full results remain separate evaluator artifacts.

## Exact source and dependency relation

The only new dependency union is published PR127
`31955c3dde7f0219f2ebcebb36f57a35476038af` plus PR128
`5deb840a21ad5f1f80006e0d05a6372919292acf` (measured
`42d39a4e80996706fdd1c74a24b2cb8df7b17382`). Join
`762397aa436c4bd7775a770ee4d451b4875b9f02` preserves inherited PR115 checkpoint
construction, PR117 FTS rebuild behavior and PR125 namespace clock. No native,
ordinary-startup, graph-maintenance or moving WIP parent is added.

The sole merge-conflicted file, `src/bench/controls.rs`, combines the existing
snapshot dispatch with sweepStatus/sweep and unions their capability methods.
Halley source-reviewed this resolution and the automatically merged
`FrozenControls::capture` busy-permit/invalidation check used by checkpoint
capture. No second lock, planner, clock, hash, registry, clone or reset algorithm
was introduced. Core timeout/cancellation/known-commit behavior retains PR128's
four regressions; this child does not claim a separate runtime timeout/checkpoint
manifest probe merely from that source review.

Full measured source **13e4dd5c06603533c0f9b82aedd0ef045eca483e** passes:
**1,057 library + 2 CLI + 10 example tests; 4 sweep, 1 namespace-clock, 27 FTS and
6 payload focused checks; 16 Python checks, 7 runner checks, 9 public episodes and
3 owner sweep episodes**. Formatting passed. Clippy exited 0 with warnings.
Test executables and public executable hashes match before/after execution.

Corrected e16c109 changes only one Python replay-cache fixture key. Every Rust,
Cargo, test and example source hash matches 13e4dd5. The scoped rerun passes
16 Python checks, provenance example build, both actual consumer runs, 14
malformed manifest/annotation rejections before output creation, and the retained
strict independent-startup hash executable. The full Rust suite was not repeated.
The legacy-named unknown-source-label negative uses an unknown query key on this
track; its rejection is not separate evidence of the unknown-source-ID branch.

Executed owned-evaluator SHA256 is unchanged from the full run:
`21273fd92e5628dabc75b201345a7c7f36106aefffdd2afc868f164796a66f2c`.
Corrected provenance-pinned scorer SHA256:
`c5b73456e0baa14142b3ff86d1049bb73b16d3a72b6b44b3991b81466b1eaf9d`.
Corrected source and both executed binary hashes match before/after execution.
The evidence-only seal changes no measured runtime or Python source.

## Preserved failure and correction

The first 13e4dd5 consumer attempt failed at public store step 7 before any suffix
or hash gate. The actual embedding request was
`a6-document:chronological-sweep\n\none retained payload sweep-payload-canary`;
the fixture omitted the final tag. The actual provider returned HTTP400, the
public ACK was isError and the actor stopped without retry. Production
`src/mcp/bridge_adapters.rs:1249–1267` appends merged tags. Main reviewed the actual
wire/stderr diagnosis and approved the single-key Python correction. There is no
runtime change, generate/fallback, relaxed assertion or hash normalization.

Both the failed source/output and successful corrected source/output are archived
with per-member hashes. Original roots remain:
`/tmp/recalld-sweep-supervisor-13e4dd5c-a043vmll` and
`/tmp/recalld-sweep-supervisor-e16c1096-x8pczqyk`.
Full-suite green and failed first consumer ACK are separate evidence, never a
whole-run success claim for 13e4dd5.

## Boundaries and runtime controls

All runs used offline network-none Docker, immutable read-only source, private
source-specific cargo/target, Rust1.94.1, -j1, 2 CPUs and 4 GiB, image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Both runs report oom/oom_kill 0 (memory max/reclaim counters are not OOM events).
No personal service, ambient credentials/socket, paid model, dataset download or
training was used. All own actors/runtime owners/provider threads joined.

False public fetches demonstrate full-text loss then summary loss; Ghost retains
its existing tag, and deleted memory is absent from get/recall. This preserves
actual legacy phase semantics, rather than inventing Ghost tag erasure. Actual
malformed sweep controls reject without state change; replay-cache misses fail
closed. Sweep failures/uncertain responses are never resent; status is inspected
and the episode fails. Available known receipt/error/uncertain IDs remain in raw
reports without a fabricated aggregate receipt. No failure was injected into the
new sweep suffix beyond existing owner regressions and these explicit negatives.

The actor only receives current public steps, never future queries, labels,
private controls or inherited control descriptors. Future-query canaries are
absent from actual actor/provider paths before their step; the actor imports no
evaluator fixture factory. Public trace readGeneration stays null: post-step
barrier receipts do not turn the public-v1 response into a ReadView.

Manifest/sourceMapping/trace1, public1, private-control1, checkpoint1 and
sweep-control1 are independently named; canonical schema stays null/unattested.
No public-v2, native sweep/startup/control, all-clock/ID/RNG, arbitrary scheduling,
semantic-quality or full-A6 promotion follows. Original PR127 evidence is unchanged.
The separate ignored PR119 graph probe draft is not executed coverage for this
report. No push or publication is authorized by this evidence seal alone.
