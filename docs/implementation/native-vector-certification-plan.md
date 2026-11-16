# Proposed native vector certification boundary

Status: source plan plus a test-only old-executable fence probe. No certification
codec, accepted feature, provider invocation, migration writer, public command or
serving activation is implemented. PR159 is frozen and untouched. This audit
worktree starts at159 only to inspect the latest owned schema; it is not a proposed
catch-all shipping parent. Normative contracts are unchanged.
The final three-gap decisions are in [Final design refinement](#final-design-refinement-after-e1a47ab9-review).

## Decision proposed for review

Prefer a namespace cutover with strict subsequent writer participation. Existing
Bound profiles remain immutable identities, not certificates of old vector
production. Use the existing namespace `required_features` mechanism, with the
proposed exact feature `vector-recipe-certification-v1`. Keep namespace schema2
and existing profile IDs; do not invent a profile version that retroactively
makes known-wrong vectors compatible. Recognizing this feature must require the
complete typed cutover/history validator, never merely add it to an allowlist.

The first bounded AlreadyBound remedy should be actual re-embedding of a selected
namespace in a fresh offline destination. Do not offer an acknowledgement-only
"certify" switch. Existing UnknownLegacy migration/attestation remains its own
explicit provenance boundary; it is not a route around AlreadyBound remediation.
An external-evidence recertifier can be considered later only with a concrete
verifiable evidence contract. It is not included in this first proposal.

| Choice | Persistent/validation cost | Limitation |
| --- | --- | --- |
| Namespace cutover, recommended | One immutable inventory anchor plus subsequent writer proofs; one bounded initial inventory; re-embedding still needs content-free per-row replacement witnesses | Every referenced vector, including retained Ghost vectors, needs a valid remedy before the whole namespace is certified |
| Per-record certificates | Additional persistent per-row state and checks in every semantic selector; mixed certified/uncertified coverage | More invalidation and partial-scope rules; cannot silently omit uncertified requested rows or claim complete duplicate coverage |

A namespace anchor does not make vector replacement O(1): strict reverse history
needs each changed row's previous vector reference/hash and token. The proposed
remedy retains those content-free before/after witnesses. No old vector bytes,
stripped text or labels are copied into receipts. It is not a second mutable
certificate table, a second storage engine or a general public content-update API.

## Exact old-binary fence and actual evidence

Existing `NamespaceSchemaV2::validate` rejects any nonempty `required_features`.
`NamespaceConfig::decode` invokes that validation during native startup's
`validation::preflight_native` on disposable database copies, before
`MetadataStore::open_with_ids`. An optional unknown JSON field is not the fence.

The genuine published159 executable, source3e5fd806, SHA256
`4d4cb1ff49af47f1cce157747d2250a98bb98a0063adcfaf43c2b785a052f10b`, was preserved
before rebuilding the test driver. It has no certification implementation.
The probe uses a real migrated/adopted native fixture and that executable's
existing maintenance process entry. A valid control reaches its canonical
before-commit crash hook (exit91). Adding only the prospective required-feature
sentinel under test ownership makes two subsequent native opens exit101 with the
specific unsupported namespace schema/features error. Every file inside the
source store remains byte-identical on both rejections. This is actual old-code
execution, not a recompiled approximation. The probe passes1/1 in2.95s; see
../evidence/vector-certification-fence/README.md for source/executable hashes.

The sentinel is a negative fixture, not a valid new-format positive or a cutover
implementation. Future positive implementation must additionally prove:

- The feature and its typed cutover receipt are committed together before any
  certified destination can be published. Old code rejects both populated and
  empty certified namespaces before writable open.
- Namespace purge retains the feature in its immutable namespace witness and
  retains the required cutover outcome. After every certified namespace is gone,
  the old decoder must still reject retained required history. Test this with the
  genuine executable; the current probe does not claim that future case passed.
- New code rejects a feature without its anchor, a stripped feature with retained
  new outcomes, and row omissions while the inventory anchor remains. Rewriting
  or removing every trust anchor is outside the authenticity claim.

## Proposed shared fields, not an implemented codec

The required-feature set is exactly the recognized singleton for this first
slice; duplicates and unrelated unknown requirements still reject. Add a typed
optional namespace certification binding, absent on old bytes, required when
certified: `{version:1, origin:GenerationRef, operationId, inventoryRoot,
vectorCount}`. The marker is the old-writer fence; the optional field is not.
The binding points to one exact retained cutover outcome. Never select the
"newest" foreign certificate by comparing counters across different store IDs.

A new required re-embedding outcome retains original contract/source/scope and
normalized operator intent, prior binding, immutable namespace/profile/dimension,
before/after inventory commitments and bounded per-row replacement witnesses.
Each witness has memory ID, old/new record token and old/new vector reference/hash;
it has no retained text, tags, summary or vector bytes. No-op keyed outcomes keep
the original binding while binding the key once. Fresh generated/supplied vector
proofs must be required according to validated namespace state at the operation's
original origin/G; stripping optional evidence cannot regain the old fallback.
Exact serialized names and version dispatch still require joint owner review.

Restore uses the selected snapshot's verified current binding, never a certificate
inferred from retained historical receipts. The first slice explicitly rejects a
pre-cutover selected namespace combined with retained certified-target history
before reconciliation writes or publication. Separate fresh historical restore
may retain an uncertified namespace and its honest semantic restriction. The
existing historical-store promotion restriction also remains. The final design
decisions below specify the exact shared restore seam and acceptance matrix.

## Bounded current inventory and capture

The cutover identifies the original `GenerationRef`, stable namespace ID,
validated raw namespace digest, immutable profile ID/descriptor and dimension.
Names alone cannot select a historical binding, and a reused name cannot inherit
another namespace's certificate. Use the existing owned snapshot/vector-read
seams and validated retired namespace accessor; do not parse a second raw schema.

Enumerate canonical rows, not caches, vector-index candidates or eligible-recall
rows. Count excluded rows against the examination budget. Include Full, Summary
and retained Ghost vector references; Tombstones must have none. A missing,
invalid or corrupt referenced vector, duplicate ownership, wrong slot/epoch,
wrong profile/dimension or hash mismatch is a whole-operation error. Do not turn
invalid rows into an exclusion list and call the inventory complete.

Reuse the exact-snapshot ceilings:100,000 examined records and256MiB of accounted
metadata/vector bytes, checked before copying each vector. Offline disposable
control copies retain159's512MiB combined metadata/edges limit. Count vector
examination separately from returned/copied bytes. Stream the inventory digest
in UUID order over namespace/profile identity, record ID/token, vector slot/epoch,
dimension and exact f32 byte hash; retain only bounded numeric/hash witnesses.
Bound encoded replacement witnesses to16MiB as well, counted before growth/provider work; otherwise reject rather than publish a partial certificate. Budget exhaustion returns an explicit error and no applicable cutover token.
These are bounded first-slice limits, not claims that the existing full canonical
validator is a new bounded-memory validator.

Ownership and actual canonical generation bind the scan. For live semantic reads,
use the real ReadView/publication capture, then recheck actual GenerationRef and
raw namespace/policy binding after selection. An effects epoch is insufficient.
No std lock survives an await. The offline operator holds the same source
StoreOwnership across read-only inspection, provider work and final source
comparison; use private staged handles for generated payload. Concurrent source
writers cannot enter during this explicit offline operation.

## Actual operator remedy

A4 owns parsing and diagnostics under the existing offline schema command family;
A1 owns prepared source identity, strict inventory, durable staged publication
and lineage/history proof. Do not add an unrelated helper-only command or expose
ordinary serving just to exercise the implementation.

1. Require explicit source store/G and namespace, fresh destination, and the
   exact active descriptor matching the immutable namespace profile. Preserve
   the original profile ID. A different provider/profile requires a separately
   reviewed profile migration; dimension equality is insufficient.
2. Validate all existing canonical tables and retained receipts/ledger/effects
   without recovery or source writes. Reject active purge/batch/unresolved
   effects according to their actual shared offline validators. Do not fake
   completion or silently drop pending work to obtain a certifiable inventory.
3. Reconstruct the currently retained document representation, with exact stored
   label provenance, and reuse148's actual `NativeInsert::document_v1` composition
   and provider Document mode. Do not reconstruct deleted full text or claim to
   have recovered the original caller input. Preserve lifecycle timestamps,
   phase, strengths, labels/provenance and immutable identities.
4. Explicit re-embedding replaces vector content. Validate every output's count,
   order, dimensions, finite values and norm through the actual shared boundary.
   A configured descriptor proves configuration identity, not remote weights or
   execution authenticity. Capture an input commitment and output byte hash for
   the actual invocation; caller request identity remains separate and original.
5. A Ghost with stripped source representation cannot be re-embedded from invented
   text. Reject with a content-free remediation reason. The operator may separately
   perform an authorized forget/removal using existing writers, or leave the
   namespace uncertified. Never silently remove records/vectors to pass cutover.
6. Stage replacements and a typed, content-free before/after witness for every
   changed record. Advance affected record tokens for changed vectors; do not
   change lifecycle/policy revisions merely to label certification. The namespace
   feature changes its canonical binding and the receipt advances actual G.
   Final history must reverse replacements before validating old immutable proofs.
7. Validate the complete destination and feature/history fence before publication
   through the existing fresh-destination protocol. Payload fsync and metadata
   commit are not advertised as an atomic cross-file write. Source stays unchanged;
   interrupted destination publication is absent or wholly validated. Retain the
   already-in-hand receipt through known postcommit failures, and report unknown
   outcomes without inventing an operation ID.

The migration preserves lineage and historical receipt bytes; production restore
still forks through the existing restore owner. A cutover binding retains its
original store/G/operation after a fork, without retargeting or rehashing. Current
inventory validation and reverse history must handle foreign policy/maintenance,
purge, linked-store and graph witnesses. Original retry hash/key lookup precedes
fresh provider/configuration checks. No record payload is retained in proofs. Keyed empty/no-change requests must still bind the lifetime key and original outcome once; no silent successful None. Destination-stage/journal retry must reconcile the existing receipt before fresh provider work, with no cross-destination atomicity claim.

## Future writers, removal and slot reuse

First-slice policy proposed: existing uncertified namespaces remain unavailable
for fresh semantic vector writes until the explicit remedy completes. A fresh
correct insert alone never certifies old neighbors. New empty namespaces can
establish an empty, fully validated inventory in their real creation transaction;
absence of vectors does not authorize an inferred certificate on an old namespace.
Metadata/structural writes and exact historical replay remain available where
otherwise supported. Review this admission choice before implementation.

Once a namespace advertises the feature, every writer capable of introducing or
replacing a vector must carry mandatory execution/source evidence through the
shared canonical transaction. Generated evidence comes from the actual prepared
document/provider path, not a user-set flag. Supplied vectors retain their own
explicit profile assertion and numeric checks and must never be labeled provider-
generated. Unkeyed and batch children follow the same rule. The cutover/history
validator determines whether evidence is required at an operation's original
origin/generation; an optional None fallback cannot hide a post-cutover write.

Deletion/phase loss/purge keep ordinary loss masks and existing effect protocols.
A certificate applies to `(namespace, memory UUID, vector slot, epoch, hash)`,
not a slot number alone. A delayed free for removed A cannot affect B after slot
reuse. Reconstruct the anchored inventory through typed content-free history;
validate every current referenced payload. Removal of a baseline row is accepted
only with its actual terminal/loss witness. Unreachable freed bytes never enter
logical export or the certified retained inventory. Policy/observation-only G
changes still invalidate live captures, although they need no provider evidence.

## Actual consumers and ownership

| Surface | Required boundary and owner |
| --- | --- |
| Recall/vector search/similar/origin floors | A2/A4 check complete requested scope and actual origin vectors under coherent publications; allCompatible must not silently omit an uncertified same-profile namespace |
| Exact duplicate scan/advisory | A4 checks all referenced scope vectors, including Ghosts; bounded snapshot errors are whole errors, never false-empty/complete |
| Linked store and linked batch | Curie checks demand after matching replay, before provider/allocation where possible, and at final publication; retained parent/current-child identity survives later failure |
| Automatic semantic graph selection139 | Curie gates `NativeLinkSelection::Semantic` before both origin and neighbor vector reads; Entity/Temporal are structural |
| Existing native_selection capture | Curie separates its currently unconditional cosine work for entity-only/no-advisory settings; never silently drops configured semantic links |
| Native graph/phase/forget/purge | Hume/Pascal/A1 preserve structural/history proofs and exact removal/slot ownership; no provider evidence for nonsemantic graph changes |
| Codec, offline/restore and inventory proof | A1 owns one shared validator/dispatcher; Pascal reviews restore lineage, A4 owns options/capability wording |
| Ordinary startup/public activation | A1/Huygens compose validated consumers; metadata/FTS-only/offline access must not be blocked merely because another namespace lacks semantic certification |

MetadataOnly or supplied query input skips provider work, not semantic admission
when vector comparison is requested. Pure metadata, genuinely FTS-only, export,
inspection and explicit structural graph operations retain support. Keyed
historical replay is not a fresh semantic operation. No new engine or public
activation is included in this audit.

## Minimal parents and required proof before implementation/promotion

Actual sources are separate leaves:159b717 owns current schema/history and policy
adoption;1589c90 includes actual148 fixed document production plus API1 single-store
routing;139098135ad owns automatic graph selection. Git ancestry checks confirm
139/143 are not already contained in158 and148/139 are not contained in159. An
implementation must name the real consumer closure, not assume these mechanisms
were inherited. Start the shared offline/schema work on159, join actual158 for
real fresh-write evidence, and actual139 only with its semantic graph consumer.
Confirm the query admission149 leaf if text-query positive tests need it; do not
import a broad integration checkpoint or154 merely for fixtures. This is a
prerequisite plan, not permission to merge unvalidated WIP.

Before code: review the feature, replacement-witness shape, remedy/admission
choice and owner interfaces together. Before enabling the feature: genuine148
BEFORE vectors reject recall/similar/duplicates/requested semantic autolink, then
actual remedy makes those exact requests succeed. Cover mixed old/new vectors,
wrong bindings, absent/stripped feature/proof, anchored row omission, excluded-row
budget exhaustion before allocation, Ghost source loss, supplied versus generated
writes, batch cancellation/publication, deletion and reused slots, two restore
forks, complete purge, old-binary rejection after terminal history, and exact old
receipt replay. Keep metadata/FTS/structural positive controls. Current source
probe proves the live namespace old-binary fence only; no promotion claim follows.

## Final design refinement after e1a47ab9 review

This section records the coordinating review's three required refinements. It is
a design decision, not codec implementation approval or evidence of passing new
tests. Source basis remains159b717,1589c90 and139098135ad; the genuine old-fence
execution and its limitations remain exactly those reported above. Pascal, A4
and Curie supplied source-level seam reviews; no production code was changed for
this refinement.

### 1. Restore is a pairwise decision before reconciliation writes

On159, `backup::restore::restore_at` holds target `StoreOwnership` and its legacy
directory lock while capturing the target, pinning an archive copy, extracting a
private `TempDir` and validating the selected snapshot. The new pair check belongs
after those validated captures and lineage compatibility, before the first write
in `metadata::restoration::reconcile`, `RestoreJournal::prepared`, `stage.keep`
or publication. Direct reconciliation callers must use the same check. Private
archive copying/extraction and removal of staged derived FTS may already have
occurred; claiming rejection before private staging would be inaccurate. On a
pair rejection, RAII removes disposable staging; source/archive and existing
destination canonical bytes are unchanged and no restore journal is published.
Ownership is retained throughout; no second live open or unlocked reinspection.

The shared check takes validated selected/target certification summaries, keyed
by stable namespace identity and exact original cutover `(GenerationRef, opID)`.
They include retained terminal namespace witnesses, not just live namespaces.
These summaries are inspection results, not caller assertions or persistent
certification authority. The existing archive/lineage/ledger/history validators
remain mandatory; a matching profile alone cannot establish compatibility.

| Selected archive and retained target | First-slice decision |
| --- | --- |
| Uncertified selected namespace; target retains its cutover, even after purge | Reject explicitly before reconciliation writes. Do not choose the target's receipt as the selected inventory's certificate. |
| Different cutover anchors or an unprovable combined inventory | Reject explicitly; no counter comparison across origins, retargeting, or inference from matching profile IDs. |
| Same verified anchor; selected inventory is valid and target contributes supported, validated loss witnesses | Reconcile using those actual loss witnesses, then strictly validate the complete staged result before publication, including remaining Summary/labels/vector references against original evidence. Phase loss alone is not recipe proof; missing or contradictory evidence rejects. |
| Uncertified archive into a separate fresh historical destination | Preserve uncertified state and old receipt bytes; semantic operations remain unavailable. Offline inspection/export remain available, and metadata/FTS remain independent of certification where the existing historical/serving policy permits them. |
| Certified archive into a separate fresh destination | Preserve original certification origin and validate all selected rows/history. A new store ID is not a new certificate. |

No general restore framework, target-vector copying, re-embedding during restore,
or automatic promotion is added. A required positive/negative fixture uses an
actual pre-cutover archive and actual later-certified target, not edited JSON.
Assert rejection preserves both source and target file bytes, leaves no journal
or leaked extraction, and survives a repeated attempt. Test the same case after
target purge and after two forks. Separately test uncertified fresh restoration
with offline/metadata controls and semantic rejection, plus same-anchor loss
reconciliation and certified fork round trips. These are future implementation
tests, not results of the existing old-executable fence probe.

### 2. Owned validation once, checked transitions thereafter

Do not recompute a namespace Merkle tree or reverse all history per request.
Keep the persisted cutover root immutable. Introduce only private, in-memory
validation state owned by the same `MetadataStore`/engine: exact validated
`GenerationRef` and each namespace's verified original cutover identity. An
uncertified namespace is explicitly represented as such; it is not a failure to
construct the metadata owner. This state cannot be installed through a public
setter or reconstructed from a readiness/dirty bit.

- **Owned bootstrap:** while holding the same store ownership used for live open,
  strict inspection validates every current referenced vector and required
  history before constructing this state. Recovery's canonical changes must
  complete and be validated before semantic admission. A new owner, import,
  restore, or uncertain commit cannot reuse another handle's state.
- **Canonical transition:** under the existing `MutationGuard` and canonical TX,
  require the state's G to equal the TX's precommit G. Validate the actual touched
  before/after rows, namespace binding and mandatory execution/removal proofs.
  Read and validate new vector bytes once at the actual writer boundary; use the
  existing slot/epoch/UUID ownership guard for removal/reuse. No unrelated row
  or historical receipt scan is added. Install the resulting state only after
  successful commit, before releasing publication. The synchronous worker owns
  this obligation through async cancellation.
- **Every G participates:** catalog/policy, lifecycle/observation, keyed no-change,
  batch reservation/finalization, purge children and physical compaction also
  advance the validation generation through an explicitly checked transition,
  preserving unchanged certification where appropriate. They do not recertify
  vectors. Unknown variants or an unexpected G fail closed; they cannot simply
  carry state forward. Cutover and certified-empty creation install their actual
  validated binding. Terminal purge retains required history/fence evidence.
- **Failures:** abort leaves the state unchanged. Known canonical commit with
  failed derived publication retains the receipt and checked canonical state,
  but ordinary coherence still fails until actual derived completion. Unknown
  commit, panic, or missing state publication makes the owner unavailable for
  semantic admission and requires owned recovery/strict validation. Never clear
  `derived_dirty` or bump a number to manufacture certification/readiness.
- **Read capture:** extend the actual canonical ReadView/snapshot admission with
  a private certification check at the same TX G. Compare requested namespace
  identity/profile/raw binding against that TX, and copy only the scope's checked
  certificate identities under publication. General queries still require all
  real derived publications to match. Exact scans retain their existing bounded
  vector copying and final G/binding comparison. No std lock crosses await;
  existing bounded stale retries/error behavior remains. allCompatible checks
  every requested compatible namespace, never silently excludes uncertified ones.

Do not clone the entire namespace map when advancing G; update only touched
entries under publication, and capture only the requested entries for a read.
Incremental cost is O(touched rows/proof bytes) for a write and O(requested
namespaces) certification checks for a read, using indexed namespace lookups.
Existing search, vector scans, namespace enumeration and strict validators retain
their actual costs; this is not a claim to remove them or make queries O(1).
No per-write persisted current-inventory root is needed: validated initial state
plus exhaustive checked transitions is the ownership invariant. The disk remains
independently checked by full strict validation on reopen/offline paths. External
rewrites while violating the ownership locks and total replacement of all trust
anchors are outside the claim.

Disclose first-slice strict validation limits: the existing inventory/copy limits
above, plus at most1,000,000 examined retained receipt/progress entries and512MiB
of aggregate encoded history examined for certification, including foreign and
excluded entries. Bound lengths before decoding and account repeated traversal;
stream by original origin/G. Exhaustion rejects with no owner capability or
applicable operator token. This limits the new certification work; it does not
retroactively claim every existing schema validator is streaming or has these
bounds. Do not rescan on each request when the limit is exceeded: leave semantic
admission unavailable and expose the explicit offline limit.

Required proof instruments the new certification inventory/history enumerator:
one bootstrap scan, zero additional full scans across repeated queries/inserts
and metadata-only G changes. Test policy-only G drift, foreign-owner capability,
an unparticipating writer, unknown/postcommit failure, cancellation, derived
publication failure, terminal removal and slot reuse. Each must preserve truthful
receipt/coherence behavior; a stale capability is never a valid empty result.

### 3. Summary execution evidence describes the available document

Reuse158/148's `NativeInsert::document_v1` composition on a temporary input made
from the current canonical summary, ordered stored tags and structured labels.
Do not call `NativeInsert::labels()` to replace `LegacyRetainedOnly` provenance
with `NativeV2`. Reuse existing component hashing where possible, adding explicit
text availability because old `ContentProof.text_sha256 = None` can be unknown.

The proposed content-free execution evidence has these exact semantic fields:

| Field | Meaning |
| --- | --- |
| `recipe = document-v1`, `source = retainedCanonicalV1` | Actual shared composition and source representation used now, not a new profile ID or a claim about the original document. |
| `sourceRecordToken`, `phase` | Exact guarded canonical representation that supplied this input. |
| `summarySha256`, `tagsSha256`, `labelsSha256`, `labelProvenance` | Existing exact component commitments; ordered values/provenance are not silently normalized before hashing. |
| `fullText = Absent` or `Present { sha256 }` | Explicit current presence. Present requires actual selected bytes; unknown/missing referenced payload rejects. |
| `documentSha256` | Hash of the actual composed bytes passed to Document mode, including separators and normalized compatibility labels applied by the existing builder. |
| Output `VectorReference`, immutable namespace/profile/dimension | Exact validated generated output and its new owned slot/epoch/hash. |

A Summary with stripped full text therefore records `Absent` and hashes only the
current summary plus the builder's actual tags/labels suffix. No receipt says it
used the original full document. A Full row with retained text binds the actual
selected text bytes. An unreconstructible Ghost rejects; an old hash is not input
data. The existing profile identity describes the recipe/provider configuration;
this explicit new evidence describes which available input was executed.

Future generated inserts use corresponding actual caller-document execution
evidence; supplied vectors use a distinct caller-asserted profile/source branch,
never a generated certificate. Historical evidence and retry hashes remain
immutable. After later phase loss, validate preserved component commitments and
typed loss history, not nonexistent text. Tests use actual Summary loss, provider
spies over exact bytes, old label provenance, and tampered presence/digest/output.

### Private interfaces, ownership and minimal shipping parents

Names below are proposed interfaces, not new publicly callable helpers:

| Seam | Owner / result |
| --- | --- |
| `validate_current_inventory(owned_snapshot, limits)` | A1: strict inspection returns a non-forgeable validated summary and initial owner state. |
| `validate_restore_pair(selected, retained_target)` | A1 with Pascal review: pure compatibility result before reconcile writes; no certification inference. |
| `prepare_retained_document(record, selected_text)` | Curie/A1: reuse document_v1; return actual input plus component evidence without relabeling provenance. |
| `prepare_reembedding` / existing fresh-destination publication | A1 durable writer with A4 operator options: bounded provider work, exact source CAS, typed replacement outcome and feature in one canonical TX. |
| `validate_transition(before, canonical_tx, outcome)` | A1 shared dispatcher, each actual writer owner supplies touched proof; returns the checked postcommit state, not an arbitrary G setter. |
| `require_certified(scope, canonical_view)` | A2/A4/Curie real semantic consumers: owner/G/profile/scope capability, used together with existing derived coherence. |

Start the actual offline remedy/schema child on159b717 and join1589c90 for the
real fixed recipe and single producer. Include creation/required typed proof,
strict offline/history consumers and the real operator in that slice; do not
publish a recognized feature plus unused validator. Integrate139098135ad only
when wiring its automatic semantic producer, and149 only where the actual query
admission consumer needs it. Prepared consumers may use explicit owned fixtures
without ordinary startup activation. Batch and all other vector writers must
consume the same proof before enabling certified serving; incompatible old
writers are fenced, not silently tolerated. No broad checkpoint or new engine.

Owner reviews: Pascal confirmed the actual restore ordering and immutable reject
seam; A4 agreed on uncertified/fresh restore and scoped semantic restrictions;
Curie agreed on retained Summary composition/provenance and checked transitions
through every writer. These are source/design ACKs only. The final codec and
implementation remain held for coordinating review of this refinement.
