# Native store producer integration

Work in progress on PR91 `b033c95` plus Hume's actual native graph core. The
initial local join used `2cad0f7`; the keyed no-op correction `962aeac` and
foreign-history binding correction `ca314d3` are now consumed. Published graph
head `292fb29` and the same-insert type header `6c2bf2a` are included. The header
alone does not supply the new proof or enable its consumer. No catalog, maintenance, public activation or
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

The typed frozen-plan adapter passed all five `native_selection` tests on the
current joined source, including frozen settings and namespace/profile mismatch
rejection. Formatting passed. `selection-focused-7.log` matched zero tests because
the Rust module is named `selection`, not its filename `selection_tests`; that
script mismatch is retained and contributes no test count. The corrected filter
in `selection-focused-8.log` ran five tests. No final full-suite or Clippy result
is claimed for this producer branch.


## Version3 consumer audit before implementation

The current version coupling is concentrated in `storage/batch.rs` contexts and
caller/resolution hashes, `metadata/batch.rs` reservation/replay/final-error and
strict-row validation, `batch/native.rs` child hashes/outcome binding,
`mcp/batches.rs` parsing and `mcp/batch_adapters.rs` execution selection. Version3
must be a separate parameter from the existing native MutationContext version2;
MutationContext must continue rejecting contract3.

Exact version/domain matches must cover reservation receipts, resolved hashes,
error bookkeeping, private child capability context, frozen native/default shape,
and the exact native bookkeeping references used by lifecycle history dispatch.
No `version != 1 means newest` fallback. Existing v1/v2 fixtures must reopen and
replay with identical original bytes/hashes/IDs. New child outcomes and defaults
need strict version-specific checks rather than merely expanding an allowlist.

Native startup currently permits pending version2 manifests; the version3 join
must add only strictly validated native version3 recovery plans. Ordinary export
and snapshot still reject unresolved manifests. Logical export and retained-state
restore already preserve the raw manifest tables, so their dispatch stays generic
while actual version3 completed roundtrip, foreign-origin reconciliation and
forged-version/default/outcome controls prove no row or proof is silently dropped.
The existing pending_batch_items scan remains the accepted-work source of truth;
new unknown/payload-needed slots cannot be acknowledged as completed.

Minimal mutable link settings are frozen separately at reservation: enablement,
semantic threshold/caps, entity cap and temporal window/cap. Unrelated activation
settings are not consumed. Completed replay uses the stored expansion before
current settings, provider or candidate discovery. Generated plan/UUID/time/vector
output remains outside caller identity; the original advisory boolean after fixed
true expansion is part of the new store request identity.

## Actual private version3 binding

The same-insert core `a863804` is joined in `9263920`, with all 61 focused
native-record tests passing (119.84 seconds). Hume's test-only follow-up
`08707c7` is included through `df4d463`. Source and executable hashes are pinned
in the private validation directory before subsequent builds.

The new private reservation API accepts a lazy settings resolver. It first
reconciles the original ordered caller hash and returns an existing manifest;
only a fresh reservation evaluates current settings. Version3 insertion defaults
include frozen store settings; versions1/2 omit that optional field and retain
their original domains. The new child uses the actual shared core request hash,
checks its reserved settings and current namespace policy before allocation, and
records the new typed outcome in the same transaction as its record and effects.
Version3 means native contract2, never contract3. Version3 forget is unsupported.

Private preparation reuses the existing native canonical parser and wraps its
ordered intent in a new domain. Omitted fixed defaults (including
checkDuplicates=true and empty label/tag arrays) are equivalent to explicit
values. Original supplied vector bits and meaningful caller changes remain
distinct. Mutable scope/default resolution is not repeated for matching replay.

The initial three new tests passed. The expanded frozen run passed all 82
batch-filtered tests (72.99 seconds), including old1/2 cases, new3 strict reopen,
completed export/import, retained-state fork restore, and actual exits before
metadata commit, after commit, and after canonical acknowledgement. These are
process-exit tests, not power-loss evidence. Unknown versions, missing new
settings, and rewriting the new manifest as version2 fail closed on startup.
The temporal selection probe now delegates to the actual core selector.

Public version3 preparation still rejects requests. The private binding is a
checkpoint, not a transport handoff: actual MCP/daemon prepared execution,
provider-zero replay, current advisory readGeneration/warnings, publication
failure and accepted-root close-race tests remain before activation and final
full-suite/fmt/Clippy validation. PR91 is unchanged. Halley retains owned ID/RNG
injection; this slice introduces no competing source or reset mechanism.
