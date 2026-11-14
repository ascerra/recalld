# Serialized fresh-owner reset consumer

The named `serialized-owned-reset-synthetic-diagnostic-v1` consumer runs real
public MCP actions through the production bridge and the actual owned checkpoint
constructor. It keeps evaluator controls, labels, checkpoint state and future
questions outside each current-step actor projection. This is a bounded synthetic
contract diagnostic, not full A6, semantic quality or native/public-v2 promotion.

## Composition

Published109 `9027b5307dce09bee88aa57baf9f50cc60b2c5d6` supplies the serialized
actor/scorer/manifest consumer. Required checkpoint115 seal
`0750f13b69c3e7c6e1f6650dd26e6bb94896e15f`, runtime
`2920dec86c3d9f7a0788ef01433a3ab4f9093bab`, supplies real snapshot/restore,
retained source/ancestor locks, canonical hash validation and owned runtime state.
Required FTS117 seal `bf7c2484018297b7f9af30e98d3214581f33f351`, runtime
`7311775604539a386bdeaa9e1e881cf8eeeff9de`, fixes full FTS clear/rehydrate used by
the fresh-owner constructor. Both runtime joins and checkpoint evidence alignment
were conflict-free; exact records are adjacent. No observer, incremental publisher,
new maintenance, graph, native promotion or broad checkpoint branch is stacked.

Checkpoint owner115 and FTS owner117 each carry their own validated source and
failure records. This consumer adds no Rust runtime code and no second reset,
copy/hash/cache algorithm. The production constructor owns those mechanisms.

## Measured freezes and validation

- `6098740663689d92e9ce7cedefab47d594f272bd`: first bounded unique-vector consumer,
  before117, passed 1049 library +2 CLI +10 example tests,16 Python,6 payload,
  7 runner tests and9 public episodes; fmt passed, clippy exited0 with warnings.
  Baseline and changed-label runs each matched3 complete raw suffix replays and
  passed7 runtime negatives. This pass never established general FTS parity.
- `13e1d0c72fc250e29233df3e12f225b992de4e3f`: exact115+117 combined runtime passed
  **1052 library +2 CLI +10 example tests,27 focused FTS,6 payload,16 Python,
  7 runner tests and9 public episodes**. Fmt passed; clippy exited0 with warnings.
  Baseline and changed-label consumers each passed3 raw suffix replays and7
  runtime negatives;12 malformed/unsupported manifests rejected before side
  effects.71 actual actor requests were compared across the two label settings.
- `3ff4e3132d1e8c0598ba156fb543659a4c954e3a`: stronger public multi-term fixture.
  Every Rust/Cargo input is byte-identical to13e; the inherited-runtime receipt
  pins its logs/binaries. Only provenance-bound examples are rebuilt and targeted
  Python plus actual consumers run. Both complete episodes passed3 raw restores and7 negatives, but the newly added
  cross-independent-run whole-suffix assertion failed on private logicalHash only.
  Exposed suffix responses, IDs, score/order, state/readiness and provider matched.
  The actual failure is preserved in its own archive; it is not counted as green.
- `47bb45f21dabd46ac31312c82b2d95442d8a6f00` corrects only the cross-independent-run comparison scope;
  no production source, raw score or per-checkpoint replay equality changes.
  The exact typed input diagnosis below establishes the bounded comparison
  contract. Independent-startup hash equality remains an explicit pending gate.

Final measured source **47bb45f21dabd46ac31312c82b2d95442d8a6f00** passes16 Python
checks and the actual baseline/label-variant consumer tail. Each completes3 exact
within-checkpoint suffix restores and7 runtime negatives;12 manifest rejections
happen before episode side effects.78 actor requests, current projections, complete
exposed suffix responses/state/readiness/provider vectors compare exactly across
label settings. Environment/cache identities change. All source/executable hashes
match pre/post. Owned executable SHA256
`bfb11cf17fd037a002048ffd255a815c63eab9077fa986b964f5a3af6d3ad401`;
provenance scorer `4780df836d796e79ce7562a2ebedf4aacd4bc6a3ce5208a785b14f35da335c6b`.

