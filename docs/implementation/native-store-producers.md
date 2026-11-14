# Native store producer integration

Work in progress on PR91 `b033c95` plus Hume's actual native graph core. The
initial local join used `2cad0f7`; the keyed no-op correction `962aeac` and
foreign-history binding correction `ca314d3` are now consumed. The final actual
core head must be included before final validation. No catalog, maintenance, public activation or
broad checkpoint ancestry is required by this producer branch.

The existing accepted-work registry and derived coordinator now have a typed
standalone native graph entry. It accepts work before publication/graph waits,
checks receipt replay before current namespace/policy/clock, commits under
graph-to-storage ownership and releases both before awaiting derived completion.
Publication errors retain the exact current receipt. Cancellation leaves the
accepted root unresolved. This is a private direct-call producer boundary; it
does not make insert-then-autolink durable or enable a new public route.

The candidate module reads canonical namespace rows and hash-verified bound
vectors, with actual generation checks before and after copying. Phase and
namespace eligibility apply before caps. Semantic links use the existing hard
floor/tag adjustment and cosine score order; entity links use canonical entity
keys and their exact Jaccard cardinalities. Ties use UUID order. Temporal links
use frozen creation times and final newrecord.created_at (supplied caller time,
otherwise the core's single sampled clock). Edge timestamps use that sampled
clock. There is no second producer clock. The
complete candidate generation must be equal at the final transaction, including
negative evidence such as newly inserted candidates. Individual endpoint tokens
alone cannot establish that.

Advisory selection distinguishes disabled from requested-empty and retains only
IDs and numeric scores. Hydration reads only those IDs from current canonical
records, omits missing/ineligible content with a warning, and returns a separate
current readGeneration. Unavailable enrichment produces an explicit warning on
the already-committed store, never an apparent rollback or silent empty scan.
No receipt contains summary/text/embedding copies; no replay rediscovers neighbors.

## Required dependent writer

Hume owns the new additive same-insert creation proof/core. The agreed original
intent is NativeStoreInput containing the unchanged NativeInsert and an expanded
checkDuplicates boolean (fixed default true), with a new hash domain. Generated
UUIDs, clock/provider output, selected neighbors and graph plans are excluded from
caller identity. Mutable configuration/default expansion is frozen separately.
Historical NativeRecordWrite versions through2 keep their meaning.

The new batch consumer will use an explicit version3 mutation/manifest/child
identity for this new store writer, explicitly mapped to native contract2. No
public contract3 is introduced. Strict version maps must be audited in replay,
history dispatch, export/import and restore; unknown versions stay rejected. PR91 version1/version2 rows, hashes, reserved
IDs and outcomes remain immutable. Completed replay precedes current
configuration/provider/advisory work. Existing pending version2 work retains its
old guarded semantics; it cannot silently acquire a new child hash or proof.

The core must union explicit and semantic/entity/temporal links, validate captured
policy/profile/endpoints and exact captured generation before allocation/staging,
bind the new UUID and one clock, update each source count/token once, and commit
record, frozen edge/advisory proof, receipt and outbox in the same transaction.
The current unsupported store gate remains until this path and its actual batch
consumer pass process-exit/replay/publication regressions. A volatile WorkTicket
is not durable auxiliary intent. Full native serving, whole-store public CAS,
and whole-core coherent-view/barrier coverage are not claimed.

Pascal separately owns the awaited native maintenance coordinator with an
already-accepted parent WorkTicket; this owner retains producer registration.
No duplicate maintenance root or runner is introduced here.

## Validation in progress

Initial publication/replay test passed; the first graph-close fixture failed
because it asserted lock state at root admission before the short gate lookup
finished. Its source/executable and failed log were preserved. The corrected
fixture waits for the actual publication lease, then proves storage is free
while graph ownership blocks the producer. Two coordinator and two canonical
selection checks passed afterward. Three coordinator checks (including cancellation) and four selection/advisory
checks passed. The latter explicitly distinguish checked-empty from disabled
and exercise equal-score/entity/time UUID ties before caps. No final suite or whole native-store completion is claimed.

The next focused run passed5 graph-producer tests (including the process helper)
and4 selection/advisory tests. The actual process cases exit91 after metadata
commit and before online derived acknowledgement, then reopen and publish the
original receipt without resampling a clock or changing generation. These are
process-exit tests, not power-loss evidence. No publication is authorized for
this preparation alone: the next store child still requires its actual new proof
and consumer, with the unsupported guard retained until validation.
