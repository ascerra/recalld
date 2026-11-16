# Pending batch certification consumer: implementation ownership and freeze

2026-11-15. Feynman implementation owner; Curie reviews batch invariants and
Huygens reviews transport projection. This is a source plan, not implemented
compatibility or serving activation. It consumes the actual certification WIP
on159+158, with no additional runtime parent solely for coordination.

Curie's source design is972f8bc1488276cb613c794857f1393fedcb5be0,
`certified-semantic-producers/docs/implementation/certification-pending-batch-bridge-plan.md`.
The coordinator approved its direction, conditional on the interface/cost and
multi-namespace rules below. Fresh reservation admission is separate and currently
under focused validation; it does not make retained pending work disappear.

## Actual BEFORE and immutable history

Candidate026 ran the real batch transport on uncertified migrated Bound vectors:
parent G9→G10, reserved child durablyNotAttempted, responseUnknown with no child
receipt. After close the actual remedy rejected unfinished batch history before
provider use; source files were byte-identical and destination absent. Its final
expected-no-reservation assertion failed. Candidate027 repeated this with only a
test capture addition, preserving a genuine pending store for subsequent tests.

The compressed repository fixture is32746bytes, SHA256
`66645065799277280ce613a30903189edda33ca3c0a67fa84d308443a1894859`,
`src/storage/vector_certification/fixtures/pending-before-certification.zip`.
It contains raw canonical files, the original pending manifest and remedy request.
It is not a logical export or proof that ordinary offline export accepts pending
work. Original source/executable identities and failure logs are retained under
`docs/evidence/vector-certification/runs/candidate026` and `candidate027`.
The failure is in this proposed certification candidate, not a claim that the
previously published unsupported activation path had this production exploit.

## Concrete existing interfaces

- `metadata/batch.rs::check_policy(tx, child)` is used by native lookup and final
  write paths in `native_records.rs` and `native_store.rs`, plus legacy insertion
  and tombstone. Extend this single predicate to receive its existing
  `MetadataStore` owner; legacy callers retain exact policy-hash behavior.
- Keep `replay_child`, `load_child`, original caller hash, input order, child
  identity and full original `BatchDefaults` unchanged. Completed replay remains
  before this policy predicate. Do not add persisted fields to BatchChild or
  BatchProgress and do not rehash old outcomes.
- `history::validate_current` already validates each cutover, source origins,
  current/retired namespace binding, all live references and ledger-backed loss.
  Its serializable `CertificationSummary` is also used in restore inventory;
  **do not add transient bridge state to that serialized summary**.
- Extend the same validation implementation with an optional private owner
  collector. After complete validation, return compact per-namespace evidence
  into existing `OwnedVectorValidation`: original beforeNamespace, original full
  policy hash, expected certificate-only namespace, certification identity, and
  source-origin cutoffs indexed by store ID. No record arrays/document payloads
  or independent generation authority are retained in this cache.
- Every existing canonical-G commit hook continues to advance this same owned
  state. Offline cutover closes before a live owner opens; strict reopen builds
  the new compact evidence. Empty certified namespace creation has no prior
  namespace witness and cannot authorize the compatibility exception.

## One guarded predicate, bounded cost

The existing exact policy hash is the first path. Only native batch2/3 may use
an exception after that fails. Require current live active namespace, actual
owner/control GenerationRef equality, and matching selected certificate. The
child's frozen full policy hash must equal the validated cutover beforeNamespace
hash. Compare the entire current config with beforeNamespace plus exactly the
validated certificate and its required feature. No other policy, revision,
profile, dimensions, creation time, name or feature delta is accepted.

Read the original parent from the bounded keyed batch manifest already used by
child validation; require its exact retained receipt store/G to be covered by
that cutover's source-origin cutoff. An origin cutoff is not permission to resume
an old-store parent after fork: existing current-store lineage checks still win.
Use this exact predicate again in the final TX after provider work. A concurrent
policy change invalidates the final write. New vector-execution evidence remains
required, without changing the original request hash/provider-independent intent.
Native Forget can cross the same exact certification-only delta without becoming
a semantic vector operation; legacy1/4 never gains this exception.

Owner bootstrap uses the existing certification bounds: at most1million history
rows and512MiB encoded history,16MiB per proof,100k current rows/256MiB vectors.
Compact evidence count cannot exceed validated cutover receipts and its combined
encoded namespace/origin size cannot exceed the same512MiB history budget; check
counts/sizes before insertion into the owner map. Do not add a tighter arbitrary
format1 limit. Source cutoffs use a map, not a linear history scan on each child.
The hot predicate performs namespace lookup/config comparison, exact-G checks,
a keyed bounded parent lookup and logarithmic namespace/origin lookup. It never
loads a16MiB cutover record array or scans receipt/history tables. Instrument
bootstrap and hot calls; add unrelated policy receipts and extra namespaces to
prove no new per-query/per-insert full scans. Stale/poison/unknown owner state
stays unavailable, not a permissive fallback.

