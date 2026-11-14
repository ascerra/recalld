# Native API activation handoff

Status at the sealed A4 batch-wire source `ccae79b`, 2026-11-14. This is a
source/owner handoff, not a merged-tree validation or activation certificate.
Published refs below were checked against GitHub. Working follow-ups are named
explicitly; do not substitute a moving branch for its validated handoff.

API version, disk schema, namespace lifecycle and retrieval semantics are
independent. Omitted API remains1. Ordinary native startup must not enable a
store while sending all old callers into legacy-only writer guards. Keep the
legacy successful payload shapes/defaults, and route them to participating native
owners. Any specific unsupported operation needs an explicit documented contract
before startup activation; permanent blanket rejection is not completion.

| Surface | Actual frozen prerequisite | Present boundary and next required consumer |
| --- | --- | --- |
| Single insertion |118 `f910c00d61165500c0483ec2c646de73118242d9` | Accepted namespace-keyed native producer exists. Existing `store_memory` still calls the legacy reserved writer. Huygens owns actual single wire and API1 native routing; consume Curie's validated profile-ID child, not a full descriptor supplied as public evidence. |
| Batch insertion |106 `fe99020926eb81a6994c7e9214ad4623180eeed1`; wire `ccae79b88d348739148e8277933af95301383735` | Private actual MCP/REST/RPC consumer, retained parent CAS and complete response budgets validated. Default API1 batch1 still requires a native owner route preserving its old payload. This wire parent retains106 effects publication; generation-aware completion is a required96 consumer join. |
| Profile selector | Curie `impl/native-single-profile`, typed-error follow-up validating | Public ID resolves from exact retained proof/binding before current provider/defaults; fresh checks canonical Bound identity. Huygens owns batch deferred-ID resolution from retained NativeBatchDefaults and original item hash semantics. No equal-dimension inference, fabricated descriptor or automatic legacy attestation. |
| Catalog create / policy |103 `1b4876d9715361c1ac67358955fb1782d985c845` (actual93+94+96) | Private actual catalog/numeric policy consumers include replay and current-G completion. API1 create still constructs schema-None config through its old adapter; needs mode-aware canonical routing and explicit compatibility tests, including legacy retention endpoints and active provider width. A request's API version must not choose its namespace lifecycle. |
| Forget / feedback |110 `ee0a9063823547d7e28f584aa4b16ac979e28105`;114 `a03b92212cd6650cde3b1096a200bf1fd6922378` | Actual accepted native coordinators exist; old MCP methods still call legacy forget/reinforce helpers. Pascal owns new lifecycle wire modules, typed API adapter ports and dedicated RPCs; use real stable canonical identity, not a reused namespace name. API1 native routing remains required before activation. |
| Retired replay binding |111 `23f99c5e6b3b84f1b21e36fa39e5f314dacac1fb`; owner follow-up75bc validating | Feynman owns validated live/retired binding and completed native purge readiness reading. Pascal consumes the validated seal for forget/feedback after purge. No guessed namespace ID or legacy provenance. |
| Exposure / access recording |119 `8ab79b45cf53f277f96c587808615273b0055882` (actual113+116) | Actual query observer selects native writer from owned metadata mode, requires query policy/publication owner, preserves one sampled time, UUID dedup, per-namespace partial receipts and unknown outcomes. Preserve pre-exposure readG separately from post-selection captureG. Do not convert the internal observation identity into a public query idempotency contract. |
| Get / similar / recall |113 `149beb60365717f706b4a5461564a03a05bbb492`;96 `387c7118864040c68d7d28c44fb2600ade520a45` | Actual coherent payload owners exist;113 similarity still selects V1 semantics. Hume owns real A2 semantics2/current-history/filter-before-caps/budgets/stable UUID ties. Huygens owns public selectors/schema/envelopes. Preserve API1 selection by default; wrapping V1 neighbors is not v2 completion. |
| Metadata / namespace / tag pages |100 `ff577809f8f4b9575ad465ae677ab4bab2b13195`;112 `5951f757afbb4aed68e25703d4108f6dc7b810fd` | Actual same-transaction metadata authority and generation/namespace/filter-bound cursors, canonical labels and truthful counts. Private2 gates remain. Validate existing API1 read payloads on the final ordinary native owner. No vector-copy or active-profile dependency merely for metadata listing. |
| Duplicate exact scan |88 `3961f6653d6c3076bdf9b5b8c27fa6b01a8f0b2e` | Actual canonical vector snapshot consumer, pair cursor, coverage and whole-response bound. Native ordinary owner/public activation and200/2k/20k performance cells remain separate. Not a whole-query ReadView claim. |
| Standalone graph |97 `292fb290e24736f5ff86baf345f5b2794d17aa9d`;108 `b5673abeb39bfc79fa9ce8b22f9f784f2636cdb9` | Canonical typed outcomes/keyed no-op core, not every native public graph mutation adapter. Startup inventory must account for every graph ingress, including background/system callers; either actual participating route or explicit supported-surface exclusion. |
| Maintenance / purge |107 `785dfec4a543b585e3924eb9df298b0d0708d84d`;105 `064686cc5aa7192b45f6e9f75dc6cfd1e7f1f958` | Actual native maintenance/purge owners exist. Feynman owns completed-purge readiness correction and existing system/daemon scheduling/admission. Huygens owns any public batch/partial/unknown projection; no generic completion-stamp advance. |
| Readiness errors | Halley seal `cc865c296247a55cadb09081e349c059c8724619`, actual96 child | Real independent canonical/derived/accepted-work observation API, not yet consumed by all wire failure envelopes. Unknown stamps/count semantics stay explicit; original full receipt remains available even when observation fails. No helper-only readiness advertisement. |
| Owned allocation / clocks |104 plus Halley native allocation and namespace-clock children validating | Consume validated actual owner seams where needed. Existing root must outlive cancelled workers; do not finish unresolved work by assumption. Legacy namespace clocks must come from the owner; constructor clock fields should be reused, not duplicated. |
| Offline / recovery |82 `bb1a0863b2ab0eba940e767941ca4e85fe9b8e77`;111 and validated retired consumer | Preserve crash preflight repair-copy behavior, old immutable receipt semantics, format gates, export/import/fork proofs, deletion evidence and source immutability. MetadataFreeze is not ReadView. |

