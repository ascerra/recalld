# Legacy API2: operation admission before another selection writer

Source-only plan, 2026-11-14. Direct source parent is PR157
`5e6115d051fbe102cc22059337e24725aca362be`, containing actual PR148 and PR144.
PR157 remains frozen with Halley; this separate branch changes documentation
only. Its exact-head full CI gate remains in force.

## Contract reconciliation

A4 §9 requires immutable namespace/profile compatibility, not equal dimensions
or whichever provider is configured now. HTTP API2 supplied vectors require
`embeddingProfileId`. The existing unverified/no-profile compatibility lane is
explicitly legacy-disk API1. CONTRACTS likewise forbids inferring unknown legacy
provenance and distinguishes API version from disk/lifecycle versions.

Consequently a durable automatic-link plan would not by itself make an unbound
legacy vector set compatible. **Do not introduce a second legacy profile or
certificate table, new linked-store receipt, or batch format5 just to execute a
semantic operation that lacks the contract's profile evidence.** The earlier
proposal to extend legacy storage selection is superseded by this admission
audit. Honest per-operation rejection can be the correct complete behavior for
that unsupported combination; it is not a promise of later silent support.

This must not become a blanket rule that API2 means native storage. Nonsemantic
operations can retain their actual legacy API2 consumers. Historical matching
receipts must replay before current provider/config/profile/CAS checks, subject
to original-lineage and request-identity validation and current payload visibility.

## Actual source boundary

| Actual source at PR157 | Observation and implication |
| --- | --- |
| `embedding/runtime.rs::check_namespace` | The schema-less lane deliberately remains unverified. A current descriptor only validates itself/dimensions; it does not bind old records. |
| `storage/metadata/insertion.rs::insert_reserved_with_effects` | Accepts original context1/2 and retains canonical checks; schema-bearing namespaces/control2 reject this legacy writer. API2 acceptance here proves mutation identity/atomicity, not semantic profile certification. |
| `storage/insertion.rs::InsertMemory` | Contains the vector and explicit links, but no advisory intent, frozen automatic settings or canonical namespace-profile proof. A plain vector insertion must not be called nonsemantic solely because auxiliary selection is disabled. |
| `mcp/bridge_adapters.rs::store_memory_reserved_with_work` | Child replay precedes the API2 selection guard. Legacy API1 computes advisory from the index before insertion, then discovers semantic/entity/temporal links after commit and logs nonfatal failures. Do not reuse that sequence as API2 correctness. |
| `mcp/batch_adapters.rs::run_batch_accepted` | Fresh format4 automatic/advisory rejection runs after parent replay and before reservation/child IDs/provider work. Already-reserved children retain exact parent/current-child identities when blocked. |
| `api/handlers.rs::create_memory`, `api/adapters.rs::post_creation_links_with_work` | The historical API1 path independently has prewrite advisory and postwrite callbacks. A public API2 consumer must not accidentally fall back here. |
| `graph/autolink.rs` | Legacy helpers use derived indexes/cache, select after creation, and sample independent wall clocks. They are compatibility implementations, not frozen same-insert proofs. |
| `derived_completion/native_store.rs`, `mcp/native_store_batch.rs` | The actual native producer already has replay-first lookup, captured intent, provider work, frozen selection, same-insert proof, accepted work and awaited publication. Profile-compatible migrated stores should consume this path with the original API context. |
| `storage/schema_migration.rs`, `embedding/profile.rs::prepare_attestation` | Existing offline migration publishes to a fresh destination, preserving history and accepting explicit attestation when justified. It is not an in-place relabeling of live unknown vectors. Already-Bound recertification is not implemented by this old attestation helper. |

## Operation matrix and required consumer work

