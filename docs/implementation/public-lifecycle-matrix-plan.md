# Public lifecycle chronological matrix

Status: proposed bounded next slice; no new tests executed by this plan.
Owner: Pascal/A3. Frozen PR153 remains owned by Halley for integration and will
receive no further pushes from this workstream.

## Exact first parent and interfaces

Start solely from PR153 `a2a8866dd898dff7eae01dd49b23d8cd18532980`
(runtime `1364b27242dd3b0cf33a81eb9762a27a1f999cc8`) in
`/tmp/recalld-implementation/public-lifecycle-matrix`, branch
`impl/public-lifecycle-matrix`. No merge is needed for the first slice.

Reuse `native_records/wire_tests.rs::setup_policy`, its actual migrated/attested
fixture, `public_api1_tests.rs`, the real HTTP router/MCP dispatcher, and the
existing single-connection daemon test server. Extend test setup narrowly to
accept the same injected clock for every participating adapter. Use the existing
Clock/ManualClock facilities with a test counting wrapper; no production time
endpoint, second observer, evaluator, scheduler, storage engine or ID allocator.
Keep production client/capability/routing code frozen unless an actual regression
requires a separately reported fix. Huygens owns the store capability extension.

Physical legacy stores and native stores are different matrix rows. Native
namespaces cover both lifecycle versions using actual schema/lifecycle migration;
API1 does not select lifecycle1. Do not raw-edit native policy/tokens/receipts.
Legacy history has no fabricated native timestamp attribution.

## First executable slice

1. Public feedback across actual MCP, HTTP and Remote/daemon: identical seeded
   retained state, fixed day8 time, qualities1–4 and missing quality3. Compare
   public numerical outcomes and the persisted stability/anchor/history with
   the actual shared planner under the same resolved policy. Assert original1
   receipt hash, exactly one event, one clock sample and no pending accepted work
   on successful acknowledgement. Preserve old response shapes. Independent
   unkeyed operation UUIDs are intentionally different; do not call their complete
   logical hashes equal without an owned-ID/reset parent.
2. Chronology: advance the shared clock, then regress it for a quality1 feedback
   and explicit forget. Assert raw observed/retrieved provenance is unchanged,
   reinforced/anchor/updated values follow the current clamp, and forget keeps
   the existing phaseTimeRule marker monotone without rewriting migration history.
   A large representable forward jump must keep strength/stability finite and
   within established bounds. Unrepresentable times reject before canonical work.
3. Retained content: create Summary/Ghost through actual canonical phase-loss
   helpers before migration, then use public feedback/forget. Summary feedback
   cannot restore text; Ghost rejects feedback; low-threshold promotion remains
   sticky after quality1; explicit forget removes exempt content. Compare actual
   cache/index rebuild and strict reopen visibility, not only a response flag.
   Use an existing backup/fork helper only if this parent already supplies the
   exact needed API; otherwise retain that as separately owned evidence.
4. No-change/error trace: terminal and wrong-scope public feedback reject without
   new event/generation/clock; repeated unkeyed successful feedback is two events,
   not replay. Existing153 failures already cover retained known receipts,
   unknown worker joins, mixed-case scope and old-peer no-fallback. Reuse these
   assertions and avoid adding duplicate failure frameworks or fabricated IDs.

A day7 exposure/day8 public feedback comparator may reuse the actual existing
StorageAccessRecorder as an explicitly labeled setup operation. It proves the
public feedback consumer uses the resulting canonical anchor; it does not prove
public search/get default observation. Full public exposure selection is reserved
for the validated Huygens/Hume query consumer and existing A6 controls harness.
No prepared private2 handler will be mislabeled as public2 activation.

## Dependency-triggered followups, not automatic joins

| Requirement | Actual dependency decision / owner |
| --- | --- |
| Public feedback/forget chronology and retained phases | PR153 only; Pascal tests existing public consumers. |
| Actual manual/background sweep after public feedback | Add exact frozen135 only if testing its originalAPI producer; add147 instead when the case actually invokes selected native compaction (147 already consumes135). Feynman owns that producer/physical join. Coordinate before touching sweep files. |
| Public query exposure then feedback | Huygens public query + Hume final hydrated classification, existing132 observer. Wait for their exact validated consumer; no new exposure API or version inference. |
| Full equal-state trace/reset | Consume A6 existing owned clock/IDs/reset/barrier only when required. Helper owns its API and harness; no parallel trace runner. |
| Inherited-global mismatch/adoption | Feynman owns typed binding, preview/apply, startup admission and catalog consumers. Pascal source-reviews semantics and later consumes a frozen actual parent only for a tested public adoption case. |
| Legacy API2 insertion/graph/compound | Curie explicitly owns remaining closure and honest fresh/advisory gate. |
| Legacy API2 catalog/purge | Feynman explicitly owns the dispatch/hash audit after adoption. Native1|2 codecs alone do not certify legacy2. |

Owner replies identified no currently unowned legacyAPI2 lifecycle route:
144 supplies actual legacy feedback/forget,132 actual exposure origin. Public
parser/routing/readiness composition remains Huygens-owned. This is an inventory
statement about scoped work, not an assertion every branch is integrated.

## Source review of inherited-global adoption

Reviewed Feynman's current `capture_policy_at` / `check_native_policy_capture`
seam source-only. Existing capture wrappers require the adopted binding.
Explicit tombstone and pure structural graph operations opt out of that admission
because they do not project retention. Native control, canonical namespace,
schema, lifecycle version/revision, policy hash and record-token checks remain at
capture and final transaction. No A3 semantic blocker found in those narrow hunks.
Unknown input stays unknown; forget does not become an adoption or strength
projection. Dependent feedback/query/sweep must remain bound. Feynman retains
runtime/test ownership, including absent-binding success/rejection and stale-token
cases. This review is not an execution result or approval of a broader diff.

## Validation and resource plan

Freeze source before running each meaningful probe. Preserve a genuine BEFORE
only if a defect is reproduced; do not label missing coverage a reproduced bug.
Run new focused tests, existing153 compatibility/decoder tests, appropriate full
suite, fmt and clippy in synthetic offline Docker `recalld-ci:1.94`, Rust1.94.1,
2CPU/4GiB/jobs2, networknone, no home/credentials/socket mounts. Record exact
source/input/executable hashes and warnings. No new heavy job starts for this plan.

Central cleanup already deleted the old completed target before the retention
refinement arrived. Never recreate that released path. Reuse the one private
Cargo cache `/tmp/recalld-api1-lifecycle-validation/cargo` and, once implementation
is agreed, one new bounded target `/tmp/recalld-a3-matrix-target`. Central owner
confirmed both protected. Completed PR153 raw evidence/scripts were released;
tracked report/manifests and the small verified HOME result copy remain. No new
raw archive or per-run cache clones. Retain the reusable target between justified
runs rather than deleting dependencies merely because a PR is published.

Ordinary native startup, public2 activation, A3 full acceptance, RIF/quality/default
promotion and power-loss durability remain outside this bounded test slice.
