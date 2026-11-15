# Actual owned namespace-clock consumer

Measured source **95540eedd7764555c42e03dd006da8dfb35290c7** passes the strict
independent-startup regression on the actual serialized public MCP consumer.
All 5 hash pairs (prefix, original suffix, 3 restored suffixes) match across the
baseline and changed-label runs, without normalization. Prefix logical hash:
`09e16831c0b458959f5df1c7022afd033949a7dd8ef544378eb09d630d4d5d7f`;
suffix logical hash:
`dcead2abb7aa5914063a97c32e055684006e6820fa0901eeb72d359b4d69ed4d`.

PASS **1,053 library + 2 CLI + 10 example tests, 1 focused namespace-clock test,
27 FTS + 6 payload checks, 16 Python checks, 7 runner tests and 9 public episodes**.
Formatting passed; Clippy exited 0 with warnings. Actual baseline and label variant
each pass 3 complete within-lineage restores and 7 runtime negatives; 12 unsupported
manifests reject before side effects. 78 actor requests, current projections,
complete public suffix responses/state/readiness/provider vectors are identical
across label settings while evaluator/cache identities differ. The actual public
foreign namespace ACK reports owned time `2026-01-01 00:00:00 UTC` in both runs.
Cgroup oom/oom_kill 0; max 11901 denotes reclaim events, not an OOM.

Owned executable SHA256
`6cb87c6c8d15e82919c55baa555860f7f232e2ca9654a58aa35bdde2cd106f15`;
scorer `51527e20c44dd9e5276272644b20a73bf3fb38dd41b673aa8841317ae6ecc1f3`.
Each suffix retains 12 trace/action/readiness rows, 8 source mappings and 24 source
ledger events. The unchanged synthetic scorer separates 3 scored, 1 unknown and
1 zero-label-unscored query (changed labels score 5). No semantic quality claim.


## Exact dependency composition

Measured source `95540eedd7764555c42e03dd006da8dfb35290c7` joins published120
`3b6ab694699cac2886d93c0db04fd1860af115c3` and validated namespace-clock PR125 seal
`0b50168b64ad353e4f7a53c6459c270fe8500070`, runtime
`535605d3c1ecc38f915dbe8daccbae9d5a0f0d0b`, a direct104 child. That clock parent
adds shared Clock forwarding in production direct MCP, daemon MCP and REST namespace
adapters and samples once after work admission. It changes no checkpoint, logical
hash, ID stream or restore algorithm. The response, canonical namespace and durable
receipt share the sampled value; default SystemClock behavior remains available.

Merge is conflict-free, with mechanical auto-merges in API adapters, MCP adapters
and daemon construction. No native-ID124/118 or new sweep/maintenance publisher is
joined. Existing120 already contains required115 checkpoint and117 full FTS rebuild
fixes; their necessity and histories remain in its report. Consumer4061149 restores
the independent prefix and original/three-restored suffix hash comparisons in the
actual label-canary runner. The consumer compares every observed hash and suffix field exactly. The owner's unchanged logical hash retains its existing defined
physical-relocation handling; this child changes no hash rule. The unchanged evidence-only executable pending
gate from 120 is also run against these actual outputs.

## Regression and comparison contract

120 retained actual failed3ff independent-run hashes and Halley's typed diagnosis:
34 prefix / 41 suffix items, all 4 hashes reproduced. Exactly the foreign namespace
createdAt and corresponding namespaceCreated receipt timestamp differed. The
provider endpoint/source-root configuration differences were not hash inputs.
Namespace creation used wall time outside the owned lifecycle clock; this was a
real missing clock seam, not an FTS, label or hashing defect.

The new consumer initializes two independent private roots with the same frozen
fixture, owned clock and allocation stream. Evaluator labels and cache/environment
identities differ. Both ingest the same sources through actual public MCP. Their
prefix GenerationRef and complete logical hash must match. Each root then has an
original suffix plus 3 constructor-restored suffixes; every raw public output,
actual UUID, runtime state, readiness receipt, provider body/vector and complete
logical hash must match its prefix lineage. Across label variants, all these suffix
fields—including logicalHash—must match as well. The separately retained gate
requires all 5 prefix/original/restored hash pairs to match and exits nonzero on any
difference. Old120 failing gate output and3ffRED remain unchanged in parent evidence.

The core clock test verifies a single sampled value reaches MCP/REST response,
canonical NamespaceConfig and the actual durable receipt; the combined full suite
runs it. Actual namespace creation in the serialized prefix must use the owned
clock rather than wall time. Canonical hash equality covers the receipt too; no
second store or fabricated receipt oracle is added to the actor.

The actor remains standalone/current-step-only and inherits only public MCP
stdio. Private control descriptors are checked closed. Evaluator labels, future
schedule, checkpoint paths and clock/allocation state remain supervisor-only.
Warmups and negative probes remain outside measured suffix scorer denominators;
all suffix rejections are retained in the trace. Scorer/source mapping/trace
versions1 remain independent of publicContract1/privateControl1/checkpoint1.
Manifest source/artifact/executable pins keep older studies frozen.

## Retained evidence and limits

The source/output archive includes exact immutable source, commands, source/test/
example/public-binary hashes, full logs, all public/control transcripts, synthetic
canonical checkpoint files, provider requests, scorer outputs and negative receipts.
`archive-members.json` verifies every member; SHA256SUMS excludes itself. Source
and executed binaries match pre/post; cgroup memory events are retained. Independent
private target, package clean, pinned Rust1.94.1 image
sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef,
network none, 2 CPU / 4 GiB, -j1, immutable source RO. No ambient credentials/HOME/socket,
dataset download, paid call, executable or build cache publication.

Owner535605 initial20ede test-import compilation failure remains in the inherited
clock evidence; owner diagnostic formatting failure and3ff independent-hash RED
remain in120. Earlier609/13e/47bb bounded passes remain source-specific and are not
rewritten. This child does not normalize those historical failures away.

Passing the independent-startup gate covers these two fixture configurations and
explicit namespace creation path. It does not establish all application clocks,
IDs/RNG, retry jitter, future writers or arbitrary graph/tie schedules. Public
scores are vector scores; internal BM25/composite parity remains117's actual
regressions, not a fake public oracle. Full A6, native/public-v2, whole-response
ReadView and upstream semantic-quality/dataset claims remain separate gates.

Invocation remains `docs/implementation/reset-mcp-supervisor.md` with this source's
exact build/manifest pins. The manifest consumer and actual test script must match
the compiled scorer. To independently rerun the formerly pending gate:

```
python3 benchmarks/a6-reset-supervisor/pending-independent-startup-hash.py BASELINE_EPISODE CHANGED_LABEL_EPISODE
```
