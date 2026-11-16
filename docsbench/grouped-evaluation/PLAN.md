# Actual grouped-evaluation consumer plan

Planning only; no new runtime implementation or experiment yet. References:
the original-repo A6 specification §§4–7/10, CONTRACTS revision1, and the A6
acceptance inventory dated2026-11-14. This closes a bounded implementation gap,
not a dataset, quality, lifecycle-promotion or learning gate.

## Minimal actual parent and ownership

Direct base: published #90 `003d857dd99acca9eb33801fa6c5d7b137d124a2`.
It already contains the actual manifest/scorer/turn-ledger consumer and corrected
current-question-only separate actor. Its `scripts/public-mcp-evaluation.py`
executes production MCP stdio and the real `examples/score_mcp_episode.rs`.
The scorer reuses `examples/support/source_scoring.rs` for unique-source metrics.

#92 `0084ad30d768747b13c2815d838be8f0feaee284` adds lifecycle/restart action
fixtures; it is not needed to group independent evaluation episodes or aggregate
their existing scores. #99/control/reset/native/sweep/compaction and #153 public
lifecycle writers are likewise not necessary parents. Do not stack them for
convenience. PR154 remains READY/FROZEN with Halley and will not be edited.

Worktree: `/tmp/recalld-implementation/a6-grouped-evaluation`, branch
`impl/a6-grouped-evaluation`. No shared initializer, production action handler,
clock, receipt, retrieval or restore changes. Huygens/Pascal/Feynman retain those
seams. The next native-writer consumer waits for its actual validated adapter.

## Versioned compatibility

Add a separately named `serialized-mcp-grouped-synthetic-diagnostic-v1` track
to the existing runner. Keep manifestVersion/sourceMappingVersion/traceSchemaVersion
at1 and introduce explicit groupMapVersion1, splitMapVersion1 and a separately
named grouped aggregation profile. Per-query scoring remains the existing
`synthetic-unique-source-v1` definition.

Preserve the old `serialized-mcp-synthetic-diagnostic-v1` profile and fixture
behavior unchanged. Old manifests do not acquire grouping claims or new output
denominators. Same-binary old-profile positives and otherwise-valid manifests
with missing/unknown/mismatched grouping versions prove the boundary. New
grouped-only fields on the old profile fail closed.

Both profiles remain explicit public API1 legacy diagnostics: canonical schema
is absent/null with the existing unattested representation, never invented0/2.
No clock/RNG/reset/barrier/ReadView/public2 or full-A6 guarantee is added. Requests
claiming those unsupported capabilities continue to reject before side effects.

## Group/split inputs and deterministic construction

Use a small checked-in/generated synthetic cohort, initially seven episodes
forming five declared components, including an exact-history duplicate pair and
a declared shared-history/alias pair. Distinct source content supplies the other
components. No upstream URLs, revisions, licenses, model identities or dataset
semantics are fabricated. Split labels do not authorize training.

Before creating embedding-cache artifacts or executing any episode, build a
source-only registry and connected components from typed shared-user,
conversation/evidence-history keys, declared alias relations, and recomputed
exact-history fingerprints. Namespace relation keys by kind. Reject unknown
references, duplicate episode/source identities and ambiguous fields.

Pin the fingerprint method and its exact input fields: ordered actor-visible
role/namespace/content bytes, excluding source-ID aliases, split labels,
evaluator answers/annotations and future questions. Identity/timestamp metadata
is retained separately, not rewritten. Reordering registry entries cannot change
the result; chronological content order remains significant. This detects exact
histories plus declared relations, not an invented semantic near-duplicate
detector or a claim of real-world contamination freedom.

Canonicalize component members in stable ID order and hash that representation
to derive group IDs. Freeze the group map before scores. Assign whole groups by
sorted SHA256(`a6-split-v1` concatenated with group ID), with declared floor
rounding for60% train/20% development and remainder test; record exact counts and
assignments. The five-component fixture yields3/1/1. No balancing after scores.
Synthetic execution may exercise all assigned splits, but has no training,
tuning or pristine-holdout claim; aggregate outputs remain partitioned by split.

Preflight recomputes maps from source inputs and compares their declared hashes
and assignments. A duplicated or transitively related history assigned across
splits rejects even if the attacker recomputes all supplied artifact hashes.
Cache availability cannot override this gate. Grouping maps/relations and split
labels remain evaluator sidecars, not new actor instructions or canonical data.

## Actual existing runner and scorer integration

Extend the existing manifest dispatch, rather than create a second episode
engine. Validate the whole cohort, all artifact/source/executable pins and all
group/split relationships before the first output directory, provider or store.
Only then invoke the existing serialized episode implementation sequentially,
one isolated store per episode. Each actor still receives ingestion input and
its current question only; cache misses still error without fallback.

Bind cohort/episode identity and group/split-map hashes into the environment
identity, trace group IDs, source mappings and evaluator score inputs. Preserve
all planned/attempted/failed episode counts and partial evidence. A missing,
failed or duplicated episode cannot silently disappear from a successful
aggregate. Keep actual public UUID mappings and ledger provenance; no fabricated
GenerationRefs, readiness, receipts or canonical hash comparison.

Add one consumed grouped-input mode to the existing scorer executable. It checks
the frozen episode roster and hashed raw score inputs, then reuses the existing
per-query metrics. Output query-micro and group-macro summaries per split, plus
per-group query/scored/zero-label/unknown counts and effective scored-group count.
The macro denominator contains groups with at least one scored event; all-unknown
and zero-label-only groups are explicit and unscored, never zero or perfect.
Canonical group/episode/query order fixes floating accumulation order. No
10,000-resample bootstrap, confidence interval, reward or new quality claim yet.

## Meaningful validation

- Permuting registry/relations yields identical group IDs, assignments and
  hashes; aliases and duplicate-history chains stay in one component.
- Correctly rehashed cross-split duplicates/aliases, wrong map/version/hash,
  missing/duplicate members and unsupported profile claims reject before
  output/provider/store work.
- Hand calculation with three perfect queries in one group and one empty query
  in another gives micro0.75 versus macro0.5. Retain perfect/partial/duplicate/
  empty/unknown/zero-label cases, unequal group sizes and all-unscored groups.
- Real cohort episodes produce public UUID mappings, ledgers, source-bound scores
  and split/group summaries consumed by the actual scorer. Existing old-profile
  execution and scorer goldens remain valid.
- Label perturbation cannot change source-derived maps/splits or actor inputs/
  requests/vectors; future questions remain withheld until delivery. Environment
  hashes/cache keys may change with evaluator artifacts. No exact independent
  UUID/clock/output equality claim without owned reset; retain observed outputs.

Use targeted Python grouping/preflight/actor checks, Rust scorer-example tests,
fmt/clippy and actual isolated public episodes. Library source stays unchanged;
do not repeat the broad inherited Rust suite merely for evaluator/example work.
Retain all failures, source/executable hashes and compact repo/HOME notes.

## Resources and follow-up

Reuse the single `/tmp/recalld-owned-native-controls-validation/{cargo,target}`
cache with new small immutable source/output roots, offline2CPU/-j1 and recorded
bounded memory. Do not call the old host convenience path that clones caches.
No datasets, credentials, paid calls or training. Delete completed disposable
raw roots after PR/results under central cleanup ownership.

Later independent slices still own upstream scorer adapters/goldens, fuller
trace/token/accounting fields, statistical uncertainty and validated native
writer/lifecycle coverage. This grouping slice does not close those gates.
