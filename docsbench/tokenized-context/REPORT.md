# Actual tokenized context and packed-evidence consumer

**PASS — encoding-specific synthetic public-v1 diagnostic.** Final executed source
`5426da206a354924e26be8920a735b755550f1c7`; only required parent is published
#90 `003d857dd99acca9eb33801fa6c5d7b137d124a2`. Production Rust, Cargo manifests
and examples are byte-identical to90. No167/grouping/trace/native/control parent.
The evidence seal is documentation only;167 remains frozen with integration.

Final focused proof:16unique Python tests(7inherited,9context), necessary actual
CLI/scorer provenance build, six real serialized episodes /86 MCP actions and
20 actor-rendered contexts. Seven manifest/asset negatives rejected before run
output/store/provider creation. An internally consistent forged context receipt
was rejected against the actual public payload before packed-score output.
No unchanged Rust suite, broad fmt or Clippy run was repeated.

## Actual consumed path

The new named `serialized-mcp-tokenized-context-synthetic-diagnostic-v1` track
uses the existing production CLI/MCP actor. Stores, false-mode recall, namespace
validation, error handling and replay-cache misses remain real serialized paths.
After each successful recall, `public-mcp-actor.py` uses the pinned local tokenizer
to render fixed instructions, current question, ranked UUID/summary entries and
all delimiters. It emits a context receipt; no evaluator label, source mapping,
future schedule or private control enters the actor module.

`context-mcp-evaluation.py::consume` obtains the real memories from corresponding
public wire outcomes. `context_tokenizer.py::verify` reconstructs the entire
expected receipt from those actual payloads and the current question/policy.
It compares exact text, entries, UUID order, hashes, token IDs/counts and prefix
selection. Actor-reported IDs/hashes alone cannot establish validity. Only then
does the supervisor map included UUIDs through existing evaluator source mappings
and run the existing scorer on `packed-context-score-input.json`. Original
`evaluator/scores.json` remains the retrieval result; packed-context scoring is
separate and does not relabel retrieval/extraction failures.

This is a consumed rendered-context/evidence boundary, not a model call. The
receipt wrapper and omitted-entry diagnostic metadata are not the bounded rendered
text. No model-specific chat framing, token usage, generated answer, answer quality
or dollar cost is fabricated. Synthetic provider protocol usage0 stays unmeasured.

## Exact tokenizer and offline assets

Library: tiktoken0.12.0, verified official tag/source
`97e49cbadd500b5cc9dbb51a486f0b42e6701bee`, explicit cl100k_base. The exact
CPython3.11/aarch64 wheel SHA256 is
`f18f249b041851954217e9fd8e5c00b024ab2315ffda5ed77665a05fa91f42dc`.
The installed native tiktoken extension SHA256 is
`43c9259a79c49d8998e0c885c4d1750e833654a48fe8af16529b6e9500a1914e`.
Official rank-table SHA256:
`223921b76ee99bde995b7ff738513eef100fb51d18c93597a113bcffe865b2a7`;
configuration SHA256:
`b9b32d2f562688ec43d7a8066f807cbfb9f7675f0c25e2c93735091a2ffbd5c9`;
installed-file/wheel lock SHA256:
`8ba60a27fc57e6fe692e328f071330ad9741b1f28f29bd443ead4e0034253342`.

All seven prefetched wheel hashes were verified against official PyPI release
metadata. Total wheels2,602,465bytes plus rank table1,681,126bytes; no weights or
dataset download. `dependency-inventory.json` records exact versions, URLs,
licenses and source hashes. The library MIT notice is retained at
`scripts/TIKTOKEN-LICENSE.txt`, SHA256
`418cb499b436128d653d79941333a5437b7be2ea9213dcc2f04d15d5d2c51d86`.
No separate notice is embedded in the official rank table; that absence is recorded
rather than inventing a model/dataset license. Wheels/rank table are not vendored.
Official immutable constructor, Encoding API, loader and license references are
linked in PLAN.md; the chosen asset matches its prescribed hash.

