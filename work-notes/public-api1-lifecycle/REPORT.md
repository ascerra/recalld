# Public API1 feedback/forget: source-specific handoff

Status: source-specific validation and evidence sealed. Publication/READY
ownership is recorded separately; this report does not transfer branch ownership.

Runtime: `1364b27242dd3b0cf33a81eb9762a27a1f999cc8`.
Actual dependency: published PR144 `31a0463b2d7e0e6920c2daf6025759fb2e1d73ca`
only. Plan518804d, initial runtime6d17724, narrow corrections1364b27.
Worktree `/tmp/recalld-implementation/public-api1-lifecycle`, branch
`impl/public-api1-lifecycle`. Frozen144 has not been edited or pushed.

## Implemented boundary

Actual API1 MCP reinforce/forget and HTTP reinforce/delete now use the existing
canonical owners for both physical formats, with original1 and their configured
policy/clock. Success fields remain API1. Native numerical outcomes come from
typed receipts. Legacy fresh numerical outcomes come from the same transaction
as the state, access history and receipt. A real accepted second write overtakes
the first response barrier without contaminating that first numerical result.

The existing connection mutex owns capability negotiation, selection and the one
mutation send. `api1LifecycleV1` selects new nonretrying aliases; absent capability
selects the exact older scoped/unscoped route before work. Reconnect resets the
capability. An advertised unknown method, decoder failure or lost response never
falls back to an old mutation. Tests emulate historical handler shapes through
actual Remote/framing; no historical daemon binary was executed.

API1 scope assertions use the existing stable namespace binding under the same
accepted guard, preserve its authored spelling, and compare case-insensitively.
Private144 keyed hashes/replay do not pass through that normalization. Stored
names, original receipts, lifecycle anchors, schema and defaults are unchanged.

Legacy forget retains its existing timestamp behavior and status-only receipt;
this does not claim a new clock sample or historical timestamp there.

One existing accepted root covers transaction/publication/current barrier; the
HTTP handler's duplicate derived deletion work is removed. Known failures keep
full matching receipts. Unknown outcomes carry only legitimately known identity
and never a fabricated receipt/readiness/rollback. Native worker joins are now
explicitly `WorkerOutcomeUnknown`. Precommit clock-panic regressions prove that
classification and unresolved tracker state, not postcommit-panic durability.
Existing real postcommit-error probes separately preserve allocated identity.
No postcommit-panic hook was found; no new production fault framework was added.

A terminal ledger hit maps to content-unavailable only at feedback's
`ensure_insertable` call. Generic DuplicateId allocation/idempotency errors are
not reclassified. Actual tombstone/hard-delete HTTP409 probes show no new
canonical generation or clock sample; live feedback remains successful.

## Actual evidence and attribution

| Frozen source / probe | Result |
| --- | --- |
| Parent144 + `public-before-test.patch` | 1 legacy comparator PASS; 4 actual native public HTTP/MCP FAIL,3.04s |
| Initial implementation second run | 5/5 public-path PASS |
| Fifth intermediate run | 12 PASS /2 FAIL,10.10s: one real terminal ledger error classification; one publication fixture without a live survivor |
| Runtime6d17724, fresh package rebuild | 15 focused PASS,12.60s; full1205 PASS /1 FAIL /1 ignored,546.40s |
| Sole6d full failure | Existing unchanged namespace-case assertion; no CLI/fmt/clippy ran after library failure |
| Runtime6d + `join-before-test.patch` | 0 PASS /2 FAIL,1.29s: both actual worker joins reported notCommitted after precommit clock panic |
| Corrected1364b27 focused | 18 PASS,14.74s |
| Corrected1364b27 existing scope tests | 5 PASS,1.02s |
| Corrected1364b27 full | 1209 library PASS /0 FAIL /1 ignored,549.18s;2 CLI PASS (1.94s/1.01s);4 doctests ignored |
| Corrected1364b27 fmt/clippy | Both exit0; clippy949 lib warnings,1012 lib-test warnings (920 duplicates),1 bin warning |

Additional non-runtime validation failures remain preserved in the protected
scratch: initial test-private-import compile error; seven first-implementation
compile errors; two generic-argument errors in new tests; fourth run stopped
with12 passes/one null-vs-absent fixture assertion/one fixture opening a second
connection against a single-accept server. The sixth14/14 diagnostic run had a
format-only manifest race and is not used as final immutable-source proof.
The later fresh package clean/rebuild removes that provenance ambiguity.
The final evidence driver initially expected two test binaries, but all-features
emitted six test binaries; it stopped before tests, then resumed recording every
executable. No production source change or test retry is hidden by that driver fix.

All tests use synthetic stores in offline Docker, image `recalld-ci:1.94`
(`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`),
Rust1.94.1,2CPU/4GiB/jobs2. Runtime network is none; no home, credential or socket
mounts. One private Cargo/target is reused; the one authorized registry is mounted
read-only. No personal daemon/data is accessed. Original-repository source is not modified
or mounted into tests; authoritative specs are read-only.

Before/after source and executable SHA256 manifests accompany this report.
`corrected-all-executables.json` includes all nine emitted executables, including
production CLI binaries used by integration tests. `build-inputs-sha256.json`
covers295 compiled/source input files. Git runtime commit pins the remaining
tracked repository context. Raw logs/caches remain protected until results and
publication are recorded; no raw HOME archive is required.

## Compatibility and remaining gates

Actual linked native insertions create32 edges; API1 forget retains those edges,
emits one primary proof and zero edge-proof entries, and strictly reopens. This
is not hard deletion's peer witness. No speculative storage budget was added.
New aliases conservatively require8MiB negotiation before work. Error text is
bounded to1024 UTF-8 bytes while keeping the receipt; a full daemon error-envelope
probe uses a real receipt and a large control-character diagnostic. Old aliases
on the new server preserve their old scalar/error behavior and reject unsupported
native legacy writers before mutation. Old peers do not acquire new receipt
semantics by using the compatibility route.

Actual public2 and ordinary native startup remain gated. This is not insertion,
profile/query activation, public keyed mutation/query semantics, all-writer
concurrency certification, A3 completion, A5 performance or A6 deterministic-ID
proof. Huygens owns public routing/query/insertion joins. Feynman owns inherited
global-policy adoption/preview and published147 compaction; Pascal's contribution
there is source-only semantic review. Lifecycle-default/quality promotion remains
an independent empirical gate.

Composition note: Curie's actual148+144 join replaces inherited manual Readiness
construction with the existing `report.into()` conversion for the nullable-stamp/
observed-work shape. This child has only144; that mechanical join is required when
consuming the newer shared observation shape. Do not invent pending counts/stamps.

## Resource handoff

PROTECT while validation/review is active:
`/tmp/recalld-api1-lifecycle-validation` and the assigned implementation worktree.
Do not recreate centrally deleted `/tmp/recalld-legacy-api2-validation`.
After completed PR plus recorded results, the central cleanup owner may release
the standalone validation root; preserve code, reports, worktree and `.git`.
No other agent's resource is deleted here. Cleanup bytes:0 by this agent; the one
private cache/target remains active until that explicit ownership handoff.

Small HOME results copy (no raw archive):
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-public-api1-lifecycle-1364b27`.
The exact verified COPY digest is recorded in `home-seal.json`; the existing
research notebook receives an append-only result entry.
