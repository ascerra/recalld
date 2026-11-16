# Bound vectors and historical producer certification

Source-only activation audit, requested during inherited-policy adoption. No
profile codec, receipt or activation behavior is changed by this note. Owners:
Feynman A1 startup/history; Curie actual148 producer; Huygens profile/public
admission. Proposal coordination is pending, not implementation approval.

## Actual source evidence

The published producer correction's runtime is
`16c1cb163db807d70413f3e5d65fbb569eb790bd`, with frozen148 head
`508d98c65b0f824bbcc0290812c63e0dea426edd` confirmed by Curie. It changes fresh native single/linked/batch provider
inputs to `NativeInsert::document_v1()`. The old single path supplied full text
alone, or summary alone. No retained producer-certification marker is added.

At actual143+135 parent0d50, `GeneratedEmbedding` contains vector values and the
advertised profile. `VectorReference` retains slot, allocation epoch and vector
SHA256. `ContentProof` retains the profile ID and component commitments;
`NativeRecordWrite` has no provider input recipe/version proof. Namespace
`BindingProvenance::NativeV2` distinguishes creation from operator attestation,
but does not distinguish the old incorrect native producer from the corrected
one. The original request hash is caller intent; it must not be repurposed as
proof of the provider's actual input or rewritten after the fix.

Therefore merely reopening with corrected148, matching dimensions/model/profile,
or comparing vector hashes cannot certify the missing fact. Epoch/G is ordering,
not a software-build boundary. A mixed namespace may contain old and new vectors.
Do not infer production build from wall-clock timestamps, flattened lineage or
receipt-generation cutoffs.

## Existing remediation is insufficient for this case

`embedding::profile::prepare_attestation` only accepts UnknownLegacy and returns
AlreadyBound for an existing Bound namespace, even if its advertised profile is
the same. The schema migration operator cannot silently reinterpret already-Bound
history as a fresh attestation. Existing enum Reembedding by itself is not an
implemented re-embedding writer or evidence of provider input correctness.

Before ordinary native/query activation, agree an explicit admission/remediation
consumer. A candidate boundary is an operator-reviewed recertification or actual
re-embedding of the affected current vector set, bound to its canonical source,
namespace/profile/dimension and retained ownership/hash inventory. Original
profile IDs and historical mutation receipts remain immutable. Re-embedding may
only use retained payload, never resurrect stripped text/labels; insufficient
retained content requires explicit operator evidence or continued semantic
rejection for that namespace. Old receipt replay and raw/export/restore history
remain available, and the personal physical-legacy/API1 lane is unaffected.

The existing codecs do not let us distinguish all old/new generated rows. Any
required additive certification proof must be reviewed with the owners before
implementation. A report-only warning, or declaring all Bound vectors safe, does
not close this activation gate. New fresh producer output also needs a durable,
validated admission path; an unrecorded claim that a root was created by a fixed
binary is not sufficient across restart/import/fork.

## Compact owner proposal (source-only, not a codec commitment)

Curie confirmed frozen148 `508d98c65b0f824bbcc0290812c63e0dea426edd`, runtime16c1cb16,
and genuine BEFOREd9e629c4: actual single+batch provider spies failed2/2. The
old receipt fixture exporter is2550da536db05c1c430d45b506b728fbb52c302f. A4
independently corroborated the lack of persisted executed-recipe evidence.
Both owners agree Feynman consolidates admission/history design; Curie retains
producer implementation and A4 retains public selection/routing. No competing
profile codec is being implemented.

