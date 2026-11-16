# Next actual A6 controls after the owned-pipe endpoint

Progress at d09ca498: retained all-view capture leases, timeout/join control exclusion,
shared lifecycle clock and canonical allocation injection have actual core/private
consumer proof. Snapshot/reset/caches and suffix replay remain unimplemented in
that measured source. The canonical staging child is separate and unsealed. See
[owned controls report](../benchmarks/owned-controls/REPORT.md). Halley owns IDs;
Curie has separately accepted get/similar coherent payload consumers on PR96.

Original ownership plan (superseded where noted above): Halley retains same-core constructor, coherent search/payload,
registry/readiness, frozen checkpoint/reset and shared clock/ID/RNG until an
explicit owner handoff. The ID/RNG sidecar is disjoint and available, not blocked
on the endpoint. Base bb1a0863b2ab0eba940e767941ca4e85fe9b8e77; exact allocation
boundary is [owned-ID sidecar boundary](owned-id-sidecar.md). No parallel ID edits exist.

1. Wire owned runtime sources into the real constructors. Preserve Recalld::new
   production defaults and its existing with_clock seam. The isolated constructor
   owns one ManualClock and one scoped allocation source on the existing metadata
   owner. Route memory, operation/store, batch/purge continuation and selected-text
   identities through that source. Pass the existing sampled operation time;
   never add a second clock or process-global RNG override. Checkpoint a versioned
   cursor under the freeze; install it only on a fresh restored owner. Inventory
   noncanonical request/session IDs and retry jitter separately. Actual ordinary
   insertion, keyed replay and failure/collision tests must prove the wiring.

2. Add one retained frozen-core callback to the current coordinator, not another
   registry. Close/drain all accepted roots and descendants before publication
   and graph/vector/FTS/entity locks, then the actual engine guard. Compare actual
   canonical GenerationRef with all required stamps and real effects/purge/batch
   progress. Keep closure and guards through capture; cancellation must not release
   them while a blocking copy still runs. Deadlines are monotonic/cooperative,
   with loop checks; no hard deadline promise over blocking OS I/O. This extends
   the current readiness observation into an actual capture boundary.

3. Implement A1-owned canonical checkpoint under that boundary. Sync payloads;
   copy meta.db and edges.db through disposable-copy inspection only, then selected
   text descriptor/file + fulltext gate and validated namespace vectors.dat files
   (free/ownership bitmaps are inside vectors.dat). Preserve all canonical tables,
   including receipts/keys/ledger/lineage/schema/policies/catalog highwater and
   completed batch/purge history. Never copy unknown untracked directories or
   label an unchecked live copy a snapshot. Close the disposable DBs and strictly
   validate the entire fresh stage with normal OfflineSnapshot/canonical_files;
   compare captured actual G. Publish a manifest only after validation. Invalid
   stages are discarded; no repair of original files. Feynman confirmed this seam.

4. Capture runtime state with that checkpoint: clock, allocation cursor, exact
   config/profile/index versions and behavior-affecting caches. Audit cache
   semantics and add actual export/restore or an explicitly declared empty-cache
   policy proven equivalent for the track; do not silently omit caches. Compute
   a stable canonical logical hash over typed values and references, not redb
   physical layout and not an effects_epoch substitute. Unknown required state or
   unsupported participant blocks snapshot success. Enable capabilities only for
   controls whose real consumer and negative tests pass.

5. Reset through the isolated supervisor/constructor, as CONTRACTS requires:
   terminate and join the prior child/workers, clone a validated immutable
   synthetic checkpoint into a fresh inaccessible root, restore clock/config/ID
   state/caches before opening the new owner, rebuild actual views, then verify
   barrier and canonical logical hash before admitting actor actions. Never
   overlay a live instance or use ordinary reopen as a reset. Preserved lineage
   is confined to this isolated fixture domain; production restore keeps its
   independent fresh-lineage semantics. The helper owns the serialized supervisor
   consumer after its validation-only integration task, while Halley owns the
   constructor and checkpoint correctness.

6. Prove the actual round trip: public store/supersession/query/feedback/forget
   and scripted time advance/sweep, snapshot, execute suffix, terminate/reset,
   execute the same suffix. Compare complete canonical logical state, receipts,
   IDs/references and ordered results. Include blocked-writer/transitive drain,
   pending work, copy corruption, foreign lineage/schema/config/cursor mismatch,
   old-worker exclusion and no-existing-destination-change negatives. Stable
   neighbor/equal-score tie rules and native-v2/read-view participation remain
   prerequisites for general deterministic replay; a seed or normalized UUIDs
   cannot substitute. Keep stronger get/similar payload capture and actual native
   catalog/batch/policy joins visible. No paid models, dataset downloads or RL.

Shipping unit: controls with their actual same-core public consumer and round-trip
proof, not an unused allocation/registry module or a permanent capabilities-false
foundation. Staged checkpoints may be validated internally while full snapshot/
reset capability remains gated; manifests must state the exact implemented scope.
Human judge calibration and semantic-quality experiments remain external gates.
