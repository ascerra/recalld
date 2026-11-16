# Actual tokenized rendered-context consumer — source plan

2026-11-15. **Plan and small dependency/source inventory only. No implementation,
tokenizer execution, consumer tests, paid calls, model weights or quality result.**
Necessary direct parent: published #90
`003d857dd99acca9eb33801fa6c5d7b137d124a2`. New isolated worktree
`/tmp/recalld-implementation/a6-tokenized-context`, branch
`impl/a6-tokenized-context`. Frozen167 remains with Halley; no edits/pushes there.

## Gap and smallest consumed boundary

A6 §5 requires a pinned tokenizer, deterministic ranked context truncation,
4,096 answer-evidence tokens including provenance, and separately reported fixed
prompt/question overhead. CONTRACTS §68 keeps that cap distinct from A5 stress
arms. Current90 `public-mcp-actor.py::run` consumes actual public recall outcomes
but does not assemble or budget answer context. `public-mcp-evaluation.py` already
projects only the current question and builds actual mapped score inputs.

Add one explicit synthetic context track through that same actor/runner. After a
successful real public recall, the actor packs its returned summaries and UUIDs,
renders the current question plus fixed prompt and selected evidence, and emits
a versioned context receipt. The supervisor consumes this exact emitted artifact
and its included IDs to produce a **separate packed-context score input** for the
existing `score_mcp_episode` source scorer. Preserve original retrieval scores.
This makes truncation change an actual downstream consumer's evidence coverage;
it is not an unused tokenizer helper or a second harness.

There is no answer model in this slice. The boundary is the actual rendered
context artifact and consumed context-evidence scoring, not a fabricated API
invocation or model input measurement. Answer generation, model-specific framing,
provider token usage, dollar cost and answer-quality attribution remain
unavailable. A later matching answerer can consume the same declared artifact.
No167/grouping/163/native/control dependency is needed for this independent gap.

## Verified dependency, encoding and license pins

Choose **tiktoken0.12.0 / cl100k_base**, independent of any model-name inference.
The official tag resolves to
`97e49cbadd500b5cc9dbb51a486f0b42e6701bee`; metadata was fetched from GitHub's
public tag API and immutable files verified. The exact CPython3.11 Linuxaarch64
wheel was prefetched from PyPI and SHA256 checked against PyPI release metadata:
`f18f249b041851954217e9fd8e5c00b024ab2315ffda5ed77665a05fa91f42dc`.
The existing pinned validation image actually reports Python3.11.2/aarch64.