The image has no pip. `prepare-context-assets.py` installs the fixed verified
wheel members without resolver/network/hooks or .data processing; no general
installer is introduced. All141 installed files, including four native extensions,
are checked against the pinned wheel-derived hashes. Runtime load rechecks the
complete installed roster, rank/config hashes and Python3.11/aarch64 ABI before
import; loaded module paths must be inside the verified site. No ambient cache,
user site, dynamic encoding plugin, model-name lookup or download fallback.
All three runs' installation/pre/post maps match exactly, including after negative
tests on a separate corrupted copy. Those receipts are in each run's
`installed-tokenizer-prepost.json`; package/rank binaries are excluded from repo evidence.

## Counts, caps and actual truncation

Use actual BPE over the whole rendered text, with strict UTF-8 validation and
special-token-looking strings treated as ordinary text. The evidence block is
also encoded separately; its cap includes UUID/rank/JSON/newline provenance.
Fixed prompt/question overhead is measured using an empty evidence block.
Separate counts are explicitly not assumed additive; final whole-render counts
are authoritative. This does not assert a match to any model tokenizer or window.

The default evidence/full-render caps are4096/4608. The separately named boundary
fixture uses128/256; it is not an A5 stress benchmark or a redefinition of the
4096 default. Whole entries are considered in actual public rank order. At the
first non-fitting entry, omit that and every later entry; no skip-ahead packing,
reranking, token slicing, question truncation or canonical deletion. The full
prompt/question alone exceeding its cap rejects before opening the actor's RPC
pipes; a focused actual-actor test uses invalid descriptors to verify no dispatch.
Exact256 fit and one-over, Unicode/combining/emoji/special literals, invalid UTF-8,
empty evidence, first-too-large and duplicate identities are focused proof.

| Actual episode | MCP actions | Context queries | Evidence tokens | Full-render tokens q0–q2 /q3 | Included source prefix | Retrieval Recall /packed Recall |
|---|---:|---:|---:|---|---|---|
| default4096 |15|4|313|336 /341|0,1,2|0.75 /0.75|
| boundary128 |15|4|32|55 /60|0|0.75 /0.25|
| oversized first128 |15|4|0|23 /28|empty|0.75 /0|
| empty store128 |11|4|0|23 /28|empty|null /null|
| changed labels128 |15|4|35|58 /63|0|0 /0, changed annotations|
| old profile |15|0|unavailable|unavailable|original behavior|original scorer|

Each nonempty ordinary context episode retains2scored/1zero-label/1unknown query.
The empty episode retains0scored/3zero-label/1unknown. Changed labels have1scored/
3unknown; these are a canary, not a comparable quality arm. Foreign source3 is
excluded by actual public recall. Default includes all three domestic results;
boundary drops source1 and source2 even though source2 alone could fit. Oversized
first drops all. The default run does not saturate4096; saturation is not claimed.
Every episode retains its real replay-miss provider400/tool error with no fallback.

## Canary and independent payload proof

Before any episode outcome, all fixture/policy/label manifests are frozen and
hashed. Current-step actor code, inputs, requests and provider inputs/vectors match
between boundary and changed-label runs. Environment/cache identities remain
separate. Existing future-query/evaluator-label checks pass. Each of20 actual
actor receipts exactly equals the independent supervisor re-render from the same
public response; no hypothetical response or actor-provided summary substitutes.

Independent legacy UUIDs differ and are rendered/tokenized unchanged. The actual
boundary/changed-label counts differ32→35 evidence tokens and55→58(q0–q2),
60→63(q3) full tokens. Raw hashes differ and are retained. Source-ID bijections
compare evidence identity only, never token/render equality. Selected prefixes
did not diverge in this run; the checker records rather than rejects a divergence
if different UUID token lengths cross a cap. No normalization hides that possibility.