## Agreed narrow next composition

After the batch seal, Huygens consumes only that actual batch branch and Curie's
validated profile child (which includes118/96). This is the actual dependency for
single insertion wire, public profile-ID resolution and current-G readiness after
single/batch/replay/failure repair, with exact retained receipts. No catalog,
list or119 exposure convenience parents. The batch selector must preserve old
invalid/fallback item hashes and original float bits; descriptor resolution is
after full parent replay, then same-TX fresh binding or retained pending binding.

Pascal's lifecycle wire consumes actual110/114 plus88 shared contract and the
validated retired replay consumer, not batch or119 merely as convenience.
Reserved files are `serialization/native_lifecycle.rs`,
`mcp/native_lifecycle_adapter.rs`, lifecycle-specific API/bridge methods and
`feedback_native_v1` / `forget_native_v1` RPC arms. Huygens retains insertion
wire, `serialization/native_batch.rs`, batch preparation/adapter/budget and
metadata reservation changes. Shared selectors/Envelope/WriteOptions are reused;
public `require_supported`, advertised versions and discovery stay centrally
owned by Huygens. New ports never fall back to legacy RPCs.

Feynman owns existing engine/system/daemon native admission and the complete
writer inventory. Huygens owns API1 compatibility, strict public wire/schema and
activation; Pascal owns lifecycle wire; Hume owns actual A2 selection. Before
opening public2, the final actual composition needs positive public MCP/REST/RPC
conformance, default-API1 native calls, readAccess true/false, same-key replay and
unknown/partial failures, full schema validation and capabilities matching real
callers. No single component's passing suite establishes that combined result.

## Post-cleanup ownership and validation checkpoint

The current single/profile-ID batch composition is frozen for validation at
`d12a9a3`, with actual parents PR122, PR123, readiness-observation `cc865c2`,
and published PR121 `3fa91d4`. It is not yet a validated startup prerequisite.
The single consumer preserves retained namespace identity and full known
receipts; the batch consumer requires the actual current-generation publication
barrier after completing its root, rather than trusting per-item booleans.

Curie owns the next linked-insertion and compound original-API context child,
including a typed caller-vector validation error so HTTP422 does not depend on
parsing storage error strings. Provider-output failures remain unavailable;
corrupt persisted-vector ownership is not caller-invalid input. Huygens owns
public routing, response projection, strict schemas and the final activation
matrix. The narrow plain-record API1 proof does not establish linked-insertion
or compound API1 support.

Pascal owns bounded legacy-disk API2 feedback, exposure and forget consumers,
including necessary private plumbing and the legacy tombstone admission proof.
Feynman owns purge/catalog original-context and startup closure. These are
independent of native-disk API1 compatibility: neither proof substitutes for the
other, and no blanket guard widening is authorized. Legacy receipt-backed
lifecycle data must retain only fields actually present in those outcomes; it
must not invent native strength, phase-history or policy revision.

Hume's frozen PR133 `27a6cd4` supplies actual A2 V2 selection. Its final response
coverage includes post-supplemental graph and supersession counters and omission
diagnostics, not only the earlier pipeline counters. The separately documented
V1 similarity correction filters canonical eligibility before candidate caps;
it preserves legacy ranking/history and default scope while honoring explicit
`sameNamespace:false` only across provably compatible profiles. Neither that
work nor this insertion consumer opens public2 independently.

The next native API1 REST insertion wrapper may add `embeddingProfileId` without
changing existing success fields. Native supplied vectors require an explicit
matching Bound ID: missing identity is a semantic422 with a profile-required
remediation, malformed ID is400 and a different binding is409. Legacy-disk
supplied vectors with no ID retain their documented unverified compatibility
lane. An explicitly supplied ID must be verified or explicitly rejected as
unsupported; it must never be silently ignored. Native generated vectors use
the actual provider identity. MCP API1 single-store's historical ignored
`embedding` field remains unchanged. Tests must cover both formats before
ordinary native startup consumes this wrapper; this is an approved follow-up,
not implemented by the frozen explicit2 wire.