Each suffix has12 actual actions/trace rows/readiness receipts,8 source mappings
and24 source ledger events. Five queries yield3 scored,1 unknown,1 zero-label
unscored; synthetic Recall=.8333333333333334, nDCG=.8710490642551528,
all-required=.6666666666666666. Changed labels score5 queries with Recall0.
Observed suffix G sequence:11,12,13,13,13,13,13,13,14,14,14,14.
The three multi-term public scores are .6461623311042786,.5743665099143982,
.5025706887245178, in source order turn6,turn7,turn5, identical across all restores.
These are exposed vector scores, not BM25/composite values.

## Actual replay and actor boundaries

Seven acknowledged prefix sources include a foreign-namespace asymmetric corpus:
`document A` with full text `alpha` plus20 filler tokens, `beta beta beta`, and
`beta`. Distinct vectors, explicit namespace, depth0 and limit3 avoid a tie-based
oracle. Query `alpha beta` requests compact=false and retains all three raw
public scores, order and fields. The assertion checks membership/namespace and
exact replay equality; it does not prescribe an invented upstream ranking.
Legacy public MCP exposes `SearchResult.score` (raw vector score), not internal
`fts_score` or `composite_score`: `src/search/pipeline.rs:1947` and
`src/mcp/bridge_adapters.rs:466`. The fixture therefore proves exposed score/order
replay, not independent BM25 equality. Exact internal FTS score and order parity
is the separate owner117 regression proof, rerun in the27 focused combined tests.
Distinct vectors can mask an internal FTS rank change; no negative sensitivity
claim against pre117 is made for this public fixture.

The supervisor warms the actual provider twice, snapshots, executes the original
suffix, terminates/joins its actor and runtime, and invokes three sequential fresh
constructors in absent isolated sibling roots. Before actor admission it verifies
exact controlVersion1 capabilities, core snapshot logical hash, clock/cursor,
store lineage/G, all four required derived generations and pending0. The suffix
advances lifecycle time, stores a superseding replacement, observes, reinforces,
queries, forgets, checks the content-free tombstone, and records invalid quality
and a genuine fail-closed provider replay miss. Complete tool results, actual
UUIDs, raw scores/order, runtime state, readiness, provider vectors/requests and
logical suffix hash compare exactly without normalization or tolerance.

Changing evaluator labels reruns the actual complete consumer. All current-step
projection bytes, actor requests/IDs, whole raw suffix outputs, provider bodies/
vectors and returned source ranks must stay identical. Environment hashes and
cache keys must change. The future-query canary is absent from actor code, input,
wire and provider calls until its current delivery. Actor code is unchanged from109
and imports no evaluator factory. Only MCP descriptors are inherited; both actual
private control descriptors are checked closed. This is trusted-process isolation,
not a filesystem security sandbox or hostile-code boundary.

Scorer/sourceMappingVersion1/traceSchemaVersion1 consume actual ACK mappings and
returned IDs. Prefix mappings are hash-referenced, retained for both versions of
superseded content. Every suffix action remains in trace, including rejections;
source events record seen/attempted/accepted for acknowledged prefix and suffix
stores. Warmups and negative constructor/worker probes are conditioning/diagnostic
operations outside the measured suffix/scorer denominator. Missing annotation is
unknown; empty labels are unscored. Primary memories then serialized neighbors
form synthetic unique-source rank input; this depth0 fixture has no neighbors.
The separate109 nonempty-neighbor proof remains pinned.

## Negative cases and preserved failures

Seven actual negatives: active original owner; a restored owner with a real blocked
provider worker; existing destination; symlink destination; wrong checkpoint hash;
foreign sourceRoot; and unsupported checkpoint clock2^48 rejected before a new
manifest is published. Positive construction requires a joined previous owner
before launch; deliberate runtime-negative probes are explicitly marked separately.
Source/checkpoint bytes and destination sentinel are unchanged. All owners and the
loopback provider thread are joined. No manual rebuild or readiness stamp rewrite
is used by the test to repair a negative.

