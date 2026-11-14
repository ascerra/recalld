# Native insertion batch wire and retained reservation CAS

Approved actual parents: PR106 `fe99020926eb81a6994c7e9214ad4623180eeed1`
and PR88 `3961f6653d6c3076bdf9b5b8c27fa6b01a8f0b2e`; local join `0f1b322`.
No catalog/policy/list or moving writer ancestry. Implementation and tests are
in progress; this document is not a passing validation claim.

## Retained precondition

Private batch writer version3 gains optional `reservationPrecondition` with
`proofVersion:1` and decimal `expectedGeneration`. It is absent when unused,
retaining old1/2/3-None encodings and hash meaning. Some proof is permitted only
for version3; explicit null, unknown fields/versions, invalid ranges or stripped
proof fail closed. No public version bump or activation is implied.

BatchInput forwards the original precondition to the parent context. The same
reservation transaction persists its manifest, proof and receipt. The existing
length-delimited hash includes the expected generation. Matching key or explicit
operation-ID replay checks that full identity before fresh CAS or mutable
settings. Child contexts and rejection bookkeeping retain no CAS. New CAS is
not silently accepted on old public v1 batch dispatch.

Validation reconstructs the parent context with the retained precondition.
Checked expectedGeneration +1 must equal the ORIGINAL reservation receipt's
committedGeneration, including after a fork. Current fork generation is not
substituted. Existing ancestry constraints and completed-foreign-batch checks
remain. No receipt or child hash normalization changes are intended.

Curie and Feynman reviewed the bounded plan against frozen106, not mutable join
contents: BatchProgress lacked retained precondition; BatchInput::context fixed
None; metadata/batch.rs rejected native Some and reconstructed None at startup.
Their review requires original-origin equality, strict restored proof validation,
operation-ID full hash checks, and preservation of old omission semantics.
They own no duplicate edits. Actual patch source review is required before PR.

Required proof: fresh/stale source immutability and no provider work; matching
replay after later writes; changed/omitted CAS conflict; concurrent reservations;
null/stripped/changed/future proof rejection; process exits before/after parent
and child commits; reopen/export/import/fork retention; genuine old-None artifact
compatibility. No generic mocked-ready or new-key retry claim.

## Actual public consumer

Planned insertion batch contract2 maps internally to the frozen native3 ordered
runner. It retains every indexed outcome, full original receipt, reservation
identity, pending/unknown state and advisory coverage. All-ready200, mixed or
committed-unready207, all-final-errors422, zero-success-unresolved503; non-ok
MCP isError true. No aggregate committed generation or atomicity claim. Strict
shared parsing and receipt-preserving errors serve MCP/REST/dedicated RPC.
Public contract2 remains closed pending complete activation gates.

Single store is NOT a one-item catalog batch: Curie owns the accepted
namespace-keyed producer. Native forget/feedback/exposure use Pascal accepted
owners. Private writer2 forget batches do not gain CAS through this proof;
that remaining contract must be explicit rather than silently discarding CAS.

## API1 on native-schema stores

API version, retrieval semantics, lifecycle and disk versions are independent.
Omitted API remains1. Successful v1 payload shapes/defaults must survive native
owner routing. Disk schema2 never selects explicit-feedback lifecycle policy by
itself. Startup cannot enable a store while accidentally stranding all v1 callers
on legacy-only writers. Feynman owns startup; A4 owns public compatibility
routing; explicit unsupported operations require an agreed contract.

Known frozen boundaries before this child:

| Surface | Actual owner/reference | Remaining native API1/API2 routing |
| --- | --- | --- |
| Batch insertion |106 internal run_batch/private3 | This child wire/CAS; legacy batch1 payload compatibility on native remains a separate required route |
| Single store | Curie planned106+96 accepted namespace-keyed producer | No single-as-batch substitute; wire awaits actual owner |
| Catalog/policy |103 accepted owner wrappers | API1 native compatibility and final shared activation composition |
| Forget |110 accepted native core | Public wrappers/context and retired-name proof join |
| Feedback | Pascal native accepted child | Public reinforce/quality/context; no legacy method fallback |
| Exposure | Pascal after feedback; Curie/Hume consume | Same query sample clock, returned UUID dedup/namespace groups/full partial receipts; false writeszero |
| Recall/get/similar |96 coherence, Curie frozen8a0d260 wrapper stillV1selection | Native exposure and actual A2semantics2 budgets/filter-before-caps/current-history/ties |
| Purge/sweep |105/107 actual owners | Wire/full partial and unknown outcomes |
| Failure observation | Halley frozenf8765b2 awaiting full seal | Unknown applied stamps stay unknown; receipt independent of observation availability |
| Ordinary native startup | Feynman existing owner/daemon | Complete per-operation API1/native routing and all-writer/coherence matrix |

A permanent unsupported gate is not final A4 completion. Public schema,
discovery and capability activation follow actual native transport tests.