The official constructor specifies the rank asset
[cl100k_base.tiktoken](https://openaipublic.blob.core.windows.net/encodings/cl100k_base.tiktoken),
1,681,126bytes, verified SHA256
`223921b76ee99bde995b7ff738513eef100fb51d18c93597a113bcffe865b2a7`.
Pin its pattern and special-token configuration from the same immutable
[constructor source](https://github.com/openai/tiktoken/blob/97e49cbadd500b5cc9dbb51a486f0b42e6701bee/tiktoken_ext/openai_public.py).
Do not infer an upstream model revision from this encoding.

The library's [MIT license](https://github.com/openai/tiktoken/blob/97e49cbadd500b5cc9dbb51a486f0b42e6701bee/LICENSE)
is captured and hashed. The official rank table contains no separate embedded
license notice; inventory records that absence instead of inventing a model or
dataset license. This plan uses the publicly prescribed asset locally, does not
vendor the rank table/wheels into the PR, and makes no independent redistribution
rights claim. Preserve upstream notices for copied constructor configuration.

`dependency-inventory.json` records every exact URL, release version, wheel hash,
byte size, upstream file hash and declared package license. The prefetched
transitive versions are regex2026.11.15,requests2.34.2,charset-normalizer3.5.2,
idna3.20,urllib3 2.8.0,certifi2026.10.23; all seven wheels total2,602,465bytes.
No unpinned resolver/network is permitted during validation. These are actual
prefetched versions, not claims they are uniquely required or model equivalents.
Keep `/tmp/recalld-tokenizer-prefetch` protected as this task's small future input.

The official [Encoding API](https://github.com/openai/tiktoken/blob/97e49cbadd500b5cc9dbb51a486f0b42e6701bee/tiktoken/core.py)
supports constructing an encoding from explicit configuration and encoding
special-token-looking strings as ordinary text. Use `encode_ordinary`, with
strict UTF-8 validation first so lone surrogates cannot be silently replaced.
The default [asset loader](https://github.com/openai/tiktoken/blob/97e49cbadd500b5cc9dbb51a486f0b42e6701bee/tiktoken/load.py)
can fetch on a missing/bad cache. Do not use that fallback: verify and parse the
explicit local rank file, construct `Encoding` from pinned parameters, and reject
missing/corrupt assets before runtime/output admission. No ambient tokenizer cache,
user-site package, plugin encoding registry or model-name auto-selection.

## Manifest and actor isolation

New named `serialized-mcp-tokenized-context-synthetic-diagnostic-v1` track;
manifest1/sourceMapping1/trace1 remain independent of context/encoding versions.
Pin tokenizer distribution/source revision, every installed wheel, encoding asset
and configuration hashes, renderer/prompt version/hash, special-token and UTF-8
policy, exact evidence/rendered caps, source fixture and actual source/binary
provenance. Unsupported versions/hash changes/undeclared assets reject before
store/provider/output creation. Whole-manifest preflight covers all current-question
fixed overheads without passing the future schedule to the actor.

Actor receives only the current question, validated context policy and public
response; it never imports evaluator fixture factories, labels, full manifests,
source mappings or future queries. A small actor-owned context module is copied
alongside the existing actor, with only the verified tokenizer/dependency path
available. Explicit allowlisted environment, offline container and current-step
input remain. Existing old track does not require these assets or silently enable
packing; old frozen studies retain their own source/fixture pins.

The renderer uses fixed delimiters/instructions and the current question. Evidence
entries contain the actual public UUID, rank and summary; all rendered provenance
counts toward evidence tokens. No private fetch/dump or source-label text is used.
Use actual public `recordAccess:false`,namespace default,depth0 and a separately
pinned retrieval limit sufficient to exercise more than one result. Store/search
still traverse the production CLI/MCP parser, validators and query engine.

## Exact budgeting and truncation

Keep two different measured constraints:

1. `evidenceTokenCap`: default4,096 for the evidence block, including rendered
   UUID/rank/delimiter provenance and any future explicitly supported graph text.
2. `renderedContextTokenCap`: explicit synthetic full-render cap, initially4,608,
   including fixed instruction, current question, separators and evidence block.
   This is a declared harness cap, **not a model context-window assertion**.

For a separately named small boundary fixture,128/256 caps can exercise actual
omission without large payloads. Do not relabel that as the default4,096 arm or an
A5 stress benchmark. Freeze both policies/fixtures before episode outcomes.

Count the complete final rendered text with the actual encoding. Also report the
empty-evidence prompt/question overhead and evidence-block count separately;
BPE boundary interactions mean these separate counts must not be assumed additive.
Both caps must pass after final rendering, not merely on concatenated per-field
estimates. Overhead alone exceeding the full cap rejects before recall dispatch;
never truncate the question/instructions to hide overflow.

Minimal deterministic policy: consider unique returned memory IDs in public rank
order; include whole entries while both candidate renders fit. At the first
non-fitting entry, omit it and every remaining ranked entry. No skip-ahead packing,
reranking, codepoint/token slicing, response rewriting or Unicode replacement.
A large first record may yield an empty evidence block, with an explicit reason.
This is whole-record context truncation, not canonical deletion or ingestion loss.

Context receipts contain current step/query ID, exact rendered UTF-8 text/hash,
actual token IDs/hash/count, encoding/config/prompt hashes, both caps, fixed
prompt/question/evidence/full counts, considered/included/omitted UUIDs in order,
per-item content/render hashes and omission reasons. Counts are encoding-specific
local observations; protocol fixture usage0 remains a separate unmeasured field.
Record refusals/errors and partial receipts. Never infer rollback or a successful
context from a missing outcome; no retry can spend externally.

## Consumed scoring/provenance

Supervisor independently checks the emitted rendered text, token count/hash,
source hashes, cap satisfaction and UUID order/subset against the actual public
outcome. It then maps included UUIDs using the existing evaluator-only mappings
and invokes the real scorer with those IDs. Output names explicitly distinguish
`retrieval-evidence` from `packed-context-evidence`; excluded evidence is reported
as context-budget loss, not extraction failure. Unknown/zero annotations retain
the existing unscored denominators. The actor receives none of this mapping or gold.

This first slice measures source-evidence availability at the rendered boundary,
not proposition preservation, semantic answer quality, model token accounting or
promotion utility. It closes an implementation seam without pretending the later
matching-model consumer is already present.

## Proposed offline validation before publication

- Pin and verify every wheel/asset/config hash; negative missing/changed versions,
  config/assets/package bytes and caps reject before store/provider/output.
  Offline install from the prefetched wheel set only, with all dependencies/hash
  requirements fixed and no network or ambient user site. Record installed
  distribution/native-extension hashes and image ABI before execution.
- Real tokenizer roundtrip/count cases: Unicode, combining characters, emoji,
  whitespace, special-token-looking literals, strict invalid-surrogate rejection,
  exact fit, one-over boundary, empty evidence and oversized fixed overhead.
  Use upstream encoding identity and hand-inspected rendered strings; no fabricated
  model-specific golden usage. Whole-render count is authoritative.
- Actual existing-runner episodes: default-cap and explicit small-cap fixtures,
  nonvacuous inclusion/omission in ranked order, public foreign-decoy exclusion,
  real cache miss failclosed, all attempts retained, context scorer observes the
  expected evidence loss. Include no-result and an overlong-first-entry case.
- Actual label and future-query canaries: actor code/current-step inputs, public
  requests, provider inputs/vectors and rendered context stay label-independent;
  environment/cache identity differences remain separate. Future questions reach
  the actor only after the previous step acknowledgment. No broad field drops.
- Legacy track compatibility and targeted Python/scorer/actual-consumer checks;
  no unchanged production Rust library rerun. Rebuild the actual scorer only when
  embedded source pins change; preserve exact prior binary/source evidence.

No implementation begins before this concrete plan is reviewed. Tests remain in
network-none Docker,2CPU/4GiB,one existing private cache,target-specific serial
builds,read-only frozen source and no personal config/credentials/socket/store.
No paid model, dataset download, large weights, training or performance claim.
After publication/results, release completed exact raw roots centrally; keep code,
reports, small next-consumer dependency inputs and one necessary reusable cache.

## Accepted canary comparison contract, frozen before outcomes

Independent legacy episodes generate different real UUIDs. UUID text is rendered
and tokenized, so its BPE length may differ and may even cross a cap/change the
selected prefix. Retain exact per-run UUIDs, entry text, renders, hashes and token
counts. A declared UUID→source-ID bijection compares evidence identity only;
it is never a normalization of token/render equality. Record any selected-prefix
divergence rather than assert invariance or erase it. Deterministically re-render
the same actual public payload, and independently reconstruct each receipt from
that response. Reject internally self-consistent fabricated entries/receipts.
Label canaries still require exact actor current-step inputs/requests and provider
inputs/vectors, with environment/cache identities reported separately.