## Private remedy inspection across every boundary

Introduce one private typed inspection purpose for explicit reembedding with
retained pending batches. Use it consistently in source snapshot, private stage
validation/publication, existing-stage replay and existing-destination replay.
Do not reuse `native_recovery`: repair/effects application remains prohibited.
Ordinary OfflineSnapshot, backup, export, import and restore keep their existing
unfinished-work rejection. No public boolean or unvalidated caller escape.

This purpose still performs all ordinary immutable inspection checks, including
strict manifests/receipts/order/lineage/schema/effects validation. It permits only
validated pending native2/3 manifests whose pending scopes are either exact
current policy or the exact previously validated certification-only delta.
Reject unresolved effects, unfinished purge, unsupported manifests and policy
mismatch before provider construction or staging. Copy all original canonical
rows/receipts/defaults/IDs unchanged. Reembedding modifies only its documented
namespace vector inventory and produces its own receipt. It never executes a
pending child without the caller's original payload.

## Multiple cutovers, mixed namespaces and retired bindings

Preserve current rejection of two cutovers for the **same namespace**. Distinct
namespaces may be remedied sequentially, each with its own original witness and
source cutoffs; never use the latest/global cutover as authority. For pending
[A,B,A], remedy A preserves B's exact old binding and all pending identities;
remedy B then preserves A's validated certificate-only delta. Do not demand all
pending scopes certified to publish A, which would create another deadlock.
Each actual subsequent child still consumes the ordered runner and current
certification. Partial completed prefixes and their receipts remain immutable.

A retired namespace witness is historical validation evidence, not an active
write target. Completed child replay retains existing retired-binding semantics.
Pending insertion cannot be rebound by reused name, and remedy cannot recreate a
purged namespace or missing document. A pending scope without an active binding
is explicitly rejected by this remedy path; actual existing terminal handling
through the original runner remains separate. Do not invent a new terminal error
or bypass active-namespace guards. Strict current/fork history continues to
validate retained retired certificates and ledger-backed vector loss normally.

## Required validation before compatibility handoff

1. Genuine027 parent: source immutable; real remedy; exact parent/default/hash/ID
   bytes preserved; close/reopen; original resubmission commits once with reserved
   child ID and real execution proof; keyed replay provider/clock/G unchanged.
2. Native2 and3, supported original API contexts, supplied/generated vectors,
   pending[A,B,A], partial completed prefix, actual unknown-child reconciliation.
   No missing-payload autoexecution. Keep old completed replay and unknown tests.
3. Changed input/context/CAS, wrong namespace incarnation/profile/dimensions,
   missing/foreign/omitted cutover, parent beyond cutoff, non-cert policy change,
   malformed manifests and missing anchors all reject; fork does not retarget IDs.
4. Real provider latch with intervening policy change rejects at final TX. Actual
   process exits around cutover and child commit preserve original IDs and replay;
   a committed cutover's provider work is not repeated. No power-loss claim.
5. Ordinary offline pending rejection remains. Private remedy rejects unresolved
   effects/purge immutably. After actual completion: strict reopen, logical
   export/import and restore/fork preserve retained history and f32 commitments.

No implementation of this pending bridge starts before owner source review of
these exact live seams. The quiescent slice may be independently reviewable after
its checks pass, but pending compatibility remains assigned work and ordinary
native/public2 activation remains closed.

Implementation refinement after owner ACK: compact evidence retains only full before/expected-after config hashes, the exact certificate and sorted boxed origin bounds. Hashing uses the existing full policy_hash encoding; no partial config subset. No duplicated NamespaceConfig stays in owned state. New evidence has a16MiB heap budget including array boxing/construction overlap, origin arrays, allocation overhead allowance and worst-case transient namespace clone/escaped-JSON capacity, checked before allocation. This additional certification-bridge limit is explicit and does not alter ordinary uncertified format1 import limits. Query/insert paths retain no full witness decoding.

## Implementation and evidence follow-up

The approved plan hash before implementation was cf9055fa6fde75a202900f015adad7519644cd893f5dd77bcc215bfd7be2cd6f. Curie reviewed the implemented bounded seam at f75ad7e58462c0838c02d4e7ff8378d547affbaf; this is a source ACK, not runtime proof. Actual run results, including genuine027/037 artifacts, rejected candidates and exact ownership/heap scope, are maintained in [REPORT.md](../evidence/vector-certification/REPORT.md). The witness16MiB budget is additional collector allocation/construction work, not the whole already resident strict-history bootstrap or process heap. No ordinary native/public2 activation follows from this compatibility consumer.