The failed3ff cross-independent whole-hash assertion is retained with full source,
raw logs, canonical checkpoints, exact hashes and post-run executable verification.
Halley's nonshipping diagnostic2d69e53 invokes the actual typed walker/encoder/
hash on manifest-verified copies and reproduces all4 expected hashes. Prefix has
34 items(31 included), suffix41(38 included). Only included items1 and10 differ:
foreign Namespace.createdAt1794686998839 versus1794687070301, and the corresponding
mutation_receipts namespaceCreated outcome.createdAt. Source GenerationRef,
request hash, operationID and every other encoded item (including excluded catalog
items) match. Provider endpoint/source-root differ in checkpoint configuration but
are not hash inputs. This proves missing owned namespace-clock coverage, not a
label leak, FTS failure or reset hash bug. No hash normalization or core fix is
introduced here. Owner diagnostic seal `08fbd464e858383dded8e083032a122c392cda0f`, sources, compile failure and exact typed output
are retained in `typed-hash-diagnosis-08fbd46.tar.gz`, with readable `owner-REPORT.md`
and `owner-typed-input-diff.json`, without adding a shipping dependency.

The desired independent-startup hash equality gate is NOT dropped. Run
`python3 pending-independent-startup-hash.py BASELINE_EPISODE CHANGED_LABEL_EPISODE`
against actual consumer outputs after Halley's clock fix. Its current actual exit1
and all5 differing prefix/suffix hash pairs are retained. Within each lineage every
one of3 restored full suffixes still compares complete hashes without omission;
the cross-independent label canary compares exact exposed suffix outputs/state/
provider while retaining differing private hashes. No independent-startup hash
or namespace-creation timestamp determinism is claimed. Earlier609 bounded success remains separate from
13e and the stronger fixture. Owner115 retains the SHA-format compile failure and
actual clock RED (snapshot published at2^48, restore rejected) plus correction.
Owner117 retains unchanged-production96 FTS score and rank REDs, exact values and
binary/source pins: full rebuild changed aggregate counts at the same generation,
then changed multi-term rank `[1,2,3]` to `[2,3,1]`. Its minimal delete-all fix is a
real dependency, not a reset-specific normalization. See
`benchmarks/owned-checkpoint/REPORT.md` and
`docs/evidence/fts-rebuild-stability/REPORT.md`. Prior109 helper/compact fixture
failures remain in its published archives. No owner failure is relabeled as a
failure of this consumer or silently replaced by a later green log.

## Provenance and limitations

Archives retain each immutable allowlisted source, full commands/logs, manifest,
source/executed-binary pre/post hashes, public and private wire receipts, actual
scorer inputs/outputs, all source mappings and synthetic canonical fixture files.
`archive-members.json` verifies every regular file and symlink; SHA256SUMS excludes
itself. No executable or target cache is committed. All measured sources and
executed consumer binaries match pre/post; initial full-suite library/CLI/example
binaries also match pre/post. Cgroup memory events are retained; no OOM kill.

Pinned image sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef,
Rust1.94.1, network none,2 CPU/4GiB,-j1, immutable RO source and independent private
targets. Dependency artifacts are privately cloned; each full runtime freeze
cleans the package before compilation. Final Python-only validation inherits exact
Rust/Cargo equality and rebuilds examples for source provenance. No ambient HOME,
credentials, daemon or Docker socket mounts, dataset downloads or paid calls.

Invocation: `docs/implementation/reset-mcp-supervisor.md`. ManifestVersion1 is
independent of track, publicContractVersion1, privateControlVersion1 and
checkpointVersion1. Canonical schema remains null/unattested. Reset RPC, whole
response ReadView, full clock/ID/RNG determinism, arbitrary graph/tie schedules,
provider retry jitter, incremental FTS delete/upsert stability and full A6 remain
unsupported. Fixed cache-only synthetic provider outputs establish contracts,
not upstream dataset/license/revision/model or semantic-quality claims. Existing
frozen studies keep their original source, track, manifest and binary pins.