| Operation/state | Required behavior / remaining work |
| --- | --- |
| Legacy-disk API1 compatibility | Preserve the existing unverified lane and documented supplied-ID restrictions. No automatic migration or personal-store changes. |
| API2 metadata inspection, canonical false-mode get, feedback, forget, catalog/purge operations | Dispatch by the actual writer/read capability; do not require vector certification merely because the API is2. Existing lifecycle/catalog/purge owners retain their work. Exposure/feedback policy and readiness remain independent requirements. |
| Explicit structural graph operations, entity/temporal-only work | Do not require a semantic vector claim when the operation does not use vectors. Keep namespace, phase, policy, final-generation and publication guards. Do not silently remove a configured semantic component from a combined request. No new public entity/temporal-only route is proposed. |
| Fresh API2 vector search, exact vector duplicate scan, semantic autolink or vector-based advisory on unknown legacy provenance | Fail diagnostically before fresh provider/selection/allocation; report the actual unsupported profile state and supported operator remedy. Do not return an invented empty result or certify old neighbors from the new query/store vector. |
| Fresh API2 vector-bearing insertion, even with advisory/autolinks disabled | It is a semantic write. A4 owner Huygens explicitly confirmed this interpretation: schema-less/UnknownLegacy cannot provide canonical Bound proof. The low-level PR157 context2 primitive is mutation-identity foundation, not permission to advertise unverified public API2 writes. |
| Completed matching insertion/batch receipt | Return the original numeric/ID outcome before fresh profile/settings/provider/clock checks; never reselect neighbors or recreate removed content. Current response hydration separately checks visibility. |
| Existing pending/unknown batch child | Preserve reserved IDs, parent/bookkeeping receipts and known current-child receipt; unsupported current admission remains unresolved, not rolled back or silently finalized. New public rejection must not discard already committed parent state. |
| Explicitly migrated/attested store | Route to the real participating native consumer using original API1/2 and existing frozen profile resolution. Passing migration is not permission to skip all-writer, historical-producer certification or public activation gates. |

The first actual code candidate is therefore a **small public/typed consumer
admission join**, only where source and BEFORE probes show a bypass or wrong
diagnostic. It should reuse the shared canonical profile authority and existing
receipt lookups, not add durable semantic selection to the unbound legacy lane.
For fresh single calls the check belongs after matching replay and before
provider/clock/allocation; for fresh batches before reservation when the actual
envelope allows that decision. Indexed invalid bookkeeping and an existing
reservation retain their specified outcomes. Repeat any relevant canonical
state/profile condition under the final mutation guard; a preliminary check is
not write authorization.

The fresh-vector-write interpretation is confirmed with A4. Exact diagnostic
vocabulary and any admission hunk still need coordinated source review before
code. Do not flatten profile incompatibility, unknown
provenance, caller-invalid vectors, provider-invalid vectors and canonical
corruption into one 422 or generic storage error. This is a contract/interface
decision under the authorized implementation work, not a request for permission
to run isolated tests.

The remaining audit must explicitly check whether internal typed RPC selectors
can bypass the advertised operation's admission. Do not infer safety solely from
the public parser, or infer a new public contract from the existence of an
internal format4 method. Conversely, do not blindly change every context2 core
primitive and strand already accepted historical work: inventory existing
completed/pending meanings and the exact replay/resume consumer first. A new
fresh gate never erases an existing reservation or declares uncertain work done.

## Concrete transport/RPC gate order to prove before implementation

At157, REST API2 insertion routes through `single_store_v2` to
`StorageEngine::store_native_wire`; MCP uses that same typed consumer. The daemon
method `store_native_single_observed_v1` also calls it. These native paths must
be checked for the right unknown-profile diagnostic and zero fresh work; do not
assume their generic native-owner rejection is the complete A4 response.

The separate daemon `mutate_batch` method decodes `BatchInput` directly and calls
`StorageEngine::mutate_batch`. It accepts internal format4. With autolinking and
advisory disabled,157's legacy runner currently reaches ordinary insertion;
neither that wrapper nor the legacy insertion primitive establishes a Bound
profile. This is the specific source-supported admission gap to probe, not a
request to add a semantic writer. Dedicated `mutate_native_batch_v1` and
`mutate_native_batch_observed_v1` enforce native3 and must not become fallbacks
for format4 or for unavailable legacy operations.

The proposed order is:

1. Decode/validate the original application context, outer shape, scope intent,
   key/lineage/CAS and ordered caller input with the **existing** hash rules.
   Register the actual accepted root before provider/file/enqueue work. Do not
   normalize a new profile/default into the historical caller hash.
2. Use the original known writer/manifest kind's receipt lookup. For format4,
   `reserve_batch_version_with_work` already loads matching parent history before
   its fresh-only settings closure. Unkeyed resume requires the existing parent
   operation ID and lineage, never allocates a replacement. Do not try arbitrary
   hash domains until one matches or coerce a different operation kind to replay.
3. Return completed outcomes through existing readiness/visibility handling.
   Check changed payload and foreign lineage before accepting replay. Current
   profile, provider, automatic settings and clock cannot redefine that outcome.
4. Only for fresh work, classify the **actual operation**. Fresh format4 Insert
   is a vector write even when all auxiliary options are false; it cannot infer
   profile compatibility from a schema-less namespace. Reject before fresh
   reservation/provider/UUID/text/vector allocation. Format4 Forget and other
   nonsemantic operations retain their independent consumer. All-invalid indexed
   bookkeeping must keep its established meaning rather than become an excuse
   to silently drop input or claim a child committed.