| Option | Initial operator work | Future writers / validation | Cost and limitation |
|---|---|---|---|
| Namespace cutover proof | Explicit owner/source-bound inventory root+count over every retained vector's record identity, ownership, dimension/profile and hash; operator attestation or actual retained-payload re-embedding | Required receipt-backed evidence for every subsequent vector introduction/replacement, plus existing deletion/phase-loss history; prove the current set from the cutover and subsequent participating writes | One initial O(N) review/scan; compact namespace anchor. Avoid recomputing an O(N) root for every insert by using the existing receipt/reverse-history machinery, subject to bounded-validation review. A fresh insert does not bless older rows. |
| Per-record certificate | Explicitly recertify or re-embed each retained vector; uncertified status stays explicit | Each vector carries its own bound certificate, validated with current ownership and original recipe/source evidence | Additional O(N) persisted state, lifecycle/restore/ledger/reused-slot consumer work and per-row admission. Easier mixed-state representation, but semantic scans must still reject incomplete requested scope rather than silently exclude uncertified neighbors. |

Prefer evaluating the namespace cutover plus required future-write proof first:
it avoids a second independently mutable certificate table and matches the
existing strict receipt/current-record validator. This preference is conditional
on proving bounded complete inventory/reverse validation and genuine old/new
writer composition; it is not authorization to invent a new generic proof engine.
Do not claim a namespace root alone fixes later insertions. The alternative may
be needed if exact per-row recertification is an actual operator requirement.

The operator proposal must distinguish (a) external attestation of the current
vectors' claimed recipe and (b) actual re-embedding from retained input. It cannot
recover stripped fields or pretend current bytes reveal the old provider input.
The existing AlreadyBound rejection remains until a reviewed explicit consumer
implements these actions. The original profile ID and every historical receipt
remain unchanged. Any new current proof identifies original source lineage and
scope and survives import and multiple forks without retargeting its origin.

Future generated writes must bind the **actual composed input** used by the
provider, advertised profile, normalized vector ownership/hash and relevant
canonical record tokens in the same final transaction. Supplied-vector writes
need a distinct explicit supplied provenance assertion and shape/norm checks;
they cannot manufacture provider execution evidence. The immutable caller hash
remains the caller's original input contract, separate from new execution evidence.
Raw/internal writers must not bypass the rule. An unkeyed write still participates.
Matching old-key replay is retained before fresh certification/admission checks.

A required capability/format fence must make older native writers reject a
certified store before writable open. Review the existing required namespace
features, strict native proof validation and old physical-format gate before
choosing a new field or version. A new process version, date or inferred G cutoff
is not evidence. Tests must use an actual old binary to prove it cannot append an
uncertified vector after cutover. Do not rewrite historical outcomes to retrofit
this requirement, and do not claim authenticity if all trust anchors are replaced.

### Dependent admission inventory

The complete requested vector scope must be certified before recall/vector search,
by-ID similar search, exact duplicate scans, vector advisory selection, and
semantic autolinking against retained neighbors. A fresh correctly generated
query/store vector does not certify those neighbors. Configured semantic autolink
or duplicate work cannot be silently dropped, converted to a false-empty answer,
or replaced with a subset to avoid the gate. Curie/A4 must identify the actual
selection entry points and ensure rejection occurs before provider/allocation or
canonical writes wherever the original requested operation can be rejected then.

Pure metadata reads, genuinely FTS-only execution, offline inspection/export,
existing receipt replay, and structural graph operations that do not use embedding
semantics should retain their current supported behavior. Entity/temporal edges
need source inspection: exact label/time matching is not vector similarity, but a
compound request that also asks for semantic neighbors must enforce that part.
This audit does not change personal physical-legacy/API1 compatibility.

### Proof gates before activation

Use genuine oldBound148 output: reject semantic serving, duplicate scan and
requested semantic autolink before remedy, with no source mutation/provider work
where admission can precede it. Then perform the real explicit remedy, prove the
same requests succeed, and preserve old receipt bytes and old-key replay. Include
mixed old/new vectors, fresh generated and supplied vectors, removed payloads,
wrong namespace/profile/dimension/slot/hash, stale owner/G, crash publication,
old-writer refusal, deletion/purge, logical export/import and repeated fork.
Metadata/FTS-only/structural positives remain separate. This activation gate is
owned and unresolved; current inherited-policy implementation does not close it.