The forged-receipt check changes the first actual summary, then recomputes an
internally valid render/hash/token count using the same real IDs. Supervisor
verification rejects it against the original real public response and creates no
packed-score output. The unit suite separately tests hidden suffix tampering.

## Preserved failures and attribution

- `aeec2ebe3821a2044139aaa79f2d40d75d8fe02e`: installed assets verified and
  inherited7passed; new suite7passed/1fixture error. Its old-profile check reused
  artifacts overwritten by the context fixture, so old hashes correctly rejected.
  No CLI build or episode ran. The fix used a separate old-profile fixture.
- `1518c449bec42553ef0cda0c917263deaed989b8`: inherited7/new8 and actual build
  passed. The first actual episode accepted source0, then source1's3620UTF-8-byte
  summary correctly exceeded the real2000-byte limit (`model::validation` and
  `SUMMARY_MAX_BYTES`). Eight MCP outcomes, one accepted mapping and the explicit
  rejected source ledger are retained. No query/context ran; no rollback claim.
  This was an invalid synthetic input, not a production runtime bug.
- Final `5426da20`: shortened only the synthetic repeated summary to1220bytes,
  still beyond128 actual BPE tokens. Added a byte-limit/token-overflow fixture
  check. All16Python checks, six actual episodes and negatives passed. Runtime
  validation was not weakened, and no Rust suite was repeated.

All240 tracked source inputs match committed Git and host/container pre/post
snapshots in each run. Executed final binaries, checked before/after actual use:
CLI `603bfa58f26c2e5eb66c3dc73372d5ed910ed014e087593a7f78e8aa1ccdfcf8`;
scorer `b64064d91d8796a9d56f1025d556a402ba511c895126ee3382a289269a9f5478`.
Source/binary evidence remains pinned to each attempt, not relabeled to the seal.

Pinned Rust1.94.1/Python3.11.2aarch64 image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`;
networknone,2CPU,4GiB memory/swap,-j1,read-only frozen source/prefetch,one reused
private Cargo/target and shared registry read-only. No personal store/config/
credentials/daemon/socket. Peaks147,034,112 /3,669,917,696 /4,294,967,296bytes;
final memory.max events1578, all OOM/kill counts0. These are correctness checks
under memory pressure, not performance/headroom evidence. Existing12 library
warnings remain; containers were removed after capture.

## Reproduction, remaining gates and cleanup

Prefetch exactly the small wheel/rank URLs from `dependency-inventory.json` into
`PREFETCH/wheels/<filename>` and `PREFETCH/cl100k_base.tiktoken`; the installer
verifies all bytes. Prefetch is a separately allowed network phase, never a test
fallback. In the pinned offline image with matching source/build metadata:

```sh
python3 scripts/prepare-context-assets.py --prefetch /prefetch --out /output/tokenizer
python3 scripts/test_context_mcp_evaluation.py --assets /output/tokenizer
python3 scripts/check_context_mcp_evaluation.py \
  --binary /target/debug/recalld --scorer /target/debug/examples/score_mcp_episode \
  --source-root /work/source --build /output/build.json \
  --assets /output/tokenizer --out /output/actual
```

The existing runner's `prepare --track serialized-mcp-tokenized-context-synthetic-diagnostic-v1
--context-profile default|boundary|oversized-first|empty --context-assets ...` and
`run --context-assets ...` consume this directly. Old track requires no tokenizer
assets and remains pinned to its own inputs/source. This slice closes actual
encoding/budget/truncation consumption, not matching-model generation, upstream
scorer semantics, native/public2 activation, runtime reset or full-A6 acceptance.

Three context raw runs remain protected pending resulting PR/results, then will
be centrally deleted. One reusable cache and small tokenizer prefetch remain for
necessary follow-up; source/worktrees/reports/HOME stay protected. Separately,
central cleanup through batch43 records434 exact released-path actions; latest
observed free space1239.904GiB. APFS/concurrent changes are not exclusive reclaimed
bytes. No unknown or active cache was deleted.