5. For an already reserved parent, inspect finalized child state before the
   fresh child gate. Return completed children unchanged. A pending blocked
   child retains its reserved ID, `NotAttempted`/unavailable status, exact parent
   and bookkeeping receipts. A commit-unknown child retains its unknown identity
   until reconciliation; a known committed publication failure retains that
   child's full receipt. Do not substitute the prior parent receipt for it.
6. The actual final mutation boundary must still reject stale/wrong owner,
   namespace, policy and profile evidence before payload stage. Adapter admission
   is not a bypass token. If an admission change affects old pending work, prove
   its recovery behavior explicitly; do not silently finalize it, migrate a
   blocked store, invent a cancellation receipt, or declare a barrier ready.

The first BEFORE probes should exercise this raw format4 daemon route as well as
the normal REST/MCP routes, with supplied and generated vectors and both auxiliary
options disabled. Source evidence is not a newly executed transport result.
Pending-work recovery/diagnostic semantics are a required join review point;
there is no claim that an unsupported pending operation is automatically drained.

## Minimal parents, owners and delivery sequence

1. Keep this plan directly on157. No unrelated native policy, query explanation,
   broad checkpoint or moving batch branch is needed for the audit.
2. Huygens owns upcoming native API1 public batch changes in `mcp/batches.rs` and
   `mcp/batch_adapters.rs`; he acknowledged the ownership boundary. Consume his
   exact validated consumer only if the actual admission join calls its API.
   Native3 + retained original1 needs no legacy4 dependency merely for context.
   Huygens subsequently froze PR158 at
   `9c90f8201d476918077d3d08920fdb989269c205`; that actual single-store consumer
   is a read-only audit reference, not an automatic extra parent of this plan.
3. Feynman owns profile/provenance authority and the separate native Bound-but-
   uncertified proposal. He recommends the existing migration/justified
   attestation route, otherwise real re-embedding; no second legacy authority.
   His current inherited-policy work does not edit legacy compound/effects.
4. Curie owns the actual insertion/graph producer admission joins and regression
   proofs once the required operations and shared seam are fixed. No competing
   codec or receipt-kind implementation starts from this document.
5. Ship a runtime child only with an actual caller and reproducible correctness
   failure fixed. If the current per-operation guard is already correct, record
   conformance evidence and improve the exact diagnostic/operator guidance
   without pretending an unused helper closes a missing writer.

## Proofs to run for that bounded join

- Actual MCP/REST/daemon unknown-profile semantic requests: no provider/clock,
  payload/record/receipt/G changes on fresh rejection; stable typed diagnostic.
  Disable advisory and autolinks to test vector-write admission independently.
- Nonsemantic API2 controls on the same legacy store still operate through
  their real writers; false-mode point reads remain write-free.
- Genuine old1 and retained legacy2 completed receipts replay full outcomes,
  with changed current config/provider and zero fresh calls. Changed request or
  foreign lineage still conflicts. Existing pending parents retain exact state.
- Configured semantic work cannot disappear into an entity/temporal-only success.
  Requested-empty advisory is distinguished from disabled only after a real
  authorized selection, not by fabricating a report.
- Actual explicit migration/attestation round trip preserves raw history and
  produces the expected profile state. Unknown/unjustified provenance remains
  blocked; no inference from dimensions, current provider or new fresh output.
- On a bound native consumer, ordinary provider/selection/final-commit races,
  cancellation/unknown/publication faults retain existing receipts, accepted work
  and graph→storage order. Add only tests touched by the actual join.

These are proposed tests, not executed results. PR157's tests and CI are separate.

## Separate native producer-certification gate

PR148 corrected the fresh document-v1 provider input but did not persist an
executed-recipe marker or certify old generated vectors. Existing Bound profile,
vector hash, namespace provenance, receipt generation and binary version cannot
establish that missing fact. Known-wrong BEFORE vectors cannot be blessed by
operator acknowledgement; independent evidence for that exact current inventory
or actual correct re-embedding is needed, otherwise semantic admission stays
closed. Feynman's proposal must cover the required old-binary fence, removals,
slot reuse and future participating writes before a new certificate codec.

That gate also covers old neighbors used by native semantic autolink, advisory
and exact duplicate scans, including retained Ghost vectors where the operation
allows them. True FTS, metadata, structural/entity/temporal operations do not gain
a new vector dependency. Native selection's current unconditional cosine scan
needs explicit demand separation if that consumer is changed; silently dropping
requested semantic work is not a solution. No such implementation is claimed here.