A4's source review adds three coverage requirements: exact scans include retained
Ghost vectors even though recall eligibility differs; allCompatible may not
silently omit a same-profile uncertified namespace and report complete coverage;
similar's inherited floors need certification of the actual origin vectors, not
only returned terminal records. MetadataOnly or supplied query input avoids a
provider call but does not waive certification when semantic vector work is still
requested. New execution evidence can certify configured recipe participation,
not authenticity of remote weights/model revisions.

Curie's actual selector trace at148/current identifies the insertion seams:
`derived_completion/native_store.rs::store_native_accepted` performs historical
lookup before capture, provider(document_v1), native_selection capture and prepared
link application. Fresh certification admission belongs after matching replay and
before provider/capture work, with a final canonical proof recheck. In linked
batch children, `mcp/native_store_batch.rs::run_linked_batch_child` resolves the
profile then looks up the child before capture/provider/selection; an already
reserved parent/child identity must survive a later admission failure.

`graph/native_selection::capture` currently reads canonical vectors and computes
cosines even for entity-only/no-advisory settings. A broad guard there would
therefore accidentally reject operations intended to be structural. Review the
actual demand and ordering rules and deliberately separate work that needs vector
semantics before gating; do not silently change requested linking or pretend the
current unconditional scan authorizes blocking all metadata/structural operations.
Curie retains this producer/selector integration, with A4 request-shape review.

Curie's separate actual139 producer source, frozen
`098135ad5d8d6c4ccbe9f46640ebeb6ed34de375`, has a narrower structural boundary:
`graph/native_producer.rs::select` reads vectors only for
`NativeLinkSelection::Semantic`. Admission belongs before both origin and neighbor
vector reads in that branch. Entity/Temporal selection uses labels/times and stays
available. The actual entry is
`DerivedCompletion::native_graph(GraphRequest::Automatic(...))` through
`native_graph_accepted`; it rejects keyed volatile selection before clock sampling
and checks the final selected generation. Explicit structural requests and receipt
replay are separate. The existing Bound check alone is insufficient evidence.
This139 source is separately attributed, not claimed as148 ancestry.

Operator recertification must attest independent evidence of recipe-compatible
inputs for the exact current vector inventory. Acknowledging that the known wrong
producer was used cannot certify its output under the unchanged descriptor. The
genuine148 BEFORE fixture needs re-embedding from correct retained payload, or
independent valid vector provenance; otherwise semantic operations remain rejected.
Fresh inserts do not certify older neighbors. Curie and A4 reviewed these source
boundaries; this proposal introduces no codec or activation change.

Implementation is not approved by this source-only proposal. Before any codec
change, choose the exact existing required-feature/version rejection fence and
run a genuine old binary proving rejection before writable open; an optional
unknown JSON field is insufficient. The concrete design must bound current
inventory validation and cover removal, slot reuse and every future writer, with
explicit operator remedy semantics. Namespace cutover is only the conditional
preference until those points have an executable proof. Descriptor/digest equality
alone does not attest provenance or model authenticity. Finish policy adoption
independently; no certification schema is included in that slice.

Curie's separate legacy-disk API2 inventory has the same evidence problem without
native Bound metadata: schema-less namespace rows have no immutable profile.
A1's source-only recommendation is to reuse the existing explicit offline,
fresh-destination schema migration and justified profile-attestation path, or
actual re-embedding, rather than introduce a second legacy profile/certificate
table. A4 section9 expressly reserves the unverified compatibility lane to
legacy-disk API1. Unknown-profile semantic/advisory work must reject with a useful
remediation diagnostic before fresh work; supported nonsemantic legacy API2
operations remain independently routed. This is a per-operation admission
boundary, not a blanket assertion that API2 requires native disk. Main/A4 review
of this proposal and the future producer consumer is still required. Existing
AlreadyBound attestation cannot repair known wrong native148 vectors.
